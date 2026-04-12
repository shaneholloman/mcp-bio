use super::*;

#[test]
fn normalize_variant_target_label_rejects_exact_gene_match() {
    assert_eq!(normalize_variant_target_label("EGFR", "EGFR"), None);
}

#[test]
fn normalize_variant_target_label_keeps_spaced_protein_change() {
    assert_eq!(
        normalize_variant_target_label("BRAF V600E", "BRAF").as_deref(),
        Some("BRAF V600E")
    );
}

#[test]
fn normalize_variant_target_label_normalizes_egfr_roman_suffix() {
    assert_eq!(
        normalize_variant_target_label("EGFR VIII", "EGFR").as_deref(),
        Some("EGFRvIII")
    );
    assert_eq!(
        normalize_variant_target_label("EGFRVIII", "EGFR").as_deref(),
        Some("EGFRvIII")
    );
}

#[test]
fn extract_variant_targets_from_civic_deduplicates_and_filters_by_generic_target() {
    let civic = CivicContext {
        evidence_total_count: 2,
        assertion_total_count: 2,
        evidence_items: vec![
            CivicEvidenceItem {
                id: 1,
                name: "EID1".to_string(),
                molecular_profile: "EGFR VIII".to_string(),
                evidence_type: "PREDICTIVE".to_string(),
                evidence_level: "A".to_string(),
                significance: "SENSITIVITYRESPONSE".to_string(),
                disease: None,
                therapies: vec!["rindopepimut".to_string()],
                status: "ACCEPTED".to_string(),
                citation: None,
                source_type: None,
                publication_year: None,
            },
            CivicEvidenceItem {
                id: 2,
                name: "EID2".to_string(),
                molecular_profile: "PDGFRA D842V".to_string(),
                evidence_type: "PREDICTIVE".to_string(),
                evidence_level: "A".to_string(),
                significance: "SENSITIVITYRESPONSE".to_string(),
                disease: None,
                therapies: vec!["rindopepimut".to_string()],
                status: "ACCEPTED".to_string(),
                citation: None,
                source_type: None,
                publication_year: None,
            },
        ],
        assertions: vec![
            CivicAssertion {
                id: 3,
                name: "AID3".to_string(),
                molecular_profile: "EGFRVIII".to_string(),
                assertion_type: "PREDICTIVE".to_string(),
                assertion_direction: "SUPPORTS".to_string(),
                amp_level: None,
                significance: "SENSITIVITYRESPONSE".to_string(),
                disease: None,
                therapies: vec!["rindopepimut".to_string()],
                status: "ACCEPTED".to_string(),
                summary: None,
                approvals_count: 0,
            },
            CivicAssertion {
                id: 4,
                name: "AID4".to_string(),
                molecular_profile: "EGFR".to_string(),
                assertion_type: "PREDICTIVE".to_string(),
                assertion_direction: "SUPPORTS".to_string(),
                amp_level: None,
                significance: "SENSITIVITYRESPONSE".to_string(),
                disease: None,
                therapies: vec!["rindopepimut".to_string()],
                status: "ACCEPTED".to_string(),
                summary: None,
                approvals_count: 0,
            },
        ],
    };

    assert_eq!(
        extract_variant_targets_from_civic(&civic, &["EGFR".to_string()]),
        vec!["EGFRvIII".to_string()]
    );
}
