//! Tier 4 — live upstream smoke. Ignored so normal gates stay pure and fast.

use crate::sources::mychem::{MYCHEM_FIELDS_SEARCH, MyChemClient};

#[tokio::test]
#[ignore = "live network"]
async fn live_query_with_fields_returns_drug_hits() {
    let client = MyChemClient::new().expect("client");
    let resp = client
        .query_with_fields("imatinib", 1, 0, MYCHEM_FIELDS_SEARCH)
        .await
        .expect("live mychem query");
    assert!(!resp.hits.is_empty());
}
