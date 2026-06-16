//! Shared CLI test helpers used by sidecar CLI test modules.

#[allow(unused_imports)]
pub(crate) use crate::test_support::{EnvVarGuard, TempDirGuard, set_env_var};
pub(crate) use wiremock::matchers::{method, path, query_param};
pub(crate) use wiremock::{Mock, MockServer, ResponseTemplate};

pub(crate) async fn lock_env() -> tokio::sync::MutexGuard<'static, ()> {
    crate::test_support::env_lock().lock().await
}
