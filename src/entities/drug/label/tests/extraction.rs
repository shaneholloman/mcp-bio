use super::*;

#[test]
fn extract_interaction_text_from_label_uses_openfda_drug_interactions() {
    let response = serde_json::json!({
        "results": [{
            "drug_interactions": [
                "DRUG INTERACTIONS",
                "Warfarin has documented interactions with aspirin."
            ]
        }]
    });

    let text = extract_interaction_text_from_label(&response).expect("interaction text");
    assert!(text.contains("DRUG INTERACTIONS"));
    assert!(text.contains("Warfarin has documented interactions with aspirin."));
}

#[test]
fn extract_interaction_text_from_label_returns_none_when_missing() {
    let response = serde_json::json!({
        "results": [{
            "warnings_and_cautions": ["No interaction section present"]
        }]
    });

    assert_eq!(extract_interaction_text_from_label(&response), None);
}

#[test]
fn extract_label_set_id_prefers_top_level_set_id() {
    let response = serde_json::json!({
        "results": [{
            "set_id": "abc-123",
            "openfda": {
                "spl_set_id": ["fallback-456"]
            }
        }]
    });

    assert_eq!(extract_label_set_id(&response).as_deref(), Some("abc-123"));
}

#[test]
fn extract_label_set_id_falls_back_to_spl_set_id() {
    let response = serde_json::json!({
        "results": [{
            "openfda": {
                "spl_set_id": ["fallback-456"]
            }
        }]
    });

    assert_eq!(
        extract_label_set_id(&response).as_deref(),
        Some("fallback-456")
    );
}

#[test]
fn extract_inline_label_raw_mode_preserves_truncated_raw_subsections() {
    let response = serde_json::json!({
        "results": [{
            "indications_and_usage": [
                "1 INDICATIONS AND USAGE",
                "(1.1) KEYTRUDA, in combination with chemotherapy, is indicated for the treatment of patients with high-risk early-stage triple-negative breast cancer."
            ],
            "warnings_and_cautions": ["Warnings"],
            "dosage_and_administration": ["Dosage"]
        }]
    });

    let label = extract_inline_label(&response, true).expect("raw label");
    assert!(!label.indication_summary.is_empty());
    assert!(label.indications.as_deref().is_some());
    assert!(label.warnings.as_deref().is_some());
    assert!(label.dosage.as_deref().is_some());
}
