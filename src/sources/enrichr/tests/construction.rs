//! Tier 2 - request construction. Pure: builds add-list bodies and enrich query
//! plans. No network.

use super::super::*;
use crate::sources::HttpMethod;

#[test]
fn add_list_body_trims_and_joins_genes() {
    let body = EnrichrClient::add_list_body(&[" BRAF ", "", "KRAS"]).unwrap();
    assert_eq!(body, "BRAF\nKRAS");
}

#[test]
fn add_list_body_rejects_empty_gene_lists() {
    let err = EnrichrClient::add_list_body(&[]).unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));

    let err = EnrichrClient::add_list_body(&[" ", ""]).unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
}

#[test]
fn enrich_plan_sets_user_list_id_and_library() {
    let plan = EnrichrClient::enrich_plan(42, "KEGG_2021_Human");

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "enrich");
    assert_eq!(plan.query_value("userListId"), Some("42"));
    assert_eq!(plan.query_value("backgroundType"), Some("KEGG_2021_Human"));
}
