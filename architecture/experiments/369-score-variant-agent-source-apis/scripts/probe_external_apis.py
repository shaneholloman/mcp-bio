#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# ///
"""CLI wrapper for public API probes used by ticket 369."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))

from source_api_scoring import write_external_api_report  # noqa: E402


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    output = write_external_api_report(args.out)
    print(f"wrote {args.out} ({output['probe_count']} probes)")


if __name__ == "__main__":
    main()
