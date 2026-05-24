"""Source-family summaries and contract numbers for ticket 381."""

from __future__ import annotations

from collections import Counter, defaultdict
from typing import Any

from .constants import COUNT_KEYS
from .model import JsonObject, Summary
from .rows import source_case_rows


def summarize(data: JsonObject) -> Summary:
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
