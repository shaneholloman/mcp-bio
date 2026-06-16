//! Tier 2 — request construction. Pure: builds `RequestPlan`s and asserts the exact
//! method / path / query that would be sent. Nothing is sent.

use crate::error::BioMcpError;
use crate::sources::HttpMethod;
use crate::sources::ncbi_idconv::NcbiIdConverterClient;

#[test]
fn pmid_to_pmcid_plan_builds_lookup_query() {
    let plan = NcbiIdConverterClient::pmid_to_pmcid_plan(" 22663011 ", None)
        .unwrap()
        .expect("plan");

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "");
    assert_eq!(plan.query_value("format"), Some("json"));
    assert_eq!(plan.query_value("idtype"), Some("pmid"));
    assert_eq!(plan.query_value("ids"), Some("22663011"));
    assert!(!plan.has_query("api_key"));
}

#[test]
fn pmid_to_pmcid_plan_adds_api_key_when_configured() {
    let plan = NcbiIdConverterClient::pmid_to_pmcid_plan("22663011", Some(" test-key "))
        .unwrap()
        .expect("plan");
    assert_eq!(plan.query_value("api_key"), Some("test-key"));
}

#[test]
fn pmid_to_pmcid_plan_empty_input_returns_none() {
    assert!(
        NcbiIdConverterClient::pmid_to_pmcid_plan("   ", None)
            .unwrap()
            .is_none()
    );
}

#[test]
fn pmid_to_pmcid_plan_validates_numeric_input() {
    let err = NcbiIdConverterClient::pmid_to_pmcid_plan("abc", None).unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
    assert!(err.to_string().contains("digits"));
}

#[test]
fn doi_to_pmcid_plan_builds_lookup_query() {
    let plan = NcbiIdConverterClient::doi_to_pmcid_plan(" 10.1038/nature12373 ", None)
        .unwrap()
        .expect("plan");

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "");
    assert_eq!(plan.query_value("format"), Some("json"));
    assert_eq!(plan.query_value("idtype"), Some("doi"));
    assert_eq!(plan.query_value("ids"), Some("10.1038/nature12373"));
}

#[test]
fn doi_to_pmcid_plan_validates_shape() {
    let err = NcbiIdConverterClient::doi_to_pmcid_plan("not-a-doi", None).unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
    assert!(err.to_string().contains("10."));
}
