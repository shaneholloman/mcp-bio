//! Shared test-only helpers for decomposed variant module sidecars.

#[allow(unused_imports)]
pub(super) use crate::test_support::{EnvVarGuard, set_env_var};
#[allow(unused_imports)]
pub(super) use serde_json::json;
#[allow(unused_imports)]
pub(super) use wiremock::matchers::{method, path, query_param};
#[allow(unused_imports)]
pub(super) use wiremock::{Mock, MockServer, ResponseTemplate};

pub(super) async fn lock_env() -> tokio::sync::MutexGuard<'static, ()> {
    crate::test_support::env_lock().lock().await
}
