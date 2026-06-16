//! Tier 2 - request construction. Pure: builds the GraphQL `RequestPlan` and
//! asserts the method / path / JSON body that would be sent. Nothing is sent.

use super::super::*;
use crate::error::BioMcpError;
use crate::sources::{HttpMethod, RequestBody};

#[test]
fn gene_interactions_plan_sets_graphql_body() {
    let plan = DgidbClient::gene_interactions_plan(" braf ").expect("gene interactions plan");

    assert_eq!(plan.method, HttpMethod::Post);
    assert_eq!(plan.path, "graphql");
    let RequestBody::Json(body) = &plan.body else {
        panic!("expected JSON body, got {:?}", plan.body);
    };
    assert!(
        body["query"]
            .as_str()
            .unwrap()
            .contains("DgidbGeneDruggability")
    );
    assert_eq!(body["variables"]["gene"], "BRAF");
    assert_eq!(body["variables"]["first"], 1);
}

#[test]
fn gene_interactions_plan_rejects_invalid_symbols() {
    let empty = DgidbClient::gene_interactions_plan(" ").unwrap_err();
    assert!(matches!(empty, BioMcpError::InvalidArgument(_)));

    let invalid = DgidbClient::gene_interactions_plan("BRAF!").unwrap_err();
    assert!(matches!(invalid, BioMcpError::InvalidArgument(_)));
}
