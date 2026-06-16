//! Tier 3 - response parsing and local result shaping. Pure: feeds JSON bytes
//! into decode helpers and validates output. No network.

use reqwest::StatusCode;

use super::super::*;

#[test]
fn decode_annotations_response_maps_results() {
    let resp: QuickGoAnnotationResponse = QuickGoClient::decode_json_response(
        StatusCode::OK,
        br#"{
            "results": [{
                "goId": "GO:0004672",
                "goName": "protein kinase activity",
                "goAspect": "molecular_function",
                "evidenceCode": "ECO:0000269"
            }]
        }"#,
    )
    .unwrap();

    assert_eq!(resp.results.len(), 1);
    assert_eq!(resp.results[0].go_id.as_deref(), Some("GO:0004672"));
}

#[test]
fn decode_terms_response_maps_term_metadata() {
    let resp: QuickGoTermsResponse = QuickGoClient::decode_json_response(
        StatusCode::OK,
        br#"{
            "results": [
                {"id": "GO:0004672", "name": "protein kinase activity", "aspect": "molecular_function"},
                {"id": "GO:0005524", "name": "ATP binding", "aspect": "molecular_function"}
            ]
        }"#,
    )
    .unwrap();

    assert_eq!(resp.results.len(), 2);
    assert_eq!(resp.results[0].id.as_deref(), Some("GO:0004672"));
    assert_eq!(
        resp.results[0].name.as_deref(),
        Some("protein kinase activity")
    );
}
