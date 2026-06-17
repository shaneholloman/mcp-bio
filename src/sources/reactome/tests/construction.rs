//! Tier 2 - request construction. Pure: builds `RequestPlan`s and asserts the
//! exact request shape. No network.

use super::super::*;
use crate::sources::HttpMethod;

#[test]
fn search_pathways_plan_sets_query_species_and_page_size() {
    let plan = ReactomeClient::search_pathways_plan(" MAPK ", 99).unwrap();

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "search/query");
    assert_eq!(plan.query_value("query"), Some("MAPK"));
    assert_eq!(plan.query_value("species"), Some("Homo sapiens"));
    assert_eq!(plan.query_value("pageSize"), Some("25"));
}

#[test]
fn search_pathways_plan_preserves_limit_one_probe() {
    let plan = ReactomeClient::search_pathways_plan(" ABL1 ", 1).unwrap();

    assert_eq!(plan.path, "search/query");
    assert_eq!(plan.query_value("query"), Some("ABL1"));
    assert_eq!(plan.query_value("species"), Some("Homo sapiens"));
    assert_eq!(plan.query_value("pageSize"), Some("1"));
}

#[test]
fn search_pathways_plan_rejects_empty_query() {
    let err = ReactomeClient::search_pathways_plan(" ", 5).unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
}

#[test]
fn pathway_plans_build_expected_paths_and_reject_empty_ids() {
    assert_eq!(
        ReactomeClient::top_level_pathways_plan().path,
        "data/pathways/top/Homo%20sapiens"
    );
    assert_eq!(
        ReactomeClient::get_pathway_plan(" R-HSA-5673001 ")
            .unwrap()
            .path,
        "data/query/R-HSA-5673001"
    );
    assert_eq!(
        ReactomeClient::participants_plan("R-HSA-5673001")
            .unwrap()
            .path,
        "data/participants/R-HSA-5673001"
    );
    assert_eq!(
        ReactomeClient::contained_events_plan("R-HSA-5673001")
            .unwrap()
            .path,
        "data/pathway/R-HSA-5673001/containedEvents"
    );

    let err = ReactomeClient::contained_events_plan(" ").unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
}
