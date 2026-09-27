"""Contract tests for the test-wait ceiling ratchet."""

from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "check_test_wait_ratchet", ROOT / "tools" / "check-test-wait-ratchet.py"
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)
scan = MODULE.scan
count_waits = MODULE.count_waits

INVENTORY = ROOT / "tools" / "test-wait-inventory.json"


def _run(root: Path, *args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            str(ROOT / "tools" / "check-test-wait-ratchet.py"),
            "--root",
            str(root),
            *args,
        ],
        capture_output=True,
        text=True,
        check=False,
        cwd=ROOT,
    )


def test_inventory_counts_match_the_tree() -> None:
    """The real tree passes against its pinned ceilings."""
    result = _run(ROOT)
    assert result.returncode == 0, result.stdout + result.stderr


def test_a_new_unmarked_wait_fails(tmp_path: Path) -> None:
    """A synthetic unmarked time.sleep in a scanned test file fails."""  # watchdog: synthetic literal
    root = tmp_path / "repo"
    (root / "tools").mkdir(parents=True)
    (root / "tools" / "test-wait-inventory.json").write_text(
        json.dumps({"schema": "biomcp-test-wait-inventory-v1", "files": {}}),
        encoding="utf-8",
    )
    tests_dir = root / "tests"
    tests_dir.mkdir()
    (tests_dir / "test_example.py").write_text(
        "def test_waits() -> None:\n    time.sleep(0.1)\n",  # watchdog: synthetic literal
        encoding="utf-8",
    )
    # scan() shells out to git ls-files, so stage the file.
    subprocess.run(["git", "init", "-q"], cwd=root, check=True)
    subprocess.run(["git", "add", "."], cwd=root, check=True)

    result = _run(root)
    assert result.returncode == 1
    assert "new unmarked timed waits in tests/test_example.py (1)" in result.stdout


def test_a_watchdog_marker_passes() -> None:
    lines = ["    time.sleep(0.05)  # watchdog: shared poll interval inside the helper"]
    count, violations, _ = count_waits(lines, [MODULE.PYTHON_PATTERNS[0]])
    assert count == 0
    assert violations == []


def test_a_heartbeat_helper_with_a_bare_sleep_counts() -> None:
    lines = [
        "def _heartbeat_advances(path):",
        "    before = path.read_text()",
        "    time.sleep(0.2)",  # watchdog: synthetic literal
        "    return path.read_text() != before",
    ]
    count, violations, _ = count_waits(lines, [MODULE.PYTHON_PATTERNS[0]])
    assert count == 1
    assert violations == ["3 heartbeat helper contains a bare timed wait"]


def test_watchdog_builder_deadlines_do_not_count() -> None:
    lines = [
        "    let deadline = std::time::Instant::now() + crate::test_support::watchdog(30);",
        "    let deadline = tokio::time::Instant::now() + watchdog(60);",
        "    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);",  # watchdog: synthetic literal
    ]
    count, violations, _ = count_waits(lines, MODULE.RUST_PATTERNS)
    assert count == 1
    assert violations == ["3"]


def test_an_inventory_decrease_passes_and_notes_the_ratchet_down(
    tmp_path: Path,
) -> None:
    """A count below the pinned ceiling passes; the note says re-pin down."""
    root = tmp_path / "repo"
    (root / "tools").mkdir(parents=True)
    (root / "tools" / "test-wait-inventory.json").write_text(
        json.dumps(
            {
                "schema": "biomcp-test-wait-inventory-v1",
                "files": {"tests/test_example.py": {"count": 2, "language": "python"}},
            }
        ),
        encoding="utf-8",
    )
    tests_dir = root / "tests"
    tests_dir.mkdir()
    (tests_dir / "test_example.py").write_text(
        "def test_waits() -> None:\n    time.sleep(0.1)\n",  # watchdog: synthetic literal
        encoding="utf-8",
    )
    subprocess.run(["git", "init", "-q"], cwd=root, check=True)
    subprocess.run(["git", "add", "."], cwd=root, check=True)

    result = _run(root)
    assert result.returncode == 0, result.stdout + result.stderr
    assert "dropped 2 -> 1" in result.stdout
    assert "re-pin down with --update" in result.stdout


def test_a_bare_sleep_after_the_time_import_counts() -> None:
    """The time-module sleep import plus a bare call is a timed wait."""  # watchdog: synthetic literal
    lines = [
        "from time import sleep",  # watchdog: synthetic literal
        "def test_waits() -> None:",
        "    sleep(0.1)",  # watchdog: synthetic literal
    ]
    count, _, _ = count_waits(lines, MODULE.PYTHON_PATTERNS)
    assert count == 2  # the import line and the call


def test_asyncio_sleep_counts() -> None:
    lines = ["    await asyncio.sleep(0.2)"]  # watchdog: synthetic literal
    count, _, _ = count_waits(lines, MODULE.PYTHON_PATTERNS)
    assert count == 1


def test_a_bare_rust_sleep_after_a_use_import_counts() -> None:
    """A use-import of sleep followed by a bare call is a timed wait."""  # watchdog: synthetic literal
    lines = [
        "    use std::thread::sleep;",  # watchdog: synthetic literal
        "    sleep(Duration::from_millis(25));",  # watchdog: synthetic literal
    ]
    count, _, _mk = count_waits(lines, MODULE.RUST_PATTERNS)
    assert count == 1


