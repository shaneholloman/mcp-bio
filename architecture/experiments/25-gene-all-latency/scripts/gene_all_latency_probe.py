#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import math
import os
import shutil
import statistics
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[4]
EXPERIMENT_ROOT = REPO_ROOT / "architecture" / "experiments" / "25-gene-all-latency"
WORK_ROOT = EXPERIMENT_ROOT / "work"


@dataclass(frozen=True)
class Approach:
    label: str
    env: dict[str, str]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Run isolated biomcp gene-all timing probes and write a JSON result payload."
        )
    )
    parser.add_argument(
        "--exe",
        default=str(REPO_ROOT / "target" / "release" / "biomcp"),
        help="Path to the release biomcp executable.",
    )
    parser.add_argument(
        "--approach",
        action="append",
        required=True,
        help=(
            "Approach spec in the form label or label:KEY=VALUE,KEY2=VALUE. "
            "Example: baseline or timeout-4s:BIOMCP_GENE_OPTIONAL_TIMEOUT_MS=4000"
        ),
    )
    parser.add_argument(
        "--gene",
        action="append",
        required=True,
        help="Gene symbol to profile. Repeat for multiple genes.",
    )
    parser.add_argument(
        "--runs",
        type=int,
        default=5,
        help="Number of runs per gene/output-mode/approach combination.",
    )
    parser.add_argument(
        "--timeout-seconds",
        type=int,
        default=180,
        help="Per-command timeout.",
    )
    parser.add_argument(
        "--output",
        required=True,
        help="Path to the output JSON file.",
    )
    return parser.parse_args()


def parse_approach(spec: str) -> Approach:
    label, _, env_blob = spec.partition(":")
    label = label.strip()
    if not label:
        raise ValueError(f"invalid approach spec {spec!r}: missing label")

    env: dict[str, str] = {}
    if env_blob:
        for pair in env_blob.split(","):
            pair = pair.strip()
            if not pair:
                continue
            key, sep, value = pair.partition("=")
            if not sep or not key.strip():
                raise ValueError(f"invalid env override {pair!r} in approach {spec!r}")
            env[key.strip()] = value
    return Approach(label=label, env=env)


def percentile(values: list[float], pct: float) -> float:
    if not values:
        return 0.0
    if len(values) == 1:
        return values[0]
    ordered = sorted(values)
    rank = (len(ordered) - 1) * pct
    low = math.floor(rank)
    high = math.ceil(rank)
    if low == high:
        return ordered[low]
    weight = rank - low
    return ordered[low] * (1 - weight) + ordered[high] * weight


def summarize_runs(runs: list[dict[str, Any]]) -> dict[str, Any]:
    wall_clock_ms = [run["wall_clock_ms"] for run in runs if run["returncode"] == 0]
    section_stats: dict[str, dict[str, Any]] = {}
    for run in runs:
        for section in run.get("timing_sections", []):
            stats = section_stats.setdefault(
                section["section"],
                {"elapsed_ms": [], "outcomes": {}, "data_runs": 0},
            )
            stats["elapsed_ms"].append(section["elapsed_ms"])
            outcome = section["outcome"]
            stats["outcomes"][outcome] = stats["outcomes"].get(outcome, 0) + 1
            if outcome == "data":
                stats["data_runs"] += 1

    section_summary = []
    total_runs = len(runs)
    for name, stats in sorted(
        section_stats.items(), key=lambda item: statistics.median(item[1]["elapsed_ms"]), reverse=True
    ):
        elapsed_ms = stats["elapsed_ms"]
        section_summary.append(
            {
                "section": name,
                "runs_observed": len(elapsed_ms),
                "p50_ms": round(statistics.median(elapsed_ms), 2),
                "p95_ms": round(percentile(elapsed_ms, 0.95), 2),
                "completion_rate": round(stats["data_runs"] / total_runs, 3) if total_runs else 0.0,
                "outcomes": stats["outcomes"],
            }
        )

    return {
        "successful_runs": len(wall_clock_ms),
        "p50_wall_clock_ms": round(statistics.median(wall_clock_ms), 2) if wall_clock_ms else None,
        "p95_wall_clock_ms": round(percentile(wall_clock_ms, 0.95), 2) if wall_clock_ms else None,
        "section_summary": section_summary,
    }


