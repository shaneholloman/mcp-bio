//! Tests for NCI CTS trial search helpers.

use super::super::super::test_support::*;
use super::super::{search, validate_trial_search};
use super::*;
use crate::sources::mydisease::MyDiseaseClient;
use crate::sources::nci_cts::NciCtsClient;

fn nci_search_response(nct_id: &str) -> serde_json::Value {
    json!({
        "data": [{
            "nct_id": nct_id,
            "brief_title": "Fixture NCI Trial",
            "current_trial_status": "ACTIVE",
            "phase": "II",
            "diseases": ["Melanoma"]
        }],
        "total": 1
    })
}

fn mydisease_query_response(hit: serde_json::Value) -> serde_json::Value {
    json!({
        "total": 1,
        "hits": [hit]
    })
}

fn mydisease_client_for_test(server: &MockServer) -> MyDiseaseClient {
    MyDiseaseClient::new_for_test(format!("{}/v1", server.uri())).expect("mydisease client")
}

fn nci_client_for_test(server: &MockServer) -> NciCtsClient {
    NciCtsClient::new_for_test(server.uri(), "test-key".into()).expect("nci client")
}

async fn mount_keyword_nci_search(nci: &MockServer, keyword: &str, nct_id: &str) {
    Mock::given(method("GET"))
        .and(path("/trials"))
        .and(query_param("keyword", keyword))
        .and(query_param_is_missing("diseases.nci_thesaurus_concept_id"))
        .and(query_param("size", "1"))
        .and(query_param("from", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(nci_search_response(nct_id)))
        .expect(1)
        .mount(nci)
        .await;
}

#[tokio::test]
async fn nci_search_page_prefers_grounded_disease_concept_id() {
    let mydisease = MockServer::start().await;
    let nci = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/query"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mydisease_query_response(json!({
                "_id": "MONDO:0005105",
                "mondo": {
                    "name": "Melanoma",
                    "xrefs": {
                        "ncit": ["C3224"]
                    }
                }
            }))),
        )
        .mount(&mydisease)
        .await;

    Mock::given(method("GET"))
        .and(path("/trials"))
        .and(query_param("diseases.nci_thesaurus_concept_id", "C3224"))
        .and(query_param_is_missing("keyword"))
        .and(query_param("size", "1"))
        .and(query_param("from", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(nci_search_response("NCT00000001")))
        .expect(1)
        .mount(&nci)
        .await;

    let filters = TrialSearchFilters {
        source: TrialSource::NciCts,
        condition: Some("melanoma".into()),
        ..Default::default()
    };
    let normalized = validate_trial_search(&filters).expect("filters should validate");

    let page = search_page_with_nci_clients(
        &nci_client_for_test(&nci),
        &mydisease_client_for_test(&mydisease),
        &filters,
        &normalized,
        1,
        0,
    )
    .await
    .expect("grounded NCI search should succeed");
    assert_eq!(page.results.len(), 1);
    assert_eq!(page.results[0].nct_id, "NCT00000001");
}

#[tokio::test]
async fn nci_search_page_falls_back_to_keyword_when_grounding_is_unavailable() {
    let mydisease = MockServer::start().await;
    let nci = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/query"))
        .respond_with(ResponseTemplate::new(500).set_body_string("resolver unavailable"))
        .mount(&mydisease)
        .await;

    mount_keyword_nci_search(&nci, "melanoma", "NCT00000002").await;

    let filters = TrialSearchFilters {
        source: TrialSource::NciCts,
        condition: Some("melanoma".into()),
        ..Default::default()
    };
    let normalized = validate_trial_search(&filters).expect("filters should validate");

    let page = search_page_with_nci_clients(
        &nci_client_for_test(&nci),
        &mydisease_client_for_test(&mydisease),
        &filters,
        &normalized,
        1,
        0,
    )
    .await
    .expect("keyword fallback should keep NCI search available");
    assert_eq!(page.results.len(), 1);
    assert_eq!(page.results[0].nct_id, "NCT00000002");
}

#[tokio::test]
async fn nci_search_page_falls_back_to_keyword_when_best_hit_lacks_nci_xref() {
    let mydisease = MockServer::start().await;
    let nci = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/query"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mydisease_query_response(json!({
                "_id": "MONDO:0005105",
                "mondo": {
                    "name": "Melanoma"
                }
            }))),
        )
        .mount(&mydisease)
        .await;

    mount_keyword_nci_search(&nci, "melanoma", "NCT00000008").await;

    let filters = TrialSearchFilters {
        source: TrialSource::NciCts,
        condition: Some("melanoma".into()),
        ..Default::default()
    };
    let normalized = validate_trial_search(&filters).expect("filters should validate");

    let page = search_page_with_nci_clients(
        &nci_client_for_test(&nci),
        &mydisease_client_for_test(&mydisease),
        &filters,
        &normalized,
        1,
        0,
    )
    .await
    .expect("missing NCI xrefs should fall back to keyword search");
    assert_eq!(page.results.len(), 1);
    assert_eq!(page.results[0].nct_id, "NCT00000008");
}

