//! Tier 2 - request construction. Pure: builds `RequestPlan`s and asserts the
//! exact request shape. No network.

use super::super::*;
use crate::sources::HttpMethod;

#[test]
fn normalize_hpo_id_accepts_standard_forms() {
    assert_eq!(
        normalize_hpo_id("HP:0001653").as_deref(),
        Some("HP:0001653")
    );
    assert_eq!(
        normalize_hpo_id("hp_0001653").as_deref(),
        Some("HP:0001653")
    );
    assert_eq!(normalize_hpo_id(""), None);
    assert_eq!(normalize_hpo_id("MP:0001653"), None);
}

#[test]
fn term_plan_builds_normalized_term_path() {
    let plan = HpoClient::term_plan("hp_0001653").unwrap();

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "terms/HP:0001653");
    assert!(plan.query.is_empty());
}

#[test]
fn term_plan_rejects_invalid_id() {
    let err = HpoClient::term_plan(" ").unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
}

#[test]
fn normalize_term_ids_dedupes_sorts_and_limits() {
    let ids = HpoClient::normalize_term_ids(
        &[
            "HP:0002097".into(),
            "HP:0001653".into(),
            "hp_0001653".into(),
            "NOT_AN_HPO".into(),
        ],
        20,
    );

    assert_eq!(ids, vec!["HP:0001653", "HP:0002097"]);
}

#[test]
fn search_term_ids_plan_builds_query_and_skips_empty_query() {
    let plan = HpoClient::search_term_ids_plan(" seizure ").unwrap();

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "search");
    assert_eq!(plan.query_value("q"), Some("seizure"));

    assert!(HpoClient::search_term_ids_plan(" ").is_none());
}
