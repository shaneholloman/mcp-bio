from __future__ import annotations

from clinical_features_spike import (
    extract_clinical_feature_dataset,
    extract_disease_clinical_features,
    load_hpo_rows_by_disease,
    summarize_clinical_feature_dataset,
)
from clinical_features_spike.extraction import extract_features, select_topics
from clinical_features_spike.medlineplus import all_diseases, explore_topics_by_disease
from clinical_features_spike.reports import build_full_scale_payload, build_regression_control_payload, build_validation_payload


def test_direct_page_selection_prefers_exact_disease_pages() -> None:
    topics = explore_topics_by_disease()
    diseases = {disease["key"]: disease for disease in all_diseases()}

    uterine = select_topics(diseases["uterine_fibroid"], topics["uterine_fibroid"])
    endometriosis = select_topics(diseases["endometriosis"], topics["endometriosis"])

    assert uterine["selection_policy"] == "direct_pages_only"
    assert [topic["title"] for topic in uterine["topics"]] == ["Uterine Fibroids"]
    assert endometriosis["selection_policy"] == "direct_pages_only"
    assert [topic["title"] for topic in endometriosis["topics"]] == ["Endometriosis"]


def test_extraction_preserves_source_native_rows_and_mapping() -> None:
    topics = explore_topics_by_disease()
    disease = {row["key"]: row for row in all_diseases()}["endometriosis"]
    selected = select_topics(disease, topics["endometriosis"])
    features = extract_features(disease, selected["topics"])

    labels = {feature["label"] for feature in features}
    assert {"pelvic pain", "dysmenorrhea", "dyspareunia", "infertility"} <= labels
    for feature in features:
        assert feature["source"] == "MedlinePlus"
        assert feature["source_url"].startswith("https://medlineplus.gov/")
        assert feature["evidence_tier"] == "clinical_summary"
        assert "normalized_hpo_id" in feature
        assert "mapping_confidence" in feature


def test_full_scale_fixture_validation_passes() -> None:
    full_scale = build_full_scale_payload(allow_live=False)
    regression = build_regression_control_payload(full_scale)
    validation = build_validation_payload(full_scale, regression)

    assert full_scale["summary"]["total_expected_symptoms"] == 23
    assert full_scale["summary"]["total_matched_expected_symptoms"] >= 14
    assert regression["rule_results"]["medlineplus_correctness"]["passed"] is True
    assert validation["passed"] is True


def test_public_api_extracts_importable_dataset_without_cli() -> None:
    diseases = {row["key"]: row for row in all_diseases()}
    hpo_rows = load_hpo_rows_by_disease()

    row = extract_disease_clinical_features(
        diseases["uterine_fibroid"],
        hpo_rows=hpo_rows["uterine_fibroid"],
        allow_live=False,
    )
    assert row["topic_selection"]["selection_policy"] == "direct_pages_only"
    assert row["phenotype_coverage"]["clinical_feature_count"] == 6

    dataset = extract_clinical_feature_dataset(allow_live=False)
    summary = summarize_clinical_feature_dataset(dataset)
    assert summary["clinical_feature_count"] == 15
    assert summary["output_checksum"] == "f08c35ff31306ff4696bd953eaba4b00aeed9e6746a1228469e1479238e3d34f"
