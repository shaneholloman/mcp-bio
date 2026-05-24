# Code Review Log — ticket 378 split routine validation from release live smoke

## Critique Summary

Reviewed `.march/design-draft.md`, `.march/design-final.md`, `.march/code-log.md`, and the full diff against `main` including the uncommitted implementation edits.

### Design Completeness Audit

Every design-final acceptance criterion and proof-matrix item has a corresponding implementation change:

- `.march/validation-profiles.toml` preserves March profile names and maps `spec-only` to `make spec-contracts` while keeping full profiles on `make release-gate`.
- `Makefile` adds `spec-contracts`, routes `release-gate` through `check spec-contracts`, and adds explicit `release-live-smoke` commands through `tools/biomcp-ci`.
- `spec-contracts` dry-run shows only `spec/surface/cli.md` and `spec/surface/test_parallel_isolation_contract.py`, not the broad live/cache-backed corpus.
- `release-live-smoke` dry-run covers discover, disease, article, and variant normalization without `$(SPEC_XDIST_ARGS)`.
- Docs in `spec/README-timings.md`, `architecture/technical/overview.md`, `RUN.md`, and `CONTRIBUTING.md` describe deterministic routine validation and opt-in live confidence.

### Traceability Audits

Forward traceability passed: each proof-matrix assertion landed in either `spec/surface/cli.md` or `spec/surface/test_parallel_isolation_contract.py`.

Reverse traceability found one repairable issue: the docs timing assertion for `make release-gate` had been relaxed from a numeric observed timing to any backtick string, allowing the shipped docs to say `pending` in an observed timing table. That was a silent relaxation of an existing docs-contract assertion, not an approved proof-matrix behavior change.

### Edit Discipline Audit

Actual diff size remains within the expected minimal surface for the named Make/profile/docs/tests split. No runtime source files or wrapper code were edited. No over-edit defects found.

## Repairs Applied

- Ran `/usr/bin/time -p make release-gate`; it passed with `real 763.21`.
- Replaced pending release-gate timing comments/row with the observed `763.21s` in `.march/validation-profiles.toml` and `CONTRIBUTING.md`.
- Restored the docs-contract assertion so `make release-gate` must have a numeric timing row; `make release-live-smoke` remains `operator-run` because it is intentionally opt-in/live.

## Validation Rerun

- `git diff --check` — passed
- `uv run --no-sync pytest spec/surface/test_parallel_isolation_contract.py -v` — 22 passed
- `uv run --no-sync pytest tests/test_validation_profile_contract.py tests/test_upstream_planning_analysis_docs.py -v` — 23 passed before repair
- `make spec-contracts` — 48 passed
- `cargo test --lib && cargo clippy --lib --tests -- -D warnings` — passed before and after repair
- `make check` — passed before repair
- `/usr/bin/time -p make release-gate` — passed; `real 763.21`
- Post-repair targeted check: `uv run --no-sync pytest tests/test_validation_profile_contract.py tests/test_upstream_planning_analysis_docs.py::test_repo_local_parallel_test_contract_is_documented -v` — 3 passed

## Residual Concerns

None requiring a follow-up issue. The opt-in `make release-live-smoke` target was dry-run/static-tested but not executed, as designed.

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| 1 | silently-relaxed | yes | `tests/test_upstream_planning_analysis_docs.py` relaxed the existing `make release-gate` timing assertion from numeric observed timing to any backtick string, permitting `pending` in the observed timing table. Fixed by recording the observed `763.21s` release-gate run and restoring the numeric assertion. |
