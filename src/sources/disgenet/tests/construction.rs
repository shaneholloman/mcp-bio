//! Request construction tests. Pure: build request plans and inspect them.
//! No network.

use crate::sources::HttpMethod;

use super::*;

#[test]
fn gene_associations_plan_sends_auth_header_and_gene_ncbi_id() {
    let plan = DisgenetClient::gene_associations_plan(&test_gene("7157"), 10, "test-key")
        .unwrap()
        .unwrap();

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "api/v1/gda/summary");
    assert_eq!(plan.query_value("gene_ncbi_id"), Some("7157"));
    assert_eq!(plan.query_value("page_number"), Some("0"));
    assert_eq!(plan.header_value("Authorization"), Some("test-key"));
    assert_eq!(plan.header_value("accept"), Some("application/json"));
}

#[test]
fn gene_associations_plan_falls_back_to_gene_symbol() {
    let plan = DisgenetClient::gene_associations_plan(&test_gene(""), 1, "test-key")
        .unwrap()
        .unwrap();

    assert_eq!(plan.query_value("gene_symbol"), Some("TP53"));
    assert!(!plan.has_query("gene_ncbi_id"));
}

#[test]
fn gene_associations_plan_rejects_missing_gene_id_and_symbol() {
    let mut gene = test_gene("");
    gene.symbol = "   ".to_string();

    let err = DisgenetClient::gene_associations_plan(&gene, 10, "test-key").unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
}

#[test]
fn gene_associations_plan_skips_limit_zero() {
    let plan = DisgenetClient::gene_associations_plan(&test_gene("7157"), 0, "test-key").unwrap();
    assert_eq!(plan, None);
}

#[test]
fn disease_associations_plan_uses_normalized_umls_cui() {
    let disease = test_disease("breast cancer", Some("C0678222"));
    let disease_id = disease
        .xrefs
        .get("umls_cui")
        .and_then(|value| normalize_umls_cui(value))
        .unwrap();
    let plan = DisgenetClient::disease_associations_plan(&disease_id, 10, "test-key").unwrap();

    assert_eq!(plan.path, "api/v1/gda/summary");
    assert_eq!(plan.query_value("disease"), Some("UMLS_C0678222"));
    assert_eq!(plan.query_value("page_number"), Some("0"));
    assert_eq!(plan.header_value("Authorization"), Some("test-key"));
}

#[test]
fn disease_associations_plan_skips_limit_zero() {
    let plan = DisgenetClient::disease_associations_plan("UMLS_C0678222", 0, "test-key");
    assert_eq!(plan, None);
}

#[test]
fn disease_resolution_plan_sets_free_text_query() {
    let plan = DisgenetClient::disease_resolution_plan("breast cancer", "test-key");

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "api/v1/entity/disease");
    assert_eq!(
        plan.query_value("disease_free_text_search_string"),
        Some("breast cancer")
    );
    assert_eq!(plan.header_value("Authorization"), Some("test-key"));
    assert_eq!(plan.header_value("accept"), Some("application/json"));
}

#[test]
fn missing_key_returns_api_key_required_error() {
    let client = DisgenetClient {
        client: crate::sources::test_client().unwrap(),
        base: Cow::Borrowed(DISGENET_BASE),
        api_key: None,
    };

    let err = client.require_api_key().unwrap_err();
    assert!(matches!(err, BioMcpError::ApiKeyRequired { .. }));
}
