#!/usr/bin/env -S uv run --script
#
# /// script
# requires-python = ">=3.12"
# ///
"""Small-scale source-first article Markdown coverage probe.

Live-source spike helper only. Production follow-ups should convert any accepted
source behavior into fixtures/request-contract tests rather than routine live
checks.
"""

from __future__ import annotations

import argparse
import csv
import gzip
import io
import json
import re
import sys
import tarfile
import time
import urllib.error
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET
from dataclasses import dataclass, asdict
from pathlib import Path
from typing import Any

USER_AGENT = "biomcp-ticket-381-source-coverage-spike/0.1"
TIMEOUT = 25

EUROPE_BASE = "https://www.ebi.ac.uk/europepmc/webservices/rest"
NCBI_BIOC_BASE = "https://www.ncbi.nlm.nih.gov/research/bionlp/RESTful/pmcoa.cgi"
PUBTATOR3_BASE = "https://www.ncbi.nlm.nih.gov/research/pubtator3-api"
PMC_OA_BASE = "https://www.ncbi.nlm.nih.gov/pmc/utils/oa/oa.fcgi"
S2_DATASETS_LATEST = "https://api.semanticscholar.org/datasets/v1/release/latest"

ARTICLES = [
    {
        "slug": "jats_verified_pmid",
        "pmid": "27083046",
        "pmcid": None,
        "doi": None,
        "title_query": "Synaptotagmin-1 C2B domain interacts simultaneously",
        "why": "Known BioMCP JATS verification article from prior art.",
    },
    {
        "slug": "ncbi_bioc_sample",
        "pmid": "17299597",
        "pmcid": "PMC1790863",
        "doi": None,
        "title_query": "Effects of acupuncture on rates of pregnancy",
        "why": "Official NCBI BioC PMC sample ID, should be in PMC OA BioC.",
    },
    {
        "slug": "non_pmc_control",
        "pmid": "22663011",
        "pmcid": None,
        "doi": None,
        "title_query": "Chronic myeloid leukemia imatinib resistance",
        "why": "Prior PDF-fallback control: no PMCID in BioMCP verification.",
    },
    {
        "slug": "title_lookup_only",
        "pmid": None,
        "pmcid": None,
        "doi": None,
        "title_query": "BRAF V600E melanoma vemurafenib resistance",
        "why": "Title-derived/lexical Europe PMC lookup path rather than supplied identifier.",
    },
]


def now_ms() -> int:
    return int(time.perf_counter() * 1000)


def fetch(url: str, *, params: dict[str, str] | None = None, accept: str | None = None, max_bytes: int = 8_000_000) -> dict[str, Any]:
    if params:
        sep = "&" if "?" in url else "?"
        url = url + sep + urllib.parse.urlencode(params)
    headers = {"User-Agent": USER_AGENT}
    if accept:
        headers["Accept"] = accept
    started = now_ms()
    try:
        req = urllib.request.Request(url, headers=headers)
        with urllib.request.urlopen(req, timeout=TIMEOUT) as resp:
            body = resp.read(max_bytes + 1)
            elapsed = now_ms() - started
            truncated = len(body) > max_bytes
            if truncated:
                body = body[:max_bytes]
            return {
                "ok": 200 <= resp.status < 300,
                "status": resp.status,
                "url": url,
                "elapsed_ms": elapsed,
                "content_type": resp.headers.get("content-type"),
                "bytes": len(body),
                "truncated": truncated,
                "body": body,
                "error": None,
            }
    except urllib.error.HTTPError as e:
        body = e.read(4096)
        return {
            "ok": False,
            "status": e.code,
            "url": url,
            "elapsed_ms": now_ms() - started,
            "content_type": e.headers.get("content-type") if e.headers else None,
            "bytes": len(body),
            "truncated": False,
            "body": body,
            "error": body.decode("utf-8", "replace")[:500],
        }
    except Exception as e:  # noqa: BLE001 - spike diagnostics should preserve failures
        return {
            "ok": False,
            "status": None,
            "url": url,
            "elapsed_ms": now_ms() - started,
            "content_type": None,
            "bytes": 0,
            "truncated": False,
            "body": b"",
            "error": f"{type(e).__name__}: {e}",
        }


