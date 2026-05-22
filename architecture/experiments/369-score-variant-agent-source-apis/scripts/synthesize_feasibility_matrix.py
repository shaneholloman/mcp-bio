#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# ///
"""CLI wrapper for synthesizing the ticket-369 source feasibility matrix."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))

from source_api_scoring import write_feasibility_matrix  # noqa: E402


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--external", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    output = write_feasibility_matrix(args.out, args.external)
    print(f"wrote {args.out} ({len(output['candidates'])} candidate sources)")


if __name__ == "__main__":
    main()
