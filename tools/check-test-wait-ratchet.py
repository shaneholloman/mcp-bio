#!/usr/bin/env python3
"""Ceiling ratchet against new clock-based waits in tests.

Usage: check-test-wait-ratchet.py [--update] [--root DIR]

Waits should be signals (a handshake line, an end-of-input, a kernel
state) — see ticket 1252. This ratchet scans test code for the clock
shapes that made the suite flaky under load:

  Rust   Instant::now() + ..., thread::sleep, tokio::time::sleep
  Python time.sleep

plus test helpers named *heartbeat* that contain a bare sleep. A line
carrying a `watchdog: <reason>` comment passes: it declares a bounded
watchdog that a human vouched for.

Every scanned file is pinned in tools/test-wait-inventory.json as a
per-file ceiling. Counts above the ceiling fail. Counts below it pass;
re-pin them down deliberately with `--update` (the same discipline as
tools/update-rust-source-size-inventory) so decreases are reviewed,
not automatic.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

# `Instant::now() + watchdog(..)` routes through the shared scaled
# builder and does not count; every other deadline addition does.
# `\bsleep(` catches the bare form that follows a `use
# std::thread::sleep` import (and subsumes the qualified forms);
# `sleep_until(` parks a timer on the clock like any sleep; and ANY
# comparison against an `elapsed()` value on either side of the
# operator is a deadline poll (a `<` on the right, a chained
# `.as_millis()` on the left — the shapes the old `>` -only pattern
# missed). `>=` floor assertions are not comparisons against a
# deadline, so they stay out.
RUST_PATTERNS = [
    re.compile(pattern)
    for pattern in (
        r"Instant::now\(\)\s*\+\s*(?!.*\bwatchdog\()",
        r"\bsleep\s*\(",
        r"\bsleep_until\s*\(",
        # Any comparison against elapsed() in either operand order
        # and either operator is a deadline poll; the only exception
        # is a `>=` floor assertion (test took AT LEAST this long),
        # which the floor-exemption below subtracts.
        r"elapsed\(\)(?:\s*\.\s*\w+\(\))*\s*[<>]=?",
        r"[<>]=?\s*[\w.:]*elapsed\(",
        # A poll against the clock itself: `Instant::now() < deadline`
        # in either order (`+` additions are the allowed builder).
        r"Instant::now\(\)\s*[<>]=?",
        r"[<>]=?\s*[\w.:]*Instant::now\(\)",
    )
]
PYTHON_PATTERNS = [
    re.compile(pattern)
    for pattern in (
        r"time\.sleep",
        r"asyncio\.sleep",
        r"anyio\.sleep",
        r"(?<![\w.])sleep\s*\(",
        r"from\s+time\s+import\s+[^\n]*\bsleep\b",
    )
]
# Aliased imports resolve to per-file local names:
# `import time as t` makes `t.sleep(` a timed wait, and
# `from time import sleep as snooze` makes `snooze(` one. The bare
# `from time import sleep` form is covered by its own pattern.
TIME_MODULE_ALIAS = re.compile(r"^\s*import\s+time\s+as\s+(\w+)", re.MULTILINE)
# Rust use-aliases: `use std::thread::sleep as nap` (and the tokio
# timer) make `nap(` a timed wait, resolved per file.
RUST_SLEEP_ALIAS = re.compile(
    r"use\s+(?:std::thread|tokio::time)::sleep\s+as\s+(\w+)", re.MULTILINE
)
# Python module-object assignment: `t = time` makes `t.sleep(` one.
PYTHON_TIME_ASSIGN = re.compile(r"^\s*(\w+)\s*=\s*time\s*$", re.MULTILINE)
SLEEP_RENAME_IMPORT = re.compile(
    r"^\s*from\s+time\s+import\s+sleep\s+as\s+(\w+)", re.MULTILINE
)
# A `>=` floor assertion says the test took at least this long; it
# is a lower bound on elapsed effort, not a wait, and is subtracted
# from the comparison patterns above.
FLOOR_ASSERTION = re.compile(
    r"assert[^;]*?(?:[\w.:]*(?:elapsed|Instant::now))\(\)"
    r"(?:\s*\.\s*\w+\(\))*\s*>="
    r"|assert[^;]*?>=\s*[\w.:]*(?:elapsed|Instant::now)\("
)

# A `watchdog:` marker vouches for a wait only when it carries a
# reason: at least one word of three or more characters after the
# colon. A bare `watchdog:` (or `watchdog: x`) does not pass.
WATCHDOG_REASON = re.compile(r"watchdog:\s*\w{3,}")
DEFINITION = re.compile(
    r"^\s*(?:pub(?:\(.+?\))?\s+)?(?:async\s+)?fn\s+\w*heartbeat\w*|^\s*def\s+_?\w*heartbeat\w*",
    re.IGNORECASE,
)

TEST_FILE_NAME = re.compile(r"(?:^|/)(?:tests?\.rs|test_support\.rs)$")


def tracked(root: Path, directory: str, suffix: str) -> list[str]:
    # `git ls-files -- <dir>` plus an in-process suffix filter: git's
    # `**` pathspec magic varies by version, so the glob stays ours.
    output = subprocess.run(
        ["git", "ls-files", "--", directory],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    return sorted(
        line for line in output.splitlines() if line and line.endswith(suffix)
    )


def rust_test_region(path: Path) -> str | None:
    """Whole file for test-only files; after the first #[cfg(test)] otherwise."""
    text = path.read_text(encoding="utf-8")
    posix = path.as_posix()
    if "/tests/" in posix or posix.startswith("tests/") or TEST_FILE_NAME.search(posix):
        return text
    marker = text.find("#[cfg(test)]")
    if marker == -1:
        return None
    return text[marker:]