def text(resp: dict[str, Any]) -> str:
    return resp.get("body", b"").decode("utf-8", "replace")


def json_body(resp: dict[str, Any]) -> Any | None:
    if not resp["ok"]:
        return None
    try:
        return json.loads(text(resp))
    except Exception:
        return None


def strip_ns(tag: str) -> str:
    return tag.rsplit("}", 1)[-1]


def parse_xml(resp: dict[str, Any]) -> ET.Element | None:
    if not resp["ok"]:
        return None
    try:
        return ET.fromstring(resp["body"])
    except ET.ParseError:
        cleaned = re.sub(rb"(?is)<!DOCTYPE[^>]*>", b"", resp["body"])
        try:
            return ET.fromstring(cleaned)
        except ET.ParseError:
            return None


def iter_elems(root: ET.Element, local: str):
    for elem in root.iter():
        if strip_ns(elem.tag) == local:
            yield elem


def elem_text(elem: ET.Element | None) -> str:
    if elem is None:
        return ""
    return " ".join("".join(elem.itertext()).split())


def summarize_jats_xml(resp: dict[str, Any]) -> dict[str, Any]:
    root = parse_xml(resp)
    if root is None:
        return base_summary(resp) | {"format": "jats_xml", "parse_ok": False}
    titles = [elem_text(e) for e in iter_elems(root, "article-title") if elem_text(e)]
    abstracts = [elem_text(e) for e in iter_elems(root, "abstract") if elem_text(e)]
    sec_titles = [elem_text(e) for e in iter_elems(root, "title") if elem_text(e)]
    return base_summary(resp) | {
        "format": "jats_xml",
        "parse_ok": True,
        "has_title": bool(titles),
        "title_sample": titles[0] if titles else None,
        "has_abstract": bool(abstracts),
        "abstract_chars": len(abstracts[0]) if abstracts else 0,
        "section_count": sum(1 for _ in iter_elems(root, "sec")),
        "section_title_count": len(sec_titles),
        "paragraph_count": sum(1 for _ in iter_elems(root, "p")),
        "table_count": sum(1 for _ in iter_elems(root, "table-wrap")),
        "reference_count": sum(1 for _ in iter_elems(root, "ref")),
    }


def base_summary(resp: dict[str, Any]) -> dict[str, Any]:
    return {
        "ok": resp["ok"],
        "status": resp["status"],
        "elapsed_ms": resp["elapsed_ms"],
        "bytes": resp["bytes"],
        "content_type": resp["content_type"],
        "truncated": resp["truncated"],
        "error": resp["error"],
        "url": resp["url"],
    }


def documents_from_bioc_json(data: Any) -> list[dict[str, Any]]:
    docs: list[dict[str, Any]] = []

    def collect_from_collection(collection: Any) -> None:
        if isinstance(collection, dict) and isinstance(collection.get("documents"), list):
            docs.extend(doc for doc in collection["documents"] if isinstance(doc, dict))

    if isinstance(data, dict):
        if isinstance(data.get("documents"), list):
            docs.extend(doc for doc in data["documents"] if isinstance(doc, dict))
        elif isinstance(data.get("PubTator3"), list):
            docs.extend(doc for doc in data["PubTator3"] if isinstance(doc, dict))
        elif isinstance(data.get("collection"), dict):
            collect_from_collection(data["collection"])
    elif isinstance(data, list):
        for item in data:
            collect_from_collection(item)
            if isinstance(item, dict) and isinstance(item.get("passages"), list):
                docs.append(item)
    return docs


def passages_from_bioc_json(data: Any) -> list[dict[str, Any]]:
    passages: list[dict[str, Any]] = []
    for doc in documents_from_bioc_json(data):
        if isinstance(doc.get("passages"), list):
            passages.extend(p for p in doc["passages"] if isinstance(p, dict))
    return passages


