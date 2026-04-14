## Review Scope

- Read `.march/ticket.md`, `.march/design-draft.md`, `.march/design-final.md`, and `.march/code-log.md`
- Rebased onto `main` with `GIT_EDITOR=true git rebase main` and confirmed the branch was already up to date
- Reviewed `git diff main..HEAD`, `git log --oneline main..HEAD`, and `git show --stat --summary --format=fuller HEAD`
- Inspected every changed file under `src/transform/article/` plus the facade `src/transform/article.rs` and `tests/article_transform_structure.rs`
- Re-ran the relevant proof surface during critique:
  - `cargo test transform::article --lib`
  - `cargo test transform::article::jats --lib`
  - `cargo test article --lib`
  - `cargo clippy --lib --tests -- -D warnings`
  - `cargo test --test article_transform_structure`

## Critique

### Design Completeness Audit

- `design-final.md` contains no `Needs change` markers, so the audit covered the verified scope, acceptance criteria, proof matrix, and execution-order notes.
- The implementation matched the structural design:
  - `src/transform/article.rs` is a small facade with the expected private modules and re-exports.
  - `src/transform/article/` contains the focused `anchors`, `annotations`, `federation`, and `jats` modules plus their sidecar tests and `jats/refs.rs`.
  - no file under `src/transform/article/` exceeds 700 lines.
  - every new Rust file in the split starts with a `//!` module header.
  - no empty `ranking`, `calibration`, or `types` module was introduced.
  - no new dependency files were touched (`git diff --name-only main..HEAD -- Cargo.toml Cargo.lock` was empty).
- The preserved caller surface also matched the design:
  - verified caller paths still use `crate::transform::article::*` from `backends.rs`, `detail.rs`, `enrichment.rs`, `ranking.rs`, and `ranking/tests/calibration.rs`.
  - `cargo test article --lib` passed, so those caller paths still compile and behave on the article surface.
- Docs/help/spec review:
  - this ticket does not change shipped behavior, so no user-facing docs/help/spec text was expected to change.
  - the required contract-adjacent documentation for this refactor is the new module-level `//!` headers, and those landed with the new files.
  - `.march/code-log.md` records the structural proof being added before the runtime split, so there is no docs/spec ordering defect to fix.

### Test-Design Traceability

- All 25 moved unit tests required by the design are present at the expected destinations:
  - `anchors/tests.rs`: 5 tests
  - `annotations/tests.rs`: 2 tests
  - `federation/tests.rs`: 11 tests
  - `jats/tests.rs`: 7 tests
- The JATS-specific proof matrix row is satisfied by the dedicated `transform::article::jats` test subset, and those 7 tests passed.
- The structure proof exists as `tests/article_transform_structure.rs`, which is stronger than the draft's ad hoc script because it is committed regression coverage.
- Outside-in spec coverage remains unchanged, which is correct for this ticket because the refactor is organizational only.

### Findings

1. The design promises the stable `crate::transform::article::*` facade, but there was no direct smoke test for the full root re-export surface. Current callers exercised most exports indirectly, but `truncate_abstract` and `truncate_authors` had no direct root-path proof.
2. `tests/article_transform_structure.rs` checked that the expected files existed and stayed under the 700-line limit, but it did not fail if forbidden placeholder modules or unexpected extra Rust files appeared under `src/transform/article/`.

### Quality Review

- Security: no new shell, filesystem, auth, query-construction, or data-exposure surfaces were introduced. The moved code still only parses upstream data and renders text.
- Duplication: the helpers that exist elsewhere in the repo (`truncate_utf8`, `decode_html_entities`, `collapse_whitespace`) were not newly invented here; they were moved from the original `article.rs` body and remain scoped to the article transform tree.
- Performance: no algorithmic changes were introduced in the runtime path. The review fixes are proof-only and do not affect ranking, federation, or JATS logic.

## Fix Plan

- Add a crate-internal smoke test that typechecks the full stable `crate::transform::article::*` surface, including the two currently unused root re-exports.
- Tighten `tests/article_transform_structure.rs` so it asserts the exact Rust file layout under `src/transform/article/` and explicitly rejects the stale placeholder module names the design forbids.

## Repair

- Added `transform::article::tests::root_module_reexports_stable_article_transform_api` in [src/transform/article.rs](/home/ian/workspace/worktrees/191-decompose-transform-article-rs-into-ranking-submodule/src/transform/article.rs:39) to compile-check all 15 stable root exports.
- Strengthened [tests/article_transform_structure.rs](/home/ian/workspace/worktrees/191-decompose-transform-article-rs-into-ranking-submodule/tests/article_transform_structure.rs:23) to:
  - compare the actual recursive Rust file set under `src/transform/article/` to the expected layout
  - reject forbidden placeholder paths for `ranking`, `calibration`, and `types`
  - reuse the actual recursive file list for the 700-line limit check

### Post-Fix Collateral Damage Scan

- After adding the root smoke test:
  - no dead imports or shadowed bindings were introduced
  - the targeted unit test compiled and passed
- After tightening the structure regression:
  - the first attempt exposed an ordering mismatch between expected and recursive path collection
  - I fixed that by sorting the expected path list to match the recursive collector
  - the final structure test passed with no leftover dead code or stale assertions

### Validation

- `cargo fmt`
- `cargo fmt --check`
- `cargo test --lib && cargo clippy --lib --tests -- -D warnings` (`focused` profile)
- `cargo test --test article_transform_structure`

### Residual Concerns

- No blocking concerns remain.
- Verify should keep running `cargo test --test article_transform_structure` when the article module layout changes; the repo's `focused` profile is `--lib`-only and does not include that integration test automatically.
- No out-of-scope follow-up issue file was needed.

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| 1 | missing-test | no | The design promised an exact stable `crate::transform::article::*` facade, but there was no direct smoke test for the full root re-export surface, leaving `truncate_abstract` and `truncate_authors` unproven at the facade path. |
| 2 | weak-assertion | no | `tests/article_transform_structure.rs` proved required files existed and stayed small, but it did not reject forbidden placeholder modules or unexpected extra Rust files under `src/transform/article/`. |
