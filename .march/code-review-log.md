# Code Review Log — ticket 379 prune brittle live assertions

## Critique Summary

Reviewed `.march/ticket.md`, `.march/design-draft.md`, `.march/design-final.md`, `.march/spec-red-check.json`, `.march/code-log.md`, and the full diff against `main`, including the uncommitted implementation edits in the target spec files.

### Design Completeness Audit

Every design-final implementation item has a matching code/spec-corpus change:

- `spec/entity/article.md` prunes non-fixture live BioMCP blocks from the nine named article headings while preserving the fulltext fixture sections.
- `spec/entity/variant.md` prunes live MyVariant/normalization blocks from the ten named variant headings and removes stale `mustmatch-lint` skip comments left by that pruning.
- `spec/entity/disease.md` prunes live blocks from `Disease Normalization & Search`, `Genes & Diagnostics`, and `JSON Pivots`; the diagnostics count/prose pin is gone.
- `spec/entity/disease.md::NIH Funding Context` keeps section/table anchors but removes the numeric `Showing top ... grants ...` assertion.
- `spec/surface/discover.md` prunes live discover blocks from the five named headings, keeps suggest/skill coverage outside the target list, and removes stale prose/subheading collateral from the pruned canary.
- `spec/README-timings.md` records ticket 379, all four target paths, deterministic request/source/fixture/renderer ownership, and explicit `release-live-smoke` ownership.

No runtime code, CLI surface, validation-profile routing, or unrelated spec files were changed.

### Test-Design Traceability

Forward traceability passed. Each design-final proof-matrix entry has a landed assertion in `spec/surface/test_parallel_isolation_contract.py`:

1. Article/variant redundant live public-upstream pruning → `test_ticket_379_article_variant_source_specs_prune_redundant_live_blocks`.
2. Disease/discover redundant live public-upstream pruning → `test_ticket_379_disease_discover_specs_prune_redundant_live_blocks`.
3. Disease diagnostics/funding numeric count/prose pruning → `test_ticket_379_target_specs_drop_count_prose_trivia`.
4. Timing/lane ownership docs → `test_ticket_379_timing_docs_record_pruned_ownership`.

Reverse traceability also passed. The only new shipped-contract assertions are the four ticket-379 design assertions above. Removed or relaxed assertions are the design-named live public-upstream blocks or the explicit disease count/prose pins. The deleted Warfarin subheading and UMLS degraded-banner sentence are mechanical stale-doc cleanup after their live canary blocks were pruned, not independent behavioral contract changes.

### Edit Discipline Audit

Minimal implementation size for this ticket is the named live bash-block removals, the disease count/prose-pin removals, stale skip/comment cleanup caused by those removals, and one timing ownership note. The actual implementation diff is limited to the named files and contains no runtime, profile, helper-module, or unrelated formatting churn. No over-edit defects found.

### Quality Checks

- **Implementation quality:** Changes stay in the spec-corpus layer as designed. New Python helpers follow adjacent style and reuse existing markdown section readers.
- **Test quality:** Ticket-379 assertions are structural/semantic lane-ownership checks; they avoid line numbers, exact prose pins, and incidental counts.
- **Performance:** Static markdown inspection only; no new subprocess or network calls in routine proof.
- **Data completeness:** Replacement ownership is documented and existing deterministic source/renderer contract references remain in the target docs.
- **Security:** No new untrusted input flow, shell construction, secret exposure, or auth behavior changes. Existing deterministic tests cover Semantic Scholar auth redaction.
- **Duplication:** Searched for existing bash-block helpers. `_bash_blocks` is intentionally separate from `_non_skipped_bash_blocks` because ticket 379 must inspect skipped bash fences too.

## Repairs Applied

None. No defects were found, so no review commit was created.

## Validation Rerun

- `PATH="$PWD/target/release:$PATH" BIOMCP_BIN="$PWD/target/release/biomcp" uv run --no-sync pytest spec/surface/test_parallel_isolation_contract.py -k 'ticket_379' -v` — 4 passed, 22 deselected
- `make spec-contracts` — 52 passed
- `git diff --check` — passed
- `cargo test --lib` — 2007 passed
- `cargo clippy --lib --tests -- -D warnings` — passed

## Residual Concerns

None requiring a follow-up issue.

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| 1 | None found | no | No defects found. |
