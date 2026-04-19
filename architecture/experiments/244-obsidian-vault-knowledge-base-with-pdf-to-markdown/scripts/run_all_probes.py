#!/usr/bin/env python3
"""Run ticket 244 Obsidian vault knowledge-base spike probes.

The script keeps downloaded source documents and generated vault notes in the
experiment `work/` directory, which is ignored by git. It writes reproducible
measurements to `results/`.
"""

from __future__ import annotations

import argparse
import datetime as dt
import html
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import textwrap
import time
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
EXPERIMENT_DIR = SCRIPT_DIR.parent
RESULTS_DIR = EXPERIMENT_DIR / "results"
WORK_DIR = EXPERIMENT_DIR / "work"
VAULT_DIR = WORK_DIR / "temp-obsidian-vault"
RUST_MANIFEST = SCRIPT_DIR / "rust_probe" / "Cargo.toml"
RUST_TARGET_DIR = WORK_DIR / "rust-target"
RUST_PROBE_TIMEOUT_SECONDS = 90

USER_AGENT = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) BioMCP-ticket-244-spike Safari/537.36"

JATS_PMCS = ["PMC9984800", "PMC9891841"]

HTML_SOURCES = [
    {
        "id": "pmc_article_page",
        "type": "article",
        "source_name": "PMC",
        "url_template": "https://pmc.ncbi.nlm.nih.gov/articles/{pmcid}/",
        "title_hint": "PMC article page",
    },
    {
        "id": "biorxiv_preprint_page",
        "type": "preprint",
        "source_name": "bioRxiv",
        "url": "https://www.biorxiv.org/content/10.1101/178418v5",
        "title_hint": "bioRxiv preprint page",
    },
    {
        "id": "nih_news_release",
        "type": "news",
        "source_name": "NIH",
        "url": "https://www.nih.gov/news-events/news-releases/nih-funded-study-finds-long-covid-affects-adolescents-differently-younger-children",
        "title_hint": "NIH biomedical news release",
    },
]

PDF_SOURCES = [
    {
        "id": "pmc_oa_article_pdf",
        "type": "article",
        "source_name": "PMC OA",
        "pmcid": "PMC9984800",
        "title_hint": "PMC OA article PDF",
    },
    {
        "id": "dailymed_keytruda_label",
        "type": "drug",
        "source_name": "DailyMed",
        "url": "https://dailymed.nlm.nih.gov/dailymed/getFile.cfm?setid=9333c79b-d487-4538-a9f0-71b91a02b287&type=pdf&name=9333c79b-d487-4538-a9f0-71b91a02b287.pdf",
        "title_hint": "KEYTRUDA DailyMed drug label",
    },
    {
        "id": "cdc_sti_guideline",
        "type": "guideline",
        "source_name": "CDC",
        "url": "https://www.cdc.gov/std/treatment-guidelines/STI-Guidelines-2021.pdf",
        "title_hint": "CDC STI treatment guideline",
    },
]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--only",
        choices=["all", "jats", "html", "pdf", "vault"],
        default="all",
        help="Run one probe family or all probes.",
    )
    parser.add_argument("--page-limit", type=int, default=12)
    args = parser.parse_args()

    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    WORK_DIR.mkdir(parents=True, exist_ok=True)
    VAULT_DIR.mkdir(parents=True, exist_ok=True)

    context: dict[str, Any] = load_context()

    if args.only in ("all", "jats"):
        context["jats"] = run_jats_probe()
        write_json(RESULTS_DIR / "jats_ingest_results.json", context["jats"])

    if args.only in ("all", "html"):
        context.setdefault("jats", load_json_if_exists(RESULTS_DIR / "jats_ingest_results.json"))
        context["html"] = run_html_probe(context.get("jats") or {})
        write_json(RESULTS_DIR / "html_ingest_results.json", context["html"])

    if args.only in ("all", "pdf"):
        context["pdf"] = run_pdf_probe(args.page_limit)
        write_json(RESULTS_DIR / "pdf_quality_matrix.json", context["pdf"])

    if args.only in ("all", "vault"):
        context.setdefault("jats", load_json_if_exists(RESULTS_DIR / "jats_ingest_results.json"))
        context.setdefault("html", load_json_if_exists(RESULTS_DIR / "html_ingest_results.json"))
        context.setdefault("pdf", load_json_if_exists(RESULTS_DIR / "pdf_quality_matrix.json"))

    if args.only in ("all", "vault"):
        vault = populate_vault(context)
        write_json(RESULTS_DIR / "end_to_end_vault_results.json", vault)
        obsidian = run_obsidian_probe()
        write_json(RESULTS_DIR / "obsidian_cli_matrix.json", obsidian)
        summary = summarize(context, vault, obsidian)
        write_json(RESULTS_DIR / "summary.json", summary)

    return 0


def load_context() -> dict[str, Any]:
    return {
        "experiment_dir": str(EXPERIMENT_DIR),
        "work_dir": str(WORK_DIR),
        "vault_dir": str(VAULT_DIR),
        "generated_at": now_iso(),
    }


