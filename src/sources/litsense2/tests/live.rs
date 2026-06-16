//! Tier 4 — live upstream smoke. Ignored so normal gates stay pure and fast.

use crate::sources::litsense2::LitSense2Client;

#[tokio::test]
#[ignore = "live network"]
async fn live_sentence_search_returns_hits() {
    let client = LitSense2Client::new().expect("client");
    let hits = client
        .sentence_search("hirschsprung disease")
        .await
        .expect("live LitSense2 sentence search");
    assert!(!hits.is_empty());
}
