from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[1]
LINT_SCRIPT = REPO_ROOT / "bin" / "lint"


def _copy_lint_fixture(tmp_path: Path) -> Path:
    fixture_root = tmp_path / "repo"
    (fixture_root / "bin").mkdir(parents=True)
    (fixture_root / "docs").mkdir()
    shutil.copy2(LINT_SCRIPT, fixture_root / "bin" / "lint")
    subprocess.run(["git", "init"], cwd=fixture_root, check=True, capture_output=True)
    return fixture_root


def _track_files(repo_root: Path, files: dict[str, str]) -> None:
    for relative_path, contents in files.items():
        path = repo_root / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents, encoding="utf-8")
    subprocess.run(["git", "add", "."], cwd=repo_root, check=True)


def _run_lint(repo_root: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["bash", "bin/lint"],
        cwd=repo_root,
        capture_output=True,
        text=True,
        check=False,
    )


@pytest.mark.parametrize("relative_path", ["README.md", "docs/install.md"])
def test_lint_rejects_deprecated_public_doc_tokens(
    tmp_path: Path, relative_path: str
) -> None:
    repo_root = _copy_lint_fixture(tmp_path)
    _track_files(repo_root, {relative_path: "pip install biomcp-python\n"})

    result = _run_lint(repo_root)

    assert result.returncode == 1
    assert f"{relative_path}:1:pip install biomcp-python" in result.stdout
    assert "[FAIL] deprecated public-doc install string scan" in result.stdout


def test_lint_ignores_historical_biomcp_python_mentions_in_changelog(
    tmp_path: Path,
) -> None:
    repo_root = _copy_lint_fixture(tmp_path)
    _track_files(repo_root, {"CHANGELOG.md": "Historical note: biomcp-python hotfix.\n"})

    result = _run_lint(repo_root)

    assert result.returncode == 0
    assert "[PASS] deprecated public-doc install string scan" in result.stdout
    assert "CHANGELOG.md" not in result.stdout


def test_lint_reports_each_offending_public_doc_line_once(tmp_path: Path) -> None:
    repo_root = _copy_lint_fixture(tmp_path)
    _track_files(repo_root, {"README.md": "pip install biomcp-python\n"})

    result = _run_lint(repo_root)

    offending_line = "README.md:1:pip install biomcp-python"
    assert result.returncode == 1
    assert result.stdout.count(offending_line) == 1
