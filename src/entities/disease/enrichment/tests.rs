use super::super::DiseaseClinicalFeature;
use super::super::test_support::*;
use super::*;

use std::path::Path;

use crate::sources::gtr::{GTR_CONDITION_GENE_FILE, GTR_TEST_VERSION_FILE};
use crate::sources::who_ivd::WHO_IVD_CSV_FILE;
use crate::test_support::TempDirGuard;

fn write_gtr_fixture(root: &Path) {
    std::fs::write(
        root.join(GTR_TEST_VERSION_FILE),
        include_bytes!("../../../../spec/fixtures/gtr/test_version.gz"),
    )
    .expect("write test_version.gz");
    std::fs::write(
        root.join(GTR_CONDITION_GENE_FILE),
        include_str!("../../../../spec/fixtures/gtr/test_condition_gene.txt"),
    )
    .expect("write test_condition_gene.txt");
}

fn write_who_ivd_fixture(root: &Path) {
    std::fs::write(
        root.join(WHO_IVD_CSV_FILE),
        include_str!("../../../../spec/fixtures/who-ivd/who_ivd.csv"),
    )
    .expect("write who_ivd.csv");
}

fn clinical_feature_row(label: &str) -> DiseaseClinicalFeature {
    DiseaseClinicalFeature {
        rank: 1,
        label: label.to_string(),
        feature_type: "symptom".to_string(),
        source: "MedlinePlus".to_string(),
        source_url: Some("https://medlineplus.gov/example.html".to_string()),
        source_native_id: "example".to_string(),
        evidence_tier: "source_native".to_string(),
        evidence_text: format!("{label} appears in the MedlinePlus topic."),
        evidence_match: label.to_string(),
        body_system: Some("reproductive".to_string()),
        topic_title: Some("Example Topic".to_string()),
        topic_relation: Some("primary".to_string()),
        topic_selection_score: Some(1.0),
        normalized_hpo_id: None,
        normalized_hpo_label: None,
        mapping_confidence: 0.8,
        mapping_method: "pattern_match".to_string(),
    }
}

#[test]
fn funding_query_prefers_free_text_lookup() {
    let disease = test_disease(
        "MONDO:0011996",
        "chronic myelogenous leukemia, BCR-ABL1 positive",
    );

    assert_eq!(
        disease_funding_query_value(&disease, Some("chronic myeloid leukemia")),
        Some("chronic myeloid leukemia".to_string())
    );
}

#[test]
fn funding_query_uses_canonical_name_for_identifier_lookups() {
    let disease = test_disease(
        "MONDO:0011996",
        "chronic myelogenous leukemia, BCR-ABL1 positive",
    );

    assert_eq!(
        disease_funding_query_value(&disease, Some("MONDO:0011996")),
        Some("chronic myelogenous leukemia, BCR-ABL1 positive".to_string())
    );
}

#[tokio::test]
async fn apply_requested_sections_clears_funding_when_not_requested() {
    let mut disease = test_disease("MONDO:0007947", "Marfan syndrome");
    disease.funding = Some(empty_funding_section("Marfan syndrome".to_string()));
    disease.funding_note = Some(FUNDING_NO_DATA_NOTE.to_string());

    apply_requested_sections(&mut disease, DiseaseSections::default(), None)
        .await
        .expect("sections should apply");

    assert!(disease.funding.is_none());
    assert!(disease.funding_note.is_none());
}

#[tokio::test]
async fn apply_requested_sections_clears_clinical_features_when_not_requested() {
    let mut disease = test_disease("MONDO:0005105", "melanoma");
    disease
        .clinical_features
        .push(clinical_feature_row("heavy menstrual bleeding"));

    apply_requested_sections(&mut disease, DiseaseSections::default(), None)
        .await
        .expect("sections should apply");

    assert!(disease.clinical_features.is_empty());
}

#[tokio::test]
async fn apply_requested_sections_preserves_clinical_features_when_requested() {
    let mut disease = test_disease("MONDO:0005105", "melanoma");
    disease
        .clinical_features
        .push(clinical_feature_row("heavy menstrual bleeding"));
    let sections = DiseaseSections {
        include_clinical_features: true,
        ..DiseaseSections::default()
    };

    apply_requested_sections(&mut disease, sections, None)
        .await
        .expect("sections should apply");

    assert_eq!(disease.clinical_features.len(), 1);
    assert_eq!(
        disease.clinical_features[0].label,
        "heavy menstrual bleeding"
    );
}

#[tokio::test]
async fn apply_requested_sections_populates_configured_clinical_features_from_fallback() {
    let _lock = lock_env().await;
    let _medline_env = set_env_var("BIOMCP_MEDLINEPLUS_BASE", Some("http://127.0.0.1:9"));
    let mut disease = test_disease("D007889", "uterine fibroids");
    disease
        .xrefs
        .insert("MESH".to_string(), "MESH:D007889".to_string());
    let sections = DiseaseSections {
        include_clinical_features: true,
        ..DiseaseSections::default()
    };

    with_no_http_cache(async {
        apply_requested_sections(&mut disease, sections, Some("uterine leiomyoma"))
            .await
            .expect("sections should apply");
    })
    .await;

    assert!(disease.clinical_features.iter().any(|row| {
        row.label == "heavy menstrual bleeding"
            && row.source == "MedlinePlus"
            && row.source_native_id == "uterinefibroids"
            && row.evidence_tier == "clinical_summary"
            && row.normalized_hpo_id.as_deref() == Some("HP:0000132")
    }));
}

