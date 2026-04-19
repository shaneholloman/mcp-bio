#!/usr/bin/env python3
from __future__ import annotations

from typing import Any

from phenotype_spike_common import (
    DISEASES,
    RESULTS_DIR,
    ensure_results_dir,
    expected_overlap,
    main_guard,
    sparql_json,
    utc_now_iso,
    write_json,
)


def sparql_string(value: str) -> str:
    escaped = value.replace("\\", "\\\\").replace('"', '\\"')
    return f'"{escaped}"@en'


def wikidata_query(labels: list[str]) -> str:
    values = " ".join(sparql_string(label) for label in labels)
    return f"""
SELECT ?item ?itemLabel ?matchedLabel ?symptom ?symptomLabel ?icd10 ?mesh ?omim ?snomed WHERE {{
  VALUES ?matchedLabel {{ {values} }}
  ?item rdfs:label ?matchedLabel.
  OPTIONAL {{ ?item wdt:P780 ?symptom. }}
  OPTIONAL {{ ?item wdt:P494 ?icd10. }}
  OPTIONAL {{ ?item wdt:P486 ?mesh. }}
  OPTIONAL {{ ?item wdt:P492 ?omim. }}
  OPTIONAL {{ ?item wdt:P5806 ?snomed. }}
  SERVICE wikibase:label {{ bd:serviceParam wikibase:language "en". }}
}}
ORDER BY ?itemLabel ?symptomLabel
"""


def binding_value(binding: dict[str, Any], name: str) -> str | None:
    value = binding.get(name, {}).get("value")
    return value if isinstance(value, str) and value.strip() else None


def summarize_disease(disease: dict[str, Any]) -> dict[str, Any]:
    response = sparql_json(wikidata_query(disease["wikidata_labels"]))
    bindings = response.get("json", {}).get("results", {}).get("bindings", [])
    items: dict[str, dict[str, Any]] = {}
    for row in bindings:
        item = binding_value(row, "item")
        if not item:
            continue
        current = items.setdefault(
            item,
            {
                "item": item,
                "label": binding_value(row, "itemLabel"),
                "matched_labels": set(),
                "identifiers": {
                    "icd10": set(),
                    "mesh": set(),
                    "omim": set(),
                    "snomed": set(),
                },
                "symptoms": {},
            },
        )
        matched = binding_value(row, "matchedLabel")
        if matched:
            current["matched_labels"].add(matched)
        for key in ("icd10", "mesh", "omim", "snomed"):
            value = binding_value(row, key)
            if value:
                current["identifiers"][key].add(value)
        symptom = binding_value(row, "symptom")
        symptom_label = binding_value(row, "symptomLabel")
        if symptom:
            current["symptoms"][symptom] = symptom_label or symptom

    normalized_items: list[dict[str, Any]] = []
    all_symptom_labels: list[str] = []
    for item in items.values():
        symptom_rows = [
            {"id": symptom_id, "label": label}
            for symptom_id, label in sorted(item["symptoms"].items(), key=lambda pair: pair[1])
        ]
        all_symptom_labels.extend(row["label"] for row in symptom_rows)
        normalized_items.append(
            {
                "item": item["item"],
                "label": item["label"],
                "matched_labels": sorted(item["matched_labels"]),
                "identifiers": {
                    key: sorted(values)
                    for key, values in item["identifiers"].items()
                    if values
                },
                "symptom_count": len(symptom_rows),
                "symptoms": symptom_rows,
            }
        )

    normalized_items.sort(key=lambda row: (-row["symptom_count"], row["label"] or ""))
    return {
        "disease_key": disease["key"],
        "label": disease["label"],
        "queried_labels": disease["wikidata_labels"],
        "query_ok": response["ok"],
        "status": response["status"],
        "elapsed_ms": response["elapsed_ms"],
        "item_count": len(normalized_items),
        "items": normalized_items,
        "total_symptom_assertions": sum(row["symptom_count"] for row in normalized_items),
        "unique_symptom_labels": sorted(set(all_symptom_labels)),
        "expected_symptom_overlap": expected_overlap(
            sorted(set(all_symptom_labels)),
            disease["expected_symptoms"],
        ),
        "error": response.get("error"),
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
        "approach": "wikidata_p780_symptom_broad_coverage",
        "metric_definitions": {
            "total_symptom_assertions": "Number of Wikidata P780 disease-to-symptom rows found for exact English labels.",
            "expected_symptom_recall": "Manual small-set lexical recall against expected recognizable clinical symptoms for the disease.",
        },
        "summary": {
            "disease_count": len(diseases),
            "diseases_with_p780": sum(
                1 for row in diseases if row["total_symptom_assertions"] > 0
            ),
            "total_symptom_assertions": sum(
                row["total_symptom_assertions"] for row in diseases
            ),
            "total_expected_symptoms": total_expected,
            "total_matched_expected_symptoms": total_matched,
            "expected_symptom_recall": round(total_matched / total_expected, 3)
            if total_expected
            else None,
        },
        "diseases": diseases,
    }
    write_json(RESULTS_DIR / "wikidata_p780_probe.json", payload)


if __name__ == "__main__":
    main()

