//! Tier 4 — live upstream smoke. Ignored so normal gates stay pure and fast.

use crate::sources::ncbi_idconv::NcbiIdConverterClient;

#[tokio::test]
#[ignore = "live network"]
async fn live_pmid_lookup_returns_without_network_error() {
    let client = NcbiIdConverterClient::new().expect("client");
    let _ = client
        .pmid_to_pmcid("22663011")
        .await
        .expect("live idconv lookup");
}
