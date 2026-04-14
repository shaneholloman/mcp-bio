use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_source(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
}

fn assert_module_doc_header(path: &Path) {
    let source = read_source(path);
    let first_line = source.lines().next().unwrap_or_default();
    assert!(
        first_line.starts_with("//!"),
        "missing //! header: {}",
        path.display()
    );
}

fn article_submodule_files(root: &Path) -> Vec<PathBuf> {
    vec![
        root.join("src/transform/article/anchors.rs"),
        root.join("src/transform/article/anchors/tests.rs"),
        root.join("src/transform/article/annotations.rs"),
        root.join("src/transform/article/annotations/tests.rs"),
        root.join("src/transform/article/federation.rs"),
        root.join("src/transform/article/federation/tests.rs"),
        root.join("src/transform/article/jats.rs"),
        root.join("src/transform/article/jats/refs.rs"),
        root.join("src/transform/article/jats/tests.rs"),
    ]
}

#[test]
fn transform_article_split_files_exist_with_doc_headers() {
    let root = repo_root();
    let facade = root.join("src/transform/article.rs");
    assert_module_doc_header(&facade);

    for path in article_submodule_files(&root) {
        assert!(path.is_file(), "missing expected file: {}", path.display());
        assert_module_doc_header(&path);
    }
}

#[test]
fn transform_article_submodule_files_stay_under_700_lines() {
    let root = repo_root();
    let article_dir = root.join("src/transform/article");
    assert!(
        article_dir.is_dir(),
        "expected article submodule directory: {}",
        article_dir.display()
    );

    for path in article_submodule_files(&root) {
        let line_count = read_source(&path).lines().count();
        assert!(
            line_count <= 700,
            "{} exceeds 700 lines: {}",
            path.display(),
            line_count
        );
    }
}
