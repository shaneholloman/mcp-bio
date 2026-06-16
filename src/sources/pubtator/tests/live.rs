//! Tier 4 — live upstream smoke. Ignored so normal gates stay pure and fast.

use crate::sources::pubtator::PubTatorClient;

#[tokio::test]
#[ignore = "live network"]
async fn live_autocomplete_returns_braf() {
    let client = PubTatorClient::new().expect("client");
    let resp = client
        .entity_autocomplete("BRAF")
        .await
        .expect("live PubTator autocomplete");
    assert!(
        resp.iter()
            .any(|result| result.id.as_deref() == Some("@GENE_BRAF"))
    );
}
