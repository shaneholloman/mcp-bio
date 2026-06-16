//! Tier 2 - request construction. Pure: builds the OLS request plan and asserts
//! the exact query contract. No network.

use super::super::{OLS4_ONTOLOGIES, OlsClient};

#[test]
fn search_request_plan_exposes_canonical_query_contract() {
    let client = OlsClient::new_for_test("http://127.0.0.1/base".into()).expect("client");
    let plan = client.search_request_plan(" ERBB1 ");

    assert_eq!(plan.method, "GET");
    assert_eq!(plan.path, Some("/api/search"));
    assert_eq!(plan.source_label, "ols4");
    assert_eq!(plan.base_url, "http://127.0.0.1/base");
    assert_eq!(plan.cache_mode, "default");
    assert_eq!(plan.status_expectation, "non-2xx => Api");
    assert_eq!(plan.content_type_expectation, "json");
    assert_eq!(
        plan.query_params,
        vec![
            ("q", "ERBB1".to_string()),
            ("rows", "10".to_string()),
            ("groupField", "iri".to_string()),
            ("ontology", OLS4_ONTOLOGIES.to_string()),
        ]
    );
}

#[test]
fn search_request_plan_keeps_empty_query_as_no_request() {
    let client = OlsClient::new_for_test("http://127.0.0.1".into()).expect("client");
    let plan = client.search_request_plan("   ");

    assert_eq!(plan.method, "GET");
    assert_eq!(plan.path, None);
    assert!(plan.query_params.is_empty());
}
