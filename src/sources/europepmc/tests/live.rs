//! Tier 4 — live upstream smoke. Ignored so normal gates stay pure and fast.

use crate::sources::europepmc::EuropePmcClient;

#[tokio::test]
#[ignore = "live network"]
async fn live_search_by_pmid_returns_hit() {
    let client = EuropePmcClient::new().expect("client");
    let resp = client
        .search_by_pmid("22663011")
        .await
        .expect("live Europe PMC search");
    assert_eq!(resp.hit_count, Some(1));
}
