//! Shared article test helpers used by sidecar test modules.

#[allow(dead_code)]
pub(crate) fn env_lock() -> &'static tokio::sync::Mutex<()> {
    crate::test_support::env_lock()
}
