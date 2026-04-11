use super::*;

#[test]
fn extract_inline_label_summary_mode_preserves_subtype_wording() {
    let response = serde_json::json!({
        "results": [{
            "indications_and_usage": [
                "1 INDICATIONS AND USAGE",
                "(1.1) KEYTRUDA, in combination with chemotherapy, is indicated for the treatment of patients with high-risk early-stage triple-negative breast cancer.",
                "(1.2) KEYTRUDA, as a single agent, is indicated for the treatment of adult patients with renal cell carcinoma."
            ],
            "warnings_and_cautions": ["Warnings"],
            "dosage_and_administration": ["Dosage"]
        }]
    });

    let label = extract_inline_label(&response, false).expect("summary label");
    assert_eq!(
        label
            .indication_summary
            .iter()
            .map(|row| row.name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "high-risk early-stage triple-negative breast cancer",
            "renal cell carcinoma"
        ]
    );
    assert!(label.warnings.is_none());
    assert!(label.dosage.is_none());
    assert!(label.indications.is_none());
}

#[test]
fn extract_inline_label_summary_mode_trims_patient_eligibility_qualifiers() {
    let response = serde_json::json!({
        "results": [{
            "indications_and_usage": [
                "1 INDICATIONS AND USAGE",
                "(1.1) KEYTRUDA is indicated for the treatment of patients with locally advanced or metastatic urothelial carcinoma who are not eligible for cisplatin-containing chemotherapy.",
                "(1.2) KEYTRUDA is indicated for the treatment of adults with locally advanced unresectable or metastatic HER2-negative gastric or gastroesophageal junction (GEJ) adenocarcinoma whose tumors express PD-L1 (CPS ≥1) as determined by an FDA-authorized test."
            ]
        }]
    });

    let label = extract_inline_label(&response, false).expect("summary label");
    assert_eq!(
        label
            .indication_summary
            .iter()
            .map(|row| row.name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "locally advanced or metastatic urothelial carcinoma",
            "locally advanced unresectable or metastatic HER2-negative gastric or gastroesophageal junction (GEJ) adenocarcinoma"
        ]
    );
}

#[test]
fn extract_inline_label_summary_mode_uses_numbered_subsection_titles() {
    let response = serde_json::json!({
        "results": [{
            "indications_and_usage": [
                "1 INDICATIONS AND USAGE • THALOMID in combination with dexamethasone is indicated for the treatment of patients with newly diagnosed multiple myeloma (MM). ( 1.1 ) • THALOMID is indicated for the acute treatment of the cutaneous manifestations of moderate to severe erythema nodosum leprosum (ENL). THALOMID is not indicated as monotherapy for such ENL treatment in the presence of moderate to severe neuritis. THALOMID is also indicated as maintenance therapy for prevention and suppression of the cutaneous manifestations of ENL recurrence. ( 1.2 ) 1.1 Multiple Myeloma THALOMID in combination with dexamethasone is indicated for the treatment of patients with newly diagnosed multiple myeloma (MM) [see Clinical Studies (14.1) ] . 1.2 Erythema Nodosum Leprosum THALOMID is indicated for the acute treatment of the cutaneous manifestations of moderate to severe erythema nodosum leprosum (ENL). THALOMID is not indicated as monotherapy for such ENL treatment in the presence of moderate to severe neuritis. THALOMID is also indicated as maintenance therapy for prevention and suppression of the cutaneous manifestations of ENL recurrence [see Clinical Studies (14.2) ]."
            ],
            "openfda": {
                "brand_name": ["THALOMID"],
                "generic_name": ["thalidomide"]
            }
        }]
    });

    let label = extract_inline_label(&response, false).expect("summary label");
    assert_eq!(
        label
            .indication_summary
            .iter()
            .map(|row| row.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Multiple Myeloma", "Erythema Nodosum Leprosum"]
    );
}

#[test]
fn extract_inline_label_summary_mode_falls_back_to_raw_indications_when_no_rows() {
    let response = serde_json::json!({
        "results": [{
            "indications_and_usage": [
                "1 INDICATIONS AND USAGE",
                "Use with diet and exercise to improve glycemic control."
            ],
            "warnings_and_cautions": ["Warnings"],
            "dosage_and_administration": ["Dosage"]
        }]
    });

    let label = extract_inline_label(&response, false).expect("fallback label");
    assert!(label.indication_summary.is_empty());
    assert!(label.indications.as_deref().is_some());
    assert!(label.warnings.is_none());
    assert!(label.dosage.is_none());
}
