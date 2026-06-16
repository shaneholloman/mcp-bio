//! Tier 3 - response parsing and local result shaping. Pure: feeds JSON bytes
//! into decode helpers and validates output. No network.

use reqwest::StatusCode;

use super::super::*;

#[test]
fn decode_interactions_response_maps_camel_case_fields() {
    let rows: Vec<StringInteraction> = StringClient::decode_json_response(
        StatusCode::OK,
        br#"[{"preferredNameA":"BRAF","preferredNameB":"KRAS","score":0.91}]"#,
    )
    .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].preferred_name_b.as_deref(), Some("KRAS"));
}

#[test]
fn decode_interactions_response_maps_underscore_fields() {
    let rows: Vec<StringInteraction> = StringClient::decode_json_response(
        StatusCode::OK,
        br#"[{"preferredName_A":"BRAF","preferredName_B":"MAP2K1","score":0.88}]"#,
    )
    .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].preferred_name_a.as_deref(), Some("BRAF"));
    assert_eq!(rows[0].preferred_name_b.as_deref(), Some("MAP2K1"));
}
