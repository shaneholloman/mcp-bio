#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# ///
"""Small-scale inventory for ticket 371 test-strategy reset.

The script is intentionally static/lightweight: it extracts repo-local contracts,
source-test seams, spec dependency hints, validation profiles, and recent March
preflight evidence without rerunning expensive live gates.
"""

from __future__ import annotations

import json
import re
import tomllib
from collections import Counter, defaultdict
from dataclasses import dataclass, asdict
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[4]
RESULTS = ROOT / "architecture/experiments/reset-biomcp-test-strategy-around-request-contracts/results"
MARCH_RUNTIME = Path("/home/ian/workspace/.march-runtime/runs/biomcp/370-add-transcript-hgvs-normalization-proxies")

REPRESENTATIVE_SPECS = [
    ROOT / "spec/entity/disease.md",
    ROOT / "spec/surface/discover.md",
    ROOT / "spec/entity/article.md",
    ROOT / "spec/entity/variant.md",
]

SOURCE_FILES = [
    ROOT / "src/sources/ols4.rs",
    ROOT / "src/sources/mydisease.rs",
    ROOT / "src/sources/myvariant.rs",
    ROOT / "src/sources/pubmed.rs",
    ROOT / "src/sources/europepmc.rs",
    ROOT / "src/sources/semantic_scholar.rs",
]

PLAN_FILES = [
    ROOT / "src/cli/search_all/plan.rs",
    ROOT / "src/entities/article/planner.rs",
    ROOT / "src/cli/debug_plan.rs",
    ROOT / "src/cli/article/dispatch.rs",
    ROOT / "src/cli/disease/dispatch.rs",
    ROOT / "src/cli/variant/dispatch.rs",
]

DEPENDENCY_PATTERNS = {
    "ols4_discover": re.compile(r"OLS4|ols4|discover |discover\\\"|ERBB1|Arnold Chiari|MEF2", re.I),
    "disease_crosswalk": re.compile(r"crosswalk|Disease|disease|MONDO|MyDisease|phenotypes|diagnostics", re.I),
    "article_sources": re.compile(r"PubTator|Europe PMC|PubMed|Semantic Scholar|LitSense2|article|PMID", re.I),
    "variant_sources": re.compile(r"MyVariant|ClinVar|gnomAD|mutalyzer|variantvalidator|BRAF V600E|HGVS|rs113488022", re.I),
    "fixture_backed": re.compile(r"setup-|fixture|BIOMCP_.*_BASE|127\\.0\\.0\\.1|localhost", re.I),
    "live_public_api": re.compile(r"tools/biomcp-ci (search|get|discover|variant normalize)", re.I),
    "render_json_envelope": re.compile(r"--json|_meta|next_commands|jq -e|markdown|table", re.I),
    "help_list_contract": re.compile(r"--help|list |skill|suggest", re.I),
}

PROOF_PATTERNS = {
    "cli_parsing_routing": re.compile(r"--help|list |search |get |variant normalize|suggest|discover|Query:", re.I),
    "command_to_request_plan": re.compile(r"debug-plan|planner=|source_status|Resolved via|routing|filters", re.I),
    "source_request_construction": re.compile(r"BIOMCP_.*_BASE|setup-|fixture|source status|PubMed|MyVariant|OLS4|Semantic Scholar", re.I),
    "fixture_response_mapping": re.compile(r"fixture|Saved to:|fallback body text|source_status|degrades|unavailable", re.I),
    "render_json_envelope": re.compile(r"mustmatch like|mustmatch '/|jq -e|_meta|next_commands|table|# ", re.I),
    "cross_entity_workflow": re.compile(r"next_commands|follow-up|trials|articles|diagnostics|pivots|workflow", re.I),
    "live_smoke_canary": re.compile(r"tools/biomcp-ci (search|get|discover|variant normalize)", re.I),
}

@dataclass
class SpecSection:
    path: str
    heading: str
    line: int
    tags: list[str]
    proof_types: list[str]
    has_fixture_setup: bool
    uses_live_cli_wrapper: bool


def rel(path: Path) -> str:
    try:
        return str(path.relative_to(ROOT))
    except ValueError:
        return str(path)


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def extract_sections(path: Path) -> list[SpecSection]:
    text = read(path)
    lines = text.splitlines()
    headings: list[tuple[int, str]] = []
    for i, line in enumerate(lines, start=1):
        if line.startswith("## ") and not line.startswith("### "):
            headings.append((i, line[3:].strip()))
    sections: list[SpecSection] = []
    for idx, (line_no, heading) in enumerate(headings):
        end = headings[idx + 1][0] - 1 if idx + 1 < len(headings) else len(lines)
        body = "\n".join(lines[line_no - 1 : end])
        tags = [name for name, pat in DEPENDENCY_PATTERNS.items() if pat.search(body)]
        proof_types = [name for name, pat in PROOF_PATTERNS.items() if pat.search(body)]
        sections.append(
            SpecSection(
                path=rel(path),
                heading=heading,
                line=line_no,
                tags=tags,
                proof_types=proof_types,
                has_fixture_setup="fixture_backed" in tags,
                uses_live_cli_wrapper="live_public_api" in tags,
            )
        )
    return sections


