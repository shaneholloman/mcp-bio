pub(crate) struct TempDirGuard {
    inner: tempfile::TempDir,
}

impl TempDirGuard {
    pub(crate) fn new(label: &str) -> Self {
        let inner = tempfile::Builder::new()
            .prefix(&format!("biomcp-test-{label}-"))
            .tempdir()
            .expect("create temp dir");
        Self { inner }
    }

    pub(crate) fn path(&self) -> &std::path::Path {
        self.inner.path()
    }
}