#[test]
fn nci_keyword_fallback_request_uses_keyword_not_concept_id() {
    let plan = NciCtsClient::search_plan(
        "test-key",
        &NciSearchParams {
            disease: Some(NciDiseaseFilter::Keyword("melanoma".into())),
            size: 1,
            from: 0,
            ..NciSearchParams::default()
        },
    );

    assert!(plan.query.contains(&("keyword".into(), "melanoma".into())));
    assert!(
        !plan
            .query
            .iter()
            .any(|(key, _)| *key == "diseases.nci_thesaurus_concept_id")
    );
}

#[test]
fn nci_status_mapping_uses_documented_single_value_filters() {
    let cases = [
        ("recruiting", "site", "ACTIVE"),
        ("not yet recruiting", "current", "Approved"),
        (
            "enrolling by invitation",
            "current",
            "Enrolling by Invitation",
        ),
        ("active, not recruiting", "site", "CLOSED_TO_ACCRUAL"),
        ("completed", "current", "Complete"),
        ("suspended", "current", "Temporarily Closed to Accrual"),
        ("terminated", "current", "Administratively Complete"),
        ("withdrawn", "current", "Withdrawn"),
    ];

    for &(input, expected_kind, expected_value) in &cases {
        let normalized = validate_trial_search(&TrialSearchFilters {
            source: TrialSource::NciCts,
            status: Some(input.into()),
            ..Default::default()
        })
        .expect("status should normalize");
        let filter = nci_status_filter(normalized.normalized_status.as_deref())
            .expect("status should map")
            .expect("status filter");
        match (expected_kind, filter) {
            ("current", NciStatusFilter::CurrentTrialStatus(value)) => {
                assert_eq!(value, expected_value);
            }
            ("site", NciStatusFilter::SiteRecruitmentStatus(value)) => {
                assert_eq!(value, expected_value);
            }
            (_, other) => panic!("unexpected status filter for {input}: {other:?}"),
        }
    }
}

#[test]
fn nci_source_rejects_status_lists() {
    let err = nci_status_filter(Some("RECRUITING,COMPLETED"))
        .expect_err("NCI should reject comma-separated status lists");
    assert!(err.to_string().contains("one mapped status at a time"));
    assert!(err.to_string().contains("--source nci"));
}

#[test]
fn nci_phase_mapping_uses_i_ii_for_combined_phase() {
    let cases = [
        ("1", vec!["I"]),
        ("2", vec!["II"]),
        ("3", vec!["III"]),
        ("4", vec!["IV"]),
        ("na", vec!["NA"]),
        ("1/2", vec!["I_II"]),
    ];

    for (input_phase, expected) in cases {
        let normalized = validate_trial_search(&TrialSearchFilters {
            source: TrialSource::NciCts,
            phase: Some(input_phase.into()),
            ..Default::default()
        })
        .expect("phase should normalize");
        assert_eq!(
            nci_phase_filters(normalized.normalized_phase.as_deref()).expect("phase should map"),
            expected
        );
    }
}

#[test]
fn nci_source_rejects_early_phase1() {
    let err = nci_phase_filters(Some(&["EARLY_PHASE1".to_string()]))
        .expect_err("NCI should reject early_phase1");
    assert!(err.to_string().contains("early_phase1"));
    assert!(err.to_string().contains("--source nci"));
}

#[tokio::test]
async fn nci_source_rejects_essie_filters() {
    let filters = TrialSearchFilters {
        source: TrialSource::NciCts,
        prior_therapies: Some("platinum".into()),
        ..Default::default()
    };

    let err = search(&filters, 10, 0).await.expect_err("should fail");
    assert!(
        format!("{err}").contains("--prior-therapies, --progression-on, and --line-of-therapy"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn nci_source_rejects_age_filter() {
    let filters = TrialSearchFilters {
        source: TrialSource::NciCts,
        condition: Some("melanoma".into()),
        age: Some(67.0),
        ..Default::default()
    };

    let err = search(&filters, 10, 0).await.expect_err("should fail");
    assert!(
        format!("{err}").contains("--age is only supported for --source ctgov"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn nci_source_rejects_sex_filter() {
    let filters = TrialSearchFilters {
        source: TrialSource::NciCts,
        condition: Some("melanoma".into()),
        sex: Some("female".into()),
        ..Default::default()
    };

    let err = search(&filters, 10, 0).await.expect_err("should fail");
    assert!(
        format!("{err}").contains("--sex is only supported for --source ctgov"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn nci_source_rejects_sponsor_type_filter() {
    let filters = TrialSearchFilters {
        source: TrialSource::NciCts,
        condition: Some("melanoma".into()),
        sponsor_type: Some("nih".into()),
        ..Default::default()
    };

    let err = search(&filters, 10, 0).await.expect_err("should fail");
    assert!(
        format!("{err}").contains("--sponsor-type is only supported for --source ctgov"),
        "unexpected error: {err}"
    );
}
