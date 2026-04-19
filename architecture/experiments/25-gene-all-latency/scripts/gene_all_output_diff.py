#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
from pathlib import Path
from typing import Any

from gene_all_latency_probe import REPO_ROOT, WORK_ROOT, parse_approach


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Compare pre/post gene-all JSON outputs with persistent captures, "
            "checksums, and field-level diffs."
        )
    )
    parser.add_argument(
        "--exe",
        default=str(REPO_ROOT / "target" / "release" / "biomcp"),
        help="Path to the release biomcp executable.",
    )
    parser.add_argument(
        "--baseline-approach",
        required=True,
        help="Approach spec for the control path, e.g. baseline:BIOMCP_GENE_GET_STRATEGY=baseline",
    )
    parser.add_argument(
        "--candidate-approach",
        required=True,
        help="Approach spec for the exploit path, e.g. exploit or exploit:BIOMCP_GENE_GET_STRATEGY=parallel-top",
    )
    parser.add_argument(
        "--gene",
        action="append",
        required=True,
        help="Gene symbol to validate. Repeat for multiple genes.",
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
        help="Path to the output JSON report.",
    )
    return parser.parse_args()


def canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode("utf-8")


def sha256_hex(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def preview_value(value: Any, limit: int = 200) -> str:
    rendered = json.dumps(value, ensure_ascii=True, sort_keys=True)
    if len(rendered) <= limit:
        return rendered
    return rendered[: limit - 3] + "..."


def json_path(parent: str, token: str | int) -> str:
    escaped = str(token).replace("~", "~0").replace("/", "~1")
    if not parent:
        return f"/{escaped}"
    return f"{parent}/{escaped}"


def collect_diffs(baseline: Any, candidate: Any, path: str = "") -> list[dict[str, Any]]:
    diffs: list[dict[str, Any]] = []

    if isinstance(baseline, dict) and isinstance(candidate, dict):
        for key in sorted(set(baseline) | set(candidate)):
            next_path = json_path(path, key)
            if key not in baseline:
                diffs.append(
                    {
                        "kind": "added",
                        "path": next_path,
                        "baseline": None,
                        "candidate": preview_value(candidate[key]),
                    }
                )
            elif key not in candidate:
                diffs.append(
                    {
                        "kind": "dropped",
                        "path": next_path,
                        "baseline": preview_value(baseline[key]),
                        "candidate": None,
                    }
                )
            else:
                diffs.extend(collect_diffs(baseline[key], candidate[key], next_path))
        return diffs

    if isinstance(baseline, list) and isinstance(candidate, list):
        if len(baseline) != len(candidate):
            diffs.append(
                {
                    "kind": "changed",
                    "path": json_path(path, "length"),
                    "baseline": len(baseline),
                    "candidate": len(candidate),
                }
            )
        for index in range(min(len(baseline), len(candidate))):
            diffs.extend(collect_diffs(baseline[index], candidate[index], json_path(path, index)))
        for index in range(len(candidate), len(baseline)):
            diffs.append(
                {
                    "kind": "dropped",
                    "path": json_path(path, index),
                    "baseline": preview_value(baseline[index]),
                    "candidate": None,
                }
            )
        for index in range(len(baseline), len(candidate)):
            diffs.append(
                {
                    "kind": "added",
                    "path": json_path(path, index),
                    "baseline": None,
                    "candidate": preview_value(candidate[index]),
                }
            )
        return diffs

    if baseline != candidate:
        diffs.append(
            {
                "kind": "changed",
                "path": path or "/",
                "baseline": preview_value(baseline),
                "candidate": preview_value(candidate),
            }
        )

    return diffs


def run_capture(
    exe: Path,
    label: str,
    env_overrides: dict[str, str],
    gene: str,
    json_mode: bool,
    cache_root: Path,
    work_root: Path,
    timeout_seconds: int,
) -> dict[str, Any]:
    suffix = "json" if json_mode else "markdown"
    output_path = work_root / f"{label}-{gene.lower()}-{suffix}.txt"
    cmd = [str(exe)]
    if json_mode:
        cmd.append("--json")
    cmd.extend(["get", "gene", gene, "all"])

    env = os.environ.copy()
    env.update(env_overrides)
    env["BIOMCP_CACHE_DIR"] = str(cache_root)

    proc = subprocess.run(
        cmd,
        cwd=REPO_ROOT,
        env=env,
        capture_output=True,
        text=True,
        timeout=timeout_seconds,
        check=False,
    )
    output_path.write_text(proc.stdout)

    record = {
        "label": label,
        "mode": suffix,
        "command": cmd,
        "env_overrides": env_overrides,
        "output_path": str(output_path),
        "returncode": proc.returncode,
        "stderr_excerpt": proc.stderr[-1000:],
        "checksum_sha256": sha256_hex(proc.stdout.encode("utf-8")),
    }

    if proc.returncode != 0:
        record["parse_error"] = "command failed"
        return record

    if json_mode:
        try:
            parsed = json.loads(proc.stdout)
        except json.JSONDecodeError as err:
            record["parse_error"] = str(err)
            return record
        record["canonical_checksum_sha256"] = sha256_hex(canonical_json_bytes(parsed))
        record["parsed"] = parsed

    return record


def summarize_gene(
    exe: Path,
    baseline_label: str,
    baseline_env: dict[str, str],
    candidate_label: str,
    candidate_env: dict[str, str],
    gene: str,
    timeout_seconds: int,
) -> dict[str, Any]:
    gene_root = WORK_ROOT / "validation" / gene.lower()
    cache_root = gene_root / "cache"
    captures_root = gene_root / "captures"
    shutil.rmtree(gene_root, ignore_errors=True)
    captures_root.mkdir(parents=True, exist_ok=True)

    baseline_markdown = run_capture(
        exe,
        baseline_label,
        baseline_env,
        gene,
        False,
        cache_root,
        captures_root,
        timeout_seconds,
    )
    candidate_markdown = run_capture(
        exe,
        candidate_label,
        candidate_env,
        gene,
        False,
        cache_root,
        captures_root,
        timeout_seconds,
    )
    baseline_json = run_capture(
        exe,
        baseline_label,
        baseline_env,
        gene,
        True,
        cache_root,
        captures_root,
        timeout_seconds,
    )
    candidate_json = run_capture(
        exe,
        candidate_label,
        candidate_env,
        gene,
        True,
        cache_root,
        captures_root,
        timeout_seconds,
    )

    report = {
        "gene": gene,
        "cache_root": str(cache_root),
        "baseline_markdown": {
            key: value
            for key, value in baseline_markdown.items()
            if key not in {"parsed"}
        },
        "candidate_markdown": {
            key: value
            for key, value in candidate_markdown.items()
            if key not in {"parsed"}
        },
        "baseline_json": {
            key: value for key, value in baseline_json.items() if key not in {"parsed"}
        },
        "candidate_json": {
            key: value
            for key, value in candidate_json.items()
            if key not in {"parsed"}
        },
        "markdown_identical": baseline_markdown["checksum_sha256"]
        == candidate_markdown["checksum_sha256"],
        "json_identical": False,
        "mismatch_count": None,
        "added_field_count": None,
        "dropped_field_count": None,
        "changed_field_count": None,
        "sample_diffs": [],
    }

    if "parsed" not in baseline_json or "parsed" not in candidate_json:
        report["parse_error"] = "missing parsed JSON output"
        return report

    diffs = collect_diffs(baseline_json["parsed"], candidate_json["parsed"])
    report["json_identical"] = baseline_json["canonical_checksum_sha256"] == candidate_json[
        "canonical_checksum_sha256"
    ]
    report["mismatch_count"] = len(diffs)
    report["added_field_count"] = sum(diff["kind"] == "added" for diff in diffs)
    report["dropped_field_count"] = sum(diff["kind"] == "dropped" for diff in diffs)
    report["changed_field_count"] = sum(diff["kind"] == "changed" for diff in diffs)
    report["sample_diffs"] = diffs[:25]
    return report


def main() -> None:
    args = parse_args()
    exe = Path(args.exe).resolve()
    if not exe.exists():
        raise SystemExit(f"missing executable: {exe}")

    baseline = parse_approach(args.baseline_approach)
    candidate = parse_approach(args.candidate_approach)
    genes = [gene.strip().upper() for gene in args.gene if gene.strip()]

    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)

    report = {
        "experiment": "25-gene-all-latency",
        "exe": str(exe),
        "baseline": {"label": baseline.label, "env": baseline.env},
        "candidate": {"label": candidate.label, "env": candidate.env},
        "genes": genes,
        "results": [
            summarize_gene(
                exe,
                baseline.label,
                baseline.env,
                candidate.label,
                candidate.env,
                gene,
                args.timeout_seconds,
            )
            for gene in genes
        ],
    }

    output_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(output_path)


if __name__ == "__main__":
    main()
