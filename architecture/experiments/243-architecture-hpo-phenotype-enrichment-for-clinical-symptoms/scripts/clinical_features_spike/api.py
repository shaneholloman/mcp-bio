from __future__ import annotations

from collections.abc import Iterable
from pathlib import Path
from typing import Any

from .common import RESULTS_DIR, load_json, stable_checksum
from .extraction import extract_features, phenotype_coverage, select_topics
from .medlineplus import all_diseases, load_topics_for_disease
from .types import DiseaseClinicalFeatures, DiseaseInput, HpoPhenotypeRow


EXPLORE_HPO_PATH = RESULTS_DIR / "current_biomcp_hpo_baseline.json"


def load_hpo_rows_by_disease(
    path: Path = EXPLORE_HPO_PATH,
) -> dict[str, list[HpoPhenotypeRow]]:
    payload = load_json(path)
    return {
        row["disease_key"]: row.get("phenotype_rows", [])
        for row in payload.get("diseases", [])
    }


def extract_disease_clinical_features(
    disease: DiseaseInput,
    hpo_rows: Iterable[HpoPhenotypeRow] | None = None,
    *,
    allow_live: bool = True,
    refresh_cache: bool = False,
) -> DiseaseClinicalFeatures:
    hpo_phenotypes = list(hpo_rows or [])
    topic_payload = load_topics_for_disease(
        disease,
        allow_live=allow_live,
        refresh_cache=refresh_cache,
    )
    selection = select_topics(disease, topic_payload["topics"])
    features = extract_features(disease, selection["topics"])
    coverage = phenotype_coverage(disease, hpo_phenotypes, features)
    return {
        "disease_key": disease["key"],
        "label": disease["label"],
        "biomcp_query": disease["biomcp_query"],
        "source_mode": topic_payload["source_mode"],
        "fallback_used": topic_payload["fallback_used"],
        "work_dir": topic_payload["work_dir"],
        "attempts": topic_payload["attempts"],
        "topic_selection": selection,
        "phenotypes": hpo_phenotypes,
        "clinical_features": features,
        "phenotype_coverage": coverage,
        "feature_checksum": stable_checksum(coverage["feature_label_checksum_input"]),
    }


def extract_clinical_feature_dataset(
    diseases: Iterable[DiseaseInput] | None = None,
    hpo_rows_by_disease: dict[str, list[HpoPhenotypeRow]] | None = None,
    *,
    allow_live: bool = True,
    refresh_cache: bool = False,
) -> list[DiseaseClinicalFeatures]:
    disease_rows = all_diseases() if diseases is None else [dict(disease) for disease in diseases]
    hpo_by_key = hpo_rows_by_disease if hpo_rows_by_disease is not None else load_hpo_rows_by_disease()
    return [
        extract_disease_clinical_features(
            disease,
            hpo_rows=hpo_by_key.get(disease["key"], []),
            allow_live=allow_live,
            refresh_cache=refresh_cache,
        )
        for disease in disease_rows
    ]


def summarize_clinical_feature_dataset(rows: list[DiseaseClinicalFeatures]) -> dict[str, Any]:
    total_expected = sum(row["phenotype_coverage"]["expected_symptom_total"] for row in rows)
    total_matched = sum(row["phenotype_coverage"]["expected_symptom_matched"] for row in rows)
    total_features = sum(row["phenotype_coverage"]["clinical_feature_count"] for row in rows)
    summary = {
        "disease_count": len(rows),
        "total_candidate_topics": sum(row["topic_selection"]["candidate_topic_count"] for row in rows),
        "total_selected_topics": sum(row["topic_selection"]["selected_topic_count"] for row in rows),
        "total_topic_noise_reduction": sum(row["topic_selection"]["noise_reduction_count"] for row in rows),
        "direct_page_diseases": sum(
            1 for row in rows if row["topic_selection"]["selection_policy"] == "direct_pages_only"
        ),
        "clinical_feature_count": total_features,
        "mapped_feature_count": sum(row["phenotype_coverage"]["mapped_feature_count"] for row in rows),
        "unmapped_feature_count": sum(row["phenotype_coverage"]["unmapped_feature_count"] for row in rows),
        "total_expected_symptoms": total_expected,
        "total_matched_expected_symptoms": total_matched,
        "expected_symptom_recall": round(total_matched / total_expected, 3) if total_expected else None,
        "mismatch_count": total_expected - total_matched,
    }
    summary["output_checksum"] = stable_checksum(
        [
            {
                "disease_key": row["disease_key"],
                "feature_checksum": row["feature_checksum"],
                "coverage": {
                    key: row["phenotype_coverage"][key]
                    for key in [
                        "clinical_feature_count",
                        "mapped_feature_count",
                        "expected_symptom_matched",
                        "expected_symptom_recall",
                    ]
                },
            }
            for row in rows
        ]
    )
    return summary
