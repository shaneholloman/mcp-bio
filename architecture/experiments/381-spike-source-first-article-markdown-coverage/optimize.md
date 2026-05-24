# Optimize — Spike source-first article Markdown coverage

## Starting Baseline

Exploit contract numbers from `.march/exploit.md` were the correctness baseline and remained fixed throughout optimization:

| Metric | Baseline |
|---|---:|
| Europe PMC core metadata coverage | 4/4 |
| Current Europe PMC PMCID `fullTextXML` coverage | 3/3 |
| Direct Europe PMC PMID `fullTextXML` coverage | 0/4 |
| NCBI BioC PMCID coverage | 3/3 |
| NCBI BioC PMID coverage | 3/4 |
| PubTator3 PMID coverage | 4/4 |
| PubTator3 PMCID coverage | 0/3 |
| PMC OA manifest coverage | 3/3 |
| Current XML sections / paragraphs / tables / references | 91 / 432 / 3 / 485 |
| NCBI BioC PMCID passages / text chars | 935 / 296,065 |
| PubTator3 PMID annotations | 64 |

Optimization target: reduce CPU cost of the offline exploit analyzer over persisted probe JSON while preserving all coverage/quality/provenance contract artifacts. The ticket has no production BioMCP runtime benchmark target because runtime behavior changes are out of scope.

Baseline reproduction:

- Analyzer regenerated exploit summary JSON, case/source CSV, and regression-control CSV byte-for-byte identical to committed exploit artifacts.
- End-to-end wall time: 0.03 s, dominated by interpreter startup.
- In-process baseline: CLI-style workflow 371.43 µs/op; `summarize` 168.72 µs/op; `compare` 204.12 µs/op; `source_case_rows` 70.49 µs/op.
- Initial hotspot: `source_case_rows` row materialization, 3.046 s cumulative in a 2,000-iteration profile workload.

## Optimization Passes

| Pass | Hotspot | Approach | Before | After | Decision |
|---:|---|---|---:|---:|---|
| 1 | `source_case_rows` count extraction | Inline `count_value` in the inner row loop | Workflow 371.43 µs/op; rows 70.49 µs/op | Workflow 369.82 µs/op; rows 66.67 µs/op | Committed; primary +0.43%, rows +5.42% |
| 2 | `source_case_rows` quality bit extraction | Inline `quality_bits` in the inner row loop | Workflow 369.82 µs/op; rows 66.67 µs/op | Workflow 359.36 µs/op; rows 63.19 µs/op | Committed; primary +2.83%, rows +5.22% |
| 3 | Duplicate current row materialization in analyzer+comparison path | Add `compare_rows` and reuse `summary["source_case_rows"]` in `main` | CLI workflow 359.36 µs/op | CLI workflow 298.80 µs/op | Committed; primary +16.85% |
| 4 | `source_case_rows` license extraction | Inline `license_value` in the inner row loop | CLI workflow 298.80 µs/op; rows 64.27 µs/op | CLI workflow 291.53 µs/op; rows 63.64 µs/op | Committed; primary +2.43%; convergence |

All passes preserved byte-for-byte analyzer outputs.

## Final Numbers

Final validation after all committed optimizations:

- Analyzer output: byte-for-byte identical summary JSON, case/source CSV, and regression-control CSV.
- Syntax/checks: `uv run --no-sync python -m py_compile analyze_source_coverage.py` passed; `git diff --check HEAD~4..HEAD` passed.
- Final in-process benchmark, 30,000 iterations after warmup: CLI workflow 292.55 µs/op; `summarize` 159.47 µs/op; compatibility `compare` 181.79 µs/op; `source_case_rows` 60.68 µs/op.
- Final row counts: 29 source rows; 29 regression-control comparison rows.

Final contract numbers are unchanged:

| Metric | Final |
|---|---:|
| Europe PMC core metadata coverage | 4/4 |
| Current Europe PMC PMCID `fullTextXML` coverage | 3/3 |
| Direct Europe PMC PMID `fullTextXML` coverage | 0/4 |
| NCBI BioC PMCID coverage | 3/3 |
| NCBI BioC PMID coverage | 3/4 |
| PubTator3 PMID coverage | 4/4 |
| PubTator3 PMCID coverage | 0/3 |
| PMC OA manifest coverage | 3/3 |
| Current XML sections / paragraphs / tables / references | 91 / 432 / 3 / 485 |
| NCBI BioC PMCID passages / text chars | 935 / 296,065 |
| PubTator3 PMID annotations | 64 |

## Total Improvement

| Metric | Baseline | Final | Improvement |
|---|---:|---:|---:|
| Primary CLI analyzer workflow | 371.43 µs/op | 292.55 µs/op | 21.24% faster |
| `summarize` | 168.72 µs/op | 159.47 µs/op | 5.48% faster |
| `compare` compatibility wrapper | 204.12 µs/op | 181.79 µs/op | 10.94% faster |
| `source_case_rows` | 70.49 µs/op | 60.68 µs/op | 13.92% faster |
| End-to-end process wall time | 0.03 s | 0.04 s | No meaningful change; interpreter startup/noise dominates |
| Coverage/quality contract numbers | Explore/exploit baseline | Same | No regression |

## Convergence

Stopped after pass 4. The pass improved the primary CLI workflow by 2.43%, below the 5% convergence threshold, after completing more than the required three passes.

Remaining runtime is dominated by tiny 29-row Python dictionary materialization and process startup. Further micro-optimizations are likely within measurement noise or would trade readability for little value. End-to-end wall time is already dominated by interpreter startup rather than analyzer logic.

## Remaining Opportunities

- If the candidate matrix grows substantially, introduce a dedicated benchmark script and profile larger persisted probes.
- If row materialization becomes hot at larger scale, consider a typed row object or a two-phase internal summary that does not require CSV-shaped dictionaries until write time.
- If process startup matters, batch multiple analyses in one process or integrate the analyzer as a library call instead of invoking a script per probe.
- Do not optimize live-source probe latency in this ticket; those timings are external-service telemetry and are intentionally not routine gates.

## Regression Control

Final numbers match the explore/exploit regression-control baseline for every coverage and quality metric. No BioMCP runtime behavior changed.
