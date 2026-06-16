//! Tier 2 - request construction. Pure: builds QuickGO request plans and
//! validates input checks. No network.

use super::super::*;
use crate::sources::HttpMethod;

#[test]
fn annotations_plan_sets_expected_query_params() {
    let plan = QuickGoClient::annotations_plan(" P15056 ", 99).unwrap();

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "annotation/search");
    assert_eq!(plan.query_value("geneProductId"), Some("P15056"));
    assert_eq!(plan.query_value("limit"), Some("25"));
}

#[test]
fn annotations_plan_rejects_empty_gene_product_id() {
    let err = QuickGoClient::annotations_plan("   ", 5).unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
}

#[test]
fn terms_plan_sorts_dedupes_and_skips_empty_input() {
    let plan = QuickGoClient::terms_plan(&[
        "GO:0005524".into(),
        " ".into(),
        "GO:0004672".into(),
        "GO:0004672".into(),
    ])
    .unwrap();

    assert_eq!(plan.path, "ontology/go/terms/GO:0004672,GO:0005524");
    assert!(QuickGoClient::terms_plan(&[]).is_none());
}
