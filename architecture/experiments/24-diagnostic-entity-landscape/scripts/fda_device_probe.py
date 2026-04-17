#!/usr/bin/env python3
from __future__ import annotations

from typing import Any

from common import DISEASES, GENES, contains_gene_symbol, contains_phrase, pct, request_json, write_json

OPENFDA_510K_URL = "https://api.fda.gov/device/510k.json"
OPENFDA_PMA_URL = "https://api.fda.gov/device/pma.json"
CDX_DRUG_PROBES = ["pembrolizumab", "osimertinib", "vemurafenib", "trastuzumab"]


def fetch_510k_sample(limit: int = 100) -> dict[str, Any]:
    payload, latency_ms, _ = request_json(OPENFDA_510K_URL, params={"limit": limit})
    assert payload is not None
    return {
        "total": int(payload["meta"]["results"]["total"]),
        "latency_ms": latency_ms,
        "results": payload["results"],
    }


def search_openfda(
    url: str,
    query: str,
    *,
    limit: int = 25,
    allow_404: bool = True,
) -> dict[str, Any]:
    payload, latency_ms, status_code = request_json(
        url,
        params={"search": query, "limit": limit},
        allow_404=allow_404,
    )
    if payload is None:
        return {
            "query": query,
            "count": 0,
            "latency_ms": latency_ms,
            "status_code": status_code,
            "results": [],
        }
    return {
        "query": query,
        "count": int(payload["meta"]["results"]["total"]),
        "latency_ms": latency_ms,
        "status_code": status_code,
        "results": payload["results"],
    }


def relevant_gene_matches(results: list[dict[str, Any]], gene: str) -> list[dict[str, Any]]:
    matches: list[dict[str, Any]] = []
    for row in results:
        candidate_text = " ".join(
            part
            for part in [
                row.get("device_name"),
                " ".join((row.get("openfda") or {}).get("device_name", [])),
            ]
            if part
        )
        if contains_gene_symbol(candidate_text, gene):
            matches.append(row)
    return matches


def relevant_disease_matches(results: list[dict[str, Any]], disease: str) -> list[dict[str, Any]]:
    matches: list[dict[str, Any]] = []
    for row in results:
        candidate_text = " ".join(
            part
            for part in [
                row.get("device_name"),
                " ".join((row.get("openfda") or {}).get("device_name", [])),
            ]
            if part
        )
        if contains_phrase(candidate_text, disease):
            matches.append(row)
    return matches


def summarize_examples(results: list[dict[str, Any]]) -> list[dict[str, Any]]:
    examples: list[dict[str, Any]] = []
    for row in results[:10]:
        examples.append(
            {
                "k_number": row.get("k_number"),
                "device_name": row.get("device_name"),
                "applicant": row.get("applicant"),
                "decision_date": row.get("decision_date"),
                "decision_description": row.get("decision_description"),
                "advisory_committee_description": row.get("advisory_committee_description"),
                "product_code": row.get("product_code"),
            }
        )
    return examples


def exact_phrase_matches(results: list[dict[str, Any]], phrase: str) -> list[dict[str, Any]]:
    phrase_lower = phrase.lower()
    return [
        row
        for row in results
        if phrase_lower in (row.get("device_name") or "").lower()
    ]


def main() -> None:
    sample = fetch_510k_sample(limit=100)
    records = sample["results"]
    total = len(records)
    manufacturer_present = sum(1 for row in records if row.get("applicant"))
    decision_present = sum(1 for row in records if row.get("decision_date"))
    device_name_present = sum(1 for row in records if row.get("device_name"))
    committee_present = sum(1 for row in records if row.get("advisory_committee_description"))
    k_number_present = sum(1 for row in records if row.get("k_number"))

    sample_gene_matches: dict[str, dict[str, Any]] = {}
    sample_disease_matches: dict[str, dict[str, Any]] = {}
    sample_name_index: set[str] = set()

    for gene in GENES:
        query_result = search_openfda(OPENFDA_510K_URL, f'device_name:"{gene}"', limit=25)
        relevant = relevant_gene_matches(query_result["results"], gene)
        examples = summarize_examples(relevant)
        sample_gene_matches[gene] = {
            "query": query_result["query"],
            "count": len(relevant),
            "reported_total": query_result["count"],
            "latency_ms": query_result["latency_ms"],
            "examples": examples,
        }
        for example in examples:
            if example["device_name"]:
                sample_name_index.add(example["device_name"].strip().lower())

    for disease in DISEASES:
        query_result = search_openfda(OPENFDA_510K_URL, f'device_name:"{disease}"', limit=25)
        relevant = relevant_disease_matches(query_result["results"], disease)
        examples = summarize_examples(relevant)
        sample_disease_matches[disease] = {
            "query": query_result["query"],
            "count": len(relevant),
            "reported_total": query_result["count"],
            "latency_ms": query_result["latency_ms"],
            "examples": examples,
        }
        for example in examples:
            if example["device_name"]:
                sample_name_index.add(example["device_name"].strip().lower())

    companion_510k = search_openfda(OPENFDA_510K_URL, "device_name:companion diagnostic", limit=100)
    companion_pma = search_openfda(OPENFDA_PMA_URL, "device_name:companion diagnostic", limit=100)
    exact_companion_510k = exact_phrase_matches(companion_510k["results"], "companion diagnostic")
    exact_companion_pma = exact_phrase_matches(companion_pma["results"], "companion diagnostic")
    cdx_drug_queries = {
        drug: {
            "pma_count": search_openfda(OPENFDA_PMA_URL, drug, limit=1)["count"],
            "510k_count": search_openfda(OPENFDA_510K_URL, drug, limit=1)["count"],
        }
        for drug in CDX_DRUG_PROBES
    }

    payload = {
        "approach": "FDA device search probe",
        "source": "fda_device",
        "record_counts": {
            "openfda_510k_total": sample["total"],
            "sampled_records_for_schema_check": total,
        },
        "schema_completeness": {
            "device_name_pct": pct(device_name_present, total),
            "applicant_pct": pct(manufacturer_present, total),
            "decision_date_pct": pct(decision_present, total),
            "k_number_pct": pct(k_number_present, total),
            "advisory_committee_pct": pct(committee_present, total),
        },
        "api_availability": {
            "sample_latency_ms": sample["latency_ms"],
        },
        "sample_gene_matches": sample_gene_matches,
        "sample_disease_matches": sample_disease_matches,
        "sample_name_index": sorted(sample_name_index),
        "companion_diagnostic_probe": {
            "openfda_510k_token_query_count": companion_510k["count"],
            "openfda_510k_exact_phrase_hits_in_first_page": len(exact_companion_510k),
            "openfda_510k_exact_phrase_examples": summarize_examples(exact_companion_510k),
            "openfda_pma_token_query_count": companion_pma["count"],
            "openfda_pma_exact_phrase_hits_in_first_page": len(exact_companion_pma),
            "openfda_pma_exact_phrase_examples": summarize_examples(exact_companion_pma),
            "drug_name_side_probe": cdx_drug_queries,
        },
        "success_signals": {
            "all_sample_genes_have_hits": all(
                sample_gene_matches[gene]["count"] > 0 for gene in GENES
            ),
            "all_sample_diseases_have_hits": all(
                sample_disease_matches[disease]["count"] > 0 for disease in DISEASES
            ),
        },
    }
    output_path = write_json("fda_device.json", payload)
    print(output_path)


if __name__ == "__main__":
    main()
