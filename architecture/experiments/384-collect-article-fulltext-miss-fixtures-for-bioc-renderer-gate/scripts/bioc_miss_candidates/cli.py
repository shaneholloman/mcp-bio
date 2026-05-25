"""Thin CLI for the ticket 384 BioC miss/degradation fixture collector."""

from __future__ import annotations

import argparse
from pathlib import Path

from .core import run_probe


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, required=True, help="Detailed compact JSON result path")
    parser.add_argument("--matrix", type=Path, required=True, help="Compact CSV matrix path")
    parser.add_argument("--cases-from", type=Path, help="Re-measure the fixed case set from an earlier probe JSON")
    parser.add_argument("--search-limit", type=int, help="Override the per-approach Europe PMC search limit for new candidate collection")
    parser.add_argument("--run-label", default="explore-scale", help="Human-readable run label stored in the JSON")
    args = parser.parse_args(argv)

    if args.cases_from and args.search_limit is not None:
        parser.error("--cases-from and --search-limit are mutually exclusive")

    results = run_probe(
        args.out,
        args.matrix,
        cases_from=args.cases_from,
        search_limit=args.search_limit,
        run_label=args.run_label,
    )
    print(f"wrote {args.out}")
    print(f"wrote {args.matrix}")
    print(f"cases={results['case_count']} material_bioc_wins={results['material_bioc_win_count']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
