# Code Review Log — Ticket 188

## Critique

### Design completeness audit

- Read `.march/ticket.md`, `.march/design-draft.md`, `.march/design-final.md`, `.march/code-log.md`, and `git diff main..HEAD`.
- Mapped the design-required contract changes to the diff:
  - `spec/README-timings.md` supplies the required three-lane split, audit method, per-heading timing table, and smoke-only inventory.
  - `RUN.md` and `architecture/technical/overview.md` now point readers to `spec/README-timings.md`.
  - `tests/test_upstream_planning_analysis_docs.py` derives the smoke-only inventory from the actual `Makefile` deselect list instead of hardcoding guessed headings.
  - `Makefile` moves `spec/06-article.md::Keyword Search Can Force Lexical Ranking` into `SPEC_PR_DESELECT_ARGS`.
- Acceptance-criteria coverage is present in the changed surface. The remaining proof obligations stay in existing spec headings or gate commands:
  - `spec/18-source-labels.md`, `spec/02-gene.md`, `spec/07-disease.md`, and `spec/11-evidence-urls.md` still carry the outside-in coverage that the design chose to keep in `spec-pr`.
  - `spec/06-article.md::Keyword Search Can Force Lexical Ranking` still exists as the smoke-lane proof after the Makefile move.
- The execution order in `.march/code-log.md` updates docs/tests before the runtime Makefile edit, which matches the design’s sequencing requirement.

### Test-design traceability

- Proof-matrix item “Timing report exists and has the required schema” maps to `tests/test_upstream_planning_analysis_docs.py::test_spec_lane_timing_report_is_documented_and_aligned_with_makefile`.
- Proof-matrix item “Smoke-only inventory matches the actual Makefile deselect list” mapped to the same test, but the original assertion was too weak: it only checked that each Makefile node ID appeared somewhere in the report.
- Proof-matrix item “Runbook and technical overview point to the current audit” also maps to that docs-contract test and was already covered.
- The spec-level proof items still map to existing outside-in spec headings:
  - `spec/18-source-labels.md` for retained source-label coverage
  - `spec/06-article.md::Keyword Search Can Force Lexical Ranking` for the newly moved smoke-only heading
- Findings:
  1. [spec/README-timings.md:230] The report row for `spec/03-variant.md` used the node-id-like text `Searching by c::HGVS`, recorded `n/a` for both timing cells, and still labeled the row `passed`/`fast`. That violated the design’s per-heading timing requirement.
  2. [tests/test_upstream_planning_analysis_docs.py:846-851] The smoke-only inventory assertion only enforced one-way inclusion, so stale extra rows in `spec/README-timings.md` could drift from `SPEC_PR_DESELECT_ARGS` without failing CI.
  3. [tests/test_upstream_planning_analysis_docs.py:853-865] The timing-report test did not require numeric timing cells for passed rows, so the broken `c.HGVS` entry could ship undetected.

### Security / duplication / implementation quality

- Security: the changed surface is docs/tests/Makefile only; I found no new injection, secret exposure, path traversal, or auth-bypass risk.
- Duplication: searched the test file for an existing markdown-table helper before adding one and found none, so the new helper is not duplicating an existing utility.
- Implementation quality: the original change followed adjacent docs-contract patterns, but the report/test pair was not strict enough to keep the audit data trustworthy over time.

## Fix Plan

- Re-measure `spec/03-variant.md::Searching by c.HGVS` with cold and warm targeted runs and repair the bad audit row.
- Strengthen the docs-contract test to parse markdown tables, require exact equality between the report’s smoke-only inventory and the Makefile deselect set, and reject any passed timing row that lacks numeric timing data.

## Repair

### Fixes applied

- Ran supplemental targeted measurements for the broken report row and saved the evidence under:
  - `.march/review-c-hgvs-cold.log`
  - `.march/review-c-hgvs-warm.log`
- Updated `spec/README-timings.md` to:
  - explain why `Searching by c.HGVS` needed a supplemental rerun
  - correct the human-readable heading text
  - replace the unsupported `n/a` timing cells with the measured `0.49s` cold/warm timings
- Added `_markdown_table_rows()` in `tests/test_upstream_planning_analysis_docs.py` and strengthened `test_spec_lane_timing_report_is_documented_and_aligned_with_makefile` to:
  - assert the timing and smoke-only table headers structurally
  - compare the smoke-only report node IDs to the Makefile deselect list as an exact set match
  - reject duplicate smoke-only rows
  - require numeric timing cells for passed rows
  - require `gated` rows to remain `skipped`

### Post-fix collateral scan

- After the report fix:
  - verified the only remaining `n/a` rows are the four key-gated `skipped` entries
  - verified the stale `Searching by c::HGVS` text is gone
  - verified the “four gated headings” note still matches the table
- After the test fix:
  - confirmed the new helper is used and no unused imports or dead locals were introduced
  - caught a follow-on issue where I had briefly normalized the timing-audit section before table parsing; fixed that immediately so table parsing operates on the raw markdown section

## Validation

- `uv run pytest tests/test_upstream_planning_analysis_docs.py::test_makefile_spec_split_contract_is_documented_and_executable tests/test_upstream_planning_analysis_docs.py::test_repo_local_parallel_test_contract_is_documented tests/test_upstream_planning_analysis_docs.py::test_spec_lane_timing_report_is_documented_and_aligned_with_makefile -v`
- `cargo test --lib && cargo clippy --lib --tests -- -D warnings`

I did not rerun `make spec-pr` or `make check` from the review step. `.march/code-log.md` already records green full-blocking proof for the implementation, and the review repairs were limited to docs/tests.

## Residual Concerns

- No additional out-of-scope issues were filed from this review.
- No remaining blocking concerns on the touched surface.

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| 1 | stale-doc | yes | `spec/README-timings.md` recorded `spec/03-variant.md::Searching by c.HGVS` as passed/fast while leaving both timing cells as `n/a`, so the per-heading audit was incomplete. |
| 2 | weak-assertion | no | The smoke-only inventory test only checked that Makefile deselect IDs appeared in the report, not that the report matched the Makefile exactly. |
| 3 | weak-assertion | yes | The timing-report test allowed `passed` rows with non-numeric timing cells, so incomplete audit data could pass CI. |
