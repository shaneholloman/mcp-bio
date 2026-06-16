//! Tier 2 — request construction. Pure: builds `RequestPlan`s and asserts the exact
//! method / path / query that would be sent. Nothing is sent.

use super::super::*;
use crate::error::BioMcpError;
use crate::sources::HttpMethod;

#[test]
fn gene_search_plan_sets_gene_id_and_gencode_version() {
    let plan = GtexClient::gene_search_plan(" ensg00000157764.12 ").unwrap();

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "api/v2/reference/geneSearch");
    assert_eq!(plan.query_value("geneId"), Some("ENSG00000157764"));
    assert_eq!(plan.query_value("gencodeVersion"), Some("v26"));
}

#[test]
fn gene_search_plan_rejects_invalid_ensembl_ids() {
    for ensembl_id in ["", "BRCA1", "ENSG 0001", "ENSG/0001"] {
        assert!(
            matches!(
                GtexClient::gene_search_plan(ensembl_id),
                Err(BioMcpError::InvalidArgument(_))
            ),
            "expected invalid argument for {ensembl_id:?}"
        );
    }
}

#[test]
fn median_expression_plan_sets_versioned_id_and_dataset() {
    let plan = GtexClient::median_expression_plan(" ENSG00000157764.12 ");

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "api/v2/expression/medianGeneExpression");
    assert_eq!(plan.query_value("gencodeId"), Some("ENSG00000157764.12"));
    assert_eq!(plan.query_value("datasetId"), Some("gtex_v8"));
}
