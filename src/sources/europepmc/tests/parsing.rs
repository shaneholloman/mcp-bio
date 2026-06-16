//! Tier 3 — response parsing. Pure: feeds committed fixture bytes to `decode_json`
//! and response helpers. No network, no server.

use crate::error::BioMcpError;
use crate::sources::decode_json;
use crate::sources::europepmc::{EuropePmcClient, EuropePmcResult, EuropePmcSearchResponse};
use reqwest::StatusCode;
use reqwest::header::HeaderValue;

const EUROPE_PMC_API: &str = "europepmc";

macro_rules! fixture {
    ($name:expr) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/testdata/sources/europepmc/",
            $name
        ))
    };
}

fn json_ct() -> HeaderValue {
    HeaderValue::from_static("application/json")
}

#[test]
fn parses_search_response_from_real_fixture() {
    let resp: EuropePmcSearchResponse = decode_json(
        EUROPE_PMC_API,
        StatusCode::OK,
        Some(&json_ct()),
        fixture!("search_pmid_22663011.json"),
        false,
    )
    .unwrap();

    assert_eq!(resp.hit_count, Some(1));
    let result = resp
        .result_list
        .expect("result list")
        .result
        .into_iter()
        .next()
        .expect("result");
    assert_eq!(result.id.as_deref(), Some("22663011"));
    assert_eq!(result.pmid.as_deref(), Some("22663011"));
    assert_eq!(result.doi.as_deref(), Some("10.1056/nejmoa1203421"));
}

#[test]
fn europepmc_result_deserializes_first_index_date() {
    let result: EuropePmcResult = serde_json::from_value(serde_json::json!({
        "id": "22663011",
        "pmid": "22663011",
        "firstPublicationDate": "2025-01-14",
        "firstIndexDate": "2025-01-15"
    }))
    .expect("europepmc result should deserialize");

    assert_eq!(result.first_index_date.as_deref(), Some("2025-01-15"));
}

#[test]
fn decode_full_text_xml_returns_none_on_not_found() {
    let xml = EuropePmcClient::decode_full_text_xml(StatusCode::NOT_FOUND, b"missing").unwrap();
    assert!(xml.is_none());
}

#[test]
fn decode_full_text_xml_returns_body_on_success() {
    let xml = EuropePmcClient::decode_full_text_xml(StatusCode::OK, b"<article/>").unwrap();
    assert_eq!(xml, Some("<article/>".to_string()));
}

#[test]
fn decode_full_text_xml_maps_http_error_status_with_excerpt() {
    let err = EuropePmcClient::decode_full_text_xml(
        StatusCode::INTERNAL_SERVER_ERROR,
        b"upstream failure",
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(matches!(err, BioMcpError::Api { .. }));
    assert!(msg.contains("europepmc"), "got: {msg}");
    assert!(msg.contains("500"), "got: {msg}");
}

#[test]
fn decode_json_maps_http_error_status_with_excerpt() {
    let err = decode_json::<EuropePmcSearchResponse>(
        EUROPE_PMC_API,
        StatusCode::INTERNAL_SERVER_ERROR,
        None,
        b"upstream failure",
        false,
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(matches!(err, BioMcpError::Api { .. }));
    assert!(msg.contains("europepmc"), "got: {msg}");
    assert!(msg.contains("500"), "got: {msg}");
}
