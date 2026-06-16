//! Tier 3 — response parsing. Pure: feeds committed fixture bytes to response
//! decoders and response types. No network, no server.

use super::super::*;
use crate::error::BioMcpError;
use reqwest::StatusCode;

macro_rules! fixture {
    ($name:expr) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/testdata/sources/semantic_scholar/",
            $name
        ))
    };
}

#[test]
fn parses_paper_detail_fixture() {
    let paper: SemanticScholarPaper = SemanticScholarClient::decode_json_response(
        StatusCode::OK,
        fixture!("paper_detail.json"),
        false,
    )
    .unwrap();

    assert_eq!(paper.paper_id.as_deref(), Some("paper-1"));
    assert_eq!(
        paper
            .external_ids
            .as_ref()
            .and_then(|ids| ids.pubmed.as_deref()),
        Some("22663011")
    );
    assert_eq!(
        paper.tldr.as_ref().and_then(|tldr| tldr.text.as_deref()),
        Some("Compact summary")
    );
    assert_eq!(paper.citation_count, Some(12));
    assert_eq!(paper.influential_citation_count, Some(3));
}

#[test]
fn parses_batch_fixture_with_none_rows() {
    let rows: Vec<Option<SemanticScholarPaper>> = SemanticScholarClient::decode_json_response(
        StatusCode::OK,
        fixture!("paper_batch.json"),
        false,
    )
    .unwrap();

    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows[0].as_ref().and_then(|row| row.paper_id.as_deref()),
        Some("paper-1")
    );
    assert!(rows[1].is_none());
    assert_eq!(
        rows[2].as_ref().and_then(|row| row.title.as_deref()),
        Some("Two")
    );
}

#[test]
fn parses_search_fixture_and_defaults_null_data() {
    let response: SemanticScholarSearchResponse = SemanticScholarClient::decode_json_response(
        StatusCode::OK,
        fixture!("paper_search.json"),
        false,
    )
    .unwrap();

    assert_eq!(response.total, Some(1));
    assert_eq!(response.data.len(), 1);
    assert_eq!(response.data[0].paper_id.as_deref(), Some("paper-1"));
    assert_eq!(
        response.data[0].abstract_text.as_deref(),
        Some("Direct answer abstract.")
    );

    let null_data: SemanticScholarSearchResponse =
        serde_json::from_value(serde_json::json!({ "total": 0, "data": null })).unwrap();
    assert!(null_data.data.is_empty());
}

#[test]
fn parses_graph_and_recommendation_fixtures() {
    let citations: SemanticScholarGraphResponse<SemanticScholarCitationEdge> =
        SemanticScholarClient::decode_json_response(
            StatusCode::OK,
            fixture!("citations.json"),
            false,
        )
        .unwrap();
    assert_eq!(citations.data.len(), 1);
    assert_eq!(
        citations.data[0].citing_paper.paper_id.as_deref(),
        Some("citing-paper")
    );

    let recommendations: SemanticScholarRecommendationsResponse =
        SemanticScholarClient::decode_json_response(
            StatusCode::OK,
            fixture!("recommendations.json"),
            false,
        )
        .unwrap();
    assert_eq!(recommendations.recommended_papers.len(), 1);
    assert_eq!(
        recommendations.recommended_papers[0].paper_id.as_deref(),
        Some("paper-3")
    );
}

#[test]
fn shared_pool_429_returns_dedicated_guidance() {
    let err = SemanticScholarClient::decode_json_response::<SemanticScholarPaper>(
        StatusCode::TOO_MANY_REQUESTS,
        b"shared rate limit",
        true,
    )
    .unwrap_err();

    match err {
        BioMcpError::Api { api, message } => {
            assert_eq!(api, SEMANTIC_SCHOLAR_API);
            assert!(message.contains("Set S2_API_KEY"), "got: {message}");
            assert!(
                message.contains(SEMANTIC_SCHOLAR_DOCS_URL),
                "got: {message}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn authenticated_http_error_keeps_status_and_excerpt() {
    let err = SemanticScholarClient::decode_json_response::<SemanticScholarPaper>(
        StatusCode::FORBIDDEN,
        b"forbidden",
        false,
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(matches!(err, BioMcpError::Api { .. }));
    assert!(msg.contains("semantic_scholar"), "got: {msg}");
    assert!(msg.contains("403"), "got: {msg}");
    assert!(msg.contains("forbidden"), "got: {msg}");
}
