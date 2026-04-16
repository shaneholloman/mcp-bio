//! Shared test-only helpers for decomposed trial module sidecars.

#[allow(unused_imports)]
pub(super) use super::{TrialCount, TrialSearchFilters, TrialSource};
#[allow(unused_imports)]
pub(super) use crate::error::BioMcpError;
#[allow(unused_imports)]
pub(super) use crate::sources::clinicaltrials::{ClinicalTrialsClient, CtGovStudy};
#[allow(unused_imports)]
pub(super) use crate::test_support::{EnvVarGuard, set_env_var};
#[allow(unused_imports)]
pub(super) use serde_json::json;
#[allow(unused_imports)]
pub(super) use wiremock::matchers::{method, path, query_param, query_param_is_missing};
#[allow(unused_imports)]
pub(super) use wiremock::{Mock, MockServer, ResponseTemplate};

pub(super) async fn lock_env() -> tokio::sync::MutexGuard<'static, ()> {
    crate::test_support::env_lock().lock().await
}

pub(super) fn ctgov_search_study_fixture(
    nct_id: &str,
    min_age: &str,
    max_age: &str,
) -> serde_json::Value {
    json!({
        "protocolSection": {
            "identificationModule": {
                "nctId": nct_id,
                "briefTitle": format!("Trial {nct_id}")
            },
            "statusModule": {
                "overallStatus": "RECRUITING"
            },
            "eligibilityModule": {
                "minimumAge": min_age,
                "maximumAge": max_age
            }
        }
    })
}

pub(super) fn ctgov_eligibility_detail_fixture(nct_id: &str, criteria: &str) -> serde_json::Value {
    json!({
        "protocolSection": {
            "identificationModule": {
                "nctId": nct_id
            },
            "eligibilityModule": {
                "eligibilityCriteria": criteria
            }
        }
    })
}

pub(super) fn age_filtered_ctgov_filters() -> TrialSearchFilters {
    TrialSearchFilters {
        condition: Some("melanoma".into()),
        status: Some("recruiting".into()),
        age: Some(51.0),
        ..Default::default()
    }
}

pub(super) fn studies_with_age_matches(
    total: usize,
    eligible: usize,
    prefix: &str,
) -> Vec<serde_json::Value> {
    (0..total)
        .map(|index| {
            let nct_id = format!("NCT{prefix}{index:07}");
            if index < eligible {
                ctgov_search_study_fixture(&nct_id, "18 Years", "75 Years")
            } else {
                ctgov_search_study_fixture(&nct_id, "18 Years", "50 Years")
            }
        })
        .collect()
}