#[tokio::test]
async fn disease_diagnostics_section_populates_from_who_fixture() {
    let _lock = lock_env().await;
    let gtr_root = TempDirGuard::new("disease-diagnostics-gtr");
    write_gtr_fixture(gtr_root.path());
    let _gtr_env = set_env_var(
        "BIOMCP_GTR_DIR",
        Some(gtr_root.path().to_str().expect("utf-8 path")),
    );
    let who_root = TempDirGuard::new("disease-diagnostics-who-ivd");
    write_who_ivd_fixture(who_root.path());
    let _who_env = set_env_var(
        "BIOMCP_WHO_IVD_DIR",
        Some(who_root.path().to_str().expect("utf-8 path")),
    );

    let mut disease = test_disease("MONDO:0018076", "tuberculosis");
    add_diagnostics_section(&mut disease).await;

    let rows = disease.diagnostics.as_ref().expect("diagnostics rows");
    assert_eq!(rows.len(), 10);
    assert_eq!(
        disease.diagnostics_note.as_deref(),
        Some(
            "Showing first 10 diagnostic matches in this disease card. Use diagnostic search with --limit and --offset for the larger result set."
        )
    );
    assert!(rows.iter().any(|row| {
        row.source == crate::entities::diagnostic::DIAGNOSTIC_SOURCE_WHO_IVD
            && row.name == "Loopamp MTBC Detection Kit"
            && row
                .conditions
                .iter()
                .any(|condition| condition == "Mycobacterium tuberculosis complex (MTBC)")
    }));
    assert!(rows.iter().any(|row| {
        row.source == crate::entities::diagnostic::DIAGNOSTIC_SOURCE_GTR
            && row.name.starts_with("Tuberculosis Molecular Panel")
    }));
}

#[tokio::test]
async fn disease_diagnostics_unavailable_sets_note() {
    let _lock = lock_env().await;
    let root = TempDirGuard::new("disease-diagnostics-unavailable");
    let blocking_root = root.path().join("not-a-directory");
    std::fs::write(&blocking_root, b"blocks create_dir_all").expect("write blocking file");
    let _gtr_env = set_env_var(
        "BIOMCP_GTR_DIR",
        Some(blocking_root.to_str().expect("utf-8 path")),
    );

    let mut disease = test_disease("MONDO:0018076", "tuberculosis");
    add_diagnostics_section(&mut disease).await;

    assert!(disease.diagnostics.is_none());
    assert_eq!(
        disease.diagnostics_note.as_deref(),
        Some(DISEASE_DIAGNOSTICS_UNAVAILABLE_NOTE)
    );
}

#[tokio::test]
async fn add_survival_section_sets_truthful_note_for_unmapped_disease() {
    let _lock = lock_env().await;
    with_no_http_cache(async {
        let server = MockServer::start().await;
        mock_seer_catalog(&server).await;
        let _seer_base = set_env_var("BIOMCP_SEER_BASE", Some(&server.uri()));

        let mut disease = test_disease("MONDO:0007947", "Marfan syndrome");
        add_survival_section(&mut disease)
            .await
            .expect("survival section");

        assert!(disease.survival.is_none());
        assert_eq!(
            disease.survival_note.as_deref(),
            Some(SURVIVAL_NO_DATA_NOTE)
        );
    })
    .await;
}

#[tokio::test]
async fn add_survival_section_sets_unavailable_note_when_catalog_fails() {
    let _lock = lock_env().await;
    with_no_http_cache(async {
        let server = MockServer::start().await;
        let _seer_base = set_env_var("BIOMCP_SEER_BASE", Some(&server.uri()));

        Mock::given(method("GET"))
            .and(path("/get_var_formats.php"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let mut disease = test_disease("MONDO:0004952", "Hodgkin's lymphoma");
        add_survival_section(&mut disease)
            .await
            .expect("survival section");

        assert!(disease.survival.is_none());
        assert_eq!(
            disease.survival_note.as_deref(),
            Some(SURVIVAL_UNAVAILABLE_NOTE)
        );
    })
    .await;
}

pub(crate) async fn proof_enrich_sparse_disease_identity_prefers_exact_ols4_match() {
    let _guard = lock_env().await;
    with_no_http_cache(async {
        let ols4 = MockServer::start().await;
        let _ols4_env = set_env_var("BIOMCP_OLS4_BASE", Some(&ols4.uri()));

        Mock::given(method("GET"))
            .and(path("/api/search"))
            .and(query_param("q", "MONDO:0019468"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "response": {
                    "docs": [
                        {
                            "iri": "http://purl.obolibrary.org/obo/MONDO_0019469",
                            "ontology_name": "mondo",
                            "ontology_prefix": "mondo",
                            "short_form": "MONDO_0019469",
                            "obo_id": "MONDO:0019469",
                            "label": "wrong disease",
                            "description": [],
                            "exact_synonyms": ["Wrong"],
                            "type": "class"
                        },
                        {
                            "iri": "http://purl.obolibrary.org/obo/MONDO_0019468",
                            "ontology_name": "mondo",
                            "ontology_prefix": "mondo",
                            "short_form": "MONDO_0019468",
                            "obo_id": "MONDO:0019468",
                            "label": "T-cell prolymphocytic leukemia",
                            "description": [],
                            "exact_synonyms": ["T-PLL"],
                            "type": "class"
                        }
                    ]
                }
            })))
            .mount(&ols4)
            .await;

        let mut disease = test_disease("MONDO:0019468", "MONDO:0019468");
        enrich_sparse_disease_identity(&mut disease)
            .await
            .expect("identity repair should succeed");

        assert_eq!(disease.name, "T-cell prolymphocytic leukemia");
        assert_eq!(disease.synonyms, vec!["T-PLL".to_string()]);
    })
    .await;
}

#[tokio::test]
async fn enrich_sparse_disease_identity_prefers_exact_ols4_match() {
    proof_enrich_sparse_disease_identity_prefers_exact_ols4_match().await;
}
