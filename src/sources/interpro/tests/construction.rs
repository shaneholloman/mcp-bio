//! Tier 2 - request construction. Pure: builds `RequestPlan`s and asserts the
//! exact request shape. No network.

use super::super::*;
use crate::sources::HttpMethod;

#[test]
fn domains_plan_requests_expected_endpoint_and_page_size() {
    let plan = InterProClient::domains_plan(" P15056 ", 3).unwrap();

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "entry/interpro/protein/uniprot/P15056/");
    assert_eq!(plan.query_value("page_size"), Some("3"));
}

#[test]
fn domains_plan_rejects_empty_accession_and_clamps_limit() {
    let err = InterProClient::domains_plan(" ", 5).unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));

    let plan = InterProClient::domains_plan("P15056", 99).unwrap();
    assert_eq!(plan.query_value("page_size"), Some("25"));
}