def run_jats_probe() -> dict[str, Any]:
    records: list[dict[str, Any]] = []
    xml_dir = WORK_DIR / "jats" / "xml"
    md_dir = WORK_DIR / "jats" / "markdown"
    xml_dir.mkdir(parents=True, exist_ok=True)
    md_dir.mkdir(parents=True, exist_ok=True)

    for pmcid in JATS_PMCS:
        xml_path = xml_dir / f"{pmcid}.nxml"
        source_info = fetch_pmc_oa_member(pmcid, suffixes=(".nxml", ".xml"), out_path=xml_path)
        out_md = md_dir / f"{pmcid}.md"
        report = run_rust_probe(["jats", "--input", str(xml_path), "--output", str(out_md)])
        report["pmcid"] = pmcid
        report["source"] = source_info
        report["markdown_sample"] = safe_preview(out_md)
        records.append(report)

    return {
        "approach": "jats_xml_clean_ingest",
        "engine": "Rust roxmltree minimal JATS converter",
        "article_count": len(records),
        "records": records,
    }


def run_html_probe(jats_context: dict[str, Any]) -> dict[str, Any]:
    html_dir = WORK_DIR / "html" / "source"
    md_dir = WORK_DIR / "html" / "markdown"
    html_dir.mkdir(parents=True, exist_ok=True)
    md_dir.mkdir(parents=True, exist_ok=True)

    first_pmcid = first_pmcid_from_jats(jats_context) or JATS_PMCS[0]
    records: list[dict[str, Any]] = []
    for source in HTML_SOURCES:
        source = dict(source)
        if "url_template" in source:
            source["url"] = source["url_template"].format(pmcid=first_pmcid)
            source["pmcid"] = first_pmcid

        html_path = html_dir / f"{source['id']}.html"
        fetch_url(source["url"], html_path)
        out_md = md_dir / f"{source['id']}.md"
        report = run_rust_probe(
            [
                "html",
                "--input",
                str(html_path),
                "--base-url",
                source["url"],
                "--output",
                str(out_md),
            ]
        )
        report["source"] = source
        report["markdown_sample"] = safe_preview(out_md)
        records.append(report)

    return {
        "approach": "html_open_page_ingest",
        "engine": "readability-rust 0.1.0 + html2md 0.2.15",
        "page_count": len(records),
        "records": records,
    }


def run_pdf_probe(page_limit: int) -> dict[str, Any]:
    pdf_dir = WORK_DIR / "pdf" / "source"
    md_dir = WORK_DIR / "pdf" / "markdown"
    pdf_dir.mkdir(parents=True, exist_ok=True)
    md_dir.mkdir(parents=True, exist_ok=True)

    records: list[dict[str, Any]] = []
    for source in PDF_SOURCES:
        pdf_path = pdf_dir / f"{source['id']}.pdf"
        if source.get("pmcid"):
            source_info = fetch_pmc_oa_pdf(source["pmcid"], pdf_path)
        else:
            fetch_url(source["url"], pdf_path)
            source_info = {"url": source["url"], "path": str(pdf_path)}
        assert_pdf(pdf_path)

        source_records = []
        for engine in ["unpdf", "pdf-oxide"]:
            out_md = md_dir / f"{source['id']}.{engine}.md"
            report = run_rust_probe(
                [
                    "pdf",
                    "--engine",
                    engine,
                    "--input",
                    str(pdf_path),
                    "--output",
                    str(out_md),
                    "--page-limit",
                    str(page_limit),
                ]
            )
            report["source"] = source
            report["download"] = source_info
            report["markdown_sample"] = safe_preview(out_md)
            source_records.append(report)

        baseline = run_pymupdf4llm_baseline(pdf_path, md_dir / f"{source['id']}.pymupdf4llm.md", page_limit)
        baseline["source"] = source
        baseline["download"] = source_info
        baseline["markdown_sample"] = safe_preview(Path(baseline["output"]))
        source_records.append(baseline)
        records.extend(source_records)

    matrix = []
    for record in records:
        quality = (((record.get("metrics") or {}).get("quality") or {}).get("overall_score"))
        matrix.append(
            {
                "document": record["source"]["id"],
                "document_type": record["source"]["type"],
                "engine": record.get("engine"),
                "success": record.get("success"),
                "overall_score": quality,
                "heading_detection": (((record.get("metrics") or {}).get("quality") or {}).get("heading_detection")),
                "table_preservation": (((record.get("metrics") or {}).get("quality") or {}).get("table_preservation")),
                "figure_handling": (((record.get("metrics") or {}).get("quality") or {}).get("figure_handling")),
                "reference_extraction": (((record.get("metrics") or {}).get("quality") or {}).get("reference_extraction")),
                "overall_readability": (((record.get("metrics") or {}).get("quality") or {}).get("overall_readability")),
            }
        )

    return {
        "approach": "pdf_fallback_extraction",
        "page_limit": page_limit,
        "documents": PDF_SOURCES,
        "records": records,
        "quality_matrix": matrix,
    }


