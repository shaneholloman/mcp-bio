# Harden — Collect article fulltext miss fixtures for BioC renderer gate

## Decomposition

Extracted the optimized standalone collector into an importable Python package plus a thin CLI wrapper:

- Library package: `architecture/experiments/384-collect-article-fulltext-miss-fixtures-for-bioc-renderer-gate/scripts/bioc_miss_candidates/`
  - `core.py` owns case discovery/replay, source fetchers, source summarizers, classification, compact matrix writing, and the optimized bounded concurrency.
  - `cli.py` owns only argument parsing and console status output.
  - `__init__.py` re-exports the public API for downstream spikes.
- CLI wrapper: `architecture/experiments/384-collect-article-fulltext-miss-fixtures-for-bioc-renderer-gate/scripts/collect_bioc_miss_candidates.py`
  - 16 lines after extraction.
  - Keeps the `uv run --script` entry point for operators.
  - Delegates to `bioc_miss_candidates.cli.main()`.

Why this split: future BioC renderer/source-rung and article harvest spikes need to import the fixture collection/classification logic directly. They should not shell out to this ticket's script or copy the source-probe/classification code.

The ticket has no explicit dependencies. The requested Mole `spike-plan.md` does not exist in this biomcp worktree; biomcp planning identifies the relevant downstream consumers as conditional future article fulltext spikes: BioC-to-Markdown renderer, BioC/PubTator source-rung, and article Markdown harvest/Vault-handoff work.

## Public API

Import path, when running from this worktree or adding the experiment scripts directory to `PYTHONPATH`:

```python
from pathlib import Path

from bioc_miss_candidates import (
    CASE_WORKERS,
    Classification,
    ProbeResults,
    collect_cases,
    classify_sources,
    load_cases_from_probe,
    measure_case,
    run_probe,
)
```

Public types and functions:

- `Case = dict[str, Any]` — compact article candidate metadata: case name, approach, PMID, PMCID, DOI, title, why.
- `SourceSummary = dict[str, Any]` — compact per-source request/response metadata and quality counts.
- `Classification = dict[str, Any]` — current-ladder/BioC/PubTator decision fields, including `material_bioc_win`.
- `ProbeResults = dict[str, Any]` — top-level JSON result shape.
- `collect_cases(search_limit: int | None = None) -> list[Case]` — collect the bounded prior-art + Europe PMC search case set.
- `load_cases_from_probe(path: Path) -> list[Case]` — replay a prior JSON result as a fixed regression-control case set.
- `measure_case(case: Case) -> Case` — collect all source summaries for one case and attach `classification`.
- `run_probe(out: Path, matrix: Path, *, cases_from: Path | None = None, search_limit: int | None = None, run_label: str = "explore-scale") -> ProbeResults` — preferred library entry point; writes compact JSON/CSV artifacts and returns the result.
- `classify_sources(sources: dict[str, SourceSummary]) -> Classification` — reusable source decision logic for future fixture-backed BioC win/no-win checks.
- `compact_rows(results: ProbeResults) -> list[dict[str, Any]]` and `write_csv(path: Path, rows: list[dict[str, Any]]) -> None` — stable compact matrix projection/writer.
- Lower-level helpers (`fetch`, `summarize_jats`, `summarize_bioc`, `summarize_html`, `summarize_pmc_oa`) remain importable for renderer spikes that need source-family-specific probes.

Usage examples for downstream spikes:

```python
from pathlib import Path
from bioc_miss_candidates import run_probe

results = run_probe(
    Path("/tmp/bioc-fixture-candidates.json"),
    Path("/tmp/bioc-fixture-candidates.csv"),
    search_limit=8,
    run_label="future-renderer-search",
)
assert results["material_bioc_win_count"] == 0
```

```python
from pathlib import Path
from bioc_miss_candidates import load_cases_from_probe, measure_case

cases = load_cases_from_probe(Path("results/bioc_miss_candidate_probe.json"))
measured = [measure_case(case) for case in cases]
wins = [case for case in measured if case["classification"]["material_bioc_win"]]
```

```python
from bioc_miss_candidates import collect_cases, measure_case

for case in collect_cases(search_limit=4):
    measured = measure_case(case)
    print(measured["case"], measured["classification"]["decision_reason"])
```

