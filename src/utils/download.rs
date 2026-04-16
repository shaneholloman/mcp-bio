use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::io::AsyncWriteExt;

use crate::error::BioMcpError;

pub fn cache_key(id: &str) -> String {
    format!("{:x}", md5::compute(id.as_bytes()))
}

fn download_path(id: &str) -> Result<PathBuf, BioMcpError> {
    Ok(crate::cache::resolve_cache_config()?
        .cache_root
        .join("downloads")
        .join(format!("{}.txt", cache_key(id))))
}

async fn create_unique_sibling_temp(
    path: &Path,
) -> Result<(tokio::fs::File, PathBuf), BioMcpError> {
    let Some(dir) = path.parent() else {
        return Err(BioMcpError::InvalidArgument(
            "Invalid cache path (no parent directory)".into(),
        ));
    };
    tokio::fs::create_dir_all(dir).await?;

    let stem = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("tmp");
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    for attempt in 0..32_u32 {
        let candidate = dir.join(format!(
            ".{stem}.{}.tmp",
            seed.saturating_add(attempt as u128)
        ));
        match tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
            .await
        {
            Ok(file) => return Ok((file, candidate)),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err.into()),
        }
    }

    Err(BioMcpError::Io(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "Unable to allocate secure temporary cache file",
    )))
}

async fn remove_temp_if_present(path: &Path) {
    let _ = tokio::fs::remove_file(path).await;
}

async fn existing_file_matches(path: &Path, content: &[u8]) -> bool {
    matches!(tokio::fs::read(path).await, Ok(existing) if existing == content)
}

async fn existing_regular_file(path: &Path) -> bool {
    matches!(tokio::fs::metadata(path).await, Ok(metadata) if metadata.is_file())
}

pub async fn write_atomic_bytes(path: &Path, content: &[u8]) -> Result<(), BioMcpError> {
    let (mut file, tmp_path) = create_unique_sibling_temp(path).await?;
    file.write_all(content).await?;
    file.flush().await?;
    file.sync_all().await?;
    drop(file);

    match tokio::fs::rename(&tmp_path, path).await {
        Ok(()) => Ok(()),
        Err(_err) if existing_file_matches(path, content).await => {
            remove_temp_if_present(&tmp_path).await;
            Ok(())
        }
        Err(err) => {
            if !existing_regular_file(path).await {
                remove_temp_if_present(&tmp_path).await;
                return Err(err.into());
            }

            match tokio::fs::remove_file(path).await {
                Ok(()) => {}
                Err(remove_err) if remove_err.kind() == std::io::ErrorKind::NotFound => {}
                Err(remove_err) => {
                    remove_temp_if_present(&tmp_path).await;
                    return Err(remove_err.into());
                }
            }

            match tokio::fs::rename(&tmp_path, path).await {
                Ok(()) => Ok(()),
                Err(_retry_err) if existing_file_matches(path, content).await => {
                    remove_temp_if_present(&tmp_path).await;
                    Ok(())
                }
                Err(retry_err) => {
                    remove_temp_if_present(&tmp_path).await;
                    Err(retry_err.into())
                }
            }
        }
    }
}