def populate_vault(context: dict[str, Any]) -> dict[str, Any]:
    if VAULT_DIR.exists():
        for child in VAULT_DIR.iterdir():
            if child.is_file():
                child.unlink()
            elif child.is_dir():
                shutil.rmtree(child)
    (VAULT_DIR / "Sources").mkdir(parents=True, exist_ok=True)

    notes: list[dict[str, Any]] = []

    for record in (context.get("jats") or {}).get("records", []):
        if not record.get("success"):
            continue
        pmcid = record.get("pmcid")
        title = (((record.get("metrics") or {}).get("article_title")) or f"JATS article {pmcid}")
        note = write_note(
            title=title,
            note_type="article",
            source_name="PMC OA JATS",
            source_url=f"https://pmc.ncbi.nlm.nih.gov/articles/{pmcid}/",
            status="imported",
            tags=["biomcp/kb-spike", "source/jats"],
            body=Path(record["output"]).read_text(encoding="utf-8", errors="replace"),
            identifiers={"pmcid": pmcid},
        )
        notes.append(note)

    for record in (context.get("html") or {}).get("records", []):
        if not record.get("success"):
            continue
        source = record.get("source") or {}
        title = (((record.get("metrics") or {}).get("article_title")) or source.get("title_hint") or source.get("id"))
        identifiers = {"pmcid": source.get("pmcid")} if source.get("pmcid") else {}
        note = write_note(
            title=title,
            note_type=source.get("type", "article"),
            source_name=source.get("source_name", "HTML"),
            source_url=source.get("url", ""),
            status="extracted",
            tags=["biomcp/kb-spike", "source/html"],
            body=Path(record["output"]).read_text(encoding="utf-8", errors="replace"),
            identifiers=identifiers,
        )
        notes.append(note)

    best_pdf_records = best_successful_pdf_records(context.get("pdf") or {})
    for record in best_pdf_records:
        source = record.get("source") or {}
        title = source.get("title_hint") or source.get("id") or "PDF extraction"
        identifiers = {"pmcid": source.get("pmcid")} if source.get("pmcid") else {}
        body = Path(record["output"]).read_text(encoding="utf-8", errors="replace")
        note = write_note(
            title=title,
            note_type=source.get("type", "article"),
            source_name=f"{source.get('source_name', 'PDF')} via {record.get('engine')}",
            source_url=source.get("url") or f"https://pmc.ncbi.nlm.nih.gov/articles/{source.get('pmcid')}/",
            status="extracted",
            tags=["biomcp/kb-spike", "source/pdf"],
            body=body,
            identifiers=identifiers,
        )
        notes.append(note)

    searches = run_local_searches(
        [
            "type: article",
            "type: preprint",
            "pmcid: PMC9984800",
            "source/pdf",
            "KEYTRUDA",
            "long covid",
            "doi:",
        ]
    )

    frontmatter_fields = sorted(collect_frontmatter_fields())
    frontmatter_searches = run_frontmatter_searches(
        [
            "type: article",
            "type: preprint",
            "pmcid: PMC9984800",
            "tags: source/pdf",
            "doi:",
        ]
    )
    return {
        "approach": "end_to_end_temp_vault",
        "vault_path": str(VAULT_DIR),
        "note_count": len(notes),
        "notes": notes,
        "frontmatter_fields": frontmatter_fields,
        "local_search": searches,
        "frontmatter_search": frontmatter_searches,
        "success": len(notes) >= 5 and any(n["type"] == "preprint" for n in notes),
    }


def run_obsidian_probe() -> dict[str, Any]:
    obsidian_path = shutil.which("obsidian")
    xdg_mime = run_command(["xdg-mime", "query", "default", "x-scheme-handler/obsidian"], timeout=5)
    uri_examples = {
        "new": build_obsidian_uri("new", {"vault": VAULT_DIR.name, "name": "BioMCP URI Test", "content": "# BioMCP URI Test"}),
        "search": build_obsidian_uri("search", {"vault": VAULT_DIR.name, "query": "pmcid: PMC9984800"}),
    }

    commands: list[dict[str, Any]] = []
    if obsidian_path:
        command_specs = [
            ("help", [obsidian_path, "--help"]),
            ("search", [obsidian_path, "search", "query=pmcid: PMC9984800", "--vault", str(VAULT_DIR)]),
            ("create", [obsidian_path, "create", str(VAULT_DIR / "Sources" / "cli-created.md"), "--content", "CLI probe"]),
            ("read", [obsidian_path, "read", str(next_note_path())]),
            ("tags", [obsidian_path, "tags", "--vault", str(VAULT_DIR)]),
        ]
        for name, cmd in command_specs:
            result = run_command(cmd, timeout=8)
            result["command"] = name
            result["app_state"] = "not_deliberately_prestarted"
            result["works"] = result["exit_code"] == 0 and not result["timed_out"]
            commands.append(result)

    frontmatter_queries = {
        query: run_local_searches([query])[query]
        for query in ["pmcid: PMC9984800", "type: article", "tags:", "source/pdf"]
    }
    structured_frontmatter_queries = run_frontmatter_searches(
        ["pmcid: PMC9984800", "type: article", "tags: source/pdf", "doi:"]
    )

    return {
        "approach": "obsidian_cli_and_uri_probe",
        "obsidian_path": obsidian_path,
        "scheme_handler": {
            "query_command": xdg_mime,
            "registered_handler": (xdg_mime.get("stdout") or "").strip() or None,
        },
        "uri_examples": uri_examples,
        "uri_execution": "not opened; probe records handler registration and valid URI construction to avoid forcing desktop UI during automated spike",
        "commands": commands,
        "frontmatter_search_probe": {
            "local_filesystem_results": frontmatter_queries,
            "structured_frontmatter_results": structured_frontmatter_queries,
            "obsidian_cli_frontmatter_search": "not confirmed; local snap wrapper did not expose reliable noninteractive CLI behavior in this environment",
        },
        "desktop_requirement_assessment": assess_obsidian(commands),
    }