def test_a_strict_elapsed_deadline_check_counts_but_a_floor_assertion_does_not() -> (
    None
):
    lines = [
        "    if started.elapsed() > deadline {",  # watchdog: synthetic literal
        "    assert!(started.elapsed() >= Duration::from_millis(50));",
    ]
    count, violations, _ = count_waits(lines, MODULE.RUST_PATTERNS)
    assert count == 1
    assert violations == ["1"]


def test_a_watchdog_marker_without_a_reason_does_not_pass() -> None:
    """The marker must carry a reason; a bare marker vouches for nothing."""
    for line in (
        "    time.sleep(0.05)  # watchdog:",  # watchdog: synthetic literal
        "    time.sleep(0.05)  # watchdog: x",  # watchdog: synthetic literal
    ):
        count, violations, _ = count_waits([line], [MODULE.PYTHON_PATTERNS[0]])
        assert count == 1, line
        assert violations == ["1"], line


def test_the_helpers_own_poll_sleep_is_marked() -> None:
    """tests/support.py carries no unmarked wait, and its one marker
    is pinned so a second marker there needs a same-commit raise."""
    current = scan(ROOT)
    entry = current.get("tests/support.py")
    assert entry is not None and entry["count"] == 0, entry
    assert entry.get("markers") == 1, entry


def _planted_repo(tmp_path: Path, inventory: dict, files: dict[str, str]):
    """Plant a scratch repo. Keys are repo-relative paths like
    ``tests/test_one.py``."""
    (tmp_path / "tools").mkdir(parents=True)
    (tmp_path / "tools" / "test-wait-inventory.json").write_text(
        json.dumps(inventory), encoding="utf-8"
    )
    for name, body in files.items():
        target = tmp_path / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(body, encoding="utf-8")
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    subprocess.run(["git", "add", "."], cwd=tmp_path, check=True)


def test_the_new_wait_forms_go_red_on_planted_files(tmp_path: Path) -> None:
    """Each form the old patterns missed fails a planted repo."""
    root = tmp_path / "repo"
    _planted_repo(
        root,
        {"schema": "biomcp-test-wait-inventory-v1", "files": {}},
        {
            "tests/test_elapsed_left.rs": (
                "#[test]\nfn t() {\n    assert!(start.elapsed() < D(5));\n}\n"
            ),
            "tests/test_elapsed_right.rs": (
                "#[test]\nfn t() {\n    assert!(deadline < start.elapsed());\n}\n"
            ),
            "tests/test_chained_elapsed.rs": (
                "#[test]\nfn t() {\n"
                "    assert!(start.elapsed().as_millis() > 5);\n}\n"
            ),
            "tests/test_anyio.py": "def t():\n    await anyio.sleep(0.1)\n",  # watchdog: planted literal
            "tests/test_alias_time.py": (
                "import time as t\n\ndef test_waits():\n    t.sleep(0.1)\n"  # watchdog: planted literal
            ),
            "tests/test_alias_sleep.py": (
                "from time import sleep as snooze\n\ndef test_waits():\n"
                "    snooze(0.1)\n"  # watchdog: planted literal
            ),
        },
    )
    result = _run(root)
    assert result.returncode == 1
    for name in (
        "tests/test_elapsed_left.rs",
        "tests/test_elapsed_right.rs",
        "tests/test_chained_elapsed.rs",
        "tests/test_anyio.py",
        "tests/test_alias_time.py",
        "tests/test_alias_sleep.py",
    ):
        assert name in result.stdout, (name, result.stdout)


def test_rust_sleep_until_and_elapsed_go_red(tmp_path: Path) -> None:
    lines = [
        "    tokio::time::sleep_until(deadline).await;",
        "    assert!(deadline < start.elapsed());",
        "    assert!(start.elapsed().as_millis() > 5);",
        "    let x = start.elapsed();",  # no comparison: not a wait
    ]
    count, _, _ = MODULE.count_waits(lines, MODULE.RUST_PATTERNS)
    assert count == 3


def test_marker_counts_are_ratcheted(tmp_path: Path) -> None:
    """Markers are counted; a new one above the ceiling fails."""
    root = tmp_path / "repo"
    _planted_repo(
        root,
        {
            "schema": "biomcp-test-wait-inventory-v1",
            "marker_total_ceiling": 1,
            "files": {
                "tests/test_one.py": {
                    "count": 0,
                    "language": "python",
                    "markers": 1,
                }
            },
        },
        {
            "tests/test_one.py": (
                "def t():\n"
                "    time.sleep(0.1)  # watchdog: first bounded poll interval\n"  # watchdog: planted literal
            ),
            "tests/test_two.py": (
                "def t():\n"
                "    time.sleep(0.2)  # watchdog: second marker raises the total\n"  # watchdog: planted literal
            ),
        },
    )
    result = _run(root)
    assert result.returncode == 1
    # The global ceiling fails even though each file's own marker count
    # would pass once pinned; and the unpinned marker file is named.
    assert "above the global ceiling 1" in result.stdout, result.stdout
    assert "not in the inventory" in result.stdout, result.stdout
