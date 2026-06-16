//! Tests for NCI CTS trial search helpers.

use super::super::validate_trial_search;
use super::*;
use crate::entities::trial::TrialSource;
use crate::error::BioMcpError;
use crate::sources::nci_cts::NciCtsClient;

fn mydisease_hit(value: serde_json::Value) -> crate::sources::mydisease::MyDiseaseHit {
    serde_json::from_value(value).expect("valid MyDisease hit")
}

fn validation_error(filters: &TrialSearchFilters) -> BioMcpError {
    match validate_trial_search(filters) {
        Ok(_) => panic!("expected validation to fail"),
        Err(err) => err,
    }
}

#[test]
fn nci_search_prefers_grounded_disease_concept_id() {
    let filter = nci_disease_filter_from_hit(
        "melanoma",
        mydisease_hit(serde_json::json!({
            "_id": "MONDO:0005105",
            "mondo": {
                "name": "Melanoma",
                "xrefs": {
                    "ncit": ["C3224"]
                }
            }
        })),
    );
    let plan = NciCtsClient::search_plan(
        "test-key",
        &NciSearchParams {
            disease: Some(filter),
            size: 1,
            from: 0,
            ..NciSearchParams::default()
        },
    );

    assert!(
        plan.query
            .contains(&("diseases.nci_thesaurus_concept_id".into(), "C3224".into()))
    );
    assert!(!plan.query.iter().any(|(key, _)| *key == "keyword"));
}

#[test]
fn nci_search_falls_back_to_keyword_when_grounding_is_unavailable() {
    let plan = NciCtsClient::search_plan(
        "test-key",
        &NciSearchParams {
            disease: Some(NciDiseaseFilter::Keyword("melanoma".into())),
            size: 1,
            from: 0,
            ..NciSearchParams::default()
        },
    );

    assert!(plan.query.contains(&("keyword".into(), "melanoma".into())));
    assert!(
        !plan
            .query
            .iter()
            .any(|(key, _)| *key == "diseases.nci_thesaurus_concept_id")
    );
}

#[test]
fn nci_search_falls_back_to_keyword_when_best_hit_lacks_nci_xref() {
    let filter = nci_disease_filter_from_hit(
        "melanoma",
        mydisease_hit(serde_json::json!({
            "_id": "MONDO:0005105",
            "mondo": {
                "name": "Melanoma"
            }
        })),
    );

    match filter {
        NciDiseaseFilter::Keyword(value) => assert_eq!(value, "melanoma"),
        other => panic!("expected keyword fallback, got {other:?}"),
    }
}

#[test]
fn nci_keyword_fallback_request_uses_keyword_not_concept_id() {
    let plan = NciCtsClient::search_plan(
        "test-key",
        &NciSearchParams {
            disease: Some(NciDiseaseFilter::Keyword("melanoma".into())),
            size: 1,
            from: 0,
            ..NciSearchParams::default()
        },
    );

    assert!(plan.query.contains(&("keyword".into(), "melanoma".into())));
    assert!(
        !plan
            .query
            .iter()
            .any(|(key, _)| *key == "diseases.nci_thesaurus_concept_id")
    );
}

#[test]
fn nci_status_mapping_uses_documented_single_value_filters() {
    let cases = [
        ("recruiting", "site", "ACTIVE"),
        ("not yet recruiting", "current", "Approved"),
        (
            "enrolling by invitation",
            "current",
            "Enrolling by Invitation",
        ),
        ("active, not recruiting", "site", "CLOSED_TO_ACCRUAL"),
        ("completed", "current", "Complete"),
        ("suspended", "current", "Temporarily Closed to Accrual"),
        ("terminated", "current", "Administratively Complete"),
        ("withdrawn", "current", "Withdrawn"),
    ];

    for &(input, expected_kind, expected_value) in &cases {
        let normalized = validate_trial_search(&TrialSearchFilters {
            source: TrialSource::NciCts,
            status: Some(input.into()),
            ..Default::default()
        })
        .expect("status should normalize");
        let filter = nci_status_filter(normalized.normalized_status.as_deref())
            .expect("status should map")
            .expect("status filter");
        match (expected_kind, filter) {
            ("current", NciStatusFilter::CurrentTrialStatus(value)) => {
                assert_eq!(value, expected_value);
            }
            ("site", NciStatusFilter::SiteRecruitmentStatus(value)) => {
                assert_eq!(value, expected_value);
            }
            (_, other) => panic!("unexpected status filter for {input}: {other:?}"),
        }
    }
}

#[test]
fn nci_source_rejects_status_lists() {
    let err = nci_status_filter(Some("RECRUITING,COMPLETED"))
        .expect_err("NCI should reject comma-separated status lists");
    assert!(err.to_string().contains("one mapped status at a time"));
    assert!(err.to_string().contains("--source nci"));
}

#[test]
fn nci_phase_mapping_uses_i_ii_for_combined_phase() {
    let cases = [
        ("1", vec!["I"]),
        ("2", vec!["II"]),
        ("3", vec!["III"]),
        ("4", vec!["IV"]),
        ("na", vec!["NA"]),
        ("1/2", vec!["I_II"]),
    ];

    for (input_phase, expected) in cases {
        let normalized = validate_trial_search(&TrialSearchFilters {
            source: TrialSource::NciCts,
            phase: Some(input_phase.into()),
            ..Default::default()
        })
        .expect("phase should normalize");
        assert_eq!(
            nci_phase_filters(normalized.normalized_phase.as_deref()).expect("phase should map"),
            expected
        );
    }
}

#[test]
fn nci_source_rejects_early_phase1() {
    let err = nci_phase_filters(Some(&["EARLY_PHASE1".to_string()]))
        .expect_err("NCI should reject early_phase1");
    assert!(err.to_string().contains("early_phase1"));
    assert!(err.to_string().contains("--source nci"));
}

#[test]
fn nci_source_rejects_essie_filters() {
    let filters = TrialSearchFilters {
        source: TrialSource::NciCts,
        prior_therapies: Some("platinum".into()),
        ..Default::default()
    };

    let err = validation_error(&filters);
    assert!(
        format!("{err}").contains("--prior-therapies, --progression-on, and --line-of-therapy"),
        "unexpected error: {err}"
    );
}

#[test]
fn nci_source_rejects_age_filter() {
    let filters = TrialSearchFilters {
        source: TrialSource::NciCts,
        condition: Some("melanoma".into()),
        age: Some(67.0),
        ..Default::default()
    };

    let err = validation_error(&filters);
    assert!(
        format!("{err}").contains("--age is only supported for --source ctgov"),
        "unexpected error: {err}"
    );
}

#[test]
fn nci_source_rejects_sex_filter() {
    let filters = TrialSearchFilters {
        source: TrialSource::NciCts,
        condition: Some("melanoma".into()),
        sex: Some("female".into()),
        ..Default::default()
    };

    let err = validation_error(&filters);
    assert!(
        format!("{err}").contains("--sex is only supported for --source ctgov"),
        "unexpected error: {err}"
    );
}

#[test]
fn nci_source_rejects_sponsor_type_filter() {
    let filters = TrialSearchFilters {
        source: TrialSource::NciCts,
        condition: Some("melanoma".into()),
        sponsor_type: Some("nih".into()),
        ..Default::default()
    };

    let err = validation_error(&filters);
    assert!(
        format!("{err}").contains("--sponsor-type is only supported for --source ctgov"),
        "unexpected error: {err}"
    );
}
