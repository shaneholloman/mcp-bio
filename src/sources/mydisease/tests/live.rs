//! Tier 4 — live upstream smoke. Ignored so normal gates stay pure and fast.

use crate::sources::mydisease::MyDiseaseClient;

#[tokio::test]
#[ignore = "live network"]
async fn live_query_returns_disease_hits() {
    let client = MyDiseaseClient::new().expect("client");
    let resp = client
        .query("melanoma", 1, 0, None, None, None, None)
        .await
        .expect("live mydisease query");
    assert!(!resp.hits.is_empty());
}

#[tokio::test]
#[ignore = "live network"]
async fn live_get_returns_disease_hit() {
    let client = MyDiseaseClient::new().expect("client");
    let hit = client
        .get("MONDO:0005105")
        .await
        .expect("live mydisease get");
    assert_eq!(hit.id, "MONDO:0005105");
}
