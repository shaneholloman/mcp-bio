//! Tier 3 — response parsing. Pure: feeds committed fixture bytes and status values
//! to the Mutalyzer decoder. No network, no server.

use super::super::*;
use crate::entities::variant::VariantNormalizationStatus;
use reqwest::StatusCode;
use reqwest::header::HeaderValue;

macro_rules! fixture {
    ($name:expr) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/testdata/sources/mutalyzer/",
            $name
        ))
    };
}

#[test]
fn normalize_response_parses_success_and_warnings() {
    let content_type = HeaderValue::from_static("application/json");
    let result = decode_normalize_response(
        StatusCode::OK,
        Some(&content_type),
        fixture!("normalize_erbb2.json"),
    );

    assert_eq!(result.status, VariantNormalizationStatus::Success);
    assert_eq!(
        result.normalized_description.as_deref(),
        Some("NM_004448.2:c.829G>T")
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|warning| warning == "source warning")
    );
}

#[test]
fn normalize_response_maps_provider_invalid_input() {
    let result = decode_normalize_response(
        StatusCode::UNPROCESSABLE_ENTITY,
        Some(&HeaderValue::from_static("application/json")),
        br#"{
            "message": "Errors encountered. Check the custom field.",
            "custom": {"input_description": "NM_000248.3:c."}
        }"#,
    );

    assert_eq!(result.status, VariantNormalizationStatus::InvalidInput);
    assert_eq!(result.input_description.as_deref(), Some("NM_000248.3:c."));
}

#[test]
fn normalize_response_maps_success_status_error_payload_to_invalid_input() {
    let result = decode_normalize_response(
        StatusCode::OK,
        Some(&HeaderValue::from_static("application/json")),
        br#"{
            "message": "Errors encountered. Check the custom field.",
            "custom": {"input_description": "NM_000248.3:c."}
        }"#,
    );

    assert_eq!(result.status, VariantNormalizationStatus::InvalidInput);
    assert_eq!(result.input_description.as_deref(), Some("NM_000248.3:c."));
    assert!(result.normalized_description.is_none());
}

#[test]
fn normalize_response_maps_not_found_and_http_errors() {
    let result = decode_normalize_response(StatusCode::NOT_FOUND, None, b"not found");
    assert_eq!(result.status, VariantNormalizationStatus::NotFound);
    assert!(
        result
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("HTTP 404")
    );

    let result =
        decode_normalize_response(StatusCode::INTERNAL_SERVER_ERROR, None, b"upstream failed");
    assert_eq!(result.status, VariantNormalizationStatus::ServiceError);
    assert!(
        result
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("HTTP 500")
    );
}

#[test]
fn normalize_response_maps_html_response_to_service_error() {
    let html = HeaderValue::from_static("text/html");
    let result =
        decode_normalize_response(StatusCode::OK, Some(&html), b"<html>maintenance</html>");

    assert_eq!(result.status, VariantNormalizationStatus::ServiceError);
    assert!(
        result
            .message
            .as_deref()
            .is_some_and(|message| !message.is_empty())
    );
}
