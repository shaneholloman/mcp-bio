//! Tier 2 - local-data construction. Pure: checks required-file detection and
//! stale/missing sync decisions. No network.

use std::time::{Duration, SystemTime};

use super::super::*;
use crate::test_support::TempDirGuard;

#[test]
fn who_ivd_missing_files_tracks_required_contract() {
    let root = TempDirGuard::new("who-ivd-missing-files");
    let missing = who_ivd_missing_files(root.path(), WHO_IVD_REQUIRED_FILES);
    assert_eq!(missing, vec![WHO_IVD_CSV_FILE]);
}

#[test]
fn file_is_stale_tracks_age_threshold() {
    let root = TempDirGuard::new("who-ivd-stale");
    let path = root.path().join(WHO_IVD_CSV_FILE);
    std::fs::write(&path, "header\n").expect("fixture should write");
    assert!(!file_is_stale(&path));

    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .expect("file should open");
    file.set_modified(
        SystemTime::now()
            .checked_sub(WHO_IVD_STALE_AFTER + Duration::from_secs(60))
            .expect("stale time should be valid"),
    )
    .expect("mtime should update");

    assert!(file_is_stale(&path));
}

#[test]
fn sync_state_marks_missing_fresh_stale_and_force() {
    let root = TempDirGuard::new("who-ivd-sync-state");
    assert!(matches!(
        sync_state(root.path(), WhoIvdSyncMode::Auto),
        SyncState::Missing
    ));
    assert!(matches!(
        sync_state(root.path(), WhoIvdSyncMode::Force),
        SyncState::Missing
    ));

    std::fs::write(root.path().join(WHO_IVD_CSV_FILE), super::fixture_csv())
        .expect("write WHO IVD CSV");
    assert!(matches!(
        sync_state(root.path(), WhoIvdSyncMode::Auto),
        SyncState::Fresh
    ));
    assert!(matches!(
        sync_state(root.path(), WhoIvdSyncMode::Force),
        SyncState::Stale
    ));

    let stale_path = root.path().join(WHO_IVD_CSV_FILE);
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(&stale_path)
        .expect("stale file should open");
    file.set_modified(
        SystemTime::now()
            .checked_sub(WHO_IVD_STALE_AFTER + Duration::from_secs(60))
            .expect("stale time should be valid"),
    )
    .expect("stale mtime should update");

    assert!(matches!(
        sync_state(root.path(), WhoIvdSyncMode::Auto),
        SyncState::Stale
    ));
}