def summarize(context: dict[str, Any], vault: dict[str, Any], obsidian: dict[str, Any]) -> dict[str, Any]:
    pdf = context.get("pdf") or {}
    matrix = pdf.get("quality_matrix") or []
    jats_records = (context.get("jats") or {}).get("records", [])
    html_records = (context.get("html") or {}).get("records", [])
    rust_pdf_success = [
        row for row in matrix if row["engine"] in ("unpdf", "pdf_oxide") and row.get("success")
    ]
    winners = {}
    for doc_id in {row["document"] for row in matrix}:
        doc_rows = [row for row in matrix if row["document"] == doc_id and row.get("success")]
        winners[doc_id] = max(doc_rows, key=lambda row: row.get("overall_score") or 0) if doc_rows else None

    return {
        "generated_at": now_iso(),
        "jats_success_count": sum(1 for r in jats_records if r.get("success")),
        "html_success_count": sum(1 for r in html_records if r.get("success")),
        "rust_pdf_success_count": len(rust_pdf_success),
        "vault_note_count": vault.get("note_count"),
        "vault_success": vault.get("success"),
        "obsidian_cli_available": bool(obsidian.get("obsidian_path")),
        "obsidian_cli_working_commands": [
            command["command"] for command in obsidian.get("commands", []) if command.get("works")
        ],
        "pdf_winners_by_document": winners,
        "decision_inputs": {
            "jats_build_ready": all(r.get("success") for r in jats_records),
            "html_build_ready": sum(1 for r in html_records if r.get("success")) >= 3,
            "pdf_is_fallback": any((row.get("overall_score") or 0) < 4 for row in matrix if row.get("success")),
            "obsidian_cli_optional_only": not any(command.get("works") for command in obsidian.get("commands", [])),
        },
    }


def fetch_pmc_oa_member(pmcid: str, suffixes: tuple[str, ...], out_path: Path) -> dict[str, Any]:
    xml = fetch_text(f"https://www.ncbi.nlm.nih.gov/pmc/utils/oa/oa.fcgi?id={pmcid}")
    links = parse_oa_links(xml)
    tgz = next((link for link in links if link["format"] == "tgz"), None)
    if not tgz:
        if ".xml" in suffixes or ".nxml" in suffixes:
            return fetch_pmc_efetch_xml(pmcid, out_path, "no PMC OA tgz link")
        raise RuntimeError(f"No PMC OA tgz link for {pmcid}")
    archive_path = WORK_DIR / "pmc_oa" / f"{pmcid}.tar.gz"
    archive_path.parent.mkdir(parents=True, exist_ok=True)
    try:
        fetch_url(tgz["href"], archive_path)
        with tarfile.open(archive_path, "r:gz") as archive:
            for member in archive.getmembers():
                if member.isfile() and member.name.lower().endswith(suffixes):
                    extracted = archive.extractfile(member)
                    if extracted is None:
                        continue
                    out_path.parent.mkdir(parents=True, exist_ok=True)
                    out_path.write_bytes(extracted.read())
                    return {"pmcid": pmcid, "oa_url": tgz["href"], "archive_member": member.name, "path": str(out_path)}
    except Exception as exc:
        if ".xml" in suffixes or ".nxml" in suffixes:
            return fetch_pmc_efetch_xml(pmcid, out_path, f"PMC OA tgz fallback after: {exc}")
        raise
    raise RuntimeError(f"No {suffixes} member in PMC OA archive for {pmcid}")


def fetch_pmc_oa_pdf(pmcid: str, out_path: Path) -> dict[str, Any]:
    xml = fetch_text(f"https://www.ncbi.nlm.nih.gov/pmc/utils/oa/oa.fcgi?id={pmcid}")
    links = parse_oa_links(xml)
    pdf = next((link for link in links if link["format"] == "pdf"), None)
    if pdf:
        try:
            fetch_url(pdf["href"], out_path)
            assert_pdf(out_path)
            return {"pmcid": pmcid, "oa_url": pdf["href"], "path": str(out_path)}
        except Exception as exc:
            fallback = f"https://europepmc.org/articles/{pmcid}?pdf=render"
            fetch_url(fallback, out_path)
            assert_pdf(out_path)
            return {
                "pmcid": pmcid,
                "oa_url": pdf["href"],
                "fallback_url": fallback,
                "fallback_reason": str(exc),
                "path": str(out_path),
            }
    try:
        return fetch_pmc_oa_member(pmcid, suffixes=(".pdf",), out_path=out_path)
    except Exception as exc:
        fallback = f"https://europepmc.org/articles/{pmcid}?pdf=render"
        fetch_url(fallback, out_path)
        assert_pdf(out_path)
        return {"pmcid": pmcid, "fallback_url": fallback, "fallback_reason": str(exc), "path": str(out_path)}


