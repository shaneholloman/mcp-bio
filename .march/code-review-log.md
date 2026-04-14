## Review Scope

- Read `.march/ticket.md`, `.march/design-draft.md`, `.march/design-final.md`, and `.march/code-log.md`
- Rebased onto `main` with `GIT_EDITOR=true git rebase main`
- Reviewed `git diff main..HEAD` and the branch commit `deb976d`
- Inspected every changed file under `src/entities/trial/`
- Re-ran targeted and focused proof:
  - `cargo test trial --lib -- --list 2>/dev/null | rg ': test$' -c` => `128`
  - `cargo test trial --lib`
  - `cargo test --lib`
  - `cargo clippy --lib --tests -- -D warnings`
  - Trial-tree doc-comment check
  - Trial-tree line-cap check

## Critique

### Design Completeness Audit

- Acceptance criterion 1 matched: `src/entities/trial.rs` is deleted and replaced by `src/entities/trial/`
- Acceptance criterion 2 matched: every new Rust file under `src/entities/trial/` starts with `//!`
- Acceptance criterion 3 matched: [src/entities/trial/mod.rs](/home/ian/workspace/worktrees/189-decompose-trial-rs-into-trial-submodule/src/entities/trial/mod.rs:1) keeps the public facade types, `TRIAL_SECTION_NAMES`, and `pub use` re-exports for `get`, `search`, `search_page`, and `count_all`
- Acceptance criterion 4 matched: shared validation lives in [src/entities/trial/search/mod.rs](/home/ian/workspace/worktrees/189-decompose-trial-rs-into-trial-submodule/src/entities/trial/search/mod.rs:143), not `search/ctgov.rs`
- Acceptance criterion 5 matched: shared helpers live in [src/entities/trial/test_support.rs](/home/ian/workspace/worktrees/189-decompose-trial-rs-into-trial-submodule/src/entities/trial/test_support.rs:1), and `rg -n 'mod tests \\{' src/entities/trial -g '*.rs'` found no inline test blocks
- Acceptance criterion 6 matched: the trial-tree line-cap check found no file over 700 lines
- Acceptance criterion 7 matched: the trial test inventory still reports `128`
- Acceptance criteria 8-9 matched: `cargo test trial --lib` and `cargo clippy --lib --tests -- -D warnings` passed
- Unchanged docs/help/spec contract mostly matched: `git diff --name-only main..HEAD` touched only `src/entities/trial/*`, so the decomposition stayed inside the intended runtime surface

Finding:

- `spec/04-trial.md` still said the traversal-cap regression was covered in `src/entities/trial.rs` unit tests. That path was deleted by this ticket, so the executable spec note was stale after the refactor.

### Test-Design Traceability

- Proof-matrix unit tests all exist in the relocated sidecars:
  - `normalize_nct_id_uppercases_prefix` and `get_rejects_non_nct_id_with_format_hint` in [src/entities/trial/get/tests.rs](/home/ian/workspace/worktrees/189-decompose-trial-rs-into-trial-submodule/src/entities/trial/get/tests.rs:1)
  - `essie_escape_boolean_expression_preserves_or_operators` and `line_of_therapy_patterns_accepts_supported_values` in [src/entities/trial/search/essie/tests.rs](/home/ian/workspace/worktrees/189-decompose-trial-rs-into-trial-submodule/src/entities/trial/search/essie/tests.rs:1)
  - `parse_age_years_handles_standard_formats` in [src/entities/trial/search/eligibility/tests.rs](/home/ian/workspace/worktrees/189-decompose-trial-rs-into-trial-submodule/src/entities/trial/search/eligibility/tests.rs:132)
  - `ctgov_query_term_broadens_mutation_across_discovery_fields`, `build_ctgov_search_params_maps_all_shared_fields`, `age_filter_uses_native_total_semantics_across_limits`, `age_filter_total_returns_native_total_when_exhausted`, `count_all_returns_approximate_for_age_only_filters`, `count_all_returns_exact_for_no_post_filters`, and `count_all_returns_unknown_when_expensive_post_filter_hits_page_cap` in [src/entities/trial/search/ctgov/tests.rs](/home/ian/workspace/worktrees/189-decompose-trial-rs-into-trial-submodule/src/entities/trial/search/ctgov/tests.rs:1)
  - `nci_search_page_prefers_grounded_disease_concept_id` in [src/entities/trial/search/nci/tests.rs](/home/ian/workspace/worktrees/189-decompose-trial-rs-into-trial-submodule/src/entities/trial/search/nci/tests.rs:51)