## Build System

This biomcp spike artifact is Python, not Zig. There is no `build.zig` in the worktree, so no Zig module/binary build update applies.

Downstream import options:

1. Add the experiment scripts directory to `PYTHONPATH`:

```bash
PYTHONPATH=architecture/experiments/384-collect-article-fulltext-miss-fixtures-for-bioc-renderer-gate/scripts \
  uv run --no-sync your_downstream_probe.py
```

2. Keep using the operator CLI wrapper when manually collecting artifacts:

```bash
architecture/experiments/384-collect-article-fulltext-miss-fixtures-for-bioc-renderer-gate/scripts/collect_bioc_miss_candidates.py \
  --search-limit 8 \
  --out /tmp/bioc.json \
  --matrix /tmp/bioc.csv
```

The CLI and the importable library share the same `run_probe()` implementation, so downstream imports and operator runs use the same optimized source collection/classification path.

## Regression Check

Benchmark outputs were written under `/tmp/t384-harden/`.

Commands run:

```bash
EXP=architecture/experiments/384-collect-article-fulltext-miss-fixtures-for-bioc-renderer-gate
/usr/bin/time -v "$EXP/scripts/collect_bioc_miss_candidates.py" \
  --cases-from "$EXP/results/bioc_miss_candidate_probe.json" \
  --run-label harden-regression \
  --out /tmp/t384-harden/regression.json \
  --matrix /tmp/t384-harden/regression_matrix.csv

/usr/bin/time -v "$EXP/scripts/collect_bioc_miss_candidates.py" \
  --search-limit 8 \
  --run-label harden-fullscale \
  --out /tmp/t384-harden/fullscale.json \
  --matrix /tmp/t384-harden/fullscale_matrix.csv
```

Results:

| Workload | Wall time | User | Sys | Max RSS | Cases | Material BioC wins | Matrix checksum |
|---|---:|---:|---:|---:|---:|---:|---|
| Fixed regression replay | 6.43s | 0.46s | 0.21s | 60,464 KB | 16 | 0 | `bb033f79aac4fddcc0ec5b09144b2675dc12ae54f5bc8465d6e3213d9b9dbb06` |
| Full-scale target | 7.65s | 0.66s | 0.30s | 67,660 KB | 28 | 0 | `c4fb68a75b2bd6bbfcd4f1ad08fcba39933669d2ceae4ee8aa999a24be0352b5` |

The fixed regression matrix matched the explore/optimize checksum exactly on the primary harden benchmark run. The full-scale target also matched the exploit/optimize matrix checksum exactly on that run. A separate adjacent old-monolith-vs-new-library comparison showed the same fixed-regression output for old and new implementations in that live window, with no BioC-win drift; one PMC HTML/miss row varied across live full-scale runs, matching the previously documented endpoint variability rather than a refactor behavior change.

Validation outputs:

- `collect_bioc_miss_candidates.py --help` passed.
- Library import and package compile passed.
- Compact JSON/source-field validation passed for `/tmp/t384-harden/regression.json` and `/tmp/t384-harden/fullscale.json`; durable outputs still strip response bodies.
- `git diff --check` passed.
- `cargo check --all-targets` passed.

No BioMCP runtime source ladder or renderer behavior changed.

## Reusable Assets

Downstream spikes inherit:

- A reusable `bioc_miss_candidates` package, not just a CLI script.
- Bounded case collection and fixed-case regression replay.
- Source request builders for Europe PMC XML/search, NCBI EFetch PMC XML, PMC OA manifest, PMC HTML, NCBI BioC JSON, and PubTator3 BioC JSON.
- Compact source summarizers for JATS/XML, BioC JSON, PMC HTML, and PMC OA manifest metadata.
- Shared dictionary-shaped type aliases for cases, source summaries, classifications, and probe results.
- Stable BioC win/no-win classification logic preserving the current XML/HTML-first ladder assumptions.
- Compact JSON/CSV artifact writing that records request URLs, source kinds, license/reuse evidence when present, quality counts, and decision reasons without committing full article bodies.
- A thin `uv` CLI wrapper for manual collection using the same library code.
