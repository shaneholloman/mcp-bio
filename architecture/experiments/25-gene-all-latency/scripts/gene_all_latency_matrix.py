#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from collections import defaultdict
from pathlib import Path
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Flatten gene-all latency summaries into matrix-friendly JSON."
    )
    parser.add_argument(
        "--input",
        required=True,
        help="Path to a gene_all_latency_probe.py JSON result file.",
    )
    parser.add_argument(
        "--output",
        required=True,
        help="Path to write the flattened matrix JSON.",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    input_path = Path(args.input)
    output_path = Path(args.output)

    payload = json.loads(input_path.read_text())
    summary_rows: list[dict[str, Any]] = []
    section_rows: list[dict[str, Any]] = []
    wall_clock_matrix: dict[str, Any] = defaultdict(lambda: defaultdict(dict))
    section_matrix: dict[str, Any] = defaultdict(
        lambda: defaultdict(lambda: defaultdict(dict))
    )

    for summary in payload.get("summaries", []):
        approach = summary["approach"]
        gene = summary["gene"]
        mode = summary["mode"]
        row = {
            "approach": approach,
            "gene": gene,
            "mode": mode,
            "successful_runs": summary["successful_runs"],
            "p50_wall_clock_ms": summary["p50_wall_clock_ms"],
            "p95_wall_clock_ms": summary["p95_wall_clock_ms"],
        }
        summary_rows.append(row)
        wall_clock_matrix[approach][mode][gene] = {
            "p50_wall_clock_ms": summary["p50_wall_clock_ms"],
            "p95_wall_clock_ms": summary["p95_wall_clock_ms"],
            "successful_runs": summary["successful_runs"],
        }

        for section in summary.get("section_summary", []):
            section_row = {
                "approach": approach,
                "gene": gene,
                "mode": mode,
                "section": section["section"],
                "runs_observed": section["runs_observed"],
                "p50_ms": section["p50_ms"],
                "p95_ms": section["p95_ms"],
                "completion_rate": section["completion_rate"],
                "outcomes": section["outcomes"],
            }
            section_rows.append(section_row)
            section_matrix[approach][mode][section["section"]][gene] = {
                "p50_ms": section["p50_ms"],
                "p95_ms": section["p95_ms"],
                "completion_rate": section["completion_rate"],
                "runs_observed": section["runs_observed"],
                "outcomes": section["outcomes"],
            }

    result = {
        "experiment": payload.get("experiment"),
        "input": str(input_path),
        "summary_rows": summary_rows,
        "section_rows": section_rows,
        "wall_clock_matrix_ms": wall_clock_matrix,
        "section_matrix_ms": section_matrix,
    }

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    print(output_path)


if __name__ == "__main__":
    main()
