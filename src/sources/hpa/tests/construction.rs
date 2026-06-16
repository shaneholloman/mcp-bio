//! Tier 2 - request construction. Pure: builds `RequestPlan`s and asserts the
//! exact request shape. No network.

use super::super::*;
use crate::sources::HttpMethod;

#[test]
fn protein_data_plan_normalizes_ensembl_id_before_request() {
    let plan = HpaClient::protein_data_plan(" ensg00000157766.12 ").unwrap();

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "ENSG00000157766.xml");
    assert!(plan.query.is_empty());
}

#[test]
fn protein_data_plan_rejects_invalid_ensembl_id() {
    let err = HpaClient::protein_data_plan(" ").unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));

    let err = HpaClient::protein_data_plan("BRAF").unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
}