def fetch_pmc_efetch_xml(pmcid: str, out_path: Path, reason: str) -> dict[str, Any]:
    numeric = pmcid.removeprefix("PMC")
    url = (
        "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/efetch.fcgi?"
        + urllib.parse.urlencode({"db": "pmc", "id": numeric, "rettype": "xml"})
    )
    text = fetch_text(url)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(text, encoding="utf-8")
    return {"pmcid": pmcid, "efetch_url": url, "fallback_reason": reason, "path": str(out_path)}


def parse_oa_links(xml: str) -> list[dict[str, str]]:
    links = []
    for match in re.finditer(r'<link[^>]+format="([^"]+)"[^>]+href="([^"]+)"', xml):
        href = html.unescape(match.group(2))
        if href.startswith("ftp://ftp.ncbi.nlm.nih.gov/"):
            href = href.replace("ftp://ftp.ncbi.nlm.nih.gov/", "https://ftp.ncbi.nlm.nih.gov/", 1)
        elif href.startswith("ftp://"):
            href = href.replace("ftp://", "https://", 1)
        links.append({"format": match.group(1), "href": href})
    return links


def fetch_url(url: str, out_path: Path) -> dict[str, Any]:
    out_path.parent.mkdir(parents=True, exist_ok=True)
    request = urllib.request.Request(url, headers=browser_headers())
    try:
        with urllib.request.urlopen(request, timeout=90) as response:
            data = response.read()
            final_url = response.geturl()
            content_type = response.headers.get("content-type")
        out_path.write_bytes(data)
        return {
            "url": url,
            "final_url": final_url,
            "content_type": content_type,
            "bytes": len(data),
            "path": str(out_path),
        }
    except Exception as exc:
        if not shutil.which("curl"):
            raise
        proc = subprocess.run(
            [
                "curl",
                "-L",
                "--fail",
                "--max-time",
                "90",
                "-A",
                USER_AGENT,
                "-H",
                "Accept: text/html,application/xhtml+xml,application/xml,application/pdf;q=0.9,*/*;q=0.8",
                "-o",
                str(out_path),
                url,
            ],
            text=True,
            capture_output=True,
            timeout=100,
        )
        if proc.returncode != 0:
            raise RuntimeError(proc.stderr.strip() or str(exc)) from exc
        return {
            "url": url,
            "final_url": None,
            "content_type": None,
            "bytes": out_path.stat().st_size,
            "path": str(out_path),
            "fetch_fallback": "curl",
            "urllib_error": str(exc),
        }


def fetch_text(url: str) -> str:
    request = urllib.request.Request(url, headers=browser_headers())
    with urllib.request.urlopen(request, timeout=90) as response:
        return response.read().decode("utf-8", errors="replace")


def browser_headers() -> dict[str, str]:
    return {
        "User-Agent": USER_AGENT,
        "Accept": "text/html,application/xhtml+xml,application/xml,application/pdf;q=0.9,*/*;q=0.8",
        "Accept-Language": "en-US,en;q=0.9",
    }


def assert_pdf(path: Path) -> None:
    if path.read_bytes()[:5] != b"%PDF-":
        raise RuntimeError(f"{path} is not a PDF")


def run_rust_probe(
    args: list[str], timeout_seconds: int = RUST_PROBE_TIMEOUT_SECONDS
) -> dict[str, Any]:
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(RUST_TARGET_DIR)
    cmd = ["cargo", "run", "--quiet", "--manifest-path", str(RUST_MANIFEST), "--", *args]
    started = time.perf_counter()
    try:
        proc = subprocess.run(
            cmd,
            text=True,
            capture_output=True,
            env=env,
            timeout=timeout_seconds,
        )
    except subprocess.TimeoutExpired as exc:
        return {
            "kind": args[0],
            "engine": probe_engine_from_args(args),
            "success": False,
            "error": f"timed out after {exc.timeout} seconds",
            "command": cmd,
            "exit_code": None,
            "elapsed_ms": int((time.perf_counter() - started) * 1000),
            "metrics": {"timeout_seconds": exc.timeout},
        }
    if proc.returncode != 0:
        return {
            "kind": args[0],
            "engine": probe_engine_from_args(args),
            "success": False,
            "error": proc.stderr.strip() or proc.stdout.strip(),
            "command": cmd,
            "exit_code": proc.returncode,
            "elapsed_ms": int((time.perf_counter() - started) * 1000),
        }
    return json.loads(proc.stdout)