def validation_profiles() -> dict[str, Any]:
    profile_file = ROOT / ".march/validation-profiles.toml"
    data = tomllib.loads(read(profile_file))
    comments = {}
    current = None
    for line in read(profile_file).splitlines():
        if line.startswith("# observed"):
            current = line.lstrip("# ").strip()
        elif line.startswith("[profile."):
            name = line.removeprefix("[profile.").removesuffix("]")
            if current:
                comments[name] = current
                current = None
    return {
        name: {"command": body["command"], "observed_comment": comments.get(name)}
        for name, body in data.get("profile", {}).items()
    }


def makefile_targets() -> dict[str, str]:
    targets: dict[str, list[str]] = defaultdict(list)
    current = None
    for line in read(ROOT / "Makefile").splitlines():
        m = re.match(r"^([A-Za-z0-9_.-]+):(?:\s|$)", line)
        if m and not line.startswith("."):
            current = m.group(1)
            continue
        if current and line.startswith("\t"):
            targets[current].append(line.strip())
    return {k: " && ".join(v) for k, v in targets.items() if k in {"check", "release-gate", "spec", "spec-pr", "test-contracts", "focused"}}


def source_contract_inventory() -> list[dict[str, Any]]:
    rows = []
    for path in SOURCE_FILES:
        text = read(path)
        rows.append(
            {
                "path": rel(path),
                "new_for_test": text.count("new_for_test"),
                "wiremock_tests": text.count("wiremock"),
                "mock_given": text.count("Mock::given"),
                "query_param_assertions": text.count("query_param"),
                "header_assertions": text.count("header("),
                "body_assertions": text.count("body_string_contains"),
                "env_base": text.count("env_base"),
                "apply_cache_mode": text.count("apply_cache_mode"),
                "tokio_tests": text.count("#[tokio::test]"),
                "plain_tests": text.count("#[test]"),
            }
        )
    return rows


def plan_seam_inventory() -> list[dict[str, Any]]:
    rows = []
    for path in PLAN_FILES:
        text = read(path)
        rows.append(
            {
                "path": rel(path),
                "structs": re.findall(r"(?:pub\([^)]*\)\s+|pub\s+)?struct\s+(\w+)", text),
                "enums": re.findall(r"(?:pub\([^)]*\)\s+|pub\s+)?enum\s+(\w+)", text),
                "functions_with_plan_name": sorted(set(re.findall(r"fn\s+(\w*plan\w*)", text, flags=re.I))),
                "filter_struct_mentions": text.count("Filters") + text.count("SearchFilters"),
                "debug_plan_mentions": text.count("DebugPlan"),
                "direct_entity_calls": len(re.findall(r"crate::entities::[a-z_]+::", text)),
                "direct_source_calls": len(re.findall(r"crate::sources::[a-z_]+::", text)),
            }
        )
    return rows


def march_preflight_evidence() -> list[dict[str, Any]]:
    rows = []
    for path in sorted(MARCH_RUNTIME.glob("*/preflight-check.json")):
        data = json.loads(read(path))
        output = data.get("output", "")
        failed = [
            line.split(" FAILED", 1)[0].strip()
            for line in output.splitlines()
            if " FAILED" in line and line.strip().startswith("spec/")
        ]
        failed.extend(
            match.strip()
            for match in re.findall(r"FAILED\s+((?:spec/)[^\n]+?)(?:\s+-|\n)", output)
            if match.strip() not in failed
        )
        passed_timings = re.findall(r"=+\s*(\d+) passed in ([0-9.]+)s", output)
        rows.append(
            {
                "path": str(path),
                "profile": data.get("profile"),
                "command": data.get("command"),
                "ok": data.get("ok"),
                "classification": data.get("classification"),
                "timestamp": data.get("timestamp"),
                "failed_sections": failed,
                "passed_timings": [
                    {"passed": int(count), "seconds": float(seconds)}
                    for count, seconds in passed_timings
                ],
                "output_mentions": {
                    "ols4_timeout": "ols4" in output.lower() and "timed out" in output.lower(),
                    "synonym_rescue_failure": "Synonym Rescue" in output and "FAILED" in output,
                    "mef2_failure": "MEF2 relational query" in output and "FAILED" in output,
                },
                "attempts": data.get("attempts", []),
            }
        )
    return rows


