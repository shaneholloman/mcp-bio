"""Reusable current-BioMCP probe helpers for ticket 369."""

from __future__ import annotations

import json
import subprocess
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from typing import Any

from .types import CommandProbe


def default_command_probes() -> list[CommandProbe]:
    """Return the ticket-369 regression-control BioMCP command probes."""

    return [
        CommandProbe("variant", "MYD88 S219C search", ["--json", "search", "variant", "MYD88", "S219C", "--limit", "3"]),
        CommandProbe("variant", "ERBB2 D277Y search", ["--json", "search", "variant", "ERBB2", "D277Y", "--limit", "3"]),
        CommandProbe("variant", "KLHL6 L65P search", ["--json", "search", "variant", "KLHL6", "L65P", "--limit", "3"]),
        CommandProbe("variant", "KLHL6 rs148924291 population", ["--json", "get", "variant", "rs148924291", "population"]),
        CommandProbe("variant", "MITF transcript HGVS unsupported", ["--json", "get", "variant", "NM_000248.3:c.135del"]),
        CommandProbe("article", "MYD88 S219C article search", ["--json", "search", "article", "-g", "MYD88", "-k", "S219C", "--limit", "3"]),
        CommandProbe("article", "PMID 36053490 annotations", ["--json", "get", "article", "36053490", "annotations"]),
        CommandProbe("article", "PMID 29695787 fulltext", ["--json", "get", "article", "29695787", "fulltext"]),
        CommandProbe("article", "ASCO DOI via current get", ["--json", "get", "article", "10.1200/JCO.2018.36.15_suppl.e24316"]),
        CommandProbe("list", "variant list JSON", ["--json", "list", "variant"]),
        CommandProbe("list", "article list JSON", ["--json", "list", "article"]),
    ]


def summarize_biomcp_json(value: Any) -> dict[str, Any]:
    """Extract a stable, low-cardinality summary from BioMCP JSON output."""

    summary: dict[str, Any] = {}
    if isinstance(value, dict):
        summary["top_keys"] = sorted(value.keys())[:30]
        if isinstance(value.get("results"), list):
            summary["result_count"] = len(value["results"])
            first = value["results"][0] if value["results"] else None
            if isinstance(first, dict):
                summary["first_result_keys"] = sorted(first.keys())[:25]
                summary["first_id"] = first.get("id") or first.get("pmid")
                summary["first_title"] = first.get("title")
                summary["matched_sources"] = first.get("matched_sources")
        meta = value.get("_meta")
        if isinstance(meta, dict):
            summary["meta_keys"] = sorted(meta.keys())
            summary["next_commands_count"] = len(meta.get("next_commands") or [])
            summary["section_sources"] = meta.get("section_sources")
            summary["source_status"] = meta.get("source_status")
        for key in ["id", "gene", "hgvs_p", "hgvs_c", "rsid", "gnomad_af", "allele_frequency_percent", "full_text_note", "full_text_source", "pmid", "pmcid", "doi", "title"]:
            if key in value:
                summary[key] = value.get(key)
    elif isinstance(value, list):
        summary["list_count"] = len(value)
    return summary


def run_command_probe(bin_path: str, probe: CommandProbe) -> dict[str, Any]:
    """Run one current-BioMCP command probe."""

    cmd = [bin_path, *probe.args]
    started = time.perf_counter()
    proc = subprocess.run(cmd, text=True, capture_output=True, timeout=90)
    elapsed_ms = round((time.perf_counter() - started) * 1000, 2)
    parsed = None
    parse_error = None
    if probe.expect_json and proc.stdout.strip():
        try:
            parsed = json.loads(proc.stdout)
        except Exception as exc:  # noqa: BLE001
            parse_error = type(exc).__name__ + ": " + str(exc)
    return {
        "group": probe.group,
        "label": probe.label,
        "command": cmd,
        "returncode": proc.returncode,
        "ok": proc.returncode == 0,
        "elapsed_ms": elapsed_ms,
        "stdout_bytes": len(proc.stdout.encode()),
        "stderr_bytes": len(proc.stderr.encode()),
        "stderr_excerpt": proc.stderr[-500:],
        "parse_error": parse_error,
        "summary": summarize_biomcp_json(parsed) if parsed is not None else {},
    }


def run_current_biomcp_suite(
    bin_path: str,
    probes: list[CommandProbe] | None = None,
    *,
    max_workers: int = 8,
) -> dict[str, Any]:
    """Run the current-BioMCP probe suite and return the JSON report."""

    probe_list = probes or default_command_probes()
    with ThreadPoolExecutor(max_workers=max_workers) as executor:
        results = list(executor.map(lambda probe: run_command_probe(bin_path, probe), probe_list))
    return {
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "binary": bin_path,
        "probe_count": len(results),
        "results": results,
    }


def write_current_biomcp_report(
    path: Path,
    bin_path: str,
    probes: list[CommandProbe] | None = None,
    *,
    max_workers: int = 8,
) -> dict[str, Any]:
    """Run and write the current-BioMCP probe report."""

    output = run_current_biomcp_suite(bin_path, probes, max_workers=max_workers)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(output, indent=2, sort_keys=True) + "\n")
    return output
