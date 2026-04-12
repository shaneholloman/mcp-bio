use super::super::test_support::*;
use super::*;

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
async fn add_survival_section_sets_truthful_note_for_unmapped_disease() {
    let _lock = lock_env().await;
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
}

#[tokio::test]
async fn add_survival_section_sets_unavailable_note_when_catalog_fails() {
    let _lock = lock_env().await;
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
}

pub(crate) async fn proof_enrich_sparse_disease_identity_prefers_exact_ols4_match() {
    let _guard = lock_env().await;
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
}

#[tokio::test]
async fn enrich_sparse_disease_identity_prefers_exact_ols4_match() {
    proof_enrich_sparse_disease_identity_prefers_exact_ols4_match().await;
}
