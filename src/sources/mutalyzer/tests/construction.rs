//! Tier 2 — request construction. Pure: builds the Mutalyzer request plan and
//! asserts the exact method / path / expectations. Nothing is sent.

use super::super::*;
use std::borrow::Cow;

fn test_client() -> MutalyzerClient {
    MutalyzerClient {
        client: crate::sources::test_client().expect("test client"),
        base: Cow::Borrowed("http://127.0.0.1"),
    }
}

#[test]
fn normalize_request_plan_encodes_transcript_path() {
    let client = test_client();
    let plan: MutalyzerNormalizeRequestPlan = client
        .normalize_request_plan("NM_000248.3:c.135del")
        .expect("MutalyzerNormalizeRequestPlan");

    assert_eq!(plan.method, "GET");
    assert_eq!(plan.path, "/normalize/NM_000248.3:c.135del");
    assert!(plan.query_params.is_empty());
    assert_eq!(plan.cache_mode, "default");
    assert!(plan.status_expectation.contains("invalid_input"));
    assert!(plan.status_expectation.contains("not_found"));
    assert!(plan.status_expectation.contains("service_error"));
}

#[test]
fn normalize_request_plan_percent_encodes_path_segments() {
    let client = test_client();
    let encoded = client
        .normalize_request_plan("NM_004448.2:c.829G>T")
        .expect("encoded plan");

    assert_eq!(encoded.path, "/normalize/NM_004448.2:c.829G%3ET");
}
