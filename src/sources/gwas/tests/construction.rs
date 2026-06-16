//! Tier 2 — request construction. Pure: builds `RequestPlan`s and asserts the exact
//! method / path / query that would be sent. Nothing is sent.

use super::super::*;
use crate::error::BioMcpError;
use crate::sources::HttpMethod;

#[test]
fn associations_by_rsid_plan_sets_path_projection_and_limit() {
    let plan = GwasClient::associations_by_rsid_plan(" RS7903146 ", 500).unwrap();

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(
        plan.path,
        "singleNucleotidePolymorphisms/rs7903146/associations"
    );
    assert_eq!(plan.query_value("projection"), Some("associationByStudy"));
    assert_eq!(plan.query_value("page"), Some("0"));
    assert_eq!(plan.query_value("size"), Some("200"));
}

#[test]
fn search_plans_set_expected_paths_and_queries() {
    let gene = GwasClient::snps_by_gene_plan(" tcf7l2 ", 5).unwrap();
    assert_eq!(gene.path, "singleNucleotidePolymorphisms/search/findByGene");
    assert_eq!(gene.query_value("geneName"), Some("TCF7L2"));
    assert_eq!(gene.query_value("size"), Some("5"));

    let trait_plan = GwasClient::snps_by_trait_plan("type 2 diabetes", 5).unwrap();
    assert_eq!(
        trait_plan.path,
        "singleNucleotidePolymorphisms/search/findByDiseaseTrait"
    );
    assert_eq!(
        trait_plan.query_value("diseaseTrait"),
        Some("type 2 diabetes")
    );

    let studies = GwasClient::studies_by_trait_plan("type 2 diabetes", 5).unwrap();
    assert_eq!(studies.path, "studies/search/findByDiseaseTrait");
    assert_eq!(studies.query_value("diseaseTrait"), Some("type 2 diabetes"));
}

#[test]
fn study_association_plans_set_search_and_fallback_paths() {
    let search = GwasClient::associations_by_study_search_plan(" gcst000796 ", 5).unwrap();
    assert_eq!(search.path, "associations/search/findByStudyAccessionId");
    assert_eq!(search.query_value("studyAccessionId"), Some("GCST000796"));
    assert_eq!(search.query_value("projection"), Some("associationByStudy"));

    let fallback = GwasClient::associations_by_study_fallback_plan(" gcst000796 ", 5).unwrap();
    assert_eq!(fallback.path, "studies/GCST000796/associations");
    assert_eq!(
        fallback.query_value("projection"),
        Some("associationByStudy")
    );
}

#[test]
fn plans_reject_invalid_inputs() {
    assert!(matches!(
        GwasClient::associations_by_rsid_plan("7903146", 5),
        Err(BioMcpError::InvalidArgument(_))
    ));
    assert!(matches!(
        GwasClient::snps_by_gene_plan("", 5),
        Err(BioMcpError::InvalidArgument(_))
    ));
    assert!(matches!(
        GwasClient::snps_by_trait_plan("", 5),
        Err(BioMcpError::InvalidArgument(_))
    ));
    assert!(matches!(
        GwasClient::associations_by_study_search_plan("not-gcst", 5),
        Err(BioMcpError::InvalidArgument(_))
    ));
}
