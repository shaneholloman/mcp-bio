//! Tier 2 — request construction. Pure: builds `RequestPlan`s and asserts the exact
//! method / path that would be sent. Nothing is sent.

use super::super::*;
use crate::error::BioMcpError;
use crate::sources::HttpMethod;
use std::path::Path;
use std::path::PathBuf;

#[test]
fn study_list_plan_fetches_datahub_catalog() {
    let plan = CBioPortalDownloadClient::study_list_plan();

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "study_list.json");
    assert!(plan.query.is_empty());
    assert!(plan.headers.is_empty());
}

#[test]
fn study_archive_plan_fetches_tarball_for_valid_study_id() {
    let plan = CBioPortalDownloadClient::study_archive_plan(" demo_study ").unwrap();

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "demo_study.tar.gz");
    assert!(plan.query.is_empty());
}

#[test]
fn study_archive_plan_rejects_path_like_study_ids() {
    for study_id in ["../demo_study", "demo/study", "demo study", ""] {
        assert!(
            matches!(
                CBioPortalDownloadClient::study_archive_plan(study_id),
                Err(BioMcpError::InvalidArgument(_))
            ),
            "expected invalid argument for {study_id:?}"
        );
    }
}

#[test]
fn archive_relative_path_accepts_only_the_expected_top_level_study_dir() {
    assert_eq!(
        archive_relative_path("demo_study", Path::new("demo_study/meta_study.txt")).unwrap(),
        Some(PathBuf::from("meta_study.txt"))
    );
    assert_eq!(
        archive_relative_path("demo_study", Path::new("demo_study")).unwrap(),
        None
    );

    let wrong_top_level =
        archive_relative_path("demo_study", Path::new("other_study/meta_study.txt")).unwrap_err();
    assert!(matches!(wrong_top_level, BioMcpError::Api { .. }));

    let unsafe_path =
        archive_relative_path("demo_study", Path::new("demo_study/../evil.txt")).unwrap_err();
    assert!(matches!(unsafe_path, BioMcpError::Api { .. }));
}

#[test]
fn validate_study_id_trims_and_requires_a_single_safe_segment() {
    assert_eq!(validate_study_id(" demo_study ").unwrap(), "demo_study");

    for study_id in [
        "../demo_study",
        "demo/study",
        "demo study",
        "demo\tstudy",
        "",
    ] {
        assert!(
            matches!(
                validate_study_id(study_id),
                Err(BioMcpError::InvalidArgument(_))
            ),
            "expected invalid argument for {study_id:?}"
        );
    }
}
