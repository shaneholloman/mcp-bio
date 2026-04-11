#[allow(unused_imports)]
use super::super::test_support::*;
use super::*;
#[allow(unused_imports)]
use wiremock::matchers::{body_string_contains, header, method, path, query_param};
#[allow(unused_imports)]
use wiremock::{Mock, MockServer, ResponseTemplate};

#[test]
fn semantic_scholar_lookup_id_supports_arxiv_and_paper_ids() {
    assert_eq!(
        semantic_scholar_lookup_id("arXiv:2401.01234"),
        Some("ARXIV:2401.01234".to_string())
    );
    assert_eq!(
        semantic_scholar_lookup_id("0123456789abcdef0123456789abcdef01234567"),
        Some("0123456789abcdef0123456789abcdef01234567".to_string())
    );
}

#[tokio::test]
async fn citations_work_without_api_key() {
    let _guard = lock_env().await;
    let server = MockServer::start().await;
    let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&server.uri()));
    let _s2_key = set_env_var("S2_API_KEY", None);

    Mock::given(method("POST"))
        .and(path("/graph/v1/paper/batch"))
        .and(query_param(
            "fields",
            "paperId,externalIds,title,venue,year",
        ))
        .and(body_string_contains("\"PMID:22663011\""))
        .and(|request: &wiremock::Request| !request.headers.contains_key("x-api-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "paperId": "paper-1",
                "externalIds": {"PubMed": "22663011"},
                "title": "Seed paper",
                "venue": "Science",
                "year": 2012
            }
        ])))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/graph/v1/paper/paper-1/citations"))
        .and(query_param(
            "fields",
            "contexts,intents,isInfluential,citingPaper.paperId,citingPaper.externalIds,citingPaper.title,citingPaper.venue,citingPaper.year",
        ))
        .and(query_param("limit", "1"))
        .and(|request: &wiremock::Request| !request.headers.contains_key("x-api-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [{
                "contexts": ["Example context"],
                "intents": ["Background"],
                "isInfluential": false,
                "citingPaper": {
                    "paperId": "paper-2",
                    "externalIds": {"PubMed": "24200969"},
                    "title": "Related paper",
                    "venue": "Nature",
                    "year": 2024
                }
            }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let result = citations("22663011", 1)
        .await
        .expect("no-key citations should succeed");

    assert_eq!(result.article.paper_id.as_deref(), Some("paper-1"));
    assert_eq!(result.edges.len(), 1);
    assert_eq!(result.edges[0].paper.pmid.as_deref(), Some("24200969"));
}

#[tokio::test]
async fn references_work_without_api_key() {
    let _guard = lock_env().await;
    let server = MockServer::start().await;
    let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&server.uri()));
    let _s2_key = set_env_var("S2_API_KEY", None);

    Mock::given(method("POST"))
        .and(path("/graph/v1/paper/batch"))
        .and(query_param(
            "fields",
            "paperId,externalIds,title,venue,year",
        ))
        .and(body_string_contains("\"PMID:22663011\""))
        .and(|request: &wiremock::Request| !request.headers.contains_key("x-api-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "paperId": "paper-1",
                "externalIds": {"PubMed": "22663011"},
                "title": "Seed paper",
                "venue": "Science",
                "year": 2012
            }
        ])))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/graph/v1/paper/paper-1/references"))
        .and(query_param(
            "fields",
            "contexts,intents,isInfluential,citedPaper.paperId,citedPaper.externalIds,citedPaper.title,citedPaper.venue,citedPaper.year",
        ))
        .and(query_param("limit", "1"))
        .and(|request: &wiremock::Request| !request.headers.contains_key("x-api-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [{
                "contexts": ["Example context"],
                "intents": ["Background"],
                "isInfluential": false,
                "citedPaper": {
                    "paperId": "paper-2",
                    "externalIds": {"PubMed": "19424861"},
                    "title": "Referenced paper",
                    "venue": "Cell",
                    "year": 2009
                }
            }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let result = references("22663011", 1)
        .await
        .expect("no-key references should succeed");

    assert_eq!(result.article.paper_id.as_deref(), Some("paper-1"));
    assert_eq!(result.edges.len(), 1);
    assert_eq!(result.edges[0].paper.pmid.as_deref(), Some("19424861"));
}

#[tokio::test]
async fn recommendations_work_without_api_key() {
    let _guard = lock_env().await;
    let server = MockServer::start().await;
    let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&server.uri()));
    let _s2_key = set_env_var("S2_API_KEY", None);

    Mock::given(method("POST"))
        .and(path("/graph/v1/paper/batch"))
        .and(query_param(
            "fields",
            "paperId,externalIds,title,venue,year",
        ))
        .and(body_string_contains("\"PMID:22663011\""))
        .and(|request: &wiremock::Request| !request.headers.contains_key("x-api-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "paperId": "paper-1",
                "externalIds": {"PubMed": "22663011"},
                "title": "Seed paper",
                "venue": "Science",
                "year": 2012
            }
        ])))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/recommendations/v1/papers/forpaper/paper-1"))
        .and(query_param(
            "fields",
            "paperId,externalIds,title,venue,year",
        ))
        .and(query_param("limit", "1"))
        .and(|request: &wiremock::Request| !request.headers.contains_key("x-api-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "recommendedPapers": [{
                "paperId": "paper-3",
                "externalIds": {"PubMed": "28052061"},
                "title": "Recommended paper",
                "venue": "Nature Medicine",
                "year": 2017
            }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let result = recommendations(&["22663011".to_string()], &[], 1)
        .await
        .expect("no-key recommendations should succeed");

    assert_eq!(result.positive_seeds.len(), 1);
    assert_eq!(result.recommendations.len(), 1);
    assert_eq!(result.recommendations[0].pmid.as_deref(), Some("28052061"));
}