def licenses_from_bioc_json(data: Any, passages: list[dict[str, Any]]) -> list[str]:
    out: list[str] = []
    for doc in documents_from_bioc_json(data):
        infons = doc.get("infons") if isinstance(doc.get("infons"), dict) else {}
        for key, value in infons.items():
            if "license" in str(key).lower() and value:
                out.append(str(value).strip())
    for p in passages:
        infons = p.get("infons") if isinstance(p.get("infons"), dict) else {}
        for key, value in infons.items():
            if "license" in str(key).lower() and value:
                out.append(str(value).strip())
    deduped: list[str] = []
    for value in out:
        if value and value not in deduped:
            deduped.append(value)
    return deduped[:5]


def summarize_bioc_json(resp: dict[str, Any], *, source_format: str) -> dict[str, Any]:
    data = json_body(resp)
    passages = passages_from_bioc_json(data)
    docs = documents_from_bioc_json(data)
    licenses = licenses_from_bioc_json(data, passages)
    types: dict[str, int] = {}
    section_types: dict[str, int] = {}
    annotation_count = 0
    text_chars = 0
    for p in passages:
        infons = p.get("infons") if isinstance(p.get("infons"), dict) else {}
        kind = str(infons.get("type") or infons.get("section_type") or "").strip() or "unknown"
        types[kind] = types.get(kind, 0) + 1
        st = str(infons.get("section_type") or infons.get("section") or kind).strip() or "unknown"
        section_types[st] = section_types.get(st, 0) + 1
        text_chars += len(str(p.get("text") or ""))
        anns = p.get("annotations")
        if isinstance(anns, list):
            annotation_count += len(anns)
    lower_types = {k.lower(): v for k, v in types.items()}
    has_fulltext_signal = any(k not in {"title", "abstract", "front", "unknown"} for k in lower_types)
    return base_summary(resp) | {
        "format": source_format,
        "parse_ok": data is not None,
        "document_count": len(docs),
        "licenses": licenses,
        "passage_count": len(passages),
        "passage_types": dict(sorted(types.items(), key=lambda kv: (-kv[1], kv[0]))[:12]),
        "section_type_count": len(section_types),
        "text_chars": text_chars,
        "has_title": "title" in lower_types,
        "has_abstract": "abstract" in lower_types,
        "has_fulltext_signal": has_fulltext_signal,
        "has_tables": any("table" in k for k in lower_types),
        "has_references": any("ref" in k or "back" in k for k in lower_types),
        "annotation_count": annotation_count,
        "has_entity_annotations": annotation_count > 0,
    }


def europe_search(article: dict[str, Any]) -> dict[str, Any]:
    if article.get("pmid"):
        query = f"EXT_ID:{article['pmid']} AND SRC:MED"
    elif article.get("pmcid"):
        query = f"PMCID:{article['pmcid']}"
    elif article.get("doi"):
        query = f"DOI:{article['doi']}"
    else:
        query = article["title_query"]
    resp = fetch(f"{EUROPE_BASE}/search", params={"query": query, "format": "json", "resultType": "core", "pageSize": "1"})
    data = json_body(resp)
    result = None
    if isinstance(data, dict):
        result_list = data.get("resultList", {}).get("result", [])
        if result_list:
            result = result_list[0]
    return base_summary(resp) | {
        "query": query,
        "found": result is not None,
        "result": {
            "pmid": result.get("pmid"),
            "pmcid": result.get("pmcid"),
            "doi": result.get("doi"),
            "title": result.get("title"),
            "isOpenAccess": result.get("isOpenAccess"),
            "license": result.get("license"),
            "hasTextMinedTerms": result.get("hasTextMinedTerms"),
            "fullTextIdList": result.get("fullTextIdList"),
            "fullTextUrlList": result.get("fullTextUrlList"),
        } if result else None,
    }


def europe_fulltext_xml(identifier: str) -> dict[str, Any]:
    resp = fetch(f"{EUROPE_BASE}/{urllib.parse.quote(identifier)}/fullTextXML", accept="application/xml")
    return summarize_jats_xml(resp)


