#!/usr/bin/env -S uv run --script
#
# /// script
# requires-python = ">=3.12"
# ///
"""Summarize ticket 381 source-coverage probe results.

This is an offline exploit helper: it reads persisted probe JSON and emits
stable decision/contract artifacts without touching live article sources.
"""

from __future__ import annotations

import argparse
import csv
import json
import sys
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

SOURCE_FAMILIES: dict[str, str] = {
    "europe_pmc_core_search": "europe_pmc_core_metadata",
    "current_europe_pmc_fullTextXML_by_pmcid": "current_europe_pmc_fulltextxml_pmcid",
    "current_europe_pmc_fullTextXML_by_pmid": "current_europe_pmc_fulltextxml_pmid",
    "ncbi_bioc_pmc_by_pmcid": "ncbi_bioc_pmcid",
    "ncbi_bioc_pmc_by_pmid": "ncbi_bioc_pmid",
    "pubtator3_biocjson_by_pmcid": "pubtator3_pmcid",
    "pubtator3_biocjson_by_pmid": "pubtator3_pmid",
    "pmc_oa_manifest": "pmc_oa_manifest",
}

QUALITY_KEYS = [
    "has_title",
    "has_abstract",
    "section_count",
    "paragraph_count",
    "table_count",
    "reference_count",
    "has_fulltext_signal",
    "has_tables",
    "has_references",
    "has_entity_annotations",
    "has_tgz",
]

COUNT_KEYS = [
    "abstract_chars",
    "section_count",
    "section_title_count",
    "paragraph_count",
    "table_count",
    "reference_count",
    "document_count",
    "passage_count",
    "section_type_count",
    "text_chars",
    "annotation_count",
]


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text())


def is_available(source_name: str, source: dict[str, Any]) -> bool:
    if source_name == "europe_pmc_core_search":
        return bool(source.get("ok") and source.get("found"))
    if source_name == "pmc_oa_manifest":
        return bool(source.get("ok") and source.get("parse_ok") and source.get("record_found"))
    return bool(source.get("ok") and source.get("parse_ok", True) and source.get("bytes", 0) > 0)


def quality_bits(source: dict[str, Any]) -> list[str]:
    bits: list[str] = []
    for key in QUALITY_KEYS:
        value = source.get(key)
        if value is True or (isinstance(value, int) and value > 0):
            bits.append(key)
    return bits


def count_value(source: dict[str, Any], key: str) -> int:
    value = source.get(key)
    return value if isinstance(value, int) else 0


