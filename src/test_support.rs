use std::sync::OnceLock;

use tokio::sync::Mutex;

pub(crate) fn env_lock() -> &'static Mutex<()> {
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    ENV_LOCK.get_or_init(|| Mutex::new(()))
}

pub(crate) struct EnvVarGuard {
    name: &'static str,
    previous: Option<String>,
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        // Safety: tests serialize environment mutation with `env_lock()`, `lock_env()`, or
        // `env_lock_async()` before calling `set_env_var()`.
        unsafe {
            match &self.previous {
                Some(value) => std::env::set_var(self.name, value),
                None => std::env::remove_var(self.name),
            }
        }
    }
}

pub(crate) fn set_env_var(name: &'static str, value: Option<&str>) -> EnvVarGuard {
    let previous = std::env::var(name).ok();
    // Safety: tests serialize environment mutation with `env_lock()`, `lock_env()`, or
    // `env_lock_async()` before calling `set_env_var()`.
    unsafe {
        match value {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
    }
    EnvVarGuard { name, previous }
}

pub(crate) struct TempDirGuard {
    path: std::path::PathBuf,
}

impl TempDirGuard {
    pub(crate) fn new(label: &str) -> Self {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "biomcp-test-{label}-{}-{suffix}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    pub(crate) fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        if let Err(err) = std::fs::remove_dir_all(&self.path) {
            tracing::warn!(
                path = %self.path.display(),
                error = %err,
                "failed to clean up temp dir"
            );
        }
    }
}
