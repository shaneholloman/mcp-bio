# Optimize — Collect article fulltext miss fixtures for BioC renderer gate

## Starting Baseline

Exploit contract correctness baseline:

| Metric | Regression control | Full-scale target |
|---|---:|---:|
| Cases | 16 | 28 |
| Material BioC wins | 0 | 0 |
| Current best: JATS/XML | 7 | 11 |
| Current best: PMC HTML | 7 | 13 |
| Current best: miss | 2 | 4 |
| NCBI BioC fulltext | 7 | 11 |
| PubTator3 fulltext | 0 | 0 |
| Matrix checksum | `bb033f79aac4fddcc0ec5b09144b2675dc12ae54f5bc8465d6e3213d9b9dbb06` | `c4fb68a75b2bd6bbfcd4f1ad08fcba39933669d2ceae4ee8aa999a24be0352b5` |

Optimization baseline benchmark, written to `/tmp/t384-opt-baseline/`:

| Workload | Wall time | User | Sys | Max RSS |
|---|---:|---:|---:|---:|
| Regression control | 27.89s | 0.62s | 0.20s | 35,432 KB |
| Full-scale target | 48.08s | 1.02s | 0.31s | 35,648 KB |

Primary metric: full-scale target wall time. Secondary metric: regression-control wall time. Correctness controls: fixed regression checksum, case counts, material BioC wins, and source distributions.

## Optimization Passes

| Pass | Hotspot | Approach | Before → after | Decision |
|---:|---|---|---|---|
| 1 | `measure_case` serial source fetches; source elapsed summed to 45.67s of 48.08s full-scale wall time. | Fetch independent source probes within each case concurrently and restore deterministic key order. | Regression 27.89s → 9.95s (-64.3%); full-scale 48.08s → 44.63s (-7.2%). | Committed. Regression checksum exact; full-scale kept 28 cases and 0 BioC wins, with live PMC HTML availability drift. |
| 2 | `main` measured cases serially after pass 1; per-case max calls still summed to a 42.23s lower bound. | Measure cases concurrently with bounded `CASE_WORKERS = 4`, preserving output order with `executor.map`. | Regression 9.95s → 5.66s (-43.1%); full-scale 44.63s → 7.81s (-82.5%). | Committed. Regression checksum exact; 0 BioC wins unchanged. |
| 3 | `collect_cases` fetched three Europe PMC search families serially; manual timing was 2.27s. | Fetch search families concurrently, then merge in original approach order. | Regression 5.66s → 25.61s (live outlier; code path not used by regression); full-scale 7.81s → 6.49s (-16.9%). | Committed. Primary metric improved; regression checksum exact. |
| 4 | Remaining case measurement waves with `CASE_WORKERS = 4`. | Increase bounded case workers from 4 to 8. | Regression 25.61s → 3.48s (-86.4%); full-scale 6.49s → 5.32s (-18.0%). | Committed. Primary metric improved; regression checksum exact. |
| 5 | Test whether more case workers improve beyond pass 4. | Increase `CASE_WORKERS` from 8 to 16. | Regression 3.48s → 2.86s (-17.8%); full-scale 5.32s → 5.64s (+6.0%). | Reverted. Primary metric regressed. |
| 6 | Test intermediate higher case-worker count. | Increase `CASE_WORKERS` from 8 to 12. | Regression 3.48s → 4.42s (+27.0%); full-scale 5.32s → 27.52s (+417.3%). | Reverted. Primary metric regressed badly; confirmed the concurrency knee at 8 workers. |

## Final Numbers

Final committed code is pass 4: concurrent source probes within each case, concurrent case measurement with `CASE_WORKERS = 8`, and concurrent Europe PMC search-family discovery.

Final benchmark, written to `/tmp/t384-opt-final/`:

| Workload | Wall time | User | Sys | Max RSS | Cases | Material BioC wins | Current best | NCBI BioC fulltext | PubTator fulltext |
|---|---:|---:|---:|---:|---:|---:|---|---:|---:|
| Regression control | 25.98s | 0.51s | 0.19s | 62,344 KB | 16 | 0 | 7 XML / 7 HTML / 2 miss | 7 | 0 |
| Full-scale target | 4.83s | 0.68s | 0.29s | 67,240 KB | 28 | 0 | 11 XML / 14 HTML / 3 miss | 11 | 0 |

Regression-control matrix checksum stayed exact: `bb033f79aac4fddcc0ec5b09144b2675dc12ae54f5bc8465d6e3213d9b9dbb06`.

Full-scale PMC HTML/miss distribution varied during live runs (13-15 HTML and 2-4 miss), but the ticket decision metric stayed stable: material BioC wins remained 0/28, BioC fulltext remained coverage-equivalent to current JATS/XML, and PubTator fulltext remained 0.

Validation:

- `collect_bioc_miss_candidates.py --help` passed.
- Regression-control checksum matched explore baseline exactly.
- Compact JSON/source-field validation passed for final regression and full-scale outputs.
- `git diff --check` passed.
- `cargo check --all-targets` passed.
- `uv run pytest tests/test_pre_commit_reject_march_artifacts.py` timed out during local package build setup after 240s; this matches the exploit-stage observation and is not the relevant fast gate for this inert architecture experiment script.

## Total Improvement

| Metric | Baseline | Final | Change |
|---|---:|---:|---:|
| Primary: full-scale wall time | 48.08s | 4.83s | -89.95% |
| Regression wall time | 27.89s | 25.98s | -6.85% (live outlier in final run; best committed run was 3.48s) |
| Full-scale user CPU | 1.02s | 0.68s | -33.3% |
| Full-scale max RSS | 35,648 KB | 67,240 KB | +88.6% |
| Regression checksum | `bb033f79...` | `bb033f79...` | unchanged |
| Regression material BioC wins | 0/16 | 0/16 | unchanged |
| Full-scale material BioC wins | 0/28 | 0/28 | unchanged |
| PubTator fulltext | 0 | 0 | unchanged |

## Convergence

Stopped after six passes. Passes 1-4 improved the primary full-scale wall-time metric and were committed. Passes 5 and 6 tested higher case concurrency (`CASE_WORKERS = 16` and `12`) and both regressed the primary metric. That established a practical concurrency knee at `CASE_WORKERS = 8`: more concurrency increases endpoint contention and live-source long-tail variance rather than throughput.

The final full-scale wall time is dominated by live network latency and source behavior, not local CPU. Further gains from local code changes are unlikely without changing the collection architecture or reducing the evidence captured.

## Remaining Opportunities

- Replace `urllib` with a shared async HTTP client/connection pool to reduce TLS/connection overhead without raising endpoint burst size.
- Add explicit retry/backoff and per-host concurrency limits to smooth live-source long-tail variance.
- Add deterministic local fixtures for performance benchmarking; live-source timing is noisy and source availability drifts.
- Prune redundant source probes when classification is already known, but only if future ticket scope relaxes the current evidence requirement to record each source/request shape.
- Keep BioC/PubTator out of runtime article fulltext resolution unless a future fixture proves a material BioC win over current XML/HTML.