def run_case(
    exe: Path,
    approach: Approach,
    gene: str,
    json_mode: bool,
    run_index: int,
    timeout_seconds: int,
) -> dict[str, Any]:
    mode_label = "json" if json_mode else "markdown"
    case_root = WORK_ROOT / f"{approach.label}-{gene.lower()}-{mode_label}-run{run_index:02d}"
    cache_root = case_root / "cache"
    timing_path = case_root / "timing.json"
    shutil.rmtree(case_root, ignore_errors=True)
    case_root.mkdir(parents=True, exist_ok=True)

    cmd = [str(exe)]
    if json_mode:
        cmd.append("--json")
    cmd.extend(["get", "gene", gene, "all"])

    env = os.environ.copy()
    env.update(approach.env)
    env["BIOMCP_CACHE_DIR"] = str(cache_root)
    env["BIOMCP_GENE_TIMING_PATH"] = str(timing_path)

    started = time.perf_counter()
    try:
        proc = subprocess.run(
            cmd,
            cwd=REPO_ROOT,
            env=env,
            capture_output=True,
            text=True,
            timeout=timeout_seconds,
            check=False,
        )
        timed_out = False
        wall_clock_ms = round((time.perf_counter() - started) * 1000, 2)
        returncode = proc.returncode
        stdout = proc.stdout
        stderr = proc.stderr
    except subprocess.TimeoutExpired as exc:
        timed_out = True
        wall_clock_ms = round((time.perf_counter() - started) * 1000, 2)
        returncode = -1
        stdout = exc.stdout or ""
        stderr = (exc.stderr or "") + f"\nTIMEOUT after {timeout_seconds}s"

    timing = None
    if timing_path.exists():
        timing = json.loads(timing_path.read_text())

    return {
        "approach": approach.label,
        "gene": gene,
        "mode": mode_label,
        "run_index": run_index,
        "command": cmd,
        "env_overrides": approach.env,
        "returncode": returncode,
        "timed_out": timed_out,
        "wall_clock_ms": wall_clock_ms,
        "stdout_bytes": len(stdout.encode("utf-8")),
        "stderr_excerpt": stderr[-1000:],
        "timing_total_ms": timing["total_ms"] if timing else None,
        "timing_sections": timing["sections"] if timing else [],
    }


def main() -> None:
    args = parse_args()
    exe = Path(args.exe).resolve()
    if not exe.exists():
        raise SystemExit(f"missing executable: {exe}")

    approaches = [parse_approach(spec) for spec in args.approach]
    genes = [gene.strip().upper() for gene in args.gene if gene.strip()]
    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)

    payload: dict[str, Any] = {
        "experiment": "25-gene-all-latency",
        "exe": str(exe),
        "runs_per_case": args.runs,
        "timeout_seconds": args.timeout_seconds,
        "approaches": [{"label": approach.label, "env": approach.env} for approach in approaches],
        "genes": genes,
        "results": [],
        "summaries": [],
    }

    for approach in approaches:
        for gene in genes:
            for json_mode in (False, True):
                runs = [
                    run_case(exe, approach, gene, json_mode, run_index, args.timeout_seconds)
                    for run_index in range(1, args.runs + 1)
                ]
                payload["results"].extend(runs)
                payload["summaries"].append(
                    {
                        "approach": approach.label,
                        "gene": gene,
                        "mode": "json" if json_mode else "markdown",
                        **summarize_runs(runs),
                    }
                )

    output_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
    print(output_path)


if __name__ == "__main__":
    main()
