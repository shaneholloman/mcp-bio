use super::super::DiseaseClinicalFeature;
use super::super::test_support::*;
use super::*;

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

fn diagnostic_row(
    source: &str,
    accession: &str,
    name: &str,
    conditions: &[&str],
) -> crate::entities::diagnostic::DiagnosticSearchResult {
    crate::entities::diagnostic::DiagnosticSearchResult {
        source: source.to_string(),
        accession: accession.to_string(),
        name: name.to_string(),
        test_type: Some("molecular".to_string()),
        manufacturer_or_lab: Some("Example Lab".to_string()),
        genes: Vec::new(),
        conditions: conditions
            .iter()
            .map(|condition| condition.to_string())
            .collect(),
    }
}

fn seer_catalog_fixture() -> SeerSiteCatalog {
    let body = serde_json::to_vec(&serde_json::json!({
        "VariableFormats": {
            "site": {
                "1": "All Cancer Sites Combined",
                "83": "Hodgkin Lymphoma",
                "97": "Chronic Myeloid Leukemia (CML)"
            },
            "sex": {
                "1": "Both Sexes",
                "2": "Male",
                "3": "Female"
            },
            "race": {
                "1": "All Races / Ethnicities"
            },
            "age_range": {
                "1": "All Ages"
            }
        },
        "CancerSites": [
            {"value": 1, "active": true},
            {"value": 83, "active": true},
            {"value": 97, "active": true}
        ]
    }))
    .expect("catalog json");

    SeerClient::decode_site_catalog_response(
        reqwest::StatusCode::OK,
        Some(&reqwest::header::HeaderValue::from_static(
            "application/json",
        )),
        &body,
    )
    .expect("valid SEER catalog")
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

#[test]
fn disease_diagnostics_section_populates_from_rows() {
    let mut disease = test_disease("MONDO:0018076", "tuberculosis");
    apply_diagnostics_section_result(
        &mut disease,
        "tuberculosis",
        Ok(SearchPage::offset(
            vec![
                diagnostic_row(
                    crate::entities::diagnostic::DIAGNOSTIC_SOURCE_WHO_IVD,
                    "WHO-IVD-1",
                    "Loopamp MTBC Detection Kit",
                    &["Mycobacterium tuberculosis complex (MTBC)"],
                ),
                diagnostic_row(
                    crate::entities::diagnostic::DIAGNOSTIC_SOURCE_GTR,
                    "GTR000000002.1",
                    "Tuberculosis Molecular Panel",
                    &["tuberculosis"],
                ),
            ],
            Some(12),
        )),
    );

    let rows = disease.diagnostics.as_ref().expect("diagnostics rows");
    assert_eq!(rows.len(), 2);
    assert_eq!(
        disease.diagnostics_note.as_deref(),
        Some(
            "Showing 2 of 12 diagnostic matches in this disease card. Use diagnostic search with --limit and --offset for the larger result set."
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

#[test]
fn disease_diagnostics_unavailable_sets_note() {
    let mut disease = test_disease("MONDO:0018076", "tuberculosis");
    apply_diagnostics_section_result(
        &mut disease,
        "tuberculosis",
        Err(BioMcpError::SourceUnavailable {
            source_name: "gtr".to_string(),
            reason: "fixture directory is unavailable".to_string(),
            suggestion: "Run `biomcp gtr sync`".to_string(),
        }),
    );

    assert!(disease.diagnostics.is_none());
    assert_eq!(
        disease.diagnostics_note.as_deref(),
        Some(DISEASE_DIAGNOSTICS_UNAVAILABLE_NOTE)
    );
}

#[test]
fn survival_catalog_resolution_sets_truthful_note_for_unmapped_disease() {
    let mut disease = test_disease("MONDO:0007947", "Marfan syndrome");

    let site = resolve_survival_site_from_catalog_result(&mut disease, Ok(seer_catalog_fixture()));

    assert!(site.is_none());
    assert!(disease.survival.is_none());
    assert_eq!(
        disease.survival_note.as_deref(),
        Some(SURVIVAL_NO_DATA_NOTE)
    );
}

#[test]
fn survival_catalog_resolution_sets_unavailable_note_when_catalog_fails() {
    let mut disease = test_disease("MONDO:0004952", "Hodgkin's lymphoma");

    let site = resolve_survival_site_from_catalog_result(
        &mut disease,
        Err(BioMcpError::Api {
            api: "SEER Explorer".into(),
            message: "catalog failed".into(),
        }),
    );

    assert!(site.is_none());
    assert!(disease.survival.is_none());
    assert_eq!(
        disease.survival_note.as_deref(),
        Some(SURVIVAL_UNAVAILABLE_NOTE)
    );
}

pub(crate) async fn proof_enrich_sparse_disease_identity_prefers_exact_ols4_match() {
    let mut disease = test_disease("MONDO:0019468", "MONDO:0019468");
    apply_sparse_disease_identity_docs(
        &mut disease,
        "MONDO:0019468",
        vec![
            ols_doc("MONDO:0019469", "wrong disease", &["Wrong"]),
            ols_doc(
                "MONDO:0019468",
                "T-cell prolymphocytic leukemia",
                &["T-PLL"],
            ),
        ],
    );

    assert_eq!(disease.name, "T-cell prolymphocytic leukemia");
    assert_eq!(disease.synonyms, vec!["T-PLL".to_string()]);
}

#[tokio::test]
async fn enrich_sparse_disease_identity_prefers_exact_ols4_match() {
    proof_enrich_sparse_disease_identity_prefers_exact_ols4_match().await;
}

fn ols_doc(id: &str, label: &str, synonyms: &[&str]) -> crate::sources::ols4::OlsDoc {
    crate::sources::ols4::OlsDoc {
        iri: format!("http://purl.obolibrary.org/obo/{}", id.replace(':', "_")),
        ontology_name: "mondo".into(),
        ontology_prefix: "mondo".into(),
        short_form: Some(id.replace(':', "_")),
        obo_id: Some(id.into()),
        label: label.into(),
        description: Vec::new(),
        exact_synonyms: synonyms.iter().map(|value| (*value).to_string()).collect(),
        is_defining_ontology: false,
        doc_type: Some("class".into()),
    }
}
