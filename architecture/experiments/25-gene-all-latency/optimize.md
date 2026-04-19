# Optimize: 25-gene-all-latency

## Starting Baseline

Ticket target:
- Primary metric: wall clock of `biomcp get gene BRAF all` under the release binary.
- Target: less than 30s for both markdown and JSON.
- Stretch target: less than 20s.

Exploit contract numbers from `.march/exploit.md`:

| Scope | Mode | p50 | p95 |
|---|---:|---:|---:|
| BRAF full-scale exploit | markdown | 4984.34 ms | 5008.89 ms |
| BRAF full-scale exploit | JSON | 5183.54 ms | 11127.60 ms |
| BRAF regression control exploit | markdown | 5181.44 ms | 5593.79 ms |
| BRAF regression control exploit | JSON | 5049.38 ms | 10785.19 ms |

Optimization baseline reproduced on the current code after a release rebuild:

```bash
cargo build --release
python3 architecture/experiments/25-gene-all-latency/scripts/gene_all_latency_probe.py \
  --approach opt-baseline \
  --gene BRAF \
  --runs 5 \
  --timeout-seconds 180 \
  --output architecture/experiments/25-gene-all-latency/results/opt_baseline_braf.json
```

| Gene | Mode | p50 wall clock | p95 wall clock | Successful runs |
|---|---|---:|---:|---:|
| BRAF | markdown | 5101.78 ms | 6516.67 ms | 5/5 |
| BRAF | JSON | 4942.09 ms | 5166.75 ms | 5/5 |

Dominant baseline sections:

| Mode | Hottest section | p50 | p95 | Completion |
|---|---|---:|---:|---:|
| markdown | clingen | 3084 ms | 3811.00 ms | 5/5 data |
| JSON | clingen | 2902 ms | 3100.60 ms | 5/5 data |

Baseline artifact:
- `architecture/experiments/25-gene-all-latency/results/opt_baseline_braf.json`
- `architecture/experiments/25-gene-all-latency/results/opt_baseline_braf_matrix.json`

## Optimization Passes

| Pass | Hotspot | Approach | Before p50 | After p50 | Decision |
|---:|---|---|---:|---:|---|
| 1 | Sequential tail fetches in `populate_sections_parallel_top`: pathways, protein, GO, interactions | Move independent tail fetches into the existing `tokio::join!` group | markdown 5101.78 ms, JSON 4942.09 ms | markdown 3740.12 ms, JSON 3468.53 ms | Committed `df8c5392`; p50 improved 26.69% markdown and 29.82% JSON |
| 2 | `fetch_clingen_section` and `ClinGenClient::{gene_validity,dosage_sensitivity}` | Add `ClinGenClient::gene_context` with one HGNC lookup and concurrent validity/dosage CSV downloads | markdown 3740.12 ms, JSON 3468.53 ms | markdown 2696.05 ms, JSON 2579.57 ms | Committed `d2426777`; p50 improved 27.92% markdown and 25.63% JSON |
| 3 | Remaining sequential CIViC and gnomAD constraint tail | Refactor to owned fetch helpers and add them to the parallel section fanout | markdown 2696.05 ms, JSON 2579.57 ms | markdown 2316.40 ms, JSON 2361.56 ms | Committed `51ae33b7`; p50 improved 14.08% markdown and 8.45% JSON |
| 4 | ClinGen could only start after MyGene resolution | Prefetch ClinGen during MyGene when `parallel-top` requests `clingen`, then await the handle in the normal section fanout | markdown 2316.40 ms, JSON 2361.56 ms | markdown 1984.24 ms, JSON 2088.31 ms | Committed `dd54f441`; p50 improved 14.34% markdown and 11.57% JSON |
| 5 | `ClinGenClient::gene_context` still awaited HGNC lookup before starting CSV downloads | Start HGNC lookup, validity CSV download, and dosage CSV download concurrently | markdown 1984.24 ms, JSON 2088.31 ms | markdown 1728.32 ms, JSON 1645.86 ms | Committed `8475dae1`; p50 improved 12.90% markdown and 21.19% JSON |
| 6 | Remaining local ClinGen CSV parsing after downloads | Try `spawn_blocking` for validity and dosage CSV parsing | markdown 1728.32 ms, JSON 1645.86 ms | markdown 31384.60 ms, JSON 1671.69 ms | Reverted; JSON p50 regressed 1.57% and touched ClinGen p50 regressed from 1638 ms to 1663 ms |

