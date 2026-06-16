//! Tier 2 - request construction. Pure: builds the POST plan/body and validates
//! input checks. No network.

use super::super::*;
use crate::sources::{HttpMethod, RequestBody};

#[test]
fn enrich_genes_plan_posts_query_and_limit() {
    let (plan, limit) =
        GProfilerClient::enrich_genes_plan(&[" BRAF ".to_string(), "KRAS".to_string()], 1).unwrap();

    assert_eq!(plan.method, HttpMethod::Post);
    assert_eq!(plan.path, "gost/profile/");
    assert_eq!(limit, 1);
    let RequestBody::Json(body) = plan.body else {
        panic!("expected JSON body");
    };
    assert_eq!(body["organism"], "hsapiens");
    assert_eq!(body["query"], serde_json::json!(["BRAF", "KRAS"]));
}

#[test]
fn enrich_genes_plan_rejects_empty_input_and_bad_limits() {
    let err = GProfilerClient::enrich_genes_plan(&[], 5).unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));

    let err = GProfilerClient::enrich_genes_plan(&["BRAF".to_string()], 0).unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
    assert!(err.to_string().contains("--limit must be between 1 and 50"));

    let err = GProfilerClient::enrich_genes_plan(&["BRAF".to_string()], 51).unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
    assert!(err.to_string().contains("--limit must be between 1 and 50"));
}
