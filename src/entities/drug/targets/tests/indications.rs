use super::*;

#[test]
fn format_opentargets_clinical_stage_maps_known_stages() {
    assert_eq!(
        format_opentargets_clinical_stage("APPROVAL").as_deref(),
        Some("Approved")
    );
    assert_eq!(
        format_opentargets_clinical_stage("PHASE_3").as_deref(),
        Some("Phase 3")
    );
    assert_eq!(
        format_opentargets_clinical_stage("PHASE_1_2").as_deref(),
        Some("Phase 1/2")
    );
    assert_eq!(
        format_opentargets_clinical_stage("PHASE_2_3").as_deref(),
        Some("Phase 2/3")
    );
    assert_eq!(
        format_opentargets_clinical_stage("EARLY_PHASE_1").as_deref(),
        Some("Early Phase 1")
    );
}

#[test]
fn format_opentargets_clinical_stage_suppresses_unknown_and_blank() {
    assert_eq!(format_opentargets_clinical_stage("UNKNOWN"), None);
    assert_eq!(format_opentargets_clinical_stage("   "), None);
}

#[test]
fn format_opentargets_clinical_stage_falls_back_for_future_values() {
    assert_eq!(
        format_opentargets_clinical_stage("PRECLINICAL").as_deref(),
        Some("Preclinical")
    );
}