def probe_engine_from_args(args: list[str]) -> str | None:
    if "--engine" not in args:
        return None
    index = args.index("--engine")
    if index + 1 >= len(args):
        return None
    return args[index + 1]


def run_pymupdf4llm_baseline(pdf_path: Path, out_md: Path, page_limit: int) -> dict[str, Any]:
    out_md.parent.mkdir(parents=True, exist_ok=True)
    started = time.perf_counter()
    helper = r"""
import json
import pathlib
import sys

import fitz
import pymupdf4llm

pdf_path = sys.argv[1]
out_path = pathlib.Path(sys.argv[2])
page_limit = int(sys.argv[3])
doc = fitz.open(pdf_path)
pages = list(range(min(page_limit, doc.page_count)))
markdown = pymupdf4llm.to_markdown(pdf_path, pages=pages)
out_path.write_text(markdown, encoding="utf-8")
print(json.dumps({"page_count": doc.page_count, "pages_processed": len(pages)}))
"""
    if shutil.which("uv"):
        cmd = [
            "uv",
            "run",
            "--no-project",
            "--with",
            "pymupdf4llm",
            "--with",
            "pymupdf",
            "python",
            "-c",
            helper,
            str(pdf_path),
            str(out_md),
            str(page_limit),
        ]
    else:
        cmd = [sys.executable, "-c", helper, str(pdf_path), str(out_md), str(page_limit)]

    try:
        proc = subprocess.run(cmd, text=True, capture_output=True, timeout=90)
    except subprocess.TimeoutExpired as exc:
        return run_pymupdf_text_fallback(
            pdf_path,
            out_md,
            page_limit,
            elapsed_ms=int((time.perf_counter() - started) * 1000),
            fallback_reason=f"pymupdf4llm timed out after {exc.timeout} seconds",
        )
    elapsed_ms = int((time.perf_counter() - started) * 1000)
    if proc.returncode != 0:
        return {
            "kind": "pdf",
            "engine": "pymupdf4llm",
            "input": str(pdf_path),
            "output": str(out_md),
            "elapsed_ms": elapsed_ms,
            "success": False,
            "error": proc.stderr.strip() or proc.stdout.strip(),
            "metrics": {"page_limit": page_limit},
        }

    page_metrics = json.loads(proc.stdout.strip().splitlines()[-1])
    markdown = out_md.read_text(encoding="utf-8", errors="replace")
    metrics = markdown_metrics(markdown)
    metrics.update(page_metrics)
    metrics["page_limit"] = page_limit
    metrics["quality"] = score_pdf(metrics)
    return {
        "kind": "pdf",
        "engine": "pymupdf4llm",
        "input": str(pdf_path),
        "output": str(out_md),
        "elapsed_ms": elapsed_ms,
        "success": True,
        "error": None,
        "metrics": metrics,
    }


def run_pymupdf_text_fallback(
    pdf_path: Path,
    out_md: Path,
    page_limit: int,
    *,
    elapsed_ms: int,
    fallback_reason: str,
) -> dict[str, Any]:
    helper = r"""
import json
import pathlib
import sys

import fitz

pdf_path = sys.argv[1]
out_path = pathlib.Path(sys.argv[2])
page_limit = int(sys.argv[3])
doc = fitz.open(pdf_path)
pages = list(range(min(page_limit, doc.page_count)))
parts = []
for page_index in pages:
    page = doc[page_index]
    text = page.get_text("text")
    parts.append(f"# Page {page_index + 1}\n\n{text.strip()}\n")
markdown = "\n\n".join(parts)
out_path.write_text(markdown, encoding="utf-8")
print(json.dumps({"page_count": doc.page_count, "pages_processed": len(pages)}))
"""
    if shutil.which("uv"):
        cmd = [
            "uv",
            "run",
            "--no-project",
            "--with",
            "pymupdf",
            "python",
            "-c",
            helper,
            str(pdf_path),
            str(out_md),
            str(page_limit),
        ]
    else:
        cmd = [sys.executable, "-c", helper, str(pdf_path), str(out_md), str(page_limit)]
    proc = subprocess.run(cmd, text=True, capture_output=True, timeout=90)
    if proc.returncode != 0:
        return {
            "kind": "pdf",
            "engine": "pymupdf-text-fallback",
            "input": str(pdf_path),
            "output": str(out_md),
            "elapsed_ms": elapsed_ms,
            "success": False,
            "error": proc.stderr.strip() or proc.stdout.strip() or fallback_reason,
            "metrics": {"page_limit": page_limit, "fallback_reason": fallback_reason},
        }
    page_metrics = json.loads(proc.stdout.strip().splitlines()[-1])
    markdown = out_md.read_text(encoding="utf-8", errors="replace")
    metrics = markdown_metrics(markdown)
    metrics.update(page_metrics)
    metrics["page_limit"] = page_limit
    metrics["quality"] = score_pdf(metrics)
    metrics["fallback_reason"] = fallback_reason
    return {
        "kind": "pdf",
        "engine": "pymupdf-text-fallback",
        "input": str(pdf_path),
        "output": str(out_md),
        "elapsed_ms": elapsed_ms,
        "success": True,
        "error": None,
        "metrics": metrics,
    }


