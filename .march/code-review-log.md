# Code Review Log — Ticket 154

## Critique

I reviewed `.march/ticket.md`, `.march/design-draft.md`, `.march/design-final.md`,
`.march/code-log.md`, and the full `git diff main..HEAD`. The runtime
implementation covered the designed LitSense2 client, planner wiring,
dedupe+hydrate flow, federated leg, health registration, rate limiting,
search-all source registries, and the named docs/spec surfaces. I also checked
the changed paths for security issues, searched for equivalent existing helpers
before accepting the new additions, and confirmed from `.march/code-log.md`
that docs/help/spec work landed before the runtime edits.

The defects I found were proof and contract-coverage gaps rather than broken
runtime logic:

1. The repaired proof matrix says help/list surfaces must advertise
   `--source litsense2`, but the outside-in spec and `list article` unit test
   did not lock those user-visible strings.
2. The proof matrix promises unit coverage for the shared free-text article
   query helper and LitSense2 enablement rules, but those tests were missing.
3. Explicit-source validation coverage was incomplete for
   `--source litsense2 --open-access`.
4. The LitSense2 dedupe/hydration regression was too weak: it did not prove the
   highest-scoring sentence text was the one preserved, and it did not prove
   hydrated journal/date metadata survives filter application.
5. There was no direct merge/finalize proof that `matched_sources` records
   `LitSense2` when duplicate PMIDs collapse across backends.

## Fix Plan

1. Tighten the article help/list executable spec and `list article` unit test
   so the LitSense2 source roster is locked where users see it.
2. Add article-layer unit tests for the shared free-text query helper,
   LitSense2 enablement rules, and `--open-access` rejection.
3. Strengthen LitSense2 candidate tests to prove strongest-hit preservation and
   hydrated date/journal filter survival.
4. Add a merge regression proving `matched_sources` gains `LitSense2` on merged
   duplicate PMIDs.

## Repair

- Added `build_free_text_article_query_preserves_mixed_semantic_anchors`,
  `litsense2_search_enabled_requires_keyword_and_non_strict_filters`, and
  `planner_rejects_litsense2_open_access_filter` in
  `src/entities/article.rs`.
- Strengthened `litsense2_candidates_deduplicate_and_hydrate_pubmed_metadata`
  to assert hydrated journal/date fields and strongest-sentence snippet
  preservation.
- Added
  `litsense2_candidates_apply_hydrated_journal_and_date_filters` to prove
  hydrated PubMed metadata survives LitSense2 post-filtering.
- Added `merge_federated_pages_records_litsense2_in_matched_sources` to lock
  merged `matched_sources` behavior.
- Extended `src/cli/list.rs` coverage so `list article` now locks the
  `litsense2` source roster and explicit-source guidance.
- Extended `spec/06-article.md` so article help/list output asserts the
  `litsense2` source roster and LitSense2 validation now covers
  `--open-access` rejection.
- Re-ran targeted collateral scans after each fix:
  `cargo test --lib build_free_text_article_query_preserves_mixed_semantic_anchors`,
  `cargo test --lib litsense2 -- --nocapture`,
  `cargo test --lib list_trial_and_article_include_missing_flags -- --nocapture`,
  and the targeted `spec/06-article.md` slices.
- Re-ran full verification:
  `make spec` passed with `321 passed, 6 skipped`
  and `make check` passed cleanly.

## Residual Concerns

No code-level residual defects remain from this review. Verify should only keep
normal watch for upstream variability on the live LitSense2-facing article
specs, because those assertions still depend on public NCBI availability.

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| 1 | missing-test | no | Help/list contract coverage did not explicitly prove that user-visible article source surfaces advertise `litsense2`. |
| 2 | missing-test | no | The design-required shared free-text query helper and LitSense2 enablement rules had no direct regression tests. |
| 3 | missing-test | no | Explicit-source validation coverage did not include `--source litsense2 --open-access`. |
| 4 | weak-assertion | no | The LitSense2 dedupe/hydration proof did not verify strongest-hit preservation or hydrated journal/date filter survival. |
| 5 | missing-test | no | No regression proved that merged duplicate PMIDs add `LitSense2` to `matched_sources`. |
