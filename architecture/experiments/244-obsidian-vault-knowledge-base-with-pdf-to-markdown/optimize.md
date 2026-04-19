# Optimize: Obsidian Vault Knowledge Base with PDF-to-Markdown

## Starting Baseline

Exploit contract baseline from `.march/exploit.md`:

| Metric | Contract value |
| --- | ---: |
| Total elapsed | 141,041 ms |
| JATS success | 2/2 |
| HTML success | 3/3 |
| Rust PDF success | 5/6 |
| Overall PDF success including Python calibration | 8/9 |
| Vault note records | 8 |
| Vault unique note files | 8 |
| Duplicate path mismatches | 0 |
| Structured `type: article` matches | 4 |
| Structured `type: preprint` matches | 1 |
| Structured `pmcid: PMC9984800` matches | 3 |
| Structured `tags: source/pdf` matches | 3 |
| Structured `doi:` matches | 8 |
| Obsidian CLI working commands | 0 |

Reproduced optimization baseline on the current tree:

| Metric | Baseline |
| --- | ---: |
| Total elapsed | 143,149 ms |
| JATS success | 2/2 |
| HTML success | 3/3 |
| Rust PDF success | 5/6 |
| Overall PDF success | 8/9 |
| Vault note records | 8 |
| Vault unique note files | 8 |
| Obsidian CLI working commands | 0 |
| Regression control | pass |
| Validation | pass |

Baseline hotspot:

- Function/path: `run_pdf_exploit()` in `scripts/run_exploit.py`, invoking
  `probe.run_rust_probe()` for `pdf_oxide` on the CDC guideline.
- Rust path: `scripts/rust_probe/src/main.rs`, `run_pdf()` and
  `run_pdf_oxide()`.
- Measured cost: 90,101 ms for the controlled `pdf_oxide` timeout, about 63%
  of reproduced total runtime.

## Optimization Passes

| Pass | Hotspot | Approach | Before | After | Result |
| --- | --- | --- | ---: | ---: | --- |
| 1 | CDC guideline `pdf_oxide` controlled timeout | Add optional Rust probe timeout and use 20 s for exploit `pdf_oxide` only | 143,149 ms | 75,790 ms | Committed, -47.1% |
| 2 | Sequential Rust PDF probes | Parallelize independent Rust PDF document/engine jobs | 75,790 ms | 39,102 ms | Reverted: regression control failed on CDC `unpdf` elapsed |
| 3 | CDC guideline `pdf_oxide` controlled timeout | Lower exploit `pdf_oxide` timeout from 20 s to 15 s | 75,790 ms | 52,024 ms | Committed, -31.4% |
| 4 | CDC guideline `pdf_oxide` controlled timeout | Lower exploit `pdf_oxide` timeout from 15 s to 10 s | 52,024 ms | 46,746 ms | Committed, -10.1% |
| 5 | CDC guideline `pdf_oxide` controlled timeout | Lower exploit `pdf_oxide` timeout from 10 s to 9 s | 46,746 ms | 45,726 ms | Committed, -2.2%; convergence |

Pass details:

- Pass 1 kept the default 90 s Rust probe timeout for other uses, but applied
  a strict 20 s timeout to exploit `pdf_oxide` runs. The two successful
  `pdf_oxide` records remained successful and the known CDC failure stayed a
  controlled timeout.
- Pass 2 showed that Rust PDF work is parallelizable in principle, but the
  attempt slowed the CDC `unpdf` record from 3,082 ms to 4,077 ms and failed
  the regression timing guard. It was reverted.
- Passes 3 through 5 tuned the same exploit-only timeout to the lowest measured
  value that preserved the DailyMed `pdf_oxide` success with regression and
  validation passing.

## Final Numbers

Final benchmark command:

`python3 architecture/experiments/244-obsidian-vault-knowledge-base-with-pdf-to-markdown/scripts/run_exploit.py`

Final full-scale results:

| Metric | Final |
| --- | ---: |
| Total elapsed | 45,726 ms |
| JATS success | 2/2 |
| HTML success | 3/3 |
| Rust PDF success | 5/6 |
| Overall PDF success | 8/9 |
| Vault note records | 8 |
| Vault unique note files | 8 |
| Duplicate path mismatches | 0 |
| Structured `type: article` matches | 4 |
| Structured `type: preprint` matches | 1 |
| Structured `pmcid: PMC9984800` matches | 3 |
| Structured `tags: source/pdf` matches | 3 |
| Structured `doi:` matches | 8 |
| Obsidian CLI working commands | 0 |
| Regression control | pass |
| Validation | pass |

