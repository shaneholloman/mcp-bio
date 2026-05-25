#!/usr/bin/env -S uv run --script
#
# /// script
# requires-python = ">=3.12"
# ///
"""Collect bounded article fulltext miss/degradation candidates for BioC gate.

Live-source spike helper only. It records compact request/response metadata and
quality counts; it deliberately avoids committing full article bodies or large
archives.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import csv
import json
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Any

USER_AGENT = "biomcp-ticket-384-bioc-miss-fixtures/0.1"
TIMEOUT = 25
MAX_BODY = 2_000_000
EXCERPT_CHARS = 500

EUROPE_BASE = "https://www.ebi.ac.uk/europepmc/webservices/rest"
EFETCH = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/efetch.fcgi"
NCBI_BIOC = "https://www.ncbi.nlm.nih.gov/research/bionlp/RESTful/pmcoa.cgi"
PUBTATOR3 = "https://www.ncbi.nlm.nih.gov/research/pubtator3-api"
PMC_OA = "https://www.ncbi.nlm.nih.gov/pmc/utils/oa/oa.fcgi"
PMC_ARTICLE = "https://pmc.ncbi.nlm.nih.gov/articles"

PRIOR_ART_CASES = [
    {
        "case": "prior_jats_verified",
        "approach": "prior_art_control",
        "pmid": "27083046",
        "pmcid": "PMC4878868",
        "doi": "10.7554/elife.14211",
        "why": "Ticket 381 JATS/XML fulltext success control.",
    },
    {
        "case": "prior_ncbi_bioc_sample",
        "approach": "prior_art_control",
        "pmid": "17299597",
        "pmcid": "PMC1790863",
        "doi": "10.1371/journal.pone.0000217",
        "why": "Official NCBI BioC PMC sample from ticket 381.",
    },
    {
        "case": "prior_table_article",
        "approach": "prior_art_control",
        "pmid": "41807883",
        "pmcid": "PMC12976322",
        "doi": "10.1186/s43556-026-00425-4",
        "why": "Ticket 381 table-containing title-derived case.",
    },
    {
        "case": "prior_non_pmc_control",
        "approach": "pubtator_non_pmc_probe",
        "pmid": "22663011",
        "pmcid": None,
        "doi": "10.1056/nejmoa1203421",
        "why": "Known non-PMC/current fulltext miss control; tests PubTator fulltext claim.",
    },
]

SEARCH_APPROACHES = [
    {
        "approach": "non_oa_html_only_search",
        "query": "SRC:PMC AND OPEN_ACCESS:N",
        "limit": 4,
        "why": "PMC records where Europe PMC XML/OA archive may be absent and current ladder may fall to HTML.",
    },
    {
        "approach": "non_oa_med_fulltext_search",
        "query": "SRC:MED AND HAS_FT:Y AND OPEN_ACCESS:N",
        "limit": 4,
        "why": "MED records with fulltext markers but no OA license; tests whether BioC fills XML misses.",
    },
    {
        "approach": "oa_bioc_equivalence_search",
        "query": "SRC:PMC AND HAS_FT:Y AND OPEN_ACCESS:Y",
        "limit": 4,
        "why": "Open-access records likely covered by both JATS/XML and BioC; tests structural deltas.",
    },
]

FULLTEXT_TYPES = {"paragraph", "title_1", "title_2", "title_3", "fig_caption", "fig_title_caption", "table", "table_caption", "ref", "footnote", "back"}
ABSTRACT_ONLY_TYPES = {"title", "abstract", "front", "unknown", "abstract_title_1"}


def fetch(url: str, *, params: dict[str, str] | None = None, accept: str | None = None, max_body: int = MAX_BODY) -> dict[str, Any]:
    if params:
        url = f"{url}?{urllib.parse.urlencode(params)}"
    headers = {"User-Agent": USER_AGENT}
    if accept:
        headers["Accept"] = accept
    started = time.perf_counter()
    try:
        request = urllib.request.Request(url, headers=headers)
        with urllib.request.urlopen(request, timeout=TIMEOUT) as response:
            body = response.read(max_body + 1)
            truncated = len(body) > max_body
            if truncated:
                body = body[:max_body]
            return {
                "ok": 200 <= response.status < 300,
                "status": response.status,
                "url": url,
                "elapsed_ms": round((time.perf_counter() - started) * 1000),
                "content_type": response.headers.get("content-type"),
                "bytes": len(body),
                "truncated": truncated,
                "body": body,
                "error": None,
            }
    except urllib.error.HTTPError as exc:
        body = exc.read(4096)
        return {
            "ok": False,
            "status": exc.code,
            "url": url,
            "elapsed_ms": round((time.perf_counter() - started) * 1000),
            "content_type": exc.headers.get("content-type") if exc.headers else None,
            "bytes": len(body),
            "truncated": False,
            "body": body,
            "error": body.decode("utf-8", "replace")[:EXCERPT_CHARS],
        }
    except Exception as exc:  # noqa: BLE001 - spike diagnostics should preserve failures
        return {
            "ok": False,
            "status": None,
            "url": url,
            "elapsed_ms": round((time.perf_counter() - started) * 1000),
            "content_type": None,
            "bytes": 0,
            "truncated": False,
            "body": b"",
            "error": f"{type(exc).__name__}: {exc}",
        }


def text_body(resp: dict[str, Any]) -> str:
    return resp.get("body", b"").decode("utf-8", "replace")


def response_meta(resp: dict[str, Any]) -> dict[str, Any]:
    body_text = text_body(resp)
    return {
        "request_url": resp["url"],
        "status": resp["status"],
        "ok": resp["ok"],
        "content_type": resp["content_type"],
        "bytes": resp["bytes"],
        "truncated": resp["truncated"],
        "elapsed_ms": resp["elapsed_ms"],
        "error": resp["error"],
        "excerpt": " ".join(body_text[:EXCERPT_CHARS].split()),
    }


def parse_json(resp: dict[str, Any]) -> Any | None:
    if not resp["ok"]:
        return None
    try:
        return json.loads(text_body(resp))
    except Exception:
        return None


def parse_xml(resp: dict[str, Any]) -> ET.Element | None:
    if not resp["ok"]:
        return None
    body = resp.get("body", b"")
    for candidate in (body, re.sub(rb"(?is)<!DOCTYPE[^>]*>", b"", body)):
        try:
            return ET.fromstring(candidate)
        except ET.ParseError:
            pass
    return None


def local_name(tag: str) -> str:
    return tag.rsplit("}", 1)[-1]


def iter_elems(root: ET.Element, name: str):
    for elem in root.iter():
        if local_name(elem.tag) == name:
            yield elem


def elem_text(elem: ET.Element | None) -> str:
    if elem is None:
        return ""
    return " ".join("".join(elem.itertext()).split())


def summarize_jats(resp: dict[str, Any], *, source_kind: str) -> dict[str, Any]:
    root = parse_xml(resp)
    base = response_meta(resp) | {"source_kind": source_kind, "parse_ok": root is not None}
    if root is None:
        return base | {
            "has_title": False,
            "has_abstract": False,
            "has_fulltext_signal": False,
            "section_count": 0,
            "paragraph_count": 0,
            "table_count": 0,
            "reference_count": 0,
            "license": None,
            "reuse_evidence": None,
        }
    titles = [elem_text(e) for e in iter_elems(root, "article-title") if elem_text(e)]
    abstracts = [elem_text(e) for e in iter_elems(root, "abstract") if elem_text(e)]
    return base | {
        "has_title": bool(titles),
        "title_sample": titles[0][:200] if titles else None,
        "has_abstract": bool(abstracts),
        "abstract_chars": len(abstracts[0]) if abstracts else 0,
        "has_fulltext_signal": any(True for _ in iter_elems(root, "body")),
        "section_count": sum(1 for _ in iter_elems(root, "sec")),
        "paragraph_count": sum(1 for _ in iter_elems(root, "p")),
        "table_count": sum(1 for _ in iter_elems(root, "table-wrap")),
        "reference_count": sum(1 for _ in iter_elems(root, "ref")),
        "license": None,
        "reuse_evidence": "JATS source may include license tags; this compact probe does not certify reuse.",
    }


def bioc_documents(data: Any) -> list[dict[str, Any]]:
    docs: list[dict[str, Any]] = []

    def add_collection(value: Any) -> None:
        if isinstance(value, dict) and isinstance(value.get("documents"), list):
            docs.extend(doc for doc in value["documents"] if isinstance(doc, dict))

    if isinstance(data, dict):
        if isinstance(data.get("documents"), list):
            docs.extend(doc for doc in data["documents"] if isinstance(doc, dict))
        if isinstance(data.get("PubTator3"), list):
            docs.extend(doc for doc in data["PubTator3"] if isinstance(doc, dict))
        add_collection(data.get("collection"))
    elif isinstance(data, list):
        for item in data:
            add_collection(item)
            if isinstance(item, dict) and isinstance(item.get("passages"), list):
                docs.append(item)
    return docs


def summarize_bioc(resp: dict[str, Any], *, source_kind: str) -> dict[str, Any]:
    data = parse_json(resp)
    docs = bioc_documents(data)
    passages: list[dict[str, Any]] = []
    for doc in docs:
        if isinstance(doc.get("passages"), list):
            passages.extend(p for p in doc["passages"] if isinstance(p, dict))
    types: dict[str, int] = {}
    text_chars = 0
    annotation_count = 0
    licenses: list[str] = []
    for doc in docs:
        infons = doc.get("infons") if isinstance(doc.get("infons"), dict) else {}
        for key, value in infons.items():
            if "license" in str(key).lower() and value:
                licenses.append(str(value).strip())
    for passage in passages:
        infons = passage.get("infons") if isinstance(passage.get("infons"), dict) else {}
        kind = str(infons.get("type") or infons.get("section_type") or "unknown").strip().lower()
        types[kind] = types.get(kind, 0) + 1
        text_chars += len(str(passage.get("text") or ""))
        anns = passage.get("annotations")
        if isinstance(anns, list):
            annotation_count += len(anns)
        for key, value in infons.items():
            if "license" in str(key).lower() and value:
                licenses.append(str(value).strip())
    dedup_licenses = []
    for value in licenses:
        if value and value not in dedup_licenses:
            dedup_licenses.append(value)
    has_fulltext_signal = bool(set(types) & FULLTEXT_TYPES) or any(k not in ABSTRACT_ONLY_TYPES for k in types)
    return response_meta(resp) | {
        "source_kind": source_kind,
        "parse_ok": data is not None,
        "document_count": len(docs),
        "passage_count": len(passages),
        "passage_types": dict(sorted(types.items(), key=lambda item: (-item[1], item[0]))[:16]),
        "text_chars": text_chars,
        "has_title": "title" in types,
        "has_abstract": "abstract" in types,
        "has_fulltext_signal": has_fulltext_signal,
        "has_tables": any("table" in key for key in types),
        "has_references": any("ref" in key for key in types),
        "annotation_count": annotation_count,
        "has_entity_annotations": annotation_count > 0,
        "license": dedup_licenses[0] if dedup_licenses else None,
        "licenses": dedup_licenses[:5],
        "reuse_evidence": "BioC infons license fields captured when present; article-level rights still require user review." if dedup_licenses else None,
    }


def summarize_html(resp: dict[str, Any]) -> dict[str, Any]:
    body = text_body(resp)
    title_match = re.search(r"(?is)<title[^>]*>(.*?)</title>", body)
    return response_meta(resp) | {
        "source_kind": "pmc_html",
        "parse_ok": bool(resp["ok"] and resp.get("content_type") and "html" in resp["content_type"].lower()),
        "has_title": bool(title_match),
        "title_sample": " ".join(re.sub(r"<[^>]+>", " ", title_match.group(1)).split())[:200] if title_match else None,
        "has_fulltext_signal": bool(resp["ok"] and resp.get("bytes", 0) > 0),
        "license": None,
        "reuse_evidence": None,
    }


def summarize_pmc_oa(resp: dict[str, Any]) -> dict[str, Any]:
    root = parse_xml(resp)
    record_attrs: dict[str, str] = {}
    links: list[dict[str, str]] = []
    if root is not None:
        for elem in root.iter():
            if local_name(elem.tag) == "record" and not record_attrs:
                record_attrs = dict(elem.attrib)
            if local_name(elem.tag) == "link":
                links.append(dict(elem.attrib))
    license_value = record_attrs.get("license") or record_attrs.get("license-type")
    return response_meta(resp) | {
        "source_kind": "pmc_oa_manifest",
        "parse_ok": root is not None,
        "record_found": bool(record_attrs or links),
        "record_attrs": record_attrs,
        "link_formats": sorted({link.get("format", "") for link in links if link.get("format")}),
        "has_tgz": any(link.get("format") == "tgz" for link in links),
        "license": license_value,
        "reuse_evidence": f"PMC OA manifest license={license_value}" if license_value else None,
    }


def europe_search(query: str, limit: int) -> list[dict[str, Any]]:
    resp = fetch(f"{EUROPE_BASE}/search", params={"query": query, "format": "json", "resultType": "core", "pageSize": str(limit)})
    data = parse_json(resp)
    results = []
    if isinstance(data, dict):
        for result in data.get("resultList", {}).get("result", [])[:limit]:
            if isinstance(result, dict):
                results.append(result)
    return results


def resolve_from_search(result: dict[str, Any], approach: str, index: int, why: str) -> dict[str, Any]:
    return {
        "case": f"{approach}_{index + 1}",
        "approach": approach,
        "pmid": result.get("pmid"),
        "pmcid": result.get("pmcid"),
        "doi": result.get("doi"),
        "why": why,
        "title": result.get("title"),
        "europe_pmc_metadata": {
            "source_kind": "europe_pmc_core_metadata",
            "source_url_or_query": result.get("source_url_or_query"),
            "isOpenAccess": result.get("isOpenAccess"),
            "license": result.get("license"),
            "fullTextIdList": result.get("fullTextIdList"),
            "fullTextUrlList": result.get("fullTextUrlList"),
        },
    }


def collect_cases(*, search_limit: int | None = None) -> list[dict[str, Any]]:
    cases = [dict(case) for case in PRIOR_ART_CASES]
    seen = {(case.get("pmid"), case.get("pmcid")) for case in cases}
    for search in SEARCH_APPROACHES:
        limit = search_limit if search_limit is not None else int(search["limit"])
        results = europe_search(search["query"], limit)
        added = 0
        for result in results:
            key = (result.get("pmid"), result.get("pmcid"))
            if key in seen:
                continue
            seen.add(key)
            item = resolve_from_search(result, search["approach"], added, search["why"])
            item["europe_pmc_metadata"]["source_url_or_query"] = search["query"]
            cases.append(item)
            added += 1
    return cases


def load_cases_from_probe(path: Path) -> list[dict[str, Any]]:
    """Load a prior compact result as a fixed regression-control case set."""
    data = json.loads(path.read_text(encoding="utf-8"))
    cases: list[dict[str, Any]] = []
    for item in data.get("cases", []):
        if not isinstance(item, dict):
            continue
        case = {
            "case": item.get("case"),
            "approach": item.get("approach"),
            "pmid": item.get("pmid"),
            "pmcid": item.get("pmcid"),
            "doi": item.get("doi"),
            "why": item.get("why"),
            "title": item.get("title"),
            "europe_pmc_metadata": item.get("europe_pmc_metadata"),
        }
        cases.append({key: value for key, value in case.items() if value is not None})
    return cases


def effective_approaches(*, search_limit: int | None = None) -> list[dict[str, Any]]:
    approaches = []
    for search in SEARCH_APPROACHES:
        item = dict(search)
        if search_limit is not None:
            item["limit"] = search_limit
        approaches.append(item)
    return approaches


def pmcid_digits(pmcid: str) -> str:
    return pmcid.upper().removeprefix("PMC")


def measure_case(case: dict[str, Any]) -> dict[str, Any]:
    pmid = case.get("pmid")
    pmcid = case.get("pmcid")
    source_tasks: list[tuple[str, Any]] = []
    if pmcid:
        pmcid_text = str(pmcid)
        source_tasks.extend([
            ("europe_pmc_fullTextXML_by_pmcid", lambda pmcid_text=pmcid_text: summarize_jats(fetch(f"{EUROPE_BASE}/{urllib.parse.quote(pmcid_text)}/fullTextXML", accept="application/xml"), source_kind="jats_xml")),
            ("ncbi_efetch_pmc_xml", lambda pmcid_text=pmcid_text: summarize_jats(fetch(EFETCH, params={"db": "pmc", "id": pmcid_digits(pmcid_text), "rettype": "xml"}, accept="application/xml"), source_kind="jats_xml")),
            ("pmc_oa_manifest", lambda pmcid_text=pmcid_text: summarize_pmc_oa(fetch(PMC_OA, params={"id": pmcid_text}, accept="application/xml"))),
            ("pmc_html", lambda pmcid_text=pmcid_text: summarize_html(fetch(f"{PMC_ARTICLE}/{urllib.parse.quote(pmcid_text)}/", accept="text/html", max_body=512_000))),
            ("ncbi_bioc_by_pmcid", lambda pmcid_text=pmcid_text: summarize_bioc(fetch(f"{NCBI_BIOC}/BioC_json/{urllib.parse.quote(pmcid_text)}/unicode", accept="application/json"), source_kind="ncbi_bioc_json")),
        ])
    if pmid:
        pmid_text = str(pmid)
        source_tasks.extend([
            ("europe_pmc_fullTextXML_by_pmid", lambda pmid_text=pmid_text: summarize_jats(fetch(f"{EUROPE_BASE}/{urllib.parse.quote(pmid_text)}/fullTextXML", accept="application/xml"), source_kind="jats_xml")),
            ("ncbi_bioc_by_pmid", lambda pmid_text=pmid_text: summarize_bioc(fetch(f"{NCBI_BIOC}/BioC_json/{urllib.parse.quote(pmid_text)}/unicode", accept="application/json"), source_kind="ncbi_bioc_json")),
            ("pubtator3_biocjson_by_pmid", lambda pmid_text=pmid_text: summarize_bioc(fetch(f"{PUBTATOR3}/publications/export/biocjson", params={"pmids": pmid_text}, accept="application/json"), source_kind="pubtator3_bioc_json")),
        ])
    sources: dict[str, Any] = {}
    with concurrent.futures.ThreadPoolExecutor(max_workers=len(source_tasks) or 1) as executor:
        future_by_name = {name: executor.submit(task) for name, task in source_tasks}
        for name, _ in source_tasks:
            sources[name] = future_by_name[name].result()
    return case | {"sources": sources, "classification": classify_sources(sources)}


def successful_jats(source: dict[str, Any]) -> bool:
    return bool(source.get("ok") and source.get("parse_ok") and source.get("has_fulltext_signal"))


def successful_html(source: dict[str, Any]) -> bool:
    return bool(source.get("ok") and source.get("parse_ok") and source.get("has_fulltext_signal"))


def successful_bioc_fulltext(source: dict[str, Any]) -> bool:
    return bool(source.get("ok") and source.get("parse_ok") and source.get("has_fulltext_signal"))


def classify_sources(sources: dict[str, Any]) -> dict[str, Any]:
    europe_xml = sources.get("europe_pmc_fullTextXML_by_pmcid", {})
    efetch_xml = sources.get("ncbi_efetch_pmc_xml", {})
    med_xml = sources.get("europe_pmc_fullTextXML_by_pmid", {})
    oa = sources.get("pmc_oa_manifest", {})
    html = sources.get("pmc_html", {})
    bioc_pmcid = sources.get("ncbi_bioc_by_pmcid", {})
    bioc_pmid = sources.get("ncbi_bioc_by_pmid", {})
    pubtator = sources.get("pubtator3_biocjson_by_pmid", {})

    xml_available = successful_jats(europe_xml) or successful_jats(efetch_xml) or successful_jats(med_xml) or bool(oa.get("has_tgz"))
    html_available = successful_html(html)
    bioc_available = successful_bioc_fulltext(bioc_pmcid) or successful_bioc_fulltext(bioc_pmid)
    pubtator_fulltext = successful_bioc_fulltext(pubtator)

    current = "jats_xml" if xml_available else "pmc_html" if html_available else "miss"
    material_bioc_win = current != "jats_xml" and bioc_available
    table_delta = max(int(bioc_pmcid.get("has_tables") or 0), int(bioc_pmid.get("has_tables") or 0)) and int(europe_xml.get("table_count") or 0) == 0 and successful_jats(europe_xml)

    reason_parts = []
    if material_bioc_win:
        reason_parts.append("BioC has fulltext signal while current XML/HTML control is not structured JATS.")
    if table_delta:
        reason_parts.append("BioC reports table passages while Europe PMC XML table-wrap count is zero.")
    if not reason_parts:
        if current == "jats_xml" and bioc_available:
            reason_parts.append("BioC is coverage-equivalent to current structured XML; not a miss/degradation fixture.")
        elif current == "pmc_html" and not bioc_available:
            reason_parts.append("Current ladder degrades to PMC HTML, but BioC does not supply fulltext.")
        elif current == "miss" and not bioc_available:
            reason_parts.append("Current ladder misses and BioC also lacks fulltext.")
        elif pubtator.get("ok") and not pubtator_fulltext:
            reason_parts.append("PubTator response is title/abstract/annotations only, not fulltext.")
        else:
            reason_parts.append("No material BioC improvement detected.")

    return {
        "current_ladder_best_observed": current,
        "current_xml_available_or_oa_archive": xml_available,
        "current_html_available": html_available,
        "bioc_fulltext_available": bioc_available,
        "pubtator_fulltext_available": pubtator_fulltext,
        "material_bioc_win": bool(material_bioc_win or table_delta),
        "why_current_xml_html_insufficient": "yes" if current != "jats_xml" else "no",
        "decision_reason": " ".join(reason_parts),
    }


def compact_rows(results: dict[str, Any]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for item in results["cases"]:
        classification = item["classification"]
        rows.append({
            "case": item["case"],
            "approach": item["approach"],
            "pmid": item.get("pmid") or "",
            "pmcid": item.get("pmcid") or "",
            "current_best": classification["current_ladder_best_observed"],
            "bioc_fulltext": classification["bioc_fulltext_available"],
            "pubtator_fulltext": classification["pubtator_fulltext_available"],
            "material_bioc_win": classification["material_bioc_win"],
            "reason": classification["decision_reason"],
        })
    return rows


def write_csv(path: Path, rows: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fieldnames = ["case", "approach", "pmid", "pmcid", "current_best", "bioc_fulltext", "pubtator_fulltext", "material_bioc_win", "reason"]
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=fieldnames, lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def strip_bodies(value: Any) -> Any:
    if isinstance(value, dict):
        return {key: strip_bodies(val) for key, val in value.items() if key != "body"}
    if isinstance(value, list):
        return [strip_bodies(item) for item in value]
    return value


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, required=True, help="Detailed compact JSON result path")
    parser.add_argument("--matrix", type=Path, required=True, help="Compact CSV matrix path")
    parser.add_argument("--cases-from", type=Path, help="Re-measure the fixed case set from an earlier probe JSON")
    parser.add_argument("--search-limit", type=int, help="Override the per-approach Europe PMC search limit for new candidate collection")
    parser.add_argument("--run-label", default="explore-scale", help="Human-readable run label stored in the JSON")
    args = parser.parse_args(argv)

    if args.cases_from and args.search_limit is not None:
        parser.error("--cases-from and --search-limit are mutually exclusive")
    raw_cases = load_cases_from_probe(args.cases_from) if args.cases_from else collect_cases(search_limit=args.search_limit)
    measured = [measure_case(case) for case in raw_cases]
    wins = [case for case in measured if case["classification"]["material_bioc_win"]]
    results = {
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "run_label": args.run_label,
        "note": "Live-source spike probe; compact metadata/excerpts only, not routine gate fixtures.",
        "case_count": len(measured),
        "material_bioc_win_count": len(wins),
        "cases_from": str(args.cases_from) if args.cases_from else None,
        "search_limit_override": args.search_limit,
        "approaches": effective_approaches(search_limit=args.search_limit),
        "prior_art_cases": PRIOR_ART_CASES,
        "cases": strip_bodies(measured),
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(results, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    rows = compact_rows(results)
    write_csv(args.matrix, rows)
    print(f"wrote {args.out}")
    print(f"wrote {args.matrix}")
    print(f"cases={len(measured)} material_bioc_wins={len(wins)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
