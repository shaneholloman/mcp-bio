//! Tier 2 - request construction. Pure: builds `RequestPlan`s and asserts the
//! exact request shape. No network.

use super::super::*;
use crate::sources::HttpMethod;

#[test]
fn get_record_plan_builds_accession_path_and_json_accept_header() {
    let plan = UniProtClient::get_record_plan(" P15056 ").unwrap();

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "uniprotkb/P15056.json");
    assert_eq!(plan.header_value("accept"), Some("application/json"));
    assert!(plan.query.is_empty());
}

#[test]
fn get_record_plan_rejects_blank_accession() {
    let err = UniProtClient::get_record_plan("   ").unwrap_err();
    assert!(err.to_string().contains("accession is required"));
}

#[test]
fn search_plan_sets_expected_query_params() {
    let plan = UniProtClient::search_plan(" BRAF ", 3, 0, None).unwrap();

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "uniprotkb/search");
    assert_eq!(plan.header_value("accept"), Some("application/json"));
    assert_eq!(plan.query_value("query"), Some("BRAF"));
    assert_eq!(plan.query_value("format"), Some("json"));
    assert_eq!(plan.query_value("size"), Some("3"));
    assert_eq!(plan.query_value("offset"), Some("0"));
    assert!(plan.query_value("fields").is_some());
    assert!(!plan.has_query("cursor"));
}

#[test]
fn search_plan_clamps_limit_and_uses_cursor_token_when_present() {
    let plan = UniProtClient::search_plan("BRAF", 500, 10, Some("cursor_abc")).unwrap();

    assert_eq!(plan.path, "uniprotkb/search");
    assert_eq!(plan.query_value("size"), Some("25"));
    assert_eq!(plan.query_value("cursor"), Some("cursor_abc"));
    assert!(!plan.has_query("offset"));
}

#[test]
fn search_plan_uses_absolute_next_page_url_without_rewriting_query() {
    let url = "https://rest.uniprot.org/uniprotkb/search?cursor=abc";
    let plan = UniProtClient::search_plan("BRAF", 3, 0, Some(url)).unwrap();

    assert_eq!(plan.path, url);
    assert_eq!(plan.header_value("accept"), Some("application/json"));
    assert!(plan.query.is_empty());
}

#[test]
fn search_plan_rejects_blank_query_and_bad_next_page_tokens() {
    let err = UniProtClient::search_plan("   ", 3, 0, None).unwrap_err();
    assert!(err.to_string().contains("query is required"));

    let err = UniProtClient::search_plan("BRAF", 3, 0, Some("12345"))
        .expect_err("numeric token should fail");
    assert!(err.to_string().contains("--next-page token is invalid"));
}

#[test]
fn normalize_next_page_token_accepts_cursor_url() {
    let token =
        normalize_next_page_token(Some("https://rest.uniprot.org/uniprotkb/search?cursor=abc"))
            .expect("valid URL token");
    assert!(token.is_some());
}
