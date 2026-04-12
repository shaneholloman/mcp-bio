# Code Review Log — Ticket 179

## Critique

### Design completeness

- `src/entities/disease.rs` was replaced by `src/entities/disease/`, and the required files from the final design are present: `mod.rs`, `resolution.rs`, `fallback.rs`, `associations.rs`, `enrichment.rs`, `search.rs`, `get.rs`, `test_support.rs`, and `tests.rs`.
- The stable surface is preserved:
  - runtime re-exports remain in [src/entities/disease/mod.rs](/home/ian/workspace/worktrees/179-decompose-disease-rs-into-disease-submodule/src/entities/disease/mod.rs:297)
  - crate-visible helper paths still exist for `resolve_disease_hit_by_name` and `fallback_search_page`
  - `crate::entities::disease::tests::proof_*` is preserved by [src/entities/disease/tests.rs](/home/ian/workspace/worktrees/179-decompose-disease-rs-into-disease-submodule/src/entities/disease/tests.rs:1)
- Runtime files remain under the 700-line cap and every runtime file has a module-level `//!` doc comment.
- No contract, help, or schema text changes were required because the ticket is structural-only. Existing disease and phenotype specs remain the outside-in contract.
- `src/entities/mod.rs` remains unchanged as `pub(crate) mod disease;`.

### Test-design traceability

- Every proof-matrix entry has a matching verification target:
  - disease unit surface: `cargo test entities::disease:: --lib`
  - disease-focused regression surface and proof hooks: `cargo test disease --lib`
  - disease CLI/search/detail outside-in behavior: `spec/07-disease.md`
  - phenotype outside-in behavior: `spec/23-phenotype.md`
  - structural cap/doc checks: file-count/doc-comment checks
  - full repo baseline: `make check`
- All proof-hook functions named in the design still exist and resolve through `crate::entities::disease::tests::*`.
- `spec/07-disease.md` and `spec/23-phenotype.md` still cover outside-in behavior for the unchanged public contract.

### Finding

1. Disease runtime tests that hit mock services were not isolated from the shared HTTP cache. The failure reproduced in the full disease unit suite, where [src/entities/disease/enrichment/tests.rs](/home/ian/workspace/worktrees/179-decompose-disease-rs-into-disease-submodule/src/entities/disease/enrichment/tests.rs:67) expected the SEER catalog failure path but received a cached success-path response instead. The same risk applied to the other mock-backed runtime tests in [resolution/tests.rs](/home/ian/workspace/worktrees/179-decompose-disease-rs-into-disease-submodule/src/entities/disease/resolution/tests.rs:81), [fallback/tests.rs](/home/ian/workspace/worktrees/179-decompose-disease-rs-into-disease-submodule/src/entities/disease/fallback/tests.rs:210), and [get/tests.rs](/home/ian/workspace/worktrees/179-decompose-disease-rs-into-disease-submodule/src/entities/disease/get/tests.rs:38).

## Fixes

- Added [with_no_http_cache](/home/ian/workspace/worktrees/179-decompose-disease-rs-into-disease-submodule/src/entities/disease/test_support.rs:28) to force mock-backed disease tests through the existing no-cache runtime path.
- Wrapped every disease test or proof helper that exercises real source clients against mock servers:
  - [src/entities/disease/resolution/tests.rs](/home/ian/workspace/worktrees/179-decompose-disease-rs-into-disease-submodule/src/entities/disease/resolution/tests.rs:81)
  - [src/entities/disease/fallback/tests.rs](/home/ian/workspace/worktrees/179-decompose-disease-rs-into-disease-submodule/src/entities/disease/fallback/tests.rs:210)
  - [src/entities/disease/enrichment/tests.rs](/home/ian/workspace/worktrees/179-decompose-disease-rs-into-disease-submodule/src/entities/disease/enrichment/tests.rs:45)
  - [src/entities/disease/get/tests.rs](/home/ian/workspace/worktrees/179-decompose-disease-rs-into-disease-submodule/src/entities/disease/get/tests.rs:38)
- Post-fix collateral scan:
  - `git diff --check` passed
  - no new unused imports or dead code remained after the helper extraction
  - no cleanup/ownership conflicts or stale error messages were introduced

## Verification

- `cargo test entities::disease:: --lib` passed after the fix.
- `cargo test disease --lib` passed after the fix.
- `make check` passed.
- `make spec` did not pass end-to-end because of live-service/spec drift outside this refactor:
  - `spec/02-gene.md::Gene Funding`
  - `spec/07-disease.md::Disease Funding`
  - `spec/07-disease.md::Disease Funding Beyond Cancer`
  - `spec/19-discover.md::HPO Symptom Bridge`
  - `spec/19-discover.md::Ambiguous Query`
- Ticket-relevant outside-in status:
  - `spec/07-disease.md`: all non-funding disease scenarios passed, including discover fallback, search offset/miss/no-fallback, detail retrieval, survival, crosswalk resolution, genes, variants, and ranking.
  - `spec/23-phenotype.md`: all phenotype scenarios passed.

## Residual Concerns

- NIH Reporter is currently returning a service-unavailable HTML page behind `404 Not Found`, so the funding specs are not a reliable signal for this structural ticket right now.
- Discover output for symptom/ambiguous queries has drifted from the current `spec/19-discover.md` expectations; this is separate from the disease module refactor and needs follow-up.

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| 1 | collateral-damage | no | Disease mock-server tests reused the shared HTTP cache, so the full disease suite could fail even when the same tests passed in isolation. |
