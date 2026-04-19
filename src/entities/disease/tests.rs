//! Top-level disease proof-hook facade preserved for src/lib.rs tests.

use std::collections::HashMap;

use serde::Deserialize;

use super::test_support::test_disease;
use super::{Disease, DiseaseClinicalFeature};

pub(crate) async fn proof_augment_genes_with_opentargets_merges_sources_without_duplicates() {
    super::associations::proof_augment_genes_with_opentargets_merges_sources_without_duplicates()
        .await;
}

pub(crate) async fn proof_augment_genes_with_opentargets_respects_twenty_gene_cap() {
    super::associations::proof_augment_genes_with_opentargets_respects_twenty_gene_cap().await;
}

pub(crate) async fn proof_enrich_sparse_disease_identity_prefers_exact_ols4_match() {
    super::enrichment::proof_enrich_sparse_disease_identity_prefers_exact_ols4_match().await;
}

pub(crate) async fn proof_get_disease_genes_promotes_opentargets_rows_for_cll() {
    super::get::proof_get_disease_genes_promotes_opentargets_rows_for_cll().await;
}

pub(crate) async fn proof_get_disease_genes_uses_ols4_label_fallback_for_sparse_mondo_identity() {
    super::get::proof_get_disease_genes_uses_ols4_label_fallback_for_sparse_mondo_identity().await;
}

fn clinical_feature_row() -> DiseaseClinicalFeature {
    DiseaseClinicalFeature {
        rank: 1,
        label: "heavy menstrual bleeding".to_string(),
        feature_type: "symptom".to_string(),
        source: "MedlinePlus".to_string(),
        source_url: Some("https://medlineplus.gov/uterinefibroids.html".to_string()),
        source_native_id: "uterinefibroids".to_string(),
        evidence_tier: "source_native".to_string(),
        evidence_text: "Heavy menstrual bleeding is a common symptom.".to_string(),
        evidence_match: "heavy menstrual bleeding".to_string(),
        body_system: Some("reproductive".to_string()),
        topic_title: Some("Uterine Fibroids".to_string()),
        topic_relation: Some("primary".to_string()),
        topic_selection_score: Some(0.97),
        normalized_hpo_id: Some("HP:0000132".to_string()),
        normalized_hpo_label: Some("Menorrhagia".to_string()),
        mapping_confidence: 0.91,
        mapping_method: "pattern_match".to_string(),
    }
}

#[test]
fn disease_clinical_features_empty_serializes_as_absent() {
    let disease = test_disease("MONDO:0005105", "melanoma");

    let value = serde_json::to_value(&disease).expect("disease should serialize");

    assert!(value.get("clinical_features").is_none());
}

#[test]
fn disease_clinical_features_missing_json_deserializes_empty() {
    let disease: Disease = serde_json::from_str(r#"{"id":"MONDO:0005105","name":"melanoma"}"#)
        .expect("missing clinical_features should deserialize");

    assert!(disease.clinical_features.is_empty());
}

#[test]
fn disease_clinical_features_nonempty_serializes_rows() {
    let mut disease = test_disease("MONDO:0005105", "melanoma");
    disease.clinical_features.push(clinical_feature_row());

    let value = serde_json::to_value(&disease).expect("disease should serialize");
    let rows = value["clinical_features"]
        .as_array()
        .expect("clinical_features should serialize as rows");

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["rank"], 1);
    assert_eq!(rows[0]["label"], "heavy menstrual bleeding");
    assert_eq!(rows[0]["source"], "MedlinePlus");
    assert_eq!(rows[0]["source_native_id"], "uterinefibroids");
    assert_eq!(rows[0]["normalized_hpo_id"], "HP:0000132");
    assert!(rows[0]["mapping_confidence"].as_f64().unwrap_or_default() > 0.9);
}

#[derive(Debug, Deserialize)]
struct ClinicalFeatureDiseaseFixture {
    key: String,
    label: String,
    biomcp_query: String,
    identifiers: HashMap<String, String>,
    body_system: String,
    source_queries: Vec<String>,
    expected_symptoms: Vec<ExpectedSymptomFixture>,
}

#[derive(Debug, Deserialize)]
struct ExpectedSymptomFixture {
    label: String,
    patterns: Vec<String>,
}

#[test]
fn clinical_features_config_fixture_matches_spike_order() {
    let raw = include_str!("fixtures/clinical_features_config.json");
    let fixtures: Vec<ClinicalFeatureDiseaseFixture> =
        serde_json::from_str(raw).expect("clinical features fixture should parse");

    assert_eq!(fixtures.len(), 3);
    assert_eq!(
        fixtures
            .iter()
            .map(|fixture| fixture.key.as_str())
            .collect::<Vec<_>>(),
        vec![
            "uterine_fibroid",
            "endometriosis",
            "chronic_venous_insufficiency"
        ]
    );
    assert_eq!(
        fixtures
            .iter()
            .map(|fixture| fixture.body_system.as_str())
            .collect::<Vec<_>>(),
        vec!["reproductive", "reproductive", "vascular"]
    );
    assert_eq!(fixtures[0].label, "uterine fibroid");
    assert_eq!(fixtures[0].biomcp_query, "uterine leiomyoma");
    assert_eq!(
        fixtures[0].identifiers.get("mesh").map(String::as_str),
        Some("D007889")
    );
    assert_eq!(fixtures[2].source_queries[2], "venous leg ulcer");
    assert_eq!(
        fixtures
            .iter()
            .map(|fixture| fixture.expected_symptoms.len())
            .collect::<Vec<_>>(),
        vec![8, 7, 8]
    );
    assert_eq!(
        fixtures[0].expected_symptoms[0].label,
        "heavy menstrual bleeding"
    );
    assert_eq!(
        fixtures[2]
            .expected_symptoms
            .last()
            .map(|row| row.label.as_str()),
        Some("heaviness")
    );
    assert!(
        fixtures[0].expected_symptoms[0]
            .patterns
            .iter()
            .any(|pattern| pattern == "prolonged menstrual bleeding")
    );
    assert!(
        fixtures[1].expected_symptoms[2]
            .patterns
            .iter()
            .any(|pattern| pattern == "pain during or after sex")
    );
    assert!(
        fixtures[2].expected_symptoms[5]
            .patterns
            .iter()
            .any(|pattern| pattern == "venous eczema")
    );
}
