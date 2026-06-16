//! Tier 2 — request construction. Pure: builds the GraphQL `RequestPlan` and
//! asserts the exact method / path / JSON body that would be sent. Nothing is sent.

use super::super::*;
use crate::error::BioMcpError;
use crate::sources::{HttpMethod, RequestBody};

#[test]
fn gene_constraint_plan_posts_graphql_query_and_symbol() {
    let plan = GnomadClient::gene_constraint_plan(" TP53 ").unwrap();

    assert_eq!(plan.method, HttpMethod::Post);
    assert_eq!(plan.path, "");
    assert!(plan.query.is_empty());
    let RequestBody::Json(body) = &plan.body else {
        panic!("expected JSON body, got {:?}", plan.body);
    };
    assert!(body["query"].as_str().unwrap().contains("GeneConstraint"));
    assert_eq!(body["variables"]["symbol"], "TP53");
}

#[test]
fn gene_constraint_plan_rejects_invalid_gene_symbols() {
    for symbol in ["", "TP 53", "TP53/ALK"] {
        assert!(
            matches!(
                GnomadClient::gene_constraint_plan(symbol),
                Err(BioMcpError::InvalidArgument(_))
            ),
            "expected invalid argument for {symbol:?}"
        );
    }
}
