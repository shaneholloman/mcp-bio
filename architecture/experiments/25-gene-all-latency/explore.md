# Explore: 25-gene-all-latency

## Spike Question

Which per-section HTTP legs dominate `biomcp get gene <symbol> all` wall clock, and can the command stay under a 30s budget on the beelink baseline by reducing timeouts, short-circuiting empty sections, parallelizing sequential legs, or removing redundant work?

## Prior Art Summary

- The live gene implementation is in `src/entities/gene.rs`, not the stale `src/entities/gene/get/` path named in the ticket.
- `all` currently includes pathways, ontology, diseases, protein, go, interactions, civic, expression, hpa, druggability, clingen, and constraint. It still excludes `funding` and `disgenet`, matching the current spec contract in `spec/02-gene.md`.
- The dominant design pattern before this spike was sequential section population after a MyGene base lookup, with one important exception: the gene druggability section already used `tokio::join!` internally to overlap DGIdb and OpenTargets.
- The shared HTTP client in `src/sources/mod.rs` still carries a 30s request timeout plus retry/cache middleware. Most optional gene sections sit behind an 8s outer timeout, but not all section fetches are wrapped individually.
- OpenTargets clinical context is always fetched for the base gene card. Before the spike, that path and the OpenTargets druggability leg each did their own `resolve_target_id(symbol)` search even though MyGene had already provided `ensembl_id`.

## Approaches Tried

### 1. Baseline Instrumentation

What:
- Added per-section timing capture in `src/entities/gene.rs`, emitted through `BIOMCP_GENE_TIMING_PATH`.
- Added a reproducible probe runner in `architecture/experiments/25-gene-all-latency/scripts/gene_all_latency_probe.py`.

How:
- Used the release binary at `target/release/biomcp`.
- Isolated cache state per run with `BIOMCP_CACHE_DIR`.
- Measured markdown and JSON separately.
- Small-scale comparison used `BRAF`, 5 runs, isolated cache each run.

Measurements:

| Approach | Mode | BRAF p50 | BRAF p95 |
|---|---:|---:|---:|
| baseline | markdown | 7993.97 ms | 8100.42 ms |
| baseline | json | 7915.95 ms | 8227.49 ms |

Top baseline section contributors for `BRAF`:
- `clingen`: ~2.93s to 2.96s p50
- `hpa`: ~0.75s p50
- `clinical_context`: ~0.64s p50
- `druggability`: ~0.64s p50

What it revealed:
- The original ticket premise no longer reproduces on April 18, 2026 in this workspace. The current release baseline is already comfortably under the 30s target.
- Rendering is not the story. Markdown and JSON totals are both in the same 8s band, and the section timings dominate total wall clock.

Comparison to prior art:
- Confirms the prior-art reading: the path is still mostly serial, and the critical path is dominated by section fetches, not formatting.

### 2. OpenTargets Ensembl Reuse

What:
- Added `target_*_for_target_id` helpers in `src/sources/opentargets.rs`.
- Added an `opentargets-ensembl` strategy that reuses MyGene’s `ensembl_id` instead of resolving the OpenTargets target ID from the symbol twice.

How:
- Kept the rest of the orchestration sequential.
- Compared directly against the same BRAF 5-run baseline.

Measurements:

| Approach | Mode | BRAF p50 | BRAF p95 | Delta vs baseline p50 |
|---|---:|---:|---:|---:|
| opentargets-ensembl | markdown | 7214.98 ms | 12625.49 ms | -9.7% |
| opentargets-ensembl | json | 7894.04 ms | 8189.39 ms | -0.3% |

Key section delta:
- `clinical_context` dropped from ~638-639 ms p50 to ~185-188 ms p50.
- `druggability` dropped from ~638 ms p50 to ~317-336 ms p50.

Comparison to prior art:
- Reuses the existing source composition without widening concurrency.
- Preserves the serial design but removes a redundant upstream search step the prior-art code was paying twice.

