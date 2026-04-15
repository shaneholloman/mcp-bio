# Code Review Log - Ticket 195

## Critique

Reviewed `.march/ticket.md`, `.march/design-draft.md`, `.march/design-final.md`, `.march/code-log.md`, commit history, and the full `git diff main..HEAD`.

Re-ran the relevant local gates during review:

- `cargo test --lib`
- `cargo clippy --lib --tests -- -D warnings`
- `cargo build --release --locked --bin biomcp`
- `XDG_CACHE_HOME="$PWD/.cache" PATH="$PWD/target/release:$PATH" RUST_LOG=error uv run --extra dev pytest spec/07-disease.md --mustmatch-lang bash --mustmatch-timeout 120 -v`

### Design Completeness Audit

All design-final implementation items have corresponding code changes after repair:

- Shared search JSON/meta support is present in `src/cli/shared.rs`.
- Search next-command builders are present in `src/render/markdown/related.rs` and exported from `src/render/markdown/mod.rs`.
- All in-scope search dispatchers are wired to the new helpers or custom `_meta` serializers:
  - article, trial, variant, gene, disease, drug, pgx, pathway, adverse-event, gwas
- Out-of-scope generic search surfaces stayed unchanged:
  - `src/cli/phenotype/dispatch.rs`
  - `src/cli/protein/dispatch.rs`
  - no diff on `spec/23-phenotype.md` or `spec/16-protein.md`
- `list <entity>` docs were updated for article, trial, variant, gene, disease, drug, pgx, pathway, adverse-event, and gwas in `src/cli/list.rs`.
- Entity specs were updated on the intended search JSON contract surfaces:
  - `spec/02-gene.md`
  - `spec/03-variant.md`
  - `spec/04-trial.md`
  - `spec/05-drug.md`
  - `spec/06-article.md`
  - `spec/07-disease.md`
  - `spec/08-pgx.md`
  - `spec/14-pathway.md`
- `spec/11-evidence-urls.md` and `spec/18-source-labels.md` remain scoped to `get ... --json`; that design choice is consistent with their contents.
- `.march/code-log.md` records docs/spec work before runtime edits; nothing in the review contradicted that ordering.

No unmatched `Needs change` items were present in `design-final.md`.

### Test-Design Traceability

Proof-matrix coverage after repair:

- Helper proofs:
  - `src/cli/tests/outcome.rs` covers `search_json_with_meta(...)` include/omit behavior and preserves the generic phenotype search JSON contract.
  - `src/cli/article/tests.rs` covers article search JSON context fields plus `_meta.next_commands`.
  - `src/cli/disease/tests.rs` covers direct-hit and fallback-hit disease JSON shapes.
  - `src/cli/drug/tests.rs` now exercises the WHO search JSON path with `search_json_with_meta(...)` and `_meta.next_commands`.
  - `src/cli/tests/next_commands_validity.rs` covers parser validity for the new search follow-up command shapes.
  - `src/cli/list.rs` tests cover the new doc/help contract.
- Spec proofs:
  - all design-matrix entity/spec assertions are present after the disease fallback-miss repair.

Issues found during critique:

1. `spec/07-disease.md` did not assert the proof-matrix requirement that empty discover-fallback misses keep `_meta` absent in JSON.
2. `src/cli/drug/tests.rs::search_json_preserves_who_search_fields` still exercised `search_json()` instead of the real `search_json_with_meta()` path used by WHO search results, so it would not catch `_meta.next_commands` regressions.
3. `search_next_commands_drug_eu()` only matched against EMA `name`, ignoring `active_substance`; plain parent-drug queries could therefore degrade to brand-name `biomcp get drug ...` suggestions instead of the intended parent-name follow-up.
4. The recall-specific exception from the acceptance criteria had no deterministic regression proof: non-empty recall searches should emit only `biomcp list adverse-event`.

### Implementation Quality Review

- Security review: no new shell injection, path traversal, auth bypass, data exposure, or secret-handling issues found. Follow-up commands continue to use `quote_arg()` before serialization.
- Duplication review: the implementation correctly reused and centralized the drug parent-name heuristic instead of reintroducing another chooser.
- Conventions and edge cases:
  - disease custom `_meta` migration is coherent after the repair
  - out-of-scope generic search JSON stayed stable
  - the repaired EU drug chooser now handles the parent-name case the design called out

## Fix Plan

Repair the four concrete defects directly:

1. Add the missing empty-miss `_meta` absence assertion to `spec/07-disease.md`.
2. Move the WHO drug JSON regression test onto `search_json_with_meta(...)` and assert `_meta.next_commands`.
3. Fix EMA follow-up generation to consider `active_substance` when applying the parent-name preference heuristic.
4. Add deterministic helper tests for the recall list-only exception and the device-event follow-up branch.

## Repair

Applied the fixes:

- `src/render/markdown/related.rs`
  - updated `search_next_commands_drug_eu()` to consider both EMA `name` and `active_substance`, and to fall back to `active_substance` before the brand name.
- `src/render/markdown/related/tests/drug_variant_article_trial.rs`
  - added regression coverage for:
    - EMA parent-drug follow-up selection
    - recall list-only next-command behavior
    - device-event report follow-up behavior
- `src/cli/drug/tests.rs`
  - replaced the stale WHO JSON helper test with a real `search_json_with_meta(...)` regression that preserves WHO fields and asserts `_meta.next_commands`
- `spec/07-disease.md`
  - added the missing empty-fallback-miss assertion that top-level `_meta` stays absent

### Post-Fix Collateral Scan

After each repair batch, checked the touched code for:

- dead code or orphaned imports: none introduced
- unused variables: none introduced
- cleanup conflicts: none introduced
- stale error text: unchanged
- shadowed variables: none introduced

Added one bounded improvement while touching the related-command tests:

- covered the device-event branch explicitly so the adverse-event family now has deterministic proof for both its special recall behavior and its device-report follow-up shape

## Verification

- Targeted regression checks:
  - `cargo test --lib search_json_with_meta_preserves_who_search_fields`
  - `cargo test --lib search_next_commands_drug_eu_prefers_active_substance_match`
  - `cargo test --lib search_next_commands_recalls_are_list_only`
  - `cargo test --lib search_next_commands_device_events_use_report_follow_up`
- Focused profile:
  - `cargo test --lib && cargo clippy --lib --tests -- -D warnings`
  - result: passed
- Changed executable spec:
  - `spec/07-disease.md`
  - result: `31 passed`

## Residual Concerns

- The shipped contract is repaired and the changed disease spec is green.
- Recall/device search JSON behavior now has deterministic unit proof, but not separate live-data spec sections. Verify should keep an eye on that family if future tickets widen the adverse-event executable spec surface.
- No out-of-scope issue was substantial enough to file under `~/workspace/planning/biomcp/issues/`.

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| 1 | missing-test | no | `spec/07-disease.md` was missing the design proof that empty fallback misses keep top-level `_meta` absent. |
| 2 | weak-assertion | no | `src/cli/drug/tests.rs` still exercised `search_json()` instead of the WHO search JSON path that now uses `search_json_with_meta()`, so `_meta.next_commands` regressions would have escaped. |
| 3 | contract-bug | no | `search_next_commands_drug_eu()` ignored EMA `active_substance`, which could select a brand-name `get drug` follow-up instead of the requested parent drug. |
| 4 | missing-test | no | The recall-search acceptance-criteria exception had no deterministic regression coverage for the list-only `_meta.next_commands` behavior. |
