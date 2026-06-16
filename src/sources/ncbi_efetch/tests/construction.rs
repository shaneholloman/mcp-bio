//! Tier 2 — request construction. Pure: builds `RequestPlan`s and asserts the exact
//! method / path / query that would be sent. Nothing is sent.

use crate::error::BioMcpError;
use crate::sources::HttpMethod;
use crate::sources::ncbi_efetch::NcbiEfetchClient;

#[test]
fn full_text_xml_plan_uses_numeric_pmcid() {
    let plan = NcbiEfetchClient::full_text_xml_plan(" PMC123456 ", None)
        .unwrap()
        .expect("plan");

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "efetch.fcgi");
    assert_eq!(plan.query_value("db"), Some("pmc"));
    assert_eq!(plan.query_value("id"), Some("123456"));
    assert_eq!(plan.query_value("rettype"), Some("xml"));
    assert!(!plan.has_query("api_key"));
}

#[test]
fn full_text_xml_plan_adds_api_key_when_configured() {
    let plan = NcbiEfetchClient::full_text_xml_plan("PMC123456", Some(" test-key "))
        .unwrap()
        .expect("plan");
    assert_eq!(plan.query_value("api_key"), Some("test-key"));
}

#[test]
fn full_text_xml_plan_empty_input_returns_none() {
    assert!(
        NcbiEfetchClient::full_text_xml_plan("   ", None)
            .unwrap()
            .is_none()
    );
}

#[test]
fn full_text_xml_plan_validates_pmcid_shape() {
    let err = NcbiEfetchClient::full_text_xml_plan("PMCabcd", None).unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
    assert!(err.to_string().contains("digits"));
}

#[test]
fn normalize_pmcid_accepts_prefixed_and_numeric_values() {
    assert_eq!(
        NcbiEfetchClient::normalize_pmcid("PMC123456")
            .unwrap()
            .as_deref(),
        Some("123456")
    );
    assert_eq!(
        NcbiEfetchClient::normalize_pmcid("123456")
            .unwrap()
            .as_deref(),
        Some("123456")
    );
}
