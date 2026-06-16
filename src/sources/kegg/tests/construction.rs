//! Tier 2 - request construction. Pure: builds KEGG path segments and validates
//! input checks. No network.

use super::super::*;

#[test]
fn search_pathways_segments_build_find_pathway_request() {
    let segments = KeggClient::search_pathways_segments(" MAPK ").unwrap();
    assert_eq!(segments, vec!["find", "pathway", "MAPK"]);
}

#[test]
fn search_pathways_segments_rejects_empty_query() {
    let err = KeggClient::search_pathways_segments(" ").unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
}

#[test]
fn get_pathway_segments_build_get_request_and_reject_empty_id() {
    let segments = KeggClient::get_pathway_segments(" hsa05200 ").unwrap();
    assert_eq!(segments, vec!["get", "hsa05200"]);

    let err = KeggClient::get_pathway_segments(" ").unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
}
