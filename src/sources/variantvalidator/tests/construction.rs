//! Tier 2 — request construction. Pure: builds the VariantValidator request plan
//! and asserts the exact method / path / query expectations. Nothing is sent.

use super::super::*;
use std::borrow::Cow;

fn test_client() -> VariantValidatorClient {
    VariantValidatorClient {
        client: crate::sources::test_client().expect("test client"),
        base: Cow::Borrowed("http://127.0.0.1"),
    }
}

#[test]
fn normalize_request_plan_encodes_transcript_path_and_json_query() {
    let client = test_client();
    let plan: VariantValidatorNormalizeRequestPlan = client
        .normalize_request_plan("NM_000248.3:c.135del")
        .expect("VariantValidatorNormalizeRequestPlan");

    assert_eq!(plan.method, "GET");
    assert_eq!(
        plan.path,
        "/VariantValidator/variantvalidator/GRCh38/NM_000248.3:c.135del/all"
    );
    assert!(
        plan.query_params
            .contains(&("content-type", "application/json".to_string()))
    );
    assert_eq!(plan.cache_mode, "default");
}

#[test]
fn normalize_request_plan_percent_encodes_path_segments() {
    let client = test_client();
    let encoded = client
        .normalize_request_plan("NM_004448.2:c.829G>T")
        .expect("encoded plan");

    assert_eq!(
        encoded.path,
        "/VariantValidator/variantvalidator/GRCh38/NM_004448.2:c.829G%3ET/all"
    );
}
