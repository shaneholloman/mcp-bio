//! Tier 4 — live upstream smoke. Ignored so normal gates stay pure and fast.

use crate::sources::semantic_scholar::SemanticScholarClient;

#[tokio::test]
#[ignore = "live network"]
async fn live_paper_search_returns_braf_hits() {
    let client = SemanticScholarClient::new().expect("client");
    let response = client
        .paper_search("braf melanoma", 1, None)
        .await
        .expect("live Semantic Scholar search");
    assert!(!response.data.is_empty());
}
