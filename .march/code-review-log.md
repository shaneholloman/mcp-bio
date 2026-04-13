# Code Review Log — Ticket 187

## Critique

### Design completeness audit

- Read `.march/design-draft.md`, `.march/design-final.md`, `.march/code-log.md`, and `git diff main..HEAD`.
- Mapped every repaired design item and acceptance criterion to the current repo surface:
  - `Makefile` now uses `cargo nextest run`, defines `SPEC_SERIAL_FILES`, defines `SPEC_XDIST_ARGS`, and splits `spec` / `spec-pr` into xdist bulk plus serial tail.
  - `pyproject.toml` and `uv.lock` now carry `pytest-xdist`.
  - `CONTRIBUTING.md`, `RUN.md`, and `architecture/technical/overview.md` now document the repo-local parallel test contract.
  - `tests/test_upstream_planning_analysis_docs.py` now pins the Makefile/docs contract for the split runner strategy.
- No design item was missing a corresponding code/doc change.
- The code log records the required order: baseline timings first, then docs/tests red-state work, then runtime edits. I did not find evidence contradicting that sequence.

### Test-design traceability

- Verified matching proof coverage for the Makefile split contract, docs contract, serial-tail study proof, validation-profile contract, and xdist-backed spec proof.
- Found one missing automated proof: the design required proof that `pytest-xdist` is present in the dev dependency contract, but no test pinned `pyproject.toml`/`uv.lock` or importability.
- Found one weak assertion: the docs contract only checked timing-table row prefixes, so `TBD` placeholders or blank cells could slip through while the test still passed.
- Found one stale docs contract: `RUN.md` still said the pre-commit hook skips `cargo test` in a repo-local pre-merge section, even though the local Rust test lane is now `cargo nextest run`.

### Security / duplication / quality checks

- Security: no new untrusted-input flow, shell interpolation, path injection, or secret exposure was introduced by the ticket surface I reviewed.
- Duplication: searched for existing equivalents of the new Makefile variables and doc contract coverage; the implementation reused the existing contract-test file rather than inventing a parallel abstraction.
- Quality: the implementation follows adjacent repo patterns and keeps the runner split explicit in the Makefile instead of hiding it in a helper script.

## Fix Plan

- Add an automated contract test for the `pytest-xdist` dev dependency and runtime importability.
- Strengthen the timing-table assertions so the docs contract requires measured values instead of placeholders.
- Update the runbook’s pre-commit wording to reference `cargo nextest run`, then pin that wording in the existing docs-contract test.

## Repair

### Fixes applied

- Added `test_parallel_test_dependency_contract_is_declared` in `tests/test_upstream_planning_analysis_docs.py` to assert:
  - `pytest-xdist` is listed in `pyproject.toml` dev dependencies
  - `uv.lock` contains the dev-extra dependency edge and package entry
  - `xdist` is importable in the repo test environment
- Strengthened `test_repo_local_parallel_test_contract_is_documented` so the timing table must contain concrete `NN.NNs` before/after values and no `TBD` placeholders.
- Updated `RUN.md` so the pre-commit section says it does not run `cargo nextest run`, not `cargo test`.
- Updated `test_validation_profile_and_hook_contract_docs_are_pinned` to pin the corrected runbook wording.

### Post-fix collateral scan

- After the test-file edits:
  - no dead imports were introduced
  - no shadowed variables were introduced
  - no stale error text or cleanup paths were affected
- After the runbook wording fix:
  - the pinned docs-contract test was updated in the same change
  - no additional stale `cargo test` reference remained in the repo-local pre-merge section

## Validation

- `uv run pytest tests/test_upstream_planning_analysis_docs.py::test_repo_local_parallel_test_contract_is_documented tests/test_upstream_planning_analysis_docs.py::test_parallel_test_dependency_contract_is_declared tests/test_upstream_planning_analysis_docs.py::test_validation_profile_and_hook_contract_docs_are_pinned tests/test_upstream_planning_analysis_docs.py::test_makefile_spec_split_contract_is_documented_and_executable tests/test_validation_profile_contract.py tests/test_directory_submission_contract.py::test_study_chart_dimensions_spec_runs_as_a_targeted_heading -v`
- `uv run pytest spec/01-overview.md --mustmatch-lang bash --mustmatch-timeout 120 -n auto --dist loadfile -k Version -v`
- `cargo test --lib && cargo clippy --lib --tests -- -D warnings`

## Residual Concerns

- No additional scope-external issues were filed from this review.
- Verify should still run the scheduled `full-blocking` gate (`make check && make spec-pr`) in step 05; I did not rerun the full runtime gate from this review step because the review fixes were limited to docs/tests.

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| 1 | missing-test | yes | Design proof item for `pytest-xdist` availability had no matching automated contract test for `pyproject.toml` / `uv.lock` or importability. |
| 2 | weak-assertion | no | Timing-table test only checked row prefixes, so placeholders or empty before/after cells could pass despite the design requiring measured values. |
| 3 | stale-doc | no | `RUN.md` pre-merge guidance still referenced `cargo test` instead of the shipped repo-local `cargo nextest run` lane. |
