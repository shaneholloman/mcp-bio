//! Tier 4 — live upstream smoke. Ignored so normal gates stay pure and fast.

use crate::sources::cbioportal::CBioPortalClient;

#[tokio::test]
#[ignore = "live network"]
async fn live_mutation_summary_runs_for_braf() {
    let client = CBioPortalClient::new().expect("client");
    let summary = client
        .get_mutation_summary("BRAF")
        .await
        .expect("live cBioPortal mutation summary");
    assert_eq!(summary.study_id, "msk_impact_2017");
}