def count_waits(
    lines: list[str],
    patterns: list[re.Pattern[str]],
    alias_patterns: list[re.Pattern[str]] | None = None,
) -> tuple[int, list[str], int]:
    """Unmarked waits, their line numbers, and the marked count.

    `alias_patterns` are the per-file local names bound to the time
    module or its sleep (from `import time as t` / `from time import
    sleep as s`): a call through any of them is a timed wait.
    """
    marked = 0
    violations: list[str] = []
    in_heartbeat_helper = False
    alias_patterns = alias_patterns or []
    assert_open = False
    for number, line in enumerate(lines, start=1):
        if DEFINITION.search(line):
            in_heartbeat_helper = True
        elif re.match(r"^\s*(?:pub(?:\(.+?\))?\s+)?(?:async\s+)?fn\s|^\s*def\s", line):
            in_heartbeat_helper = False
        waited = any(p.search(line) for p in patterns) or any(
            p.search(line) for p in alias_patterns
        )
        if waited and (FLOOR_ASSERTION.search(line) or (assert_open and ">=" in line)):
            waited = False
        # A multi-line assert! opens here: a following `elapsed >=`
        # line is its floor assertion.
        assert_open = bool(re.search(r"assert\w*[!(]?\s*\($", line))
        if "watchdog:" in line:
            if WATCHDOG_REASON.search(line) and waited:
                marked += 1
                continue
            # A marker without a reason text does not vouch for the
            # wait; fall through and count it like any other.
        if waited:
            if in_heartbeat_helper:
                violations.append(
                    f"{number} heartbeat helper contains a bare timed wait"
                )
            else:
                violations.append(f"{number}")
    return len(violations), violations, marked


def local_time_aliases(text: str) -> list[re.Pattern[str]]:
    aliases: list[str] = []
    for match in TIME_MODULE_ALIAS.finditer(text):
        aliases.append(re.escape(match.group(1)) + r"\.sleep\s*\(")
    for match in SLEEP_RENAME_IMPORT.finditer(text):
        aliases.append(r"(?<![\w.])" + re.escape(match.group(1)) + r"\s*\(")
    for match in PYTHON_TIME_ASSIGN.finditer(text):
        aliases.append(re.escape(match.group(1)) + r"\.sleep\s*\(")
    return [re.compile(a) for a in aliases]


def rust_sleep_aliases(text: str) -> list[re.Pattern[str]]:
    aliases: list[str] = []
    for match in RUST_SLEEP_ALIAS.finditer(text):
        aliases.append(r"(?<![\w.])" + re.escape(match.group(1)) + r"\s*\(")
    return [re.compile(a) for a in aliases]


