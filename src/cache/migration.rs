use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug)]
pub(crate) enum MigrationOutcome {
    Renamed,
    SkippedOldMissing,
    SkippedTargetPresent,
}

fn directory_exists(path: &Path, label: &str) -> Result<bool, io::Error> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => Ok(true),
        Ok(metadata) if metadata.file_type().is_symlink() => match fs::metadata(path) {
            Ok(target_metadata) if target_metadata.is_dir() => Ok(true),
            Ok(_) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{label} {} exists but is not a directory", path.display()),
            )),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{label} {} exists but points to a missing target",
                    path.display()
                ),
            )),
            Err(err) => Err(err),
        },
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} {} exists but is not a directory", path.display()),
        )),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err),
    }
}

pub(crate) fn migrate_http_cache(cache_root: &Path) -> Result<MigrationOutcome, io::Error> {
    let old = cache_root.join("http-cacache");
    let new = cache_root.join("http");

    if !directory_exists(&old, "legacy cache path")? {
        return Ok(MigrationOutcome::SkippedOldMissing);
    }

    if directory_exists(&new, "runtime cache target")? {
        return Ok(MigrationOutcome::SkippedTargetPresent);
    }

    fs::rename(&old, &new)?;
    Ok(MigrationOutcome::Renamed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDirGuard;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    fn assert_invalid_input_contains(
        result: Result<MigrationOutcome, io::Error>,
        expected: &[&str],
    ) {
        match result {
            Err(err) => {
                assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
                let message = err.to_string();
                for needle in expected {
                    assert!(
                        message.contains(needle),
                        "expected error to contain {needle:?}, got: {message}"
                    );
                }
            }
            Ok(_) => panic!("expected invalid-input error"),
        }
    }

    #[test]
    fn renames_legacy_http_cache_directory_when_only_legacy_dir_exists() {
        let root = TempDirGuard::new("rename");
        let legacy_dir = root.path().join("http-cacache");
        let target_dir = root.path().join("http");
        std::fs::create_dir_all(&legacy_dir).expect("create legacy dir");
        let sentinel = legacy_dir.join("sentinel.txt");
        std::fs::write(&sentinel, b"cached payload").expect("write sentinel");

        let result = migrate_http_cache(root.path());

        assert!(matches!(result, Ok(MigrationOutcome::Renamed)));
        assert!(target_dir.is_dir(), "runtime dir should exist after rename");
        assert!(
            target_dir.join("sentinel.txt").is_file(),
            "sentinel file should move into runtime dir"
        );
        assert_eq!(
            std::fs::read(target_dir.join("sentinel.txt")).expect("read sentinel"),
            b"cached payload"
        );
        assert!(
            !legacy_dir.exists(),
            "legacy dir should not remain after successful rename"
        );
    }

    #[test]
    fn skips_when_legacy_http_cache_directory_is_missing() {
        let root = TempDirGuard::new("old-missing");

        let result = migrate_http_cache(root.path());

        assert!(matches!(result, Ok(MigrationOutcome::SkippedOldMissing)));
    }

    #[test]
    fn skips_when_runtime_http_directory_already_exists() {
        let root = TempDirGuard::new("target-present");
        let legacy_dir = root.path().join("http-cacache");
        let target_dir = root.path().join("http");
        std::fs::create_dir_all(&legacy_dir).expect("create legacy dir");
        std::fs::create_dir_all(&target_dir).expect("create target dir");
        std::fs::write(legacy_dir.join("legacy.txt"), b"legacy").expect("write legacy file");
        std::fs::write(target_dir.join("runtime.txt"), b"runtime").expect("write runtime file");

        let result = migrate_http_cache(root.path());

        assert!(matches!(result, Ok(MigrationOutcome::SkippedTargetPresent)));
        assert!(legacy_dir.join("legacy.txt").is_file());
        assert!(target_dir.join("runtime.txt").is_file());
    }

    #[test]
    fn errors_when_legacy_path_is_not_a_directory() {
        let root = TempDirGuard::new("legacy-file");
        std::fs::write(root.path().join("http-cacache"), b"not a dir").expect("write legacy file");

        assert_invalid_input_contains(
            migrate_http_cache(root.path()),
            &["legacy cache path", "not a directory"],
        );
    }

    #[test]
    fn errors_when_runtime_http_target_is_not_a_directory() {
        let root = TempDirGuard::new("target-file");
        std::fs::create_dir_all(root.path().join("http-cacache")).expect("create legacy dir");
        std::fs::write(root.path().join("http"), b"not a dir").expect("write target file");

        assert_invalid_input_contains(
            migrate_http_cache(root.path()),
            &["runtime cache target", "not a directory"],
        );
    }

    #[cfg(unix)]
    #[test]
    fn errors_when_legacy_path_is_a_dangling_symlink() {
        let root = TempDirGuard::new("legacy-dangling-symlink");
        symlink(
            root.path().join("missing-legacy"),
            root.path().join("http-cacache"),
        )
        .expect("create dangling legacy symlink");

        assert_invalid_input_contains(
            migrate_http_cache(root.path()),
            &["legacy cache path", "missing target"],
        );
    }

    #[cfg(unix)]
    #[test]
    fn errors_when_runtime_http_target_is_a_dangling_symlink() {
        let root = TempDirGuard::new("target-dangling-symlink");
        std::fs::create_dir_all(root.path().join("http-cacache")).expect("create legacy dir");
        symlink(root.path().join("missing-target"), root.path().join("http"))
            .expect("create dangling runtime symlink");

        assert_invalid_input_contains(
            migrate_http_cache(root.path()),
            &["runtime cache target", "missing target"],
        );
    }
}
