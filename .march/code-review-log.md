## Review Scope

- Read `.march/ticket.md`, `.march/design-draft.md`, `.march/design-final.md`, and `.march/code-log.md`
- Rebased onto `main` with `GIT_EDITOR=true git rebase main`
- Reviewed `git diff main..HEAD`, `git log --oneline main..HEAD`, and `git show --name-only --format=medium --no-renames HEAD`
- Inspected every changed file under `src/entities/variant/` plus the deleted legacy `src/entities/variant.rs`
- Re-ran the relevant proof surface:
  - `cargo test variant --lib`
  - `cargo clippy --lib --tests -- -D warnings`
  - `make check`
  - `make spec-pr`

## Critique

### Design Completeness Audit

- `design-final.md` contains no items marked `Needs change`; the audit therefore covered the stable surface, module layout, implementation order, acceptance criteria, and proof matrix.
- Acceptance criteria 1-8 matched the code:
  - `src/entities/variant/` exists with `mod.rs`, `resolution.rs`, `search/mod.rs`, `gwas.rs`, `get.rs`, `test_support.rs`, and sidecar tests under `resolution/`, `search/`, `gwas/`, and `get/`
  - `src/entities/variant.rs` is removed
  - every new file begins with a `//!` module doc comment
  - every file under `src/entities/variant/` stays under the 700-line cap
  - no caller outside `src/entities/variant/` required an import-path edit; `git diff --name-only main..HEAD -- . ':(exclude)src/entities/variant/**' ':(exclude)src/entities/variant.rs'` returned nothing
  - the stable facade in `src/entities/variant/mod.rs` preserves the public and `pub(crate)` re-exports consumed by CLI, render, transform, discover, and source callers
- Acceptance criteria 9-10 matched the rerun gates:
  - `make check` passed
  - `make spec-pr` passed
- Implementation-order deviation found:
  - `.march/design-final.md` step 8 required deleting `src/entities/variant.rs` only after the replacement tree was wired and compiling
  - `.march/code-log.md` recorded an earlier delete to force a red state, but still claimed `Deviations from Design: None`
  - that made `.march/code-log.md` internally inconsistent and inaccurate

### Test-Design Traceability

- The design draft's named resolution tests all exist in `src/entities/variant/resolution/tests.rs`:
  - `parse_variant_id_examples`
  - `parse_variant_id_egfr_l858r`
  - `parse_variant_id_kras_g12c`
  - `parse_variant_id_normalizes_uppercase_rsid_prefix`
  - `parse_variant_id_accepts_long_form_gene_protein_change`
  - `parse_variant_id_accepts_prefixed_short_gene_protein_change`
  - `classify_variant_input_detects_search_only_shorthand`
  - `classify_variant_input_normalizes_long_form_single_token_protein_change`
  - `parse_variant_id_points_search_only_shorthand_to_search_variant`
  - `parse_variant_id_points_long_form_single_token_to_search_variant`
  - `parse_variant_id_suggests_search_for_complex_alteration_text`
- The design draft's named search tests all exist in `src/entities/variant/search/tests.rs`:
  - `search_query_summary_includes_hgvsc_and_rsid`
  - `search_query_summary_includes_residue_alias_marker`
  - `exon_deletion_fallback_preserves_non_exon_filters`
  - `quality_score_prioritizes_significance_and_frequency`
- The GWAS helper coverage required by the design exists in `src/entities/variant/gwas/tests.rs`:
  - `collect_supporting_pmids_dedupes_case_insensitively`
- The design draft's get/enrichment tests exist in `src/entities/variant/get/tests.rs` or, where the draft explicitly said to move them, in `src/entities/variant/gwas/tests.rs`:
  - `variant_json_omits_legacy_name_when_absent`
  - `parse_sections_supports_new_variant_sections`
  - `gwas_only_request_detection_matches_section_flags`
  - `gwas_only_variant_stub_keeps_requested_rsid`
  - `civic_molecular_profile_name_prefers_gene_and_hgvs_p`
  - `gwas_only_request_returns_variant_when_gwas_is_unavailable`
  - `therapies_from_oncokb_truncation_shows_count`
  - `collect_supporting_pmids_dedupes_case_insensitively` in `gwas/tests.rs`, matching the draft's relocation note
- The proof matrix's outside-in spec surface is still present and green:
  - `spec/03-variant.md` still contains `Searching by Gene`, `Finding a Specific Variant`, `Getting Variant Details`, `GWAS Supporting PMIDs`, and `Variant to Trials`
  - `make spec-pr` passed on the blocking spec surface
- The proof matrix's two smoke-only headings still exist:
  - `spec/12-search-positionals.md::GWAS Positional Query`
  - `spec/03-variant.md::Variant to Articles`
- I did not find a design-required test missing from the relocated surface.

### Quality Review

- Implementation quality: the refactor follows the established entity-facade pattern used by `trial` and adjacent modules, keeps scoped visibility for internal helpers, and preserves the stable import surface without caller edits.
- Duplication: `src/entities/variant/test_support.rs` mirrors the existing entity test-support pattern rather than inventing a new abstraction.
- Security: the refactor did not introduce new path, shell, auth, or data-exposure flows; moved validation still happens before backend calls.
- Performance: this is structural movement only; I did not find a new algorithmic or I/O regression in the touched runtime surface.

## Fix Plan

- Repair `.march/code-log.md` so its execution-order summary and deviations section match the approved design and the recorded history.
- Replace the stale `.march/code-review-log.md` carry-over with the actual ticket 190 review log.

## Repair

- Updated `.march/code-log.md`:
  - the execution-order summary now matches the recorded red-state sequence
  - the deviations section now records the one implementation-order deviation instead of claiming none
- Rewrote `.march/code-review-log.md` for ticket 190.
- No Rust source files needed changes.
- No out-of-scope issue file was needed.

### Post-Fix Collateral Damage Scan

- Dead code: not introduced; the repairs were `.march`-only and did not touch runtime Rust modules
- Unused imports/variables: not introduced
- Resource cleanup conflicts: not applicable
- Stale error messages: not introduced
- Shadowed variables: not introduced

### Validation

- `cargo test variant --lib` passed: 117 tests passed
- `cargo clippy --lib --tests -- -D warnings` passed
- `make check` passed: 1513 tests passed, 1 skipped
- `make spec-pr` passed:
  - first lane: 221 passed, 4 skipped
  - second lane: 99 passed, 2 skipped
- The review repair changed only `.march` artifacts, so no post-repair focused Rust rerun was required.

### Residual Concerns

- No blocking concerns remain.
- Optional verify smoke: the non-blocking live-backed headings `spec/12-search-positionals.md::GWAS Positional Query` and `spec/03-variant.md::Variant to Articles` were not rerun here because the design marks them smoke-only and `make spec-pr` explicitly deselects them.

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| 1 | stale-doc | no | `.march/code-log.md` was internally inconsistent and falsely claimed no deviations from design even though it recorded deleting `src/entities/variant.rs` before the replacement tree was fully wired and compiling. |
| 2 | stale-doc | no | Existing `.march/code-review-log.md` was a stale carry-over from ticket 189 and did not document ticket 190. |
