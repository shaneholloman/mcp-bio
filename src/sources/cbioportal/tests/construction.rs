//! Tier 2 — request construction. Pure: builds `RequestPlan`s and asserts the exact
//! method / path / query / body that would be sent. Nothing is sent.

use super::super::*;
use crate::error::BioMcpError;
use crate::sources::{HttpMethod, RequestBody};

#[test]
fn gene_resolution_plan_sets_keyword_query() {
    let plan = CBioPortalClient::gene_resolution_plan(" BRAF ").unwrap();

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "genes");
    assert_eq!(plan.query_value("keyword"), Some("BRAF"));
    assert_eq!(plan.query_value("pageSize"), Some("1"));
    assert_eq!(plan.query_value("pageNumber"), Some("0"));
}

#[test]
fn gene_resolution_plan_rejects_empty_gene() {
    assert!(matches!(
        CBioPortalClient::gene_resolution_plan("   "),
        Err(BioMcpError::InvalidArgument(_))
    ));
}

#[test]
fn study_and_mutation_plans_set_paths_and_queries() {
    let study = CBioPortalClient::study_plan("msk_impact_2017");
    assert_eq!(study.method, HttpMethod::Get);
    assert_eq!(study.path, "studies/msk_impact_2017");

    let mutations = CBioPortalClient::mutations_plan(
        "msk_impact_2017_mutations",
        "msk_impact_2017_all",
        673,
        500,
        2,
    );
    assert_eq!(
        mutations.path,
        "molecular-profiles/msk_impact_2017_mutations/mutations"
    );
    assert_eq!(
        mutations.query_value("sampleListId"),
        Some("msk_impact_2017_all")
    );
    assert_eq!(mutations.query_value("entrezGeneId"), Some("673"));
    assert_eq!(mutations.query_value("pageSize"), Some("500"));
    assert_eq!(mutations.query_value("pageNumber"), Some("2"));
}

#[test]
fn clinical_data_plan_posts_sample_filter_body() {
    let sample_ids = vec!["SAMPLE-1".to_string(), "SAMPLE-2".to_string()];
    let plan = CBioPortalClient::clinical_data_plan("msk_impact_2017", &sample_ids);

    assert_eq!(plan.method, HttpMethod::Post);
    assert_eq!(plan.path, "studies/msk_impact_2017/clinical-data/fetch");
    assert_eq!(plan.query_value("clinicalDataType"), Some("SAMPLE"));
    let RequestBody::Json(body) = &plan.body else {
        panic!("expected JSON body, got {:?}", plan.body);
    };
    assert_eq!(
        body["attributeIds"],
        serde_json::json!(["CANCER_TYPE_DETAILED"])
    );
    assert_eq!(body["ids"], serde_json::json!(sample_ids));
}
