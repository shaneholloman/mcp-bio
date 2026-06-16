//! Tier 2 — request construction. Pure: builds `RequestPlan`s and asserts the exact
//! method / path / query / auth header that would be sent. Nothing is sent.

use super::super::*;
use crate::error::BioMcpError;
use crate::sources::HttpMethod;

#[test]
fn annotate_plan_sets_query_and_auth_header() {
    let plan =
        OncoKBClient::annotate_by_protein_change_plan(" BRAF ", " V600E ", " test-token ").unwrap();

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "annotate/mutations/byProteinChange");
    assert_eq!(plan.query_value("hugoSymbol"), Some("BRAF"));
    assert_eq!(plan.query_value("alteration"), Some("V600E"));
    assert_eq!(
        plan.header_value("authorization"),
        Some("Bearer test-token")
    );
}

#[test]
fn annotate_plan_requires_gene_and_alteration() {
    let err = OncoKBClient::annotate_by_protein_change_plan("", "V600E", "test-token")
        .expect_err("empty gene should fail");
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));

    let err = OncoKBClient::annotate_by_protein_change_plan("BRAF", "", "test-token")
        .expect_err("empty alteration should fail");
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
}

#[test]
fn annotate_plan_requires_api_key() {
    let err = OncoKBClient::annotate_by_protein_change_plan("BRAF", "V600E", " ")
        .expect_err("empty token should fail");

    assert!(matches!(err, BioMcpError::ApiKeyRequired { .. }));
}

#[test]
fn protein_change_attempts_try_original_and_prefixed_forms_without_duplicates() {
    assert_eq!(
        protein_change_attempts("V600E"),
        vec!["V600E".to_string(), "p.V600E".to_string()]
    );
    assert_eq!(
        protein_change_attempts("p.V600E"),
        vec!["p.V600E".to_string(), "V600E".to_string()]
    );
    assert_eq!(
        protein_change_attempts(" P.V600E "),
        vec!["P.V600E".to_string(), "V600E".to_string()]
    );
}
