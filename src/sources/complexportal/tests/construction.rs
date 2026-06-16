//! Tier 2 - request construction. Pure: builds request plans and validates
//! input checks. No network.

use super::super::*;
use crate::sources::HttpMethod;

#[test]
fn complexes_plan_sets_endpoint_filters_and_page_size() {
    let plan = ComplexPortalClient::complexes_plan(" P15056 ", 10)
        .unwrap()
        .unwrap();

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "search/P15056");
    assert_eq!(plan.query_value("number"), Some("25"));
    assert_eq!(
        plan.query_value("filters"),
        Some(r#"species_f:("Homo sapiens")"#)
    );
}

#[test]
fn complexes_plan_rejects_empty_accession_and_skips_zero_limit() {
    let err = ComplexPortalClient::complexes_plan(" ", 10).unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));

    assert!(
        ComplexPortalClient::complexes_plan("P15056", 0)
            .unwrap()
            .is_none()
    );
}
