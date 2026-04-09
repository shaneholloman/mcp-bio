# Code Review Log — Ticket 156

## Critique

I reviewed `.march/ticket.md`, `.march/design-draft.md`, `.march/design-final.md`,
`.march/code-log.md`, and the full `git diff main..HEAD`, then reran the local
gates independently.

Design completeness audit:

- The approved runtime items in `.march/design-final.md` are implemented in the
  changed article ranking pipeline, CLI wiring, markdown/JSON rendering, and
  search-all article leg.
- The design's docs-and-contract requirement was not fully satisfied on the
  first pass. `architecture/technical/overview.md` still documented the old
  article `--source` roster (`all|pubtator|europepmc`) even though the shipped
  CLI now exposes `pubmed` and `litsense2`.
- That stale contract also leaked into
  `tests/test_upstream_planning_analysis_docs.py`, which was still enforcing the
  outdated roster. Because the ticket changed a shipped CLI contract, the stale
  doc plus stale doc-contract test was a blocking defect, not a follow-up.
- `.march/code-log.md` shows the code step updated help/docs/specs before the
  runtime implementation. The review defect was incompleteness inside that docs
  batch, not an incorrect implementation order.

Test-design traceability audit:

- The ranking calibration proofs required by the design are present:
  `ranking_calibration::hybrid_default_weights_orders_example_one`,
  `ranking_calibration::lexical_mode_matches_current_ordering`,
  `ranking_calibration::hybrid_entity_only_falls_back_without_nan`, and
  `ranking_calibration::hybrid_custom_weights_shift_ordering`.
- The public proof for preserved article metadata in `search all` exists at
  `spec/09-search-all.md::JSON Search All Preserves Article Metadata`.
- I found two blocking proof gaps against the design:
  - `search_article_ranking_flags_validate_cleanly` did not cover an explicit
    negative hybrid weight or the entity-only default-lexical case with an
    overridden weight, even though acceptance criterion 6 requires finite,
    non-negative, non-degenerate validation and rejection outside effective
    hybrid mode.
  - The design requires `search all` to share the same keyword-sensitive default
    ranking logic as `search article`, but there was no explicit proof that a
    keyword-bearing `search all` article leg emits hybrid ranking.

Implementation quality review:

- I did not find a new ticket-scoped security issue. The ranking changes do not
  introduce new shell, filesystem, auth, or data-exposure flows.
- I checked for duplication around the new ranking/default logic. The current
  implementation reuses the article ranking helpers rather than introducing a
  redundant parallel policy surface.
- The changed runtime behavior is now consistent with the docs/spec contract:
  keyword-bearing article paths report hybrid ranking; entity-only article paths
  default to lexical; weight validation rejects invalid configurations before
  execution.

## Fix Plan

I fixed every blocking issue directly in this review pass:

- update the stale architecture overview contract text and its Python
  doc-contract assertion,
- strengthen article ranking validation coverage for the missing error cases,
- add a `search all` unit proof for keyword-dependent article ranking defaults,
- add an outside-in executable spec proving keyword-bearing `search all`
  requests emit hybrid article ranking metadata.

## Repair

Applied fixes:

- Updated `architecture/technical/overview.md` so the article `--source`
  contract matches the shipped CLI surface:
  `all|pubtator|europepmc|pubmed|litsense2`.
- Updated `tests/test_upstream_planning_analysis_docs.py` to enforce the
  corrected contract instead of the stale roster.
- Expanded `search_article_ranking_flags_validate_cleanly` in
  `src/entities/article.rs` to cover:
  - entity-only default lexical mode rejecting overridden weights, and
  - negative hybrid weights rejecting with the correct flag-specific error.
- Added `article_filters_follow_keyword_dependent_ranking_defaults` in
  `src/cli/search_all.rs` so `search all` proves keyword queries resolve to
  hybrid ranking and entity-only queries resolve to lexical ranking.
- Added `spec/09-search-all.md::JSON Keyword Search Uses Hybrid Article Ranking`
  as an outside-in contract proof, and switched the affected JSON headings to
  use `BIOMCP_BIN` fallback resolution so focused spec runs use the intended
  binary consistently.

Post-fix collateral damage scan:

- After each edit I checked the touched area for dead branches, orphaned
  imports, stale error text, cleanup conflicts, and shadowed variables.
- No collateral issue was introduced by the review fixes.

Independent verification:

- `uv run pytest tests/test_upstream_planning_analysis_docs.py -q`
- `cargo test search_article_ranking_flags_validate_cleanly -- --nocapture`
- `cargo test article_filters_follow_keyword_dependent_ranking_defaults -- --nocapture`
- `make spec` -> `322 passed, 6 skipped`
- `make check` -> clean, including `1357` Rust tests plus the integration/doc-test stages

## Residual Concerns

- No ticket-scoped defects remain in the ranking/docs/spec contract after the
  review repairs.
- I filed one adjacent follow-up issue outside this ticket's scope for a drug
  target-family fallback edge case:
  `/home/ian/workspace/planning/biomcp/issues/156-drug-target-family-opentargets-fallback-gap.md`

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| 1 | stale-doc | yes | `architecture/technical/overview.md` still advertised the old article `--source` roster and no longer matched the shipped CLI contract. |
| 2 | stale-doc | yes | `tests/test_upstream_planning_analysis_docs.py` enforced the stale article source roster, blocking the required contract update. |
| 3 | missing-test | no | Design acceptance criterion 6 required explicit proof for negative hybrid weights and entity-only default-lexical weight rejection, but `search_article_ranking_flags_validate_cleanly` did not cover those cases. |
| 4 | missing-test | no | Design acceptance criteria 7 and 8 required `search all` to share keyword-sensitive hybrid defaults with `search article`, but no unit/spec proof covered the keyword-bearing article leg. |