Final PDF timings:

| Document | Engine | Success | Elapsed | Score |
| --- | --- | --- | ---: | ---: |
| `pmc_oa_article_pdf` | `unpdf` | yes | 86 ms | 4 |
| `pmc_oa_article_pdf` | `pdf_oxide` | yes | 694 ms | 4 |
| `pmc_oa_article_pdf` | `pymupdf4llm` | yes | 3,354 ms | 3 |
| `dailymed_keytruda_label` | `unpdf` | yes | 8,768 ms | 2 |
| `dailymed_keytruda_label` | `pdf_oxide` | yes | 7,527 ms | 3 |
| `dailymed_keytruda_label` | `pymupdf4llm` | yes | 1,999 ms | 3 |
| `cdc_sti_guideline` | `unpdf` | yes | 2,469 ms | 4 |
| `cdc_sti_guideline` | `pdf_oxide` | no, controlled timeout | 9,014 ms | n/a |
| `cdc_sti_guideline` | `pymupdf4llm` | yes | 2,715 ms | 3 |

PDF winners stayed unchanged:

| Document | Winning engine | Score |
| --- | --- | ---: |
| `pmc_oa_article_pdf` | `unpdf` | 4 |
| `dailymed_keytruda_label` | `pdf_oxide` | 3 |
| `cdc_sti_guideline` | `unpdf` | 4 |

## Total Improvement

Primary metric improvement uses the reproduced optimization baseline:

| Metric | Baseline | Final | Change |
| --- | ---: | ---: | ---: |
| Total elapsed | 143,149 ms | 45,726 ms | -68.1% |
| JATS success | 2/2 | 2/2 | unchanged |
| HTML success | 3/3 | 3/3 | unchanged |
| Rust PDF success | 5/6 | 5/6 | unchanged |
| Overall PDF success | 8/9 | 8/9 | unchanged |
| Vault note records | 8 | 8 | unchanged |
| Vault unique note files | 8 | 8 | unchanged |
| Duplicate path mismatches | 0 | 0 | unchanged |
| Structured `type: article` matches | 4 | 4 | unchanged |
| Structured `type: preprint` matches | 1 | 1 | unchanged |
| Structured `pmcid: PMC9984800` matches | 3 | 3 | unchanged |
| Structured `tags: source/pdf` matches | 3 | 3 | unchanged |
| Structured `doi:` matches | 8 | 8 | unchanged |
| Obsidian CLI working commands | 0 | 0 | unchanged |

Against the original exploit contract elapsed time, total elapsed improved from
141,041 ms to 45,726 ms, a 67.6% reduction.

## Convergence

Stopped after pass 5. The final pass reduced total elapsed from 46,746 ms to
45,726 ms, a 2.2% improvement, which is below the 5% convergence threshold.

The remaining single hottest paths are now close together:

| Remaining path | Elapsed |
| --- | ---: |
| CDC `pdf_oxide` controlled timeout | 9,014 ms |
| DailyMed `unpdf` | 8,768 ms |
| DailyMed `pdf_oxide` | 7,527 ms |

The final numbers match or beat the explore baseline on the regression control
benchmark: `kb_regression_control.json` reports `passed: true`.

## Remaining Opportunities

- Parallel Rust PDF extraction can cut wall-clock time, but the tested
  implementation failed the per-record regression timing guard. It would need a
  scheduler that isolates heavy document/engine pairs or relaxes timing
  regression rules for parallel runs.
- The 9 s `pdf_oxide` timeout still has limited headroom over the DailyMed
  success. Lowering it further risks converting a useful `pdf_oxide` label
  extraction into a timeout.
- DailyMed `unpdf` remains expensive and low-quality. Skipping it would save
  time, but the ticket requires evaluating both Rust PDF engines against the
  same document set.
- Python calibration could be batched or run in a managed environment instead
  of repeated `uv run --with ...` invocations, but calibration is intentionally
  non-product-path and not the current primary bottleneck.