- Proof-matrix spec scenarios all still exist as outside-in assertions in [spec/04-trial.md](/home/ian/workspace/worktrees/189-decompose-trial-rs-into-trial-submodule/spec/04-trial.md:1), including:
  - `Searching by Condition`
  - `Filtering by Status`
  - `Filtering by Phase`
  - `Combined Phase 1 and 2 Search`
  - `Mutation Search`
  - `Intervention Code Punctuation Normalization`
  - `Zero-Result Positional Hint`
  - `Getting Trial Details`
  - `Eligibility Section`
  - `Locations Section`
  - `Age Filter Count Stability`
  - `Age-Only Count Approximation Signal`
  - `Fractional Age Filter`
  - `Expensive Count Traversal Cap`
  - `NCI Source Search`
  - `NCI Terminated Status Search`
  - `Trial Help Documents NCI Source Semantics`
  - `Trial List Documents NCI Filters`
- I did not find a design-required test missing from the relocated surface.

### Quality Review

- Implementation quality: the split follows the existing entity-submodule pattern used elsewhere in the repo
- Duplication: the new `trial/test_support.rs` matches the established article/disease test-support pattern rather than inventing a new abstraction
- Security: the refactor does not introduce new path, shell, or auth flows; moved logic still validates user inputs before backend calls
- Performance: this is structural movement only; I did not find a new algorithmic or I/O regression in the touched surface

## Fix Plan

- Update the stale spec note in `spec/04-trial.md` to point at the relocated CTGov trial unit tests
- Re-scan for any remaining `src/entities/trial.rs` references after the edit

## Repair

- Updated [spec/04-trial.md](/home/ian/workspace/worktrees/189-decompose-trial-rs-into-trial-submodule/spec/04-trial.md:75) so the traversal-cap contract note now points at `src/entities/trial/search/ctgov/tests.rs`
- Re-scanned the repo with `rg -n 'src/entities/trial\\.rs' . -g '*.md' -g '*.rs'` and found no remaining stale references
- No out-of-scope issue file was needed

### Post-Fix Collateral Damage Scan

- Dead code: not introduced; the fix was doc-only
- Unused imports/variables: not introduced; the fix did not touch Rust code
- Resource cleanup conflicts: not applicable; the fix did not touch cleanup paths
- Stale error messages: not introduced; the fix updated stale documentation text only
- Shadowed variables: not introduced; the fix did not touch code scopes

### Validation

- `cargo test trial --lib -- --list 2>/dev/null | rg ': test$' -c` => `128`
- `cargo test trial --lib` passed
- `cargo test --lib` passed
- `cargo clippy --lib --tests -- -D warnings` passed
- Trial-tree doc-comment and line-cap checks passed
- I did not rerun the full blocking lane (`make check`, `make spec-pr`) during review; `.march/code-log.md` shows that the save-point commit already carried those proofs, and the review reran targeted plus focused validation only

### Residual Concerns

- None beyond the normal verify pass. The only defect found was the stale spec path, and that is repaired.

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| 1 | stale-doc | yes | `spec/04-trial.md` claimed the traversal-cap regression was covered in deleted `src/entities/trial.rs` unit tests after the refactor moved that coverage to `src/entities/trial/search/ctgov/tests.rs`. |
