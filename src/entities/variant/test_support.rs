//! Shared test-only helpers for decomposed variant module sidecars.

#[allow(unused_imports)]
pub(super) use serde_json::json;
#[allow(unused_imports)]
pub(super) use wiremock::matchers::{method, path, query_param};
#[allow(unused_imports)]
pub(super) use wiremock::{Mock, MockServer, ResponseTemplate};

pub(super) async fn lock_env() -> tokio::sync::MutexGuard<'static, ()> {
    crate::test_support::env_lock().lock().await
}

pub(super) struct EnvVarGuard {
    name: &'static str,
    previous: Option<String>,
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        // Safety: tests serialize environment mutation with `lock_env()`.
        unsafe {
            match &self.previous {
                Some(value) => std::env::set_var(self.name, value),
                None => std::env::remove_var(self.name),
            }
        }
    }
}

pub(super) fn set_env_var(name: &'static str, value: Option<&str>) -> EnvVarGuard {
    let previous = std::env::var(name).ok();
    // Safety: tests serialize environment mutation with `lock_env()`.
    unsafe {
        match value {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
    }
    EnvVarGuard { name, previous }
}
