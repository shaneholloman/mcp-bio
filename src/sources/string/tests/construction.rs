//! Tier 2 - request construction. Pure: builds request plans and validates
//! input checks. No network.

use super::super::*;
use crate::sources::HttpMethod;

#[test]
fn interactions_plan_sets_expected_query_params() {
    let plan = StringClient::interactions_plan(" BRAF ", 9606, 99).unwrap();

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "json/network");
    assert_eq!(plan.query_value("identifiers"), Some("BRAF"));
    assert_eq!(plan.query_value("species"), Some("9606"));
    assert_eq!(plan.query_value("limit"), Some("25"));
}

#[test]
fn interactions_plan_rejects_empty_identifiers() {
    let err = StringClient::interactions_plan("   ", 9606, 5).unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
}
