//! Tests for NCI CTS trial search helpers.

use super::super::super::test_support::*;
use super::super::{search, search_page, validate_trial_search};
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

#[tokio::test]
async fn nci_search_page_falls_back_to_keyword_when_grounding_returns_no_hit() {
    let mydisease = MockServer::start().await;
    let nci = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "total": 0,
            "hits": []
        })))
        .mount(&mydisease)
        .await;

    mount_keyword_nci_search(&nci, "melanoma", "NCT00000009").await;

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
    .expect("missing MyDisease hits should fall back to keyword search");
    assert_eq!(page.results.len(), 1);
    assert_eq!(page.results[0].nct_id, "NCT00000009");
}

#[tokio::test]
async fn nci_status_mapping_uses_documented_single_value_filters() {
    let _lock = lock_env().await;
    let nci = MockServer::start().await;
    let _nci_base = set_env_var("BIOMCP_NCI_CTS_BASE", Some(&nci.uri()));
    let _nci_key = set_env_var("NCI_API_KEY", Some("test-key"));

    let cases = [
        (
            "recruiting",
            "sites.recruitment_status",
            "ACTIVE",
            "NCT00000003",
        ),
        (
            "not yet recruiting",
            "current_trial_status",
            "Approved",
            "NCT00000004",
        ),
        (
            "enrolling by invitation",
            "current_trial_status",
            "Enrolling by Invitation",
            "NCT00000005",
        ),
        (
            "active, not recruiting",
            "sites.recruitment_status",
            "CLOSED_TO_ACCRUAL",
            "NCT00000006",
        ),
        (
            "completed",
            "current_trial_status",
            "Complete",
            "NCT00000007",
        ),
        (
            "suspended",
            "current_trial_status",
            "Temporarily Closed to Accrual",
            "NCT00000010",
        ),
        (
            "terminated",
            "current_trial_status",
            "Administratively Complete",
            "NCT00000011",
        ),
        (
            "withdrawn",
            "current_trial_status",
            "Withdrawn",
            "NCT00000012",
        ),
    ];

    for &(_, query_key, query_value, nct_id) in &cases {
        match query_key {
            "current_trial_status" => {
                Mock::given(method("GET"))
                    .and(path("/trials"))
                    .and(query_param("current_trial_status", query_value))
                    .and(query_param_is_missing("sites.recruitment_status"))
                    .and(query_param("size", "1"))
                    .and(query_param("from", "0"))
                    .respond_with(
                        ResponseTemplate::new(200).set_body_json(nci_search_response(nct_id)),
                    )
                    .expect(1)
                    .mount(&nci)
                    .await;
            }
            "sites.recruitment_status" => {
                Mock::given(method("GET"))
                    .and(path("/trials"))
                    .and(query_param("sites.recruitment_status", query_value))
                    .and(query_param_is_missing("current_trial_status"))
                    .and(query_param("size", "1"))
                    .and(query_param("from", "0"))
                    .respond_with(
                        ResponseTemplate::new(200).set_body_json(nci_search_response(nct_id)),
                    )
                    .expect(1)
                    .mount(&nci)
                    .await;
            }
            other => panic!("unexpected query key: {other}"),
        }
    }

    for &(input, _, _, nct_id) in &cases {
        let filters = TrialSearchFilters {
            source: TrialSource::NciCts,
            status: Some(input.into()),
            ..Default::default()
        };

        let page = search_page(&filters, 1, 0, None)
            .await
            .unwrap_or_else(|_| panic!("{input} should map to a documented NCI status"));
        assert_eq!(page.results.len(), 1);
        assert_eq!(page.results[0].nct_id, nct_id);
    }
}

#[tokio::test]
async fn nci_source_rejects_status_lists() {
    let _lock = lock_env().await;
    let nci = MockServer::start().await;
    let _nci_base = set_env_var("BIOMCP_NCI_CTS_BASE", Some(&nci.uri()));
    let _nci_key = set_env_var("NCI_API_KEY", Some("test-key"));

    Mock::given(method("GET"))
        .and(path("/trials"))
        .respond_with(ResponseTemplate::new(200).set_body_json(nci_search_response("NCT00000005")))
        .mount(&nci)
        .await;

    let filters = TrialSearchFilters {
        source: TrialSource::NciCts,
        status: Some("recruiting,completed".into()),
        ..Default::default()
    };

    let err = search(&filters, 1, 0)
        .await
        .expect_err("NCI should reject comma-separated status lists");
    assert!(err.to_string().contains("one mapped status at a time"));
    assert!(err.to_string().contains("--source nci"));
}

#[tokio::test]
async fn nci_phase_mapping_uses_i_ii_for_combined_phase() {
    let _lock = lock_env().await;
    let nci = MockServer::start().await;
    let _nci_base = set_env_var("BIOMCP_NCI_CTS_BASE", Some(&nci.uri()));
    let _nci_key = set_env_var("NCI_API_KEY", Some("test-key"));

    let cases = [
        ("1", "I", "NCT00000016"),
        ("2", "II", "NCT00000013"),
        ("3", "III", "NCT00000017"),
        ("4", "IV", "NCT00000018"),
        ("na", "NA", "NCT00000014"),
        ("1/2", "I_II", "NCT00000015"),
    ];

    for &(_, expected_phase, nct_id) in &cases {
        Mock::given(method("GET"))
            .and(path("/trials"))
            .and(query_param("phase", expected_phase))
            .and(query_param("size", "1"))
            .and(query_param("from", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(nci_search_response(nct_id)))
            .expect(1)
            .mount(&nci)
            .await;
    }

    for &(input_phase, _, nct_id) in &cases {
        let filters = TrialSearchFilters {
            source: TrialSource::NciCts,
            phase: Some(input_phase.into()),
            ..Default::default()
        };

        let page = search_page(&filters, 1, 0, None)
            .await
            .unwrap_or_else(|_| panic!("{input_phase} should map to a documented NCI phase"));
        assert_eq!(page.results.len(), 1);
        assert_eq!(page.results[0].nct_id, nct_id);
    }
}

#[tokio::test]
async fn nci_source_rejects_early_phase1() {
    let _lock = lock_env().await;
    let nci = MockServer::start().await;
    let _nci_base = set_env_var("BIOMCP_NCI_CTS_BASE", Some(&nci.uri()));
    let _nci_key = set_env_var("NCI_API_KEY", Some("test-key"));

    Mock::given(method("GET"))
        .and(path("/trials"))
        .respond_with(ResponseTemplate::new(200).set_body_json(nci_search_response("NCT00000007")))
        .mount(&nci)
        .await;

    let filters = TrialSearchFilters {
        source: TrialSource::NciCts,
        phase: Some("early_phase1".into()),
        ..Default::default()
    };

    let err = search(&filters, 1, 0)
        .await
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