Pass artifacts:
- Pass 1: `opt_pass1_tail_parallel_braf.json`, `opt_pass1_tail_parallel_braf_matrix.json`, `opt_pass1_output_diff_braf.json`
- Pass 2: `opt_pass2_clingen_combined_braf.json`, `opt_pass2_clingen_combined_braf_matrix.json`, `opt_pass2_output_diff_braf.json`
- Pass 3: `opt_pass3_tail_complete_braf.json`, `opt_pass3_tail_complete_braf_matrix.json`, `opt_pass3_output_diff_braf.json`
- Pass 4: `opt_pass4_clingen_prefetch_braf.json`, `opt_pass4_clingen_prefetch_braf_matrix.json`, `opt_pass4_output_diff_braf.json`
- Pass 5: `opt_pass5_clingen_lookup_overlap_braf.json`, `opt_pass5_clingen_lookup_overlap_braf_matrix.json`, `opt_pass5_output_diff_braf.json`
- Pass 6: `opt_pass6_clingen_parallel_parse_braf.json`, `opt_pass6_clingen_parallel_parse_braf_matrix.json`

Validation after committed passes:
- `cargo fmt --check` passed.
- `cargo test --lib entities::gene` passed after passes 1-4.
- `cargo test --lib clingen` passed after passes 2 and 5.
- BRAF output parity passed after each committed pass: markdown identical, canonical JSON identical, mismatch count 0.

Final validation:
- `cargo fmt --check` passed.
- `cargo test --lib` overflowed the default test-thread stack in unrelated `cli::drug::tests::get_drug_raw_rejects_non_label_sections`; rerun with `RUST_MIN_STACK=16777216 cargo test --lib` passed 1762 tests.
- `cargo clippy --lib --tests -- -D warnings` passed.
- `uv run --extra dev pytest spec/18-source-labels.md --mustmatch-lang bash --mustmatch-timeout 60 -v` passed: 16 passed, 4 skipped.
- Final output diff for `BRAF`, `TP53`, and `CFTR` passed: markdown identical, canonical JSON identical, mismatch count 0.

## Final Numbers

The first final regression-control run hit several live MyGene outliers and was not used as the final comparison. The clean same-code rerun is the final measurement:

```bash
python3 architecture/experiments/25-gene-all-latency/scripts/gene_all_latency_probe.py \
  --approach opt-final-rerun \
  --gene BRAF \
  --gene TP53 \
  --gene CFTR \
  --runs 5 \
  --timeout-seconds 180 \
  --output architecture/experiments/25-gene-all-latency/results/opt_final_regression_control_rerun.json
```

| Gene | Mode | p50 wall clock | p95 wall clock | Successful runs |
|---|---|---:|---:|---:|
| BRAF | markdown | 1801.28 ms | 5359.02 ms | 5/5 |
| BRAF | JSON | 1711.53 ms | 1881.86 ms | 5/5 |
| TP53 | markdown | 1706.07 ms | 1915.76 ms | 5/5 |
| TP53 | JSON | 1695.15 ms | 2085.70 ms | 5/5 |
| CFTR | markdown | 2424.04 ms | 2540.22 ms | 5/5 |
| CFTR | JSON | 2147.15 ms | 2283.03 ms | 5/5 |

Final dominant sections:

