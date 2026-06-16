//! Tier 3 - response parsing and local result shaping. Pure: feeds JSON bytes
//! into decode helpers and validates output/error mapping. No network.

use reqwest::StatusCode;

use super::super::*;

#[test]
fn decode_response_and_map_terms_applies_limit() {
    let response: GProfilerResponse = GProfilerClient::decode_json_response(
        StatusCode::OK,
        br#"{
            "result": [
                {"native": "R-HSA-1", "name": "A", "source": "REAC", "p_value": 0.01},
                {"native": "R-HSA-2", "name": "B", "source": "REAC", "p_value": 0.02}
            ]
        }"#,
    )
    .unwrap();

    let rows = GProfilerClient::map_enrich_response(response, 1);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].native.as_deref(), Some("R-HSA-1"));
}

#[test]
fn remap_gprofiler_error_maps_transient_statuses_to_source_unavailable() {
    let err = remap_gprofiler_error(BioMcpError::Api {
        api: GPROFILER_API.to_string(),
        message: "HTTP 503 Service Unavailable: upstream failure".to_string(),
    });
    assert!(matches!(err, BioMcpError::SourceUnavailable { .. }));
    assert!(err.to_string().contains("g:Profiler"));

    let err = remap_gprofiler_error(BioMcpError::Api {
        api: GPROFILER_API.to_string(),
        message: "HTTP 400 Bad Request: bad request".to_string(),
    });
    assert!(matches!(err, BioMcpError::Api { .. }));
}

#[test]
fn transient_status_parser_recognizes_retryable_statuses() {
    assert_eq!(
        transient_status_from_api_message("HTTP 503 Service Unavailable: upstream"),
        Some(StatusCode::SERVICE_UNAVAILABLE)
    );
    assert_eq!(
        transient_status_from_api_message("HTTP 429 Too Many Requests: upstream"),
        Some(StatusCode::TOO_MANY_REQUESTS)
    );
    assert_eq!(
        transient_status_from_api_message("HTTP 400 Bad Request: bad request"),
        None
    );
}
