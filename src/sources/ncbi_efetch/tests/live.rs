//! Tier 4 — live upstream smoke. Ignored so normal gates stay pure and fast.

use crate::sources::ncbi_efetch::NcbiEfetchClient;

#[tokio::test]
#[ignore = "live network"]
async fn live_full_text_xml_returns_article_when_available() {
    let client = NcbiEfetchClient::new().expect("client");
    let xml = client
        .get_full_text_xml("PMC212403")
        .await
        .expect("live efetch")
        .expect("article");
    assert!(xml.contains("<article"));
}
