//! Tier 2 — request construction. Pure: builds `RequestPlan`s and asserts the exact
//! method / path / query that would be sent. Nothing is sent.

use crate::error::BioMcpError;
use crate::sources::HttpMethod;
use crate::sources::mychem::{MYCHEM_FIELDS_SEARCH, MyChemClient};

#[test]
fn query_with_fields_plan_sets_path_and_core_query_params() {
    let plan =
        MyChemClient::query_with_fields_plan(" imatinib ", 5, 2, MYCHEM_FIELDS_SEARCH).unwrap();

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "query");
    assert_eq!(plan.query_value("q"), Some("imatinib"));
    assert_eq!(plan.query_value("size"), Some("5"));
    assert_eq!(plan.query_value("from"), Some("2"));
    assert_eq!(plan.query_value("fields"), Some(MYCHEM_FIELDS_SEARCH));
}

#[test]
fn query_with_fields_plan_rejects_empty_query() {
    let err = MyChemClient::query_with_fields_plan("   ", 5, 0, MYCHEM_FIELDS_SEARCH).unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
    assert!(err.to_string().contains("Query is required"));
}

#[test]
fn query_with_fields_plan_rejects_overlong_query() {
    let err = MyChemClient::query_with_fields_plan(&"x".repeat(1025), 5, 0, MYCHEM_FIELDS_SEARCH)
        .unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
    assert!(err.to_string().contains("too long"));
}

#[test]
fn query_with_fields_plan_rejects_limit_out_of_range() {
    for limit in [0, 51] {
        let err = MyChemClient::query_with_fields_plan("imatinib", limit, 0, MYCHEM_FIELDS_SEARCH)
            .unwrap_err();
        assert!(matches!(err, BioMcpError::InvalidArgument(_)));
        assert!(err.to_string().contains("--limit"));
    }
}

#[test]
fn query_with_fields_plan_rejects_offset_at_biothings_window() {
    let err = MyChemClient::query_with_fields_plan("imatinib", 5, 10_000, MYCHEM_FIELDS_SEARCH)
        .unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
    assert!(err.to_string().contains("--offset must be less than 10000"));
}

#[test]
fn query_with_fields_plan_rejects_offset_limit_window_overflow() {
    let err = MyChemClient::query_with_fields_plan("imatinib", 30, 9_980, MYCHEM_FIELDS_SEARCH)
        .unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
    assert!(
        err.to_string()
            .contains("--offset + --limit must be <= 10000")
    );
}
