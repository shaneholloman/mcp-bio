"""CLI wiring for the ticket 381 source-coverage analyzer."""

from __future__ import annotations

import argparse
from pathlib import Path

from .compare import compare_rows
from .io import load_json, write_csv, write_json
from .rows import source_case_rows
from .summary import summarize


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=Path, required=True, help="Probe JSON to summarize")
    parser.add_argument("--out", type=Path, required=True, help="Summary JSON output path")
    parser.add_argument("--rows", type=Path, required=True, help="Case/source CSV output path")
    parser.add_argument("--baseline", type=Path, help="Optional baseline probe JSON for regression comparison")
    parser.add_argument("--comparison", type=Path, help="Regression comparison CSV output path")
    args = parser.parse_args(argv)

    data = load_json(args.input)
    summary = summarize(data)
    write_json(args.out, summary)
    write_csv(args.rows, summary["source_case_rows"])

    if args.baseline or args.comparison:
        if not (args.baseline and args.comparison):
            parser.error("--baseline and --comparison must be provided together")
        baseline_data = load_json(args.baseline)
        comparison = compare_rows(source_case_rows(baseline_data), summary["source_case_rows"])
        write_csv(args.comparison, comparison)

    print(f"wrote {args.out}")
    print(f"wrote {args.rows}")
    if args.comparison:
        print(f"wrote {args.comparison}")
    return 0