def write_note(
    *,
    title: str,
    note_type: str,
    source_name: str,
    source_url: str,
    status: str,
    tags: list[str],
    body: str,
    identifiers: dict[str, Any] | None = None,
) -> dict[str, Any]:
    identifiers = identifiers or {}
    frontmatter = {
        "title": title,
        "type": note_type,
        "source_url": source_url,
        "source_name": source_name,
        "retrieved_at": now_iso(),
        "license": "",
        "doi": identifiers.get("doi", ""),
        "pmid": identifiers.get("pmid", ""),
        "pmcid": identifiers.get("pmcid", ""),
        "nct_id": identifiers.get("nct_id", ""),
        "authors": [],
        "journal": "",
        "published_at": "",
        "tags": tags,
        "biomcp_entities": [],
        "status": status,
    }
    path = VAULT_DIR / "Sources" / f"{slugify(title)[:80]}.md"
    content = "---\n" + yaml_dump(frontmatter) + "---\n\n" + body.strip() + "\n"
    path.write_text(content, encoding="utf-8")
    return {
        "title": title,
        "type": note_type,
        "path": str(path),
        "source_name": source_name,
        "source_url": source_url,
        "status": status,
        "tags": tags,
    }


def yaml_dump(data: dict[str, Any]) -> str:
    lines = []
    for key, value in data.items():
        if isinstance(value, list):
            lines.append(f"{key}:")
            for item in value:
                lines.append(f"  - {yaml_scalar(item)}")
        else:
            lines.append(f"{key}: {yaml_scalar(value)}")
    return "\n".join(lines) + "\n"


def yaml_scalar(value: Any) -> str:
    if value is None:
        return '""'
    text = str(value)
    if text == "":
        return '""'
    escaped = text.replace("\\", "\\\\").replace('"', '\\"')
    return f'"{escaped}"'


def run_local_searches(queries: list[str]) -> dict[str, Any]:
    notes = list(VAULT_DIR.rglob("*.md"))
    results: dict[str, Any] = {}
    for query in queries:
        query_lower = query.lower()
        matches = []
        for note in notes:
            text = note.read_text(encoding="utf-8", errors="replace")
            if query_lower in text.lower():
                matches.append(str(note.relative_to(VAULT_DIR)))
        results[query] = {"match_count": len(matches), "matches": matches[:10]}
    return results


def run_frontmatter_searches(queries: list[str]) -> dict[str, Any]:
    notes = list(VAULT_DIR.rglob("*.md"))
    records = [(note, parse_frontmatter(note)) for note in notes]
    results: dict[str, Any] = {}
    for query in queries:
        field, _, raw_value = query.partition(":")
        field = field.strip()
        expected = raw_value.strip()
        matches = []
        for note, frontmatter in records:
            value = frontmatter.get(field)
            if value is None:
                continue
            if expected == "":
                matches.append(str(note.relative_to(VAULT_DIR)))
            elif isinstance(value, list) and any(str(item).lower() == expected.lower() for item in value):
                matches.append(str(note.relative_to(VAULT_DIR)))
            elif not isinstance(value, list) and str(value).lower() == expected.lower():
                matches.append(str(note.relative_to(VAULT_DIR)))
        results[query] = {"match_count": len(matches), "matches": matches[:10]}
    return results


def parse_frontmatter(note: Path) -> dict[str, Any]:
    frontmatter: dict[str, Any] = {}
    in_frontmatter = False
    current_list_key: str | None = None
    for line in note.read_text(encoding="utf-8", errors="replace").splitlines():
        if line.strip() == "---":
            if not in_frontmatter:
                in_frontmatter = True
                continue
            break
        if not in_frontmatter:
            continue
        if line.startswith("  - ") and current_list_key:
            frontmatter.setdefault(current_list_key, []).append(unquote_yaml_scalar(line[4:].strip()))
            continue
        if ":" not in line or line.startswith(" "):
            continue
        key, raw_value = line.split(":", 1)
        current_list_key = None
        key = key.strip()
        raw_value = raw_value.strip()
        if raw_value == "":
            frontmatter[key] = []
            current_list_key = key
        else:
            frontmatter[key] = unquote_yaml_scalar(raw_value)
    return frontmatter


def unquote_yaml_scalar(value: str) -> str:
    if len(value) >= 2 and value[0] == '"' and value[-1] == '"':
        return value[1:-1].replace('\\"', '"').replace("\\\\", "\\")
    return value


def collect_frontmatter_fields() -> set[str]:
    fields: set[str] = set()
    for note in VAULT_DIR.rglob("*.md"):
        in_frontmatter = False
        for line in note.read_text(encoding="utf-8", errors="replace").splitlines():
            if line.strip() == "---":
                if not in_frontmatter:
                    in_frontmatter = True
                    continue
                break
            if in_frontmatter and ":" in line and not line.startswith(" "):
                fields.add(line.split(":", 1)[0])
    return fields


