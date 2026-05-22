#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# ///
"""CLI wrapper for current BioMCP probes used by ticket 369."""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))

from source_api_scoring import write_current_biomcp_report  # noqa: E402


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--bin", default=os.environ.get("BIOMCP_BIN", "target/debug/biomcp"))
    args = parser.parse_args()
    if not Path(args.bin).exists():
        raise SystemExit(f"BioMCP binary not found at {args.bin}; run cargo build --bin biomcp or set BIOMCP_BIN")
    output = write_current_biomcp_report(args.out, args.bin)
    print(f"wrote {args.out} ({output['probe_count']} probes)")


if __name__ == "__main__":
    main()
