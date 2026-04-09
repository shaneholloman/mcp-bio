# Code Review Log — Ticket 155

## Critique

I reviewed `.march/ticket.md`, `.march/design-draft.md`, `.march/design-final.md`,
`.march/code-log.md`, and the full `git diff main..HEAD`, then inspected the
changed Rust and spec surfaces directly.

Design completeness audit:

- The final design's required runtime changes all have matching code:
  - the dedicated Semantic Scholar search-enrichment client method in
    `src/sources/semantic_scholar.rs`
  - the row-level search enrichment helpers in `src/entities/article.rs`
  - the sync `finalize_article_candidates()` preservation plus async route
    wrappers around search orchestration
  - the federated row collector split that preserves the sync merge test surface
  - the required executable spec update in `spec/06-article.md`
- The docs/contract audit matched the design: no CLI help or user-guide copy
  changes were required for this ticket, and the one required contract surface
  (`spec/06-article.md` under `Source-Specific PubMed Search`) was updated.
- `.march/code-log.md` confirms the spec change landed before the runtime code,
  so the shipped behavior remained contract-first instead of retrofitted.

Test-design traceability audit:

- Every proof-matrix entry in `.march/design-final.md` has a matching proof:
  - `pubmed_source_search_enriches_citation_count_and_abstract_from_semantic_scholar_batch`
  - `federated_search_enrichment_overwrites_europepmc_zero_citation_count`
  - `article_search_enrichment_preserves_existing_nonempty_primary_metadata`
  - `article_search_semantic_scholar_batch_failure_is_non_fatal`
  - `article_search_semantic_scholar_batch_enrichment_chunks_after_500_ids`
  - `paper_batch_search_enrichment_requests_abstract_and_citation_fields`
  - `spec/06-article.md::Source-Specific PubMed Search`
- The proofs assert contract behavior rather than implementation details:
  provenance is preserved, citation counts upgrade when allowed, primary-source
  metadata wins when already populated, failures stay non-fatal, and the public
  JSON contract exposes the enriched row fields outside-in.

Implementation quality review:

- I did not find a correctness defect in the changed paths.
- I did not find a new security issue: the change adds no shell, filesystem,
  auth, or data-exposure surface.
- I searched for duplicated existing abstractions before accepting the new
  helpers. The added code intentionally mirrors the existing article-batch
  enrichment pattern; it does not reinvent a repo-local equivalent that should
  have been reused instead.
- Data completeness is satisfied for the changed contract: the search-row
  enrichment path now populates `citation_count`,
  `influential_citation_count`, `abstract_snippet`, and
  `normalized_abstract` where the design requires it, while preserving
  `source` and `matched_sources`.
- Performance is acceptable for the approved scope. The main path uses batched
  Semantic Scholar lookups with 500-ID chunking, and the additional visible-row
  fallback is bounded to visible PubMed-backed rows only and remains non-fatal.

## Fix Plan

No fixes were required. The implementation, tests, spec update, and contract
ordering already satisfy the approved design and review quality checks.

## Repair

No implementation or test edits were necessary in this review pass.

Independent verification:

- `make spec` passed: `321 passed, 6 skipped`
- `make check` passed cleanly, including the main library run
  (`1340 passed`) plus the remaining integration and doc-test stages

Post-fix collateral damage scan:

- No code changes were applied during this review, so there was no new dead
  code, unused state, cleanup conflict, stale error path, or variable shadowing
  introduced by the review step itself.

## Residual Concerns

No ticket-scoped residual defects remain from this review. Verify only needs
the normal watch for live upstream variability in the article smoke spec.

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| 1 | None found | no | No blocking or non-blocking defects were identified in the changed implementation, docs/spec contract, or proof coverage. |
