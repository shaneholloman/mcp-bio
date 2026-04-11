#[allow(unused_imports)]
use super::super::test_support::*;
use super::*;
#[allow(unused_imports)]
use wiremock::matchers::{body_string_contains, header, method, path, query_param};
#[allow(unused_imports)]
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn article_search_enrichment_preserves_existing_nonempty_primary_metadata() {
    let _guard = lock_env().await;
    let s2 = MockServer::start().await;
    let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&s2.uri()));
    let _s2_key = set_env_var("S2_API_KEY", None);

    Mock::given(method("POST"))
        .and(path("/graph/v1/paper/batch"))
        .and(query_param(
            "fields",
            "paperId,externalIds,citationCount,influentialCitationCount,abstract",
        ))
        .and(body_string_contains("\"PMID:8896569\""))
        .and(|request: &wiremock::Request| !request.headers.contains_key("x-api-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "paperId": "paper-8896569",
                "externalIds": {"PubMed": "8896569"},
                "citationCount": 231,
                "influentialCitationCount": 17,
                "abstract": "Semantic Scholar replacement abstract."
            }
        ])))
        .expect(1)
        .mount(&s2)
        .await;

    let primary_abstract = "Primary source abstract.";
    let primary_normalized =
        crate::transform::article::normalize_article_search_text(primary_abstract);
    let mut rows = vec![ArticleSearchResult {
        abstract_snippet: Some(primary_abstract.into()),
        normalized_abstract: primary_normalized.clone(),
        ..row_with(
            "8896569",
            ArticleSource::PubMed,
            Some("1996-10-24"),
            Some(99),
            Some(false),
        )
    }];

    enrich_article_search_rows_with_semantic_scholar(&mut rows).await;

    let row = &rows[0];
    assert_eq!(row.citation_count, Some(99));
    assert_eq!(row.influential_citation_count, Some(17));
    assert_eq!(row.abstract_snippet.as_deref(), Some(primary_abstract));
    assert_eq!(row.normalized_abstract, primary_normalized);
}

#[tokio::test]
async fn article_search_semantic_scholar_batch_enrichment_chunks_after_500_ids() {
    let _guard = lock_env().await;
    let s2 = MockServer::start().await;
    let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&s2.uri()));
    let _s2_key = set_env_var("S2_API_KEY", None);

    let first_chunk_body = (1..=SEMANTIC_SCHOLAR_BATCH_LOOKUP_MAX_IDS)
        .map(|pmid| {
            serde_json::json!({
                "paperId": format!("paper-{pmid}"),
                "citationCount": pmid as u64,
                "influentialCitationCount": 1,
                "abstract": format!("Abstract for PMID {pmid}.")
            })
        })
        .collect::<Vec<_>>();
    Mock::given(method("POST"))
        .and(path("/graph/v1/paper/batch"))
        .and(query_param(
            "fields",
            "paperId,externalIds,citationCount,influentialCitationCount,abstract",
        ))
        .and(body_string_contains("\"PMID:1\""))
        .and(body_string_contains("\"PMID:500\""))
        .and(|request: &wiremock::Request| {
            !String::from_utf8_lossy(&request.body).contains("\"PMID:501\"")
        })
        .respond_with(ResponseTemplate::new(200).set_body_json(first_chunk_body))
        .expect(1)
        .mount(&s2)
        .await;

    Mock::given(method("POST"))
        .and(path("/graph/v1/paper/batch"))
        .and(query_param(
            "fields",
            "paperId,externalIds,citationCount,influentialCitationCount,abstract",
        ))
        .and(body_string_contains("\"PMID:501\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "paperId": "paper-501",
                "citationCount": 501,
                "influentialCitationCount": 1,
                "abstract": "Abstract for PMID 501."
            }
        ])))
        .expect(1)
        .mount(&s2)
        .await;

    let mut rows = (1..=501)
        .map(|pmid| {
            row_with(
                &pmid.to_string(),
                ArticleSource::PubMed,
                Some("2025-01-01"),
                None,
                Some(false),
            )
        })
        .collect::<Vec<_>>();

    enrich_article_search_rows_with_semantic_scholar(&mut rows).await;

    assert_eq!(rows[0].citation_count, Some(1));
    assert_eq!(rows[499].citation_count, Some(500));
    assert_eq!(rows[500].citation_count, Some(501));
    assert!(
        rows[500]
            .abstract_snippet
            .as_deref()
            .is_some_and(|value| value.contains("Abstract"))
    );
    assert!(!rows[500].normalized_abstract.is_empty());
}
