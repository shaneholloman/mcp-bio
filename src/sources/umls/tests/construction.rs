//! Tier 2 - request construction. Pure: builds `RequestPlan`s and asserts the
//! exact request shape. No network.

use super::super::*;
use crate::sources::HttpMethod;

#[test]
fn search_plan_sets_query_auth_and_page_size() {
    let plan = UmlsClient::search_plan(" cystic fibrosis ", "test-key").unwrap();

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "rest/search/current");
    assert_eq!(plan.query_value("string"), Some("cystic fibrosis"));
    assert_eq!(plan.query_value("pageSize"), Some("5"));
    assert_eq!(plan.query_value("apiKey"), Some("test-key"));
}

#[test]
fn search_plan_skips_empty_query() {
    assert!(UmlsClient::search_plan(" ", "test-key").is_none());
}

#[test]
fn atoms_plan_sets_cui_auth_page_size_and_language() {
    let plan = UmlsClient::atoms_plan("C0010674", "test-key");

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "rest/content/current/CUI/C0010674/atoms");
    assert_eq!(plan.query_value("apiKey"), Some("test-key"));
    assert_eq!(plan.query_value("pageSize"), Some("200"));
    assert_eq!(plan.query_value("language"), Some("ENG"));
}
