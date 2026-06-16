//! Tier 2 - local-data construction. Pure: checks the DDInter sync/download
//! plan and identity terms without network.

use super::super::*;

#[test]
fn sync_plan_downloads_missing_and_stale_files() {
    let root = tempfile::tempdir().expect("tempdir");
    std::fs::write(root.path().join(DDINTER_REQUIRED_FILES[0]), b"ok").expect("write");

    let auto_plan = sync_plan(root.path(), DdinterSyncMode::Auto);
    assert_eq!(auto_plan.len(), DDINTER_REQUIRED_FILES.len() - 1);
    assert!(
        !auto_plan
            .iter()
            .any(|(file_name, _)| *file_name == DDINTER_REQUIRED_FILES[0])
    );

    let force_plan = sync_plan(root.path(), DdinterSyncMode::Force);
    assert_eq!(force_plan.len(), DDINTER_REQUIRED_FILES.len());
}

#[test]
fn ddinter_missing_files_reports_incomplete_bundle() {
    let root = tempfile::tempdir().expect("tempdir");
    std::fs::write(root.path().join(DDINTER_REQUIRED_FILES[0]), b"ok").expect("write");

    let missing = ddinter_missing_files(root.path(), DDINTER_REQUIRED_FILES);
    assert_eq!(missing.len(), DDINTER_REQUIRED_FILES.len() - 1);
    assert!(!missing.contains(&DDINTER_REQUIRED_FILES[0]));
}

#[test]
fn ddinter_identity_dedupes_alias_terms() {
    let aliases = vec!["Coumadin".to_string(), "WARFARIN".to_string()];
    let identity = DdinterIdentity::with_aliases("warfarin", Some("Warfarin"), &aliases);
    assert_eq!(
        identity.terms(),
        &["warfarin".to_string(), "coumadin".to_string()]
    );
}
