# Optimize — Reset BioMCP test strategy around request contracts

## Starting Baseline

Source: `.march/exploit.md` contract numbers and exploit control runtime for:

```bash
./architecture/experiments/reset-biomcp-test-strategy-around-request-contracts/scripts/analyze_test_strategy.py
```

Confirmed baseline on this optimize step:

| Metric | Exploit baseline | Confirmed baseline |
|---|---:|---:|
| Inventory elapsed | 0.06s | 0.06s |
| Peak RSS | 25,872 KB | 25,920 KB |
| Result checksum | `b090cba04589f3f9a6e980d2314280698d369f698d9a952f1082ae8316198527` | `b090cba04589f3f9a6e980d2314280698d369f698d9a952f1082ae8316198527` |

The optimization target was to improve the lightweight inventory benchmark while preserving every request-contract count and the regression checksum.

## Optimization Passes

| Pass | Hotspot | Approach | Before | After | Result |
|---:|---|---|---:|---:|---|
| 1 | `plan_seam_inventory()` / `march_preflight_evidence()` repeated regex compilation | Precompile reused structural/preflight regexes globally | cProfile 0.037s; internal median 0.019381s; RSS 25,920 KB | cProfile 0.035s; internal median 0.017607s; RSS 24,376 KB | Committed `564e5925`; checksum unchanged |
| 2 | `extract_sections()` section classifier, 587 regex searches | Try combined named-group regex scans | cProfile 0.035s; internal median 0.017607s; checksum baseline | cProfile 0.044s; internal median 0.022499s; checksum changed | Reverted: slower and incorrect output |
| 3 | `rel()` repeated `Path.relative_to()` path formatting | Try string prefix trimming | cProfile 0.035s; internal median 0.017607s; RSS 24,376 KB | cProfile 0.037s; internal median 0.018403s; RSS 23,784 KB | Reverted: primary runtime regressed |

## Final Numbers

Final validation command results after the only committed optimization:

| Metric | Final result |
|---|---:|
| Inventory elapsed | 0.06s |
| Peak RSS | 24,000 KB |
| Result checksum | `b090cba04589f3f9a6e980d2314280698d369f698d9a952f1082ae8316198527` |
| Validation | `cargo check --all-targets` passed |
| Runtime/product diff | none under `src Cargo.toml Cargo.lock Makefile spec tests tools docs README.md` |

## Total Improvement

| Metric | Confirmed baseline | Final | Improvement |
|---|---:|---:|---:|
| Inventory elapsed | 0.06s | 0.06s | matched baseline; at command timer/startup floor |
| Peak RSS | 25,920 KB | 24,000 KB | 1,920 KB lower / 7.4% lower |
| Internal benchmark median, 100 iterations | 0.019381s | 0.017607s | 0.001774s faster / 9.2% faster |
| cProfile total runtime | 0.037s | 0.035s | 0.002s faster / 5.4% faster |
| Result checksum | `b090cba04589f3f9a6e980d2314280698d369f698d9a952f1082ae8316198527` | `b090cba04589f3f9a6e980d2314280698d369f698d9a952f1082ae8316198527` | unchanged / PASS |

## Convergence

Stopped after the required three passes. Pass 1 improved internal runtime by removing pure regex compilation overhead. Passes 2 and 3 failed to improve the primary elapsed/internal-runtime metric and were reverted. The executable benchmark remains at 0.06s wall time, which appears to be the `uv` script startup and timer-granularity floor for this tiny static inventory. Further micro-optimizations inside the script are unlikely to move the command-level metric without changing the benchmark shape, output semantics, or checksum.

## Remaining Opportunities

- Replace broad regex section classification with a hand-maintained structured fixture if downstream tickets need a faster inventory. That would be an artifact-shape change and must deliberately reset the checksum.
- Split the inventory into cached source/spec/preflight sub-results. This could reduce repeated file reads but would add cache invalidation complexity that is not justified for a 0.06s command.
- Port the inventory to Rust or a compiled helper if it ever becomes part of a hot CI path. That is outside this spike and unnecessary for current March usage.
- Keep the request-contract test strategy implementation work in follow-up build/quickfix tickets; this optimize step intentionally did not change BioMCP runtime behavior or rewrite tests.
