from __future__ import annotations

import re
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[1]
ALLOWED = {
    "struct EnvVarGuard": {"src/test_support.rs"},
    "struct TempDirGuard": {"src/test_support.rs"},
    "fn set_env_var": {"src/test_support.rs"},
}
DEFINITION_PATTERNS = {
    "struct EnvVarGuard": re.compile(
        r"^\s*(?:pub(?:\(.+?\))?\s+)?struct\s+EnvVarGuard\b",
        re.MULTILINE,
    ),
    "struct TempDirGuard": re.compile(
        r"^\s*(?:pub(?:\(.+?\))?\s+)?struct\s+TempDirGuard\b",
        re.MULTILINE,
    ),
    "fn set_env_var": re.compile(
        r"^\s*(?:pub(?:\(.+?\))?\s+)?fn\s+set_env_var\b",
        re.MULTILINE,
    ),
}


def rust_sources_under(root: Path) -> list[Path]:
    return sorted((root / "src").rglob("*.rs"))


def files_with_marker(root: Path, marker: str) -> list[str]:
    matches = []
    for path in rust_sources_under(root):
        contents = path.read_text(encoding="utf-8")
        if DEFINITION_PATTERNS[marker].search(contents):
            matches.append(path.relative_to(root).as_posix())
    return matches


def assert_marker_is_centralized(root: Path, marker: str) -> None:
    matches = files_with_marker(root, marker)
    expected = sorted(ALLOWED[marker])
    assert matches == expected, (
        f"{marker!r} must be defined only in {expected}, found {matches}"
    )


def test_helper_definitions_are_centralized() -> None:
    for marker in ALLOWED:
        assert_marker_is_centralized(REPO_ROOT, marker)


def test_scanner_reports_duplicate_definition_in_fixture(tmp_path: Path) -> None:
    src_root = tmp_path / "src"
    src_root.mkdir()
    (src_root / "test_support.rs").write_text(
        "pub(crate) struct EnvVarGuard {}\n"
        "pub(crate) struct TempDirGuard {}\n"
        "pub(crate) fn set_env_var() {}\n",
        encoding="utf-8",
    )
    (src_root / "duplicate.rs").write_text(
        "struct EnvVarGuard {}\n",
        encoding="utf-8",
    )

    with pytest.raises(AssertionError, match="src/duplicate.rs"):
        assert_marker_is_centralized(tmp_path, "struct EnvVarGuard")


def test_scanner_ignores_imports_and_comments_in_fixture(tmp_path: Path) -> None:
    src_root = tmp_path / "src"
    src_root.mkdir()
    (src_root / "test_support.rs").write_text(
        "pub(crate) struct EnvVarGuard {}\n"
        "pub(crate) struct TempDirGuard {}\n"
        "pub(crate) fn set_env_var() {}\n",
        encoding="utf-8",
    )
    (src_root / "reexports.rs").write_text(
        "use crate::test_support::{EnvVarGuard, TempDirGuard, set_env_var};\n"
        "// struct EnvVarGuard should not count here.\n"
        "const NOTE: &str = \"fn set_env_var is centralized\";\n",
        encoding="utf-8",
    )

    for marker in ALLOWED:
        assert_marker_is_centralized(tmp_path, marker)