def ncbi_bioc(identifier: str) -> dict[str, Any]:
    resp = fetch(f"{NCBI_BIOC_BASE}/BioC_json/{urllib.parse.quote(identifier)}/unicode", accept="application/json")
    return summarize_bioc_json(resp, source_format="ncbi_bioc_json")


def pubtator_export(kind: str, identifier: str) -> dict[str, Any]:
    # BioMCP's existing client uses pubtator3-api and supports pmids. The legacy
    # PubTator Central docs also mention pmcids for full text; measure both by
    # recording the observed failure if pubtator3 rejects pmcids.
    resp = fetch(f"{PUBTATOR3_BASE}/publications/export/biocjson", params={kind: identifier}, accept="application/json")
    return summarize_bioc_json(resp, source_format=f"pubtator3_biocjson_{kind}")


def pmc_oa_manifest(pmcid: str) -> dict[str, Any]:
    resp = fetch(PMC_OA_BASE, params={"id": pmcid}, accept="application/xml")
    root = parse_xml(resp)
    record_attrs: dict[str, str] = {}
    links: list[dict[str, str]] = []
    if root is not None:
        record = next(iter_elems(root, "record"), None)
        if record is not None:
            record_attrs = dict(record.attrib)
            for link in record.iter():
                if strip_ns(link.tag) == "link":
                    links.append(dict(link.attrib))
    return base_summary(resp) | {
        "format": "pmc_oa_manifest_xml",
        "parse_ok": root is not None,
        "record_found": bool(record_attrs or links),
        "record_attrs": record_attrs,
        "link_formats": sorted({link.get("format", "") for link in links if link.get("format")}),
        "links": links[:10],
        "has_license_field": any(k.lower() == "license" or "license" in k.lower() for k in record_attrs),
        "has_tgz": any(link.get("format") == "tgz" for link in links),
        "has_pdf": any(link.get("format") == "pdf" for link in links),
    }


def semantic_scholar_dataset_fit() -> dict[str, Any]:
    resp = fetch(S2_DATASETS_LATEST, accept="application/json")
    data = json_body(resp)
    datasets = []
    if isinstance(data, dict):
        for ds in data.get("datasets", []):
            if isinstance(ds, dict) and str(ds.get("name", "")).lower() in {"s2orc", "s2orc_v2"}:
                datasets.append(ds)
            elif isinstance(ds, str) and ds.lower() in {"s2orc", "s2orc_v2"}:
                datasets.append({"name": ds})
    elif isinstance(data, list):
        for ds in data:
            if isinstance(ds, str) and ds.lower() in {"s2orc", "s2orc_v2"}:
                datasets.append({"name": ds})
    return base_summary(resp) | {
        "format": "semantic_scholar_datasets_metadata",
        "parse_ok": data is not None,
        "release_id": data.get("release_id") if isinstance(data, dict) else None,
        "matching_datasets": datasets,
        "on_demand_fulltext_endpoint": False,
        "requires_dataset_download": True,
        "bioMCP_runtime_fit": "poor: bulk dataset/API-key download rather than per-article fetch rung",
        "vault_batch_fit": "possible if license and storage policy accepted",
    }


def resolved_ids(article: dict[str, Any], search: dict[str, Any]) -> dict[str, str | None]:
    result = search.get("result") or {}
    return {
        "pmid": article.get("pmid") or result.get("pmid"),
        "pmcid": article.get("pmcid") or result.get("pmcid"),
        "doi": article.get("doi") or result.get("doi"),
    }


