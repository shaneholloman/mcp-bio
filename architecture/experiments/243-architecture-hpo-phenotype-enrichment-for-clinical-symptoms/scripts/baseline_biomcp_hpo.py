#!/usr/bin/env python3
from __future__ import annotations

import os
from typing import Any

from phenotype_spike_common import (
    DISEASES,
    RESULTS_DIR,
    ensure_results_dir,
    expected_overlap,
    main_guard,
    run_json_command,
    utc_now_iso,
    write_json,
)


def biomcp_command(query: str) -> list[str]:
    configured = os.environ.get("BIOMCP_BIN")
    if configured:
        return [configured, "--json", "get", "disease", query, "phenotypes"]
    return ["biomcp", "--json", "get", "disease", query, "phenotypes"]


def phenotype_terms(payload: dict[str, Any]) -> list[str]:
    rows = payload.get("phenotypes") or []
    terms: list[str] = []
    for row in rows:
        if not isinstance(row, dict):
            continue
        for field in ("name", "hpo_id", "evidence", "frequency_qualifier"):
            value = row.get(field)
            if isinstance(value, str) and value.strip():
                terms.append(value.strip())
    return terms


def summarize_disease(disease: dict[str, Any]) -> dict[str, Any]:
    result = run_json_command(biomcp_command(disease["biomcp_query"]))
    payload = result.get("json") if isinstance(result.get("json"), dict) else {}
    phenotypes = payload.get("phenotypes") or []
    phenotype_names = [
        row.get("name")
        for row in phenotypes
        if isinstance(row, dict) and isinstance(row.get("name"), str)
    ]
    source_labels = sorted(
        {
            row.get("source")
            for row in phenotypes
            if isinstance(row, dict) and isinstance(row.get("source"), str)
        }
    )
    section_sources = [
        source
        for item in payload.get("_meta", {}).get("section_sources", [])
        if item.get("key") == "phenotypes"
        for source in item.get("sources", [])
    ]

    return {
        "disease_key": disease["key"],
        "label": disease["label"],
        "biomcp_query": disease["biomcp_query"],
        "ticket_id": disease.get("ticket_id"),
        "command": result["command"],
        "exit_code": result["exit_code"],
        "elapsed_ms": result["elapsed_ms"],
        "resolved_id": payload.get("id"),
        "resolved_name": payload.get("name"),
        "xrefs": payload.get("xrefs", {}),
        "definition": payload.get("definition"),
        "phenotype_count": len(phenotypes),
        "phenotype_names": phenotype_names,
        "phenotype_rows": phenotypes,
        "key_features": payload.get("key_features", []),
        "source_labels": source_labels,
        "section_sources": sorted(set(section_sources)),
        "expected_symptom_overlap": expected_overlap(
            phenotype_terms(payload) + payload.get("key_features", []),
            disease["expected_symptoms"],
        ),
        "error": result.get("stderr") or result.get("stdout"),
    }


def main() -> None:
    main_guard()
    ensure_results_dir()
    diseases = [summarize_disease(disease) for disease in DISEASES]
    total_expected = sum(
        row["expected_symptom_overlap"]["expected_total"] for row in diseases
    )
    total_matched = sum(row["expected_symptom_overlap"]["matched_total"] for row in diseases)
    payload = {
        "generated_at": utc_now_iso(),
        "approach": "current_biomcp_hpo_monarch_baseline",
        "metric_definitions": {
            "phenotype_count": "Number of current BioMCP disease.phenotypes rows returned by `get disease ... phenotypes --json`.",
            "expected_symptom_recall": "Manual small-set lexical recall against expected recognizable clinical symptoms for the disease.",
        },
        "summary": {
            "disease_count": len(diseases),
            "total_phenotype_rows": sum(row["phenotype_count"] for row in diseases),
            "total_expected_symptoms": total_expected,
            "total_matched_expected_symptoms": total_matched,
            "expected_symptom_recall": round(total_matched / total_expected, 3)
            if total_expected
            else None,
        },
        "diseases": diseases,
    }
    write_json(RESULTS_DIR / "current_biomcp_hpo_baseline.json", payload)


if __name__ == "__main__":
    main()