def source_case_rows(data: dict[str, Any]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for article in data.get("articles", []):
        case = article["input"]["slug"]
        ids = article.get("resolved_ids", {})
        for source_name, source in sorted(article.get("sources", {}).items()):
            quality = []
            for key in QUALITY_KEYS:
                value = source.get(key)
                if value is True or (isinstance(value, int) and value > 0):
                    quality.append(key)
            licenses = source.get("licenses")
            if isinstance(licenses, list) and licenses:
                license_text = str(licenses[0])
            else:
                record_attrs = source.get("record_attrs")
                if isinstance(record_attrs, dict):
                    license_text = str(record_attrs.get("license") or record_attrs.get("license-type") or "")
                else:
                    result = source.get("result")
                    license_text = str(result.get("license") or "") if isinstance(result, dict) else ""
            row = {
                "case": case,
                "pmid": ids.get("pmid") or "",
                "pmcid": ids.get("pmcid") or "",
                "doi": ids.get("doi") or "",
                "source": source_name,
                "family": SOURCE_FAMILIES.get(source_name, source_name),
                "available": is_available(source_name, source),
                "ok": bool(source.get("ok")),
                "status": source.get("status") if source.get("status") is not None else "",
                "parse_ok": bool(source.get("parse_ok", source.get("found", False))),
                "elapsed_ms": source.get("elapsed_ms") if source.get("elapsed_ms") is not None else "",
                "bytes": source.get("bytes") if source.get("bytes") is not None else 0,
                "quality_bits": ";".join(quality),
                "license": license_text,
                "failure": source.get("error") or ("" if source.get("ok") else str(source.get("status") or "")),
            }
            for key in COUNT_KEYS:
                value = source.get(key)
                row[key] = value if isinstance(value, int) else 0
            rows.append(row)
    s2 = data.get("s2orc_dataset_fit") or {}
    if s2:
        rows.append(
            {
                "case": "s2orc_dataset_fit",
                "pmid": "",
                "pmcid": "",
                "doi": "",
                "source": "semantic_scholar_s2orc_dataset",
                "family": "semantic_scholar_s2orc_dataset",
                "available": bool(s2.get("ok") and s2.get("parse_ok") and s2.get("matching_datasets")),
                "ok": bool(s2.get("ok")),
                "status": s2.get("status") if s2.get("status") is not None else "",
                "parse_ok": bool(s2.get("parse_ok")),
                "elapsed_ms": s2.get("elapsed_ms") if s2.get("elapsed_ms") is not None else "",
                "bytes": s2.get("bytes") if s2.get("bytes") is not None else 0,
                "quality_bits": "dataset_metadata;bulk_json;license_context",
                "license": "ODC-BY dataset; article-level rights still apply",
                "failure": s2.get("error") or "",
                **{key: 0 for key in COUNT_KEYS},
            }
        )
    return rows


def license_value(source: dict[str, Any]) -> str:
    licenses = source.get("licenses")
    if isinstance(licenses, list) and licenses:
        return str(licenses[0])
    record_attrs = source.get("record_attrs")
    if isinstance(record_attrs, dict):
        return str(record_attrs.get("license") or record_attrs.get("license-type") or "")
    result = source.get("result")
    if isinstance(result, dict):
        return str(result.get("license") or "")
    return ""


def summarize(data: dict[str, Any]) -> dict[str, Any]:
    rows = source_case_rows(data)
    by_family: dict[str, dict[str, Any]] = {}
    family_cases: dict[str, set[str]] = defaultdict(set)
    family_available: Counter[str] = Counter()
    family_parse_failures: Counter[str] = Counter()
    family_statuses: dict[str, Counter[str]] = defaultdict(Counter)
    family_quality_counts: dict[str, Counter[str]] = defaultdict(Counter)
    family_sum_counts: dict[str, Counter[str]] = defaultdict(Counter)

    for row in rows:
        family = row["family"]
        family_cases[family].add(row["case"])
        if row["available"]:
            family_available[family] += 1
        if not row["parse_ok"]:
            family_parse_failures[family] += 1
        family_statuses[family][str(row["status"])] += 1
        for bit in str(row["quality_bits"]).split(";"):
            if bit:
                family_quality_counts[family][bit] += 1
        for key in COUNT_KEYS:
            family_sum_counts[family][key] += int(row.get(key) or 0)

    for family in sorted(family_cases):
        by_family[family] = {
            "available_rows": family_available[family],
            "total_rows": len(family_cases[family]),
            "coverage_fraction": f"{family_available[family]}/{len(family_cases[family])}",
            "parse_failure_rows": family_parse_failures[family],
            "statuses": dict(sorted(family_statuses[family].items())),
            "quality_presence_counts": dict(sorted(family_quality_counts[family].items())),
            "summed_counts": dict(sorted(family_sum_counts[family].items())),
        }

    decisions = {
        "production_candidate": "Surface provenance/quality flags from existing Europe PMC core + PMC OA metadata over the current ladder.",
        "conditional_candidate": "NCBI BioC renderer/fallback only after fixtures show it succeeds where XML/HTML misses or materially degrades.",
        "annotation_candidate": "PubTator3 BioC JSON is title/abstract entity enrichment, not a fulltext Markdown rung in observed endpoint.",
        "vault_handoff": "S2ORC/Semantic Scholar dataset JSON belongs, if anywhere, in Vault/batch ingestion, not BioMCP runtime.",
        "runtime_change": "No BioMCP runtime behavior change in this spike.",
    }

    return {
        "generated_from": data.get("generated_at"),
        "article_count": len(data.get("articles", [])),
        "row_count": len(rows),
        "source_family_summary": by_family,
        "source_case_rows": rows,
        "decision": decisions,
        "contract_numbers": contract_numbers(by_family),
    }


def contract_numbers(by_family: dict[str, dict[str, Any]]) -> dict[str, Any]:
    def coverage(family: str) -> str:
        return str(by_family.get(family, {}).get("coverage_fraction", "0/0"))

    current_xml = by_family.get("current_europe_pmc_fulltextxml_pmcid", {}).get("summed_counts", {})
    bioc_pmcid = by_family.get("ncbi_bioc_pmcid", {}).get("summed_counts", {})
    pubtator = by_family.get("pubtator3_pmid", {}).get("summed_counts", {})
    return {
        "metadata_coverage": coverage("europe_pmc_core_metadata"),
        "current_xml_pmcid_coverage": coverage("current_europe_pmc_fulltextxml_pmcid"),
        "current_xml_pmid_direct_coverage": coverage("current_europe_pmc_fulltextxml_pmid"),
        "ncbi_bioc_pmcid_coverage": coverage("ncbi_bioc_pmcid"),
        "ncbi_bioc_pmid_coverage": coverage("ncbi_bioc_pmid"),
        "pubtator_pmid_coverage": coverage("pubtator3_pmid"),
        "pubtator_pmcid_coverage": coverage("pubtator3_pmcid"),
        "pmc_oa_manifest_coverage": coverage("pmc_oa_manifest"),
        "current_xml_pmcid_total_sections": current_xml.get("section_count", 0),
        "current_xml_pmcid_total_paragraphs": current_xml.get("paragraph_count", 0),
        "current_xml_pmcid_total_tables": current_xml.get("table_count", 0),
        "current_xml_pmcid_total_references": current_xml.get("reference_count", 0),
        "ncbi_bioc_pmcid_total_passages": bioc_pmcid.get("passage_count", 0),
        "ncbi_bioc_pmcid_total_text_chars": bioc_pmcid.get("text_chars", 0),
        "pubtator_pmid_total_annotations": pubtator.get("annotation_count", 0),
    }


def compare(baseline: dict[str, Any], current: dict[str, Any]) -> list[dict[str, Any]]:
    return compare_rows(source_case_rows(baseline), source_case_rows(current))


def compare_rows(baseline_rows: list[dict[str, Any]], current_rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    base_rows = {(r["case"], r["source"]): r for r in baseline_rows}
    cur_rows = {(r["case"], r["source"]): r for r in current_rows}
    out: list[dict[str, Any]] = []
    for key in sorted(set(base_rows) | set(cur_rows)):
        b = base_rows.get(key)
        c = cur_rows.get(key)
        b_avail = bool(b and b["available"])
        c_avail = bool(c and c["available"])
        b_elapsed = int(b.get("elapsed_ms") or 0) if b else 0
        c_elapsed = int(c.get("elapsed_ms") or 0) if c else 0
        latency_ratio = (c_elapsed / b_elapsed) if b_elapsed else None
        count_regressions = []
        for count_key in COUNT_KEYS:
            if b and c and int(c.get(count_key) or 0) < int(b.get(count_key) or 0):
                count_regressions.append(count_key)
        out.append(
            {
                "case": key[0],
                "source": key[1],
                "baseline_available": b_avail,
                "current_available": c_avail,
                "coverage_delta": int(c_avail) - int(b_avail),
                "baseline_status": b.get("status", "missing") if b else "missing",
                "current_status": c.get("status", "missing") if c else "missing",
                "baseline_elapsed_ms": b_elapsed,
                "current_elapsed_ms": c_elapsed,
                "latency_ratio": round(latency_ratio, 3) if latency_ratio is not None else "",
                "quality_count_regressions": ";".join(count_regressions),
                "baseline_quality_bits": b.get("quality_bits", "") if b else "",
                "current_quality_bits": c.get("quality_bits", "") if c else "",
                "note": comparison_note(b_avail, c_avail, count_regressions, latency_ratio),
            }
        )
    return out


def comparison_note(b_avail: bool, c_avail: bool, count_regressions: list[str], latency_ratio: float | None) -> str:
    notes = []
    if c_avail and not b_avail:
        notes.append("coverage_improved")
    elif b_avail and not c_avail:
        notes.append("coverage_regressed")
    if count_regressions:
        notes.append("count_regressed")
    if latency_ratio is not None and latency_ratio > 1.03:
        notes.append("live_latency_slower_than_3pct")
    return ";".join(notes) or "pass"


def write_csv(path: Path, rows: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if not rows:
        path.write_text("")
        return
    with path.open("w", newline="") as fh:
        writer = csv.DictWriter(fh, fieldnames=list(rows[0].keys()), lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=Path, required=True, help="Probe JSON to summarize")
    parser.add_argument("--out", type=Path, required=True, help="Summary JSON output path")
    parser.add_argument("--rows", type=Path, required=True, help="Case/source CSV output path")
    parser.add_argument("--baseline", type=Path, help="Optional baseline probe JSON for regression comparison")
    parser.add_argument("--comparison", type=Path, help="Regression comparison CSV output path")
    args = parser.parse_args(argv)

    data = load_json(args.input)
    summary = summarize(data)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n")
    write_csv(args.rows, summary["source_case_rows"])

    if args.baseline or args.comparison:
        if not (args.baseline and args.comparison):
            parser.error("--baseline and --comparison must be provided together")
        baseline_data = load_json(args.baseline)
        comparison = compare_rows(source_case_rows(baseline_data), summary["source_case_rows"])
        write_csv(args.comparison, comparison)

    print(f"wrote {args.out}")
    print(f"wrote {args.rows}")
    if args.comparison:
        print(f"wrote {args.comparison}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
