//! Tier 4 — live upstream smoke. Ignored so normal gates stay pure and fast.

use crate::sources::pmc_oa::PmcOaClient;

#[tokio::test]
#[ignore = "live network"]
async fn live_archive_manifest_lookup_runs() {
    let client = PmcOaClient::new().expect("client");
    let _ = client
        .get_full_text_xml_with_manifest("PMC212403")
        .await
        .expect("live pmc oa lookup");
}