pub async fn save_atomic(id: &str, content: &str) -> Result<PathBuf, BioMcpError> {
    let path = download_path(id)?;
    if matches!(tokio::fs::metadata(&path).await, Ok(metadata) if metadata.is_file()) {
        return Ok(path);
    }

    write_atomic_bytes(&path, content.as_bytes()).await?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use tokio::sync::MutexGuard;

    use super::{cache_key, download_path, save_atomic, write_atomic_bytes};
    use crate::test_support::{TempDirGuard, set_env_var};

    fn block_on<F: Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("download test runtime")
            .block_on(future)
    }

    fn env_lock() -> MutexGuard<'static, ()> {
        crate::test_support::env_lock().blocking_lock()
    }

    #[tokio::test]
    async fn write_atomic_bytes_replaces_existing_file_contents() {
        let root = TempDirGuard::new("replace-existing");
        let target = root.path().join("ema.json");
        std::fs::write(&target, b"old").expect("existing file should be writable");

        write_atomic_bytes(&target, b"new")
            .await
            .expect("atomic write should replace existing file");

        let updated = std::fs::read(&target).expect("updated file should be readable");
        assert_eq!(updated, b"new");
    }

    #[tokio::test]
    async fn write_atomic_bytes_errors_for_non_file_destination() {
        let root = TempDirGuard::new("destination-directory");
        let target = root.path().join("ema.json");
        std::fs::create_dir_all(&target).expect("target directory should be created");

        let err = write_atomic_bytes(&target, b"new")
            .await
            .expect_err("directory destination should fail");
        assert!(
            err.to_string().contains("Is a directory")
                || err.to_string().contains("directory")
                || err.to_string().contains("Access is denied"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn download_path_default_root_resolves_to_cache_root_downloads() {
        let _lock = env_lock();
        let root = TempDirGuard::new("download-path-default");
        let cache_home = root.path().join("cache-home");
        let config_home = root.path().join("config-home");
        let _cache_home = set_env_var("XDG_CACHE_HOME", Some(&cache_home.to_string_lossy()));
        let _config_home = set_env_var("XDG_CONFIG_HOME", Some(&config_home.to_string_lossy()));
        let _cache_dir = set_env_var("BIOMCP_CACHE_DIR", None);
        let id = "pmid:12345";

        let path = download_path(id).expect("default download path should resolve");

        assert_eq!(
            path,
            cache_home
                .join("biomcp")
                .join("downloads")
                .join(format!("{}.txt", cache_key(id)))
        );
    }

    #[test]
    fn download_path_env_override_resolves_to_biomcp_cache_dir_downloads() {
        let _lock = env_lock();
        let root = TempDirGuard::new("download-path-env");
        let cache_home = root.path().join("cache-home");
        let config_home = root.path().join("config-home");
        let override_root = root.path().join("override-root");
        let _cache_home = set_env_var("XDG_CACHE_HOME", Some(&cache_home.to_string_lossy()));
        let _config_home = set_env_var("XDG_CONFIG_HOME", Some(&config_home.to_string_lossy()));
        let _cache_dir = set_env_var("BIOMCP_CACHE_DIR", Some(&override_root.to_string_lossy()));
        let id = "pmid:12345";

        let path = download_path(id).expect("override download path should resolve");

        assert_eq!(
            path,
            override_root
                .join("downloads")
                .join(format!("{}.txt", cache_key(id)))
        );
    }

    #[test]
    fn save_atomic_uses_cache_toml_root_for_download_target() {
        let _lock = env_lock();
        let root = TempDirGuard::new("save-atomic-cache-toml");
        let cache_home = root.path().join("cache-home");
        let config_home = root.path().join("config-home");
        let toml_root = root.path().join("toml-root");
        std::fs::create_dir_all(config_home.join("biomcp")).expect("config dir should exist");
        std::fs::write(
            config_home.join("biomcp").join("cache.toml"),
            format!("[cache]\ndir = \"{}\"\n", toml_root.display()),
        )
        .expect("cache.toml should be written");
        let _cache_home = set_env_var("XDG_CACHE_HOME", Some(&cache_home.to_string_lossy()));
        let _config_home = set_env_var("XDG_CONFIG_HOME", Some(&config_home.to_string_lossy()));
        let _cache_dir = set_env_var("BIOMCP_CACHE_DIR", None);
        let id = "pmid:save-atomic";

        let path = block_on(save_atomic(id, "hello world"))
            .expect("save_atomic should honor cache.toml root");

        assert_eq!(
            path,
            toml_root
                .join("downloads")
                .join(format!("{}.txt", cache_key(id)))
        );
        let content = std::fs::read_to_string(&path).expect("saved file should exist");
        assert_eq!(content, "hello world");
    }

    #[test]
    fn save_atomic_errors_when_target_path_is_directory() {
        let _lock = env_lock();
        let root = TempDirGuard::new("save-atomic-directory-target");
        let cache_home = root.path().join("cache-home");
        let config_home = root.path().join("config-home");
        let override_root = root.path().join("override-root");
        let id = "pmid:directory-target";
        let target = override_root
            .join("downloads")
            .join(format!("{}.txt", cache_key(id)));
        std::fs::create_dir_all(&target).expect("directory target should exist");
        let _cache_home = set_env_var("XDG_CACHE_HOME", Some(&cache_home.to_string_lossy()));
        let _config_home = set_env_var("XDG_CONFIG_HOME", Some(&config_home.to_string_lossy()));
        let _cache_dir = set_env_var("BIOMCP_CACHE_DIR", Some(&override_root.to_string_lossy()));

        let err = block_on(save_atomic(id, "hello world"))
            .expect_err("directory target should not short-circuit as a cached file");

        assert!(
            err.to_string().contains("Is a directory")
                || err.to_string().contains("directory")
                || err.to_string().contains("Access is denied"),
            "unexpected error: {err}"
        );
    }
}
