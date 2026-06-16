//! Tier 2 - request construction. Pure: builds `RequestPlan`s and validates
//! input checks. No network.

use super::super::*;
use crate::sources::HttpMethod;

#[test]
fn validates_wikipathways_id_shape() {
    assert!(is_wikipathways_id("WP254"));
    assert!(!is_wikipathways_id("wp254"));
    assert!(!is_wikipathways_id("R-HSA-5673001"));
    assert!(!is_wikipathways_id("WP25A"));
}

#[test]
fn search_pathways_plan_builds_search_endpoint_and_rejects_empty_query() {
    let plan = WikiPathwaysClient::search_pathways_plan(" apoptosis ").unwrap();

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "findPathwaysByText.json");
    assert!(plan.query.is_empty());

    let err = WikiPathwaysClient::search_pathways_plan(" ").unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
}

#[test]
fn pathway_plans_build_expected_paths_and_validate_ids() {
    let (plan, id) = WikiPathwaysClient::get_pathway_plan(" WP254 ").unwrap();
    assert_eq!(plan.path, "getPathwayInfo.json");
    assert_eq!(id, "WP254");

    let (plan, id) = WikiPathwaysClient::pathway_xrefs_plan("WP254").unwrap();
    assert_eq!(plan.path, "findPathwaysByXref.json");
    assert_eq!(id, "WP254");

    let err = WikiPathwaysClient::get_pathway_plan("not-a-pathway").unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
    assert!(err.to_string().contains("WP254"));
}
