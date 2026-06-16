//! Tier 4 — live upstream smoke. Ignored so normal gates stay pure and fast.

use crate::sources::nih_reporter::NihReporterClient;

#[tokio::test]
#[ignore = "live network"]
async fn live_funding_query_runs() {
    let client = NihReporterClient::new().expect("client");
    let section = client
        .funding("ERBB2")
        .await
        .expect("live NIH Reporter funding query");
    assert_eq!(section.query, "ERBB2");
}
