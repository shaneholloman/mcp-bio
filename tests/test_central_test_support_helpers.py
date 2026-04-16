from __future__ import annotations

from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[1]
ALLOWED = {
    "struct EnvVarGuard": {"src/test_support.rs"},
    "struct TempDirGuard": {"src/test_support.rs"},
    "fn set_env_var": {"src/test_support.rs"},
}


def rust_sources_under(root: Path) -> list[Path]:
    return sorted((root / "src").rglob("*.rs"))


def files_with_marker(root: Path, marker: str) -> list[str]:
    matches = []
    for path in rust_sources_under(root):
        if marker in path.read_text(encoding="utf-8"):
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