def summarize(sections: list[SpecSection], sources: list[dict[str, Any]], plans: list[dict[str, Any]], preflights: list[dict[str, Any]]) -> dict[str, Any]:
    tag_counts = Counter(tag for sec in sections for tag in sec.tags)
    proof_counts = Counter(tag for sec in sections for tag in sec.proof_types)
    live_sections = [asdict(sec) for sec in sections if sec.uses_live_cli_wrapper and not sec.has_fixture_setup]
    fixture_sections = [asdict(sec) for sec in sections if sec.has_fixture_setup]
    source_totals = Counter()
    for row in sources:
        for key, value in row.items():
            if isinstance(value, int):
                source_totals[key] += value
    return {
        "representative_spec_sections": len(sections),
        "tag_counts": dict(tag_counts),
        "proof_type_counts": dict(proof_counts),
        "live_nonfixture_sections": len(live_sections),
        "fixture_backed_sections": len(fixture_sections),
        "source_contract_totals": dict(source_totals),
        "plan_files_with_plan_functions": [row["path"] for row in plans if row["functions_with_plan_name"]],
        "march_preflight_runs": len(preflights),
        "march_preflight_failures": sum(1 for row in preflights if not row["ok"]),
        "march_failed_sections": [failure for row in preflights for failure in row["failed_sections"]],
    }


def proposed_profiles(summary: dict[str, Any]) -> list[dict[str, Any]]:
    return [
        {
            "name": "kickoff",
            "command_shape": "cargo check --all-targets + deterministic contract inventory subset",
            "live_network": False,
            "proves": ["compilation", "CLI command/filter/request-plan contracts", "source request construction for touched area"],
            "evidence": "Current preflight cargo check passed in 44.45s cold; no live upstream needed.",
        },
        {
            "name": "focused",
            "command_shape": "cargo test --lib for touched modules + clippy; optionally targeted wiremock source tests",
            "live_network": False,
            "proves": ["unit behavior", "source request/status mapping", "renderer envelope for touched code"],
            "evidence": f"Representative source files already expose {summary['source_contract_totals'].get('mock_given', 0)} wiremock Mock::given contracts.",
        },
        {
            "name": "spec-only",
            "command_shape": "fixture-backed executable specs and static surface contracts only",
            "live_network": False,
            "proves": ["CLI help/list", "fixture-backed response/rendering", "JSON envelope shape"],
            "evidence": f"Representative specs have {summary['fixture_backed_sections']} fixture-backed sections but {summary['live_nonfixture_sections']} live non-fixture sections to split/move.",
        },
        {
            "name": "verify/full-blocking",
            "command_shape": "make check + deterministic spec-only; no broad live canaries by default",
            "live_network": "minimal/controlled",
            "proves": ["repo health", "contract completeness", "quality ratchets"],
            "evidence": "Ticket 370 succeeded only after unrelated live/spec issues were resolved; keep broad checks out of kickoff.",
        },
        {
            "name": "release-live-smoke",
            "command_shape": "small serialized opt-in live smoke for OLS4/discover, disease crosswalk, article source status, variant normalization",
            "live_network": True,
            "proves": ["public upstream availability", "cache/keyless operator path", "release confidence"],
            "evidence": "Known failures are OLS4/discover/disease crosswalk; keep as explicit release/operator signal.",
        },
    ]


def main() -> None:
    RESULTS.mkdir(parents=True, exist_ok=True)
    sections = [sec for path in REPRESENTATIVE_SPECS for sec in extract_sections(path)]
    sources = source_contract_inventory()
    plans = plan_seam_inventory()
    preflights = march_preflight_evidence()
    profiles = validation_profiles()
    make_targets = makefile_targets()
    summary = summarize(sections, sources, plans, preflights)

    payload = {
        "script": rel(Path(__file__)),
        "repo_root": str(ROOT),
        "approaches": {
            "status_quo_live_partitioning": {
                "profiles": profiles,
                "make_targets": make_targets,
                "march_preflight_evidence": preflights,
            },
            "contract_first_existing_seams": {
                "representative_sections": [asdict(sec) for sec in sections],
                "source_contract_inventory": sources,
                "plan_seam_inventory": plans,
            },
            "minimal_request_plan_seam": {
                "candidate_existing_seams": plans,
                "recommended_boundaries": {
                    "cli": "clap args -> RequestCommand / typed filters; no network",
                    "entity": "RequestCommand -> source request plan(s) and renderer model orchestration",
                    "source": "SourceRequestPlan -> HTTP method/url/query/headers/body/cache/auth + fixture response mapping",
                    "render": "entity model -> markdown/JSON envelope, no upstream calls",
                },
            },
        },
        "summary": summary,
        "proposed_profiles": proposed_profiles(summary),
    }
    out = RESULTS / "test_strategy_inventory.json"
    out.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(out)


if __name__ == "__main__":
    main()