def scan(root: Path) -> dict[str, dict[str, object]]:
    files: dict[str, dict[str, object]] = {}
    rust_files = tracked(root, "src", ".rs") + tracked(root, "tests", ".rs")
    for relative in rust_files:
        path = root / relative
        region = rust_test_region(path)
        if region is None:
            continue
        count, _, marked = count_waits(
            region.splitlines(),
            RUST_PATTERNS,
            rust_sleep_aliases(region),
        )
        if count or marked:
            entry: dict[str, object] = {"count": count, "language": "rust"}
            if marked:
                entry["markers"] = marked
            files[relative] = entry
    for relative in tracked(root, "tests", ".py"):
        path = root / relative
        text = path.read_text(encoding="utf-8")
        count, _, marked = count_waits(
            text.splitlines(), PYTHON_PATTERNS, local_time_aliases(text)
        )
        if count or marked:
            entry = {"count": count, "language": "python"}
            if marked:
                entry["markers"] = marked
            files[relative] = entry
    return files


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root", type=Path, default=Path(__file__).resolve().parents[1]
    )
    parser.add_argument(
        "--update",
        action="store_true",
        help="re-pin every ceiling to the current counts (use to ratchet down)",
    )
    args = parser.parse_args()
    inventory_path = args.root / "tools/test-wait-inventory.json"
    inventory = json.loads(inventory_path.read_text(encoding="utf-8"))
    pinned: dict[str, dict] = {
        name: (entry if isinstance(entry, dict) else {"count": entry})
        for name, entry in inventory.get("files", {}).items()
    }
    current = scan(args.root)

    failures: list[str] = []
    notes: list[str] = []
    total_markers = sum(int(e.get("markers", 0)) for e in current.values())
    marker_ceiling = int(inventory.get("marker_total_ceiling", total_markers))
    for name, entry in sorted(current.items()):
        count = entry["count"]
        ceiling = pinned.get(name)
        if ceiling is None and count:
            failures.append(
                f"new unmarked timed waits in {name} ({count}); convert to a signal or mark each with `watchdog:`"
            )
        elif ceiling is None:
            # Markers only (count 0): pin the file so its marker count
            # is ratcheted too.
            failures.append(
                f"{name} carries `watchdog:` markers but is not in the inventory; "
                f"run with --update to pin it"
            )
        elif count > ceiling.get("count", count):
            failures.append(
                f"{name} has {count} unmarked waits, above the pinned ceiling {ceiling.get('count')}"
            )
        elif count < ceiling.get("count", count):
            notes.append(
                f"{name} dropped {ceiling.get('count')} -> {count}; re-pin down with --update"
            )
        pinned_markers = int(ceiling.get("markers", 0)) if ceiling else 0
        file_markers = int(entry.get("markers", 0))
        if ceiling is not None and file_markers > pinned_markers:
            failures.append(
                f"{name} carries {file_markers} `watchdog:` markers, above its pinned "
                f"{pinned_markers}; a new marker needs a reason and a same-commit "
                f"inventory raise"
            )
    for name in sorted(set(pinned) - set(current)):
        notes.append(f"{name} now has zero unmarked waits; re-pin down with --update")
    if total_markers > marker_ceiling:
        failures.append(
            f"the tree carries {total_markers} `watchdog:` markers, above the global "
            f"ceiling {marker_ceiling}; raise marker_total_ceiling in the same commit "
            f"as the new marker, with its reason"
        )

    if args.update:
        fresh = {name: entry for name, entry in sorted(current.items())}
        inventory["files"] = fresh
        inventory["marker_total_ceiling"] = sum(
            int(e.get("markers", 0)) for e in fresh.values()
        )
        inventory_path.write_text(
            json.dumps(inventory, indent=2) + "\n", encoding="utf-8"
        )
        print(f"re-pinned {len(current)} file ceilings in {inventory_path}")
        return 0

    for note in notes:
        print(f"note: {note}")
    if failures:
        print("test-wait ratchet failures:")
        for failure in failures:
            print(f"  {failure}")
        return 1
    print(
        f"test-wait ratchet ok ({len(current)} files with pinned ceilings, "
        f"{total_markers} watchdog markers under the ceiling {marker_ceiling})"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