def next_note_path() -> Path:
    try:
        return next(VAULT_DIR.rglob("*.md"))
    except StopIteration:
        return VAULT_DIR / "Sources" / "missing.md"


def build_obsidian_uri(action: str, params: dict[str, str]) -> str:
    return f"obsidian://{action}?{urllib.parse.urlencode(params)}"


def assess_obsidian(commands: list[dict[str, Any]]) -> str:
    if not commands:
        return "obsidian command not installed"
    if any(command.get("works") for command in commands):
        return "some CLI commands worked in this environment"
    if any("Loaded main app package" in ((command.get("stderr") or "") + (command.get("stdout") or "")) for command in commands):
        return "local obsidian command behaved like a desktop app wrapper, not a reliable headless CLI"
    return "commands failed or timed out without a usable noninteractive CLI result"


def run_command(cmd: list[str], timeout: int) -> dict[str, Any]:
    try:
        proc = subprocess.run(cmd, text=True, capture_output=True, timeout=timeout)
        return {
            "argv": cmd,
            "exit_code": proc.returncode,
            "timed_out": False,
            "stdout": proc.stdout[-2000:],
            "stderr": proc.stderr[-2000:],
        }
    except subprocess.TimeoutExpired as exc:
        return {
            "argv": cmd,
            "exit_code": None,
            "timed_out": True,
            "stdout": (exc.stdout or "")[-2000:] if isinstance(exc.stdout, str) else "",
            "stderr": (exc.stderr or "")[-2000:] if isinstance(exc.stderr, str) else "",
        }
    except FileNotFoundError as exc:
        return {
            "argv": cmd,
            "exit_code": None,
            "timed_out": False,
            "stdout": "",
            "stderr": str(exc),
        }


def first_pmcid_from_jats(jats_context: dict[str, Any]) -> str | None:
    for record in jats_context.get("records", []):
        if record.get("pmcid"):
            return record["pmcid"]
    return None


def best_successful_pdf_records(pdf_context: dict[str, Any]) -> list[dict[str, Any]]:
    records = [record for record in pdf_context.get("records", []) if record.get("success")]
    grouped: dict[str, list[dict[str, Any]]] = {}
    for record in records:
        grouped.setdefault(record["source"]["id"], []).append(record)
    best = []
    for doc_records in grouped.values():
        best.append(
            max(
                doc_records,
                key=lambda record: (((record.get("metrics") or {}).get("quality") or {}).get("overall_score") or 0),
            )
        )
    return best


def markdown_metrics(markdown: str) -> dict[str, Any]:
    lower = markdown.lower()
    lines = markdown.splitlines()
    nonempty = [line for line in lines if line.strip()]
    return {
        "markdown_bytes": len(markdown.encode("utf-8")),
        "word_count": len(markdown.split()),
        "heading_count": sum(1 for line in lines if line.lstrip().startswith("#")),
        "table_row_count": sum(1 for line in lines if line.lstrip().startswith("|") and "|" in line.strip()[1:]),
        "image_ref_count": markdown.count("!["),
        "link_count": markdown.count("]("),
        "nonempty_line_count": len(nonempty),
        "average_line_len": (sum(len(line.strip()) for line in nonempty) / len(nonempty)) if nonempty else 0,
        "has_reference_signal": "references" in lower or "bibliography" in lower,
        "has_figure_signal": "figure" in lower or "fig." in lower,
        "has_table_signal": "table" in lower or "|" in markdown,
        "has_doi_signal": "doi" in lower or "10." in lower,
        "replacement_char_count": markdown.count("\ufffd"),
    }


def score_pdf(metrics: dict[str, Any]) -> dict[str, int]:
    heading = 5 if metrics["heading_count"] >= 5 else 4 if metrics["heading_count"] >= 2 else 2 if metrics["heading_count"] == 1 else 1
    table = 5 if metrics["table_row_count"] >= 4 else 3 if metrics["has_table_signal"] else 1
    figure = 5 if metrics["image_ref_count"] > 0 else 3 if metrics["has_figure_signal"] else 1
    references = 5 if metrics["has_reference_signal"] and metrics["has_doi_signal"] else 3 if metrics["has_reference_signal"] else 1
    readability = 5 if metrics["word_count"] > 1500 and metrics["replacement_char_count"] == 0 else 3 if metrics["word_count"] > 500 else 1
    overall = round((heading + table + figure + references + readability) / 5)
    return {
        "heading_detection": heading,
        "table_preservation": table,
        "figure_handling": figure,
        "reference_extraction": references,
        "overall_readability": readability,
        "overall_score": overall,
    }


def slugify(value: str) -> str:
    value = re.sub(r"[^A-Za-z0-9._-]+", "-", value.strip()).strip("-")
    return value or "untitled"


def safe_preview(path: Path, limit: int = 5000) -> str:
    if not path.exists():
        return ""
    text = path.read_text(encoding="utf-8", errors="replace")
    return text[:limit]


def write_json(path: Path, data: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def load_json_if_exists(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {}
    return json.loads(path.read_text(encoding="utf-8"))


def now_iso() -> str:
    return dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z")


if __name__ == "__main__":
    raise SystemExit(main())