Assessment:
- Real improvement in markdown mode, nearly neutral in JSON mode.
- Helpful but not sufficient to be the best winner on its own.

### 3. Parallel-Top

What:
- Added a `parallel-top` strategy that overlaps the dominant post-MyGene legs:
  - clinical context
  - enrichr
  - expression
  - hpa
  - druggability
  - clingen
- Left the smaller tail sections sequential.

How:
- Built directly on the existing prior-art `tokio::join!` pattern already used inside gene druggability.
- Reused the OpenTargets Ensembl-ID shortcut inside the concurrent branch.

Measurements:

| Approach | Mode | BRAF p50 | BRAF p95 | Delta vs baseline p50 |
|---|---:|---:|---:|---:|
| parallel-top | markdown | 5751.61 ms | 11134.38 ms | -28.1% |
| parallel-top | json | 5436.81 ms | 5549.99 ms | -31.3% |

Broadened winner check, 5 runs each:

| Gene | Mode | Baseline p50 | Parallel-top p50 | Delta |
|---|---:|---:|---:|---:|
| TP53 | markdown | 8618.17 ms | 5145.13 ms | -40.3% |
| TP53 | json | 8059.19 ms | 5280.04 ms | -34.5% |
| CFTR | markdown | 7688.96 ms | 4935.72 ms | -35.8% |
| CFTR | json | 7805.20 ms | 5081.69 ms | -34.9% |

Section completion notes:
- All measured BRAF and TP53 sections returned data across all runs.
- `CFTR` consistently returned an empty `civic` section in both baseline and `parallel-top`, so the winner did not create or hide data there.

Comparison to prior art:
- This is the strongest reuse of prior art. It takes the existing local `tokio::join!` idea and applies it one level higher to the actual critical-path sections.
- Unlike the OpenTargets-only branch, it attacks the measured serial wall-clock shape directly.

## Decision

`parallel-top` wins.

Why:
- It is the only approach that materially improves p50 in both markdown and JSON on BRAF.
- It stays well under the 30s target on all measured genes and modes.
- It broadens successfully: TP53 and CFTR both land around 4.9s to 5.3s p50 instead of the baseline 7.7s to 8.6s band.
- The technical reason matches the measurements: after removing redundant OpenTargets ID resolution, `clingen` remains the dominant single leg, so the best remaining move is to overlap the other heavy sections with it rather than optimize them one by one.

Important caveat:
- The original ticket’s 44-45s problem does not reproduce in this workspace on April 18, 2026. The current baseline release binary is already below 30s before any experimental strategy is enabled.
- The winner is still real and measured, but the urgency of the exploit has changed: this is now headroom improvement, not rescue from a failing budget.

## Outcome

promote

Rationale:
- The measured winner is clear and it meaningfully reduces wall clock while preserving observed section completion.
- Even though the original budget failure is no longer reproducible, the winner provides another 30-40% of headroom and makes chained markdown+JSON calls cheaper.

## Risks for Exploit

- The ticket premise is stale relative to the current codebase and current date. Before landing an exploit patch, re-confirm the urgency on the intended beelink host instead of assuming the old 44-45s figure still holds.
- The `parallel-top` branch is a larger code change than the OpenTargets-only variant. It needs focused review for correctness, especially around preserving field-by-field parity outside latency.
- The broadened measurements still show upstream p95 noise on a few paths:
  - `BRAF` markdown `parallel-top` p95: 11134.38 ms
  - `TP53` json `parallel-top` p95: 11812.43 ms
  Those are still well under budget, but they mean upstream variance has not disappeared.
- This spike did not yet run the exploit-phase field-by-field pre/post output diff the ticket asks for. That still needs to happen before shipping the runtime path.
- The spec-side follow-up from ticket 209 should only be consolidated after the exploit patch lands and the combined markdown+JSON blocks are verified green.
