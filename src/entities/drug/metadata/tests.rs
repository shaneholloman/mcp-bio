use super::*;
use crate::sources::openfda::{DrugsFdaResult, OpenFdaResponse};

#[test]
fn map_drugsfda_approvals_extracts_key_fields() {
    let resp: OpenFdaResponse<DrugsFdaResult> = serde_json::from_value(serde_json::json!({
        "meta": {"results": {"skip": 0, "limit": 1, "total": 1}},
        "results": [{
            "application_number": "NDA021304",
            "sponsor_name": "Example Pharma",
            "openfda": {
                "brand_name": ["DrugX"],
                "generic_name": ["drugx"]
            },
            "products": [{
                "brand_name": "DrugX",
                "dosage_form": "TABLET",
                "route": "ORAL",
                "marketing_status": "Prescription",
                "active_ingredients": [{"name": "drugx", "strength": "10 mg"}]
            }],
            "submissions": [{
                "submission_type": "ORIG",
                "submission_number": "1",
                "submission_status": "AP",
                "submission_status_date": "20120101"
            }]
        }]
    }))
    .expect("response should parse");

    let rows = map_drugsfda_approvals(resp);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].application_number, "NDA021304");
    assert_eq!(rows[0].openfda_brand_names, vec!["DrugX"]);
    assert_eq!(
        rows[0].products[0].active_ingredients,
        vec!["drugx (10 mg)"]
    );
    assert_eq!(
        rows[0].submissions[0].status_date.as_deref(),
        Some("2012-01-01")
    );
}

#[test]
fn extract_top_adverse_events_ranks_by_frequency() {
    let resp: crate::sources::openfda::OpenFdaCountResponse =
        serde_json::from_value(serde_json::json!({
            "meta": {},
            "results": [
                {"term": "Rash", "count": 2},
                {"term": "Nausea", "count": 1},
                {"term": "Fatigue", "count": 2}
            ]
        }))
        .expect("valid openfda response");

    let out = extract_top_adverse_events(&resp);
    assert_eq!(out, vec!["Fatigue", "Rash", "Nausea"]);
}