def run() -> dict[str, Any]:
    out: dict[str, Any] = {
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "note": "Live-source spike probe; do not use as routine gate.",
        "articles": [],
        "s2orc_dataset_fit": semantic_scholar_dataset_fit(),
    }
    for article in ARTICLES:
        item: dict[str, Any] = {"input": article, "sources": {}}
        search = europe_search(article)
        item["sources"]["europe_pmc_core_search"] = search
        ids = resolved_ids(article, search)
        item["resolved_ids"] = ids
        if ids.get("pmcid"):
            item["sources"]["current_europe_pmc_fullTextXML_by_pmcid"] = europe_fulltext_xml(ids["pmcid"])
            item["sources"]["ncbi_bioc_pmc_by_pmcid"] = ncbi_bioc(ids["pmcid"])
            item["sources"]["pubtator3_biocjson_by_pmcid"] = pubtator_export("pmcids", ids["pmcid"])
            item["sources"]["pmc_oa_manifest"] = pmc_oa_manifest(ids["pmcid"])
        if ids.get("pmid"):
            item["sources"]["current_europe_pmc_fullTextXML_by_pmid"] = europe_fulltext_xml(ids["pmid"])
            item["sources"]["ncbi_bioc_pmc_by_pmid"] = ncbi_bioc(ids["pmid"])
            item["sources"]["pubtator3_biocjson_by_pmid"] = pubtator_export("pmids", ids["pmid"])
        out["articles"].append(item)
    return out


def compact_matrix(result: dict[str, Any]) -> list[dict[str, Any]]:
    rows = []
    for article in result["articles"]:
        base = {
            "slug": article["input"]["slug"],
            "pmid": article["resolved_ids"].get("pmid"),
            "pmcid": article["resolved_ids"].get("pmcid"),
        }
        for source_name, source in article["sources"].items():
            if source_name == "europe_pmc_core_search":
                rows.append(base | {
                    "source": source_name,
                    "available": source.get("found", False),
                    "quality": "metadata",
                    "license": (source.get("result") or {}).get("license"),
                    "is_open_access": (source.get("result") or {}).get("isOpenAccess"),
                    "elapsed_ms": source.get("elapsed_ms"),
                    "failure": source.get("error") or (None if source.get("ok") else source.get("status")),
                })
                continue
            quality_bits = []
            for key, label in [
                ("has_title", "title"),
                ("has_abstract", "abstract"),
                ("section_count", "sections"),
                ("paragraph_count", "paragraphs"),
                ("table_count", "tables"),
                ("reference_count", "refs"),
                ("has_entity_annotations", "entities"),
                ("has_fulltext_signal", "fulltext"),
                ("has_tgz", "tgz"),
            ]:
                value = source.get(key)
                if value is True or (isinstance(value, int) and value > 0):
                    quality_bits.append(label)
            rows.append(base | {
                "source": source_name,
                "available": bool(source.get("ok") and (source.get("parse_ok", True)) and (source.get("bytes", 0) > 0)),
                "quality": ",".join(quality_bits) or "none",
                "license": (source.get("licenses") or [None])[0] or source.get("record_attrs", {}).get("license") or source.get("record_attrs", {}).get("license-type"),
                "is_open_access": None,
                "elapsed_ms": source.get("elapsed_ms"),
                "failure": source.get("error") or (None if source.get("ok") else source.get("status")),
            })
    rows.append({
        "slug": "s2orc_dataset_fit",
        "pmid": None,
        "pmcid": None,
        "source": "semantic_scholar_s2orc_dataset",
        "available": bool(result["s2orc_dataset_fit"].get("matching_datasets")),
        "quality": "bulk-json-metadata-only-in-probe",
        "license": "ODC-BY at dataset level per docs; article-level rights still apply",
        "is_open_access": None,
        "elapsed_ms": result["s2orc_dataset_fit"].get("elapsed_ms"),
        "failure": result["s2orc_dataset_fit"].get("error"),
    })
    return rows


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, required=True, help="JSON result path")
    parser.add_argument("--matrix", type=Path, required=True, help="Compact CSV matrix path")
    args = parser.parse_args()

    result = run()
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")

    rows = compact_matrix(result)
    args.matrix.parent.mkdir(parents=True, exist_ok=True)
    with args.matrix.open("w", newline="") as fh:
        writer = csv.DictWriter(fh, fieldnames=["slug", "pmid", "pmcid", "source", "available", "quality", "license", "is_open_access", "elapsed_ms", "failure"])
        writer.writeheader()
        writer.writerows(rows)

    print(f"wrote {args.out}")
    print(f"wrote {args.matrix}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