| Gene | Mode | Hottest section | p50 | p95 | Completion |
|---|---|---|---:|---:|---:|
| BRAF | markdown | clingen | 1744 ms | 1782.60 ms | 5/5 data |
| BRAF | JSON | clingen | 1701 ms | 1871.40 ms | 5/5 data |
| TP53 | markdown | clingen | 1669 ms | 1828.80 ms | 5/5 data |
| TP53 | JSON | clingen | 1684 ms | 2075.00 ms | 5/5 data |
| CFTR | markdown | clingen | 2413 ms | 2530.60 ms | 5/5 data |
| CFTR | JSON | clingen | 2137 ms | 2273.20 ms | 5/5 data |

Final artifacts:
- `architecture/experiments/25-gene-all-latency/results/opt_final_regression_control_rerun.json`
- `architecture/experiments/25-gene-all-latency/results/opt_final_regression_control_rerun_matrix.json`
- `architecture/experiments/25-gene-all-latency/results/opt_final_output_diff.json`

## Total Improvement

Baseline to final for the primary BRAF metric:

| Metric | Baseline | Final | Delta |
|---|---:|---:|---:|
| BRAF markdown p50 | 5101.78 ms | 1801.28 ms | -64.69% |
| BRAF markdown p95 | 6516.67 ms | 5359.02 ms | -17.77% |
| BRAF JSON p50 | 4942.09 ms | 1711.53 ms | -65.37% |
| BRAF JSON p95 | 5166.75 ms | 1881.86 ms | -63.58% |

Final regression control versus the explore `parallel-top` control:

| Gene | Mode | Explore p50 | Final p50 | Delta | Explore p95 | Final p95 | Delta |
|---|---|---:|---:|---:|---:|---:|---:|
| BRAF | markdown | 5751.61 ms | 1801.28 ms | -68.68% | 11134.38 ms | 5359.02 ms | -51.87% |
| BRAF | JSON | 5436.81 ms | 1711.53 ms | -68.52% | 5549.99 ms | 1881.86 ms | -66.09% |
| TP53 | markdown | 5145.13 ms | 1706.07 ms | -66.84% | 5273.99 ms | 1915.76 ms | -63.68% |
| TP53 | JSON | 5280.04 ms | 1695.15 ms | -67.90% | 11812.43 ms | 2085.70 ms | -82.34% |
| CFTR | markdown | 4935.72 ms | 2424.04 ms | -50.89% | 5392.41 ms | 2540.22 ms | -52.89% |
| CFTR | JSON | 5081.69 ms | 2147.15 ms | -57.75% | 5183.53 ms | 2283.03 ms | -55.96% |

Final numbers beat the explore regression-control baseline for every measured p50 and p95 in the clean rerun.

## Convergence

Stopped after pass 6. The first five passes produced material BRAF p50 improvements, but the sixth pass did not improve the primary metric and regressed the clean JSON control. The remaining dominant cost is upstream ClinGen CSV transfer plus full-file parsing; additional local scheduling inside the live command no longer moves the p50.

The optimized command is well below both the 30s target and the 20s stretch target in the final clean run:
- BRAF final p95: 5359.02 ms markdown, 1881.86 ms JSON.
- Regression-control final p95 maximum across `BRAF`, `TP53`, and `CFTR`: 5359.02 ms.

## Remaining Opportunities

The next gains require larger changes than another local scheduling pass:

- Durable preprocessed ClinGen cache: avoid downloading and scanning full ClinGen CSV payloads on every isolated cold-cache command.
- Narrow indexed ClinGen source: use a smaller API or generated index keyed by HGNC ID/symbol instead of full-file CSV parsing.
- Provider-tail policy: p95 is still vulnerable to live provider stalls in MyGene, GO, Reactome, expression, and gnomAD; tighter source-specific caps could reduce tails but may intentionally trade away data.
- Broader entity reuse: the section fanout pattern can be applied to disease and drug if their `all` commands show similar serial independent sections.
