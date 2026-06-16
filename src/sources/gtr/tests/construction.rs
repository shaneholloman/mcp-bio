//! Tier 2 — local bundle construction checks. GTR is file-backed, so these tests
//! assert the required local file contract and root-bound client construction.

use super::super::*;
use crate::test_support::TempDirGuard;

#[test]
fn gtr_missing_files_tracks_required_contract() {
    let root = TempDirGuard::new("gtr-missing-files");
    let missing = gtr_missing_files(root.path());
    assert_eq!(
        missing,
        vec![
            GTR_TEST_VERSION_FILE.to_string(),
            GTR_CONDITION_GENE_FILE.to_string()
        ]
    );

    std::fs::write(
        root.path().join(GTR_TEST_VERSION_FILE),
        super::parsing::test_version_gz_bytes(),
    )
    .expect("write partial bundle");
    let missing = gtr_missing_files(root.path());
    assert_eq!(missing, vec![GTR_CONDITION_GENE_FILE.to_string()]);
}

#[test]
fn client_from_root_uses_supplied_root_without_env() {
    let root = TempDirGuard::new("gtr-client-root");
    let client = GtrClient::from_root(root.path());

    assert_eq!(client.root, root.path());
}
