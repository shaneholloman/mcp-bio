## Review Scope

- Read `.march/ticket.md`, `.march/design-draft.md`, `.march/design-final.md`, and `.march/code-log.md`.
- Rebased onto `main` with `GIT_EDITOR=true git rebase main` and confirmed the branch was already up to date.
- Reviewed `git diff main..HEAD`, `git log --oneline main..HEAD`, and the two ticket commits to verify contract-first execution order.
- Inspected the touched runtime, renderer, docs, spec, and test files directly.
- Re-ran the relevant proof surface:
  - `cargo test --lib && cargo clippy --lib --tests -- -D warnings`
  - `uv run --extra dev pytest tests/test_public_search_all_docs_contract.py`
  - `uv run --extra dev pytest spec/09-search-all.md --mustmatch-lang bash --collect-only -q`
  - `uv run --extra dev pytest spec/09-search-all.md --mustmatch-lang bash -q`
  - `make spec-pr`

## Critique

### Design Completeness Audit

- `design-final.md` contains no explicit `Needs change` markers, so the audit covered the file-level plan, acceptance criteria, proof matrix, and execution-order requirement.
- The runtime design items all had matching code changes:
  - dedicated counts-only JSON projection in `src/cli/search_all.rs` plus branch selection in `src/cli/outcome.rs`
  - conservative counts-only fetch-budget helper in `src/cli/search_all.rs`
  - counts-only markdown row-skipping in `src/render/markdown/discovery.rs`
- The contract-first doc/help/spec items all had matching edits:
  - `spec/09-search-all.md`
  - `docs/how-to/search-all-workflow.md`
  - `src/cli/search_all_command.rs`
  - `src/cli/list.rs`
  - `tests/test_public_search_all_docs_contract.py`
- Commit ordering matched the design requirement:
  - `0ee2ba4a Define search-all counts-only contract` contains docs/help/spec/tests only
  - `e80e490b Implement search-all counts-only JSON projection` contains runtime/renderer code

### Test-Design Traceability

- Every proof-matrix row had an intended test location, but one row was not actually executable at review time.
- Verified proof mappings:
  - Counts-only JSON sections omit `results`, `links`, and `total` -> `spec/09-search-all.md`
  - Counts-only markdown keeps follow-up commands -> `spec/09-search-all.md`
  - Counts-only JSON preserves `debug_plan` -> `src/cli/search_all.rs::counts_only_json_projection_preserves_debug_plan`
  - Counts-only markdown omits row tables and keeps links -> `src/render/markdown/discovery/tests.rs::search_all_markdown_counts_only_keeps_links_without_row_headers`
  - Counts-only fetch-budget helper reduces only safe legs -> `src/cli/search_all.rs::section_fetch_limit_reduces_only_safe_counts_only_sections`
  - Guide reflects markdown-vs-JSON split -> `tests/test_public_search_all_docs_contract.py::test_search_all_workflow_guide_distinguishes_markdown_and_json_counts_only`
  - Search-all help reflects repaired contract -> `src/cli/tests/facade/help.rs::search_all_help_mentions_counts_only_json_contract`
- Blocking finding:
  - `spec/09-search-all.md` contained a new `## Counts-only JSON Contract` section, but `uv run --extra dev pytest spec/09-search-all.md --mustmatch-lang bash --collect-only -q` collected only 8 items and skipped that section entirely.
  - Cause: the new bash block had only `jq` assertions, so the mustmatch collector did not register it as an executable spec item.

### Quality Review

- Security: no new untrusted-input flow, shell interpolation, path construction, or auth boundary issues were introduced.
- Duplication: the counts-only JSON projection follows the existing dedicated count-only JSON pattern already used in `src/cli/trial/dispatch.rs`; no redundant abstraction was introduced.
- Implementation quality: the Rust changes align with adjacent conventions, the help/doc updates match the shipped behavior, and direct CLI sampling confirmed the intended counts-only runtime contract.
- Performance: the conservative fetch-budget split is implemented as designed. No additional runtime defect was found after reviewing the dispatch path and running live `search all --counts-only` samples.

## Fix Plan

- Repair the missing proof by making the `Counts-only JSON Contract` block in `spec/09-search-all.md` collectible by the mustmatch runner.
- Re-run spec collection, the targeted `search all` spec file, and the full spec gate to confirm the proof-matrix row is now real coverage.
- No Rust runtime changes were needed; the defect was in the executable-spec surface.

## Repair

- Updated `spec/09-search-all.md` to add two `mustmatch like` anchors inside the `Counts-only JSON Contract` block:
  - `echo "$out" | mustmatch like '"entity":'`
  - `echo "$out" | mustmatch like '"count":'`
- This keeps the precise `jq` structural assertion while making the section collectible by the spec runner.

### Post-Fix Collateral Damage Scan

- Re-ran spec collection:
  - before fix: 8 collected items from `spec/09-search-all.md`
  - after fix: 9 collected items, including `Counts-only JSON Contract (line 44) [bash]`
- Re-ran the targeted spec file after rebuilding the release binary:
  - `uv run --extra dev pytest spec/09-search-all.md --mustmatch-lang bash -q` -> `9 passed`
- Confirmed no adjacent headings or bash blocks became uncollectable after the spec edit.
- No dead code, stale messages, unused imports, or shadowed variables were introduced because the repair touched only the markdown spec file.

## Validation

- `cargo test --lib && cargo clippy --lib --tests -- -D warnings` -> passed
- `uv run --extra dev pytest tests/test_public_search_all_docs_contract.py` -> `11 passed`
- `uv run --extra dev pytest spec/09-search-all.md --mustmatch-lang bash --collect-only -q` -> `9 collected`
- `uv run --extra dev pytest spec/09-search-all.md --mustmatch-lang bash -q` -> `9 passed`
- `make spec-pr` -> first shard `222 passed, 4 skipped`; second shard `99 passed, 2 skipped`

## Residual Concerns

- None. The only blocking issue found was the missing executable-spec coverage, and that proof now runs and passes.

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| 1 | missing-test | no | The design-required `spec/09-search-all.md` counts-only JSON contract section existed in markdown but was not collected by the mustmatch runner, so the proof-matrix row was effectively missing until the block gained collectible `mustmatch` anchors. |
