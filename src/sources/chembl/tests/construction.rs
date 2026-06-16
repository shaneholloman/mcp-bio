//! Tier 2 — request construction. Pure: builds `RequestPlan`s and asserts the exact
//! method / path / query that would be sent. Nothing is sent.

use super::super::*;
use crate::error::BioMcpError;
use crate::sources::HttpMethod;

#[test]
fn drug_targets_plan_requests_mechanism_endpoint() {
    let plan = ChemblClient::drug_targets_plan(" CHEMBL25 ", 99).unwrap();

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "mechanism.json");
    assert_eq!(plan.query_value("molecule_chembl_id"), Some("CHEMBL25"));
    assert_eq!(plan.query_value("limit"), Some("25"));
}

#[test]
fn target_summary_plan_sets_target_path() {
    let plan = ChemblClient::target_summary_plan(" CHEMBL3390820 ").unwrap();

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "target/CHEMBL3390820.json");
    assert!(plan.query.is_empty());
}

#[test]
fn plans_reject_empty_identifiers() {
    assert!(matches!(
        ChemblClient::drug_targets_plan(" ", 5),
        Err(BioMcpError::InvalidArgument(_))
    ));
    assert!(matches!(
        ChemblClient::target_summary_plan(" ",),
        Err(BioMcpError::InvalidArgument(_))
    ));
}
