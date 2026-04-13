## Review Scope

- Reviewed `.march/design-draft.md`, `.march/design-final.md`, `.march/code-log.md`, `.march/ticket.md`
- Rebased onto `main` (`GIT_EDITOR=true git rebase main`)
- Inspected the full branch diff with `git diff main..HEAD`
- Re-ran the changed-surface gates:
  - `cargo test cli --lib`
  - `cargo test --lib && cargo clippy --lib --tests -- -D warnings`

## Critique

### Design Completeness Audit

- `design-final` acceptance criteria 1-10 were implemented in the branch diff. The runtime seam moved into `src/cli/outcome.rs`, `src/cli/mod.rs` re-exported the public seam, runtime ownership moved into the family modules, and the documented help/spec contract remained unchanged.
- The proof-matrix owners in `.march/design-final.md:331-342` all had matching code and proof coverage in the branch diff and existing test/spec surface.
- One design item was incomplete in the implementation that was handed to review:
  - `.march/design-final.md:279-280` and `.march/design-final.md:290-294` require targeted smoke tests in each owning family test home before and during runtime extraction. Those direct parse-and-dispatch proofs were missing for the extracted `article`, `disease`, `pathway`, `protein`, `pgx`, `gwas`, `phenotype`, `trial`, and `system` owners.
- Documentation and contract audit:
  - No design item added or changed a shipped help/spec/doc contract.
  - `.march/code-log.md:1-6` records the required execution order: tests/spec surface first, runtime extraction second, targeted proofs after each batch. That matched the intended contract-first flow.

### Test-Design Traceability

- The proof-matrix cases in `.march/design-final.md:333-342` all had matching tests/specs in the repo and in `.march/code-log.md:30-32,77-82`.
- The missing coverage was not in the proof matrix rows themselves; it was in the implementation-plan requirement for owner-local smoke tests after the runtime split. Without those tests, several extracted dispatch entrypoints had no direct regression proof in their own modules.
- Review finding:
  - Missing owner-local smoke proofs for extracted runtime families were blocking because the design explicitly required them as the outside-in guardrail for the refactor.

### Quality Review

- Implementation quality: the extraction follows adjacent Rust module conventions and keeps runtime ownership local to each family. No duplicate helper abstraction was introduced.
- Security: no new untrusted-input path, shell, query-construction, or file-write risk was introduced by the refactor or by the review fixes.
- Performance: the review fixes only add tests; no runtime-path cost changed.

Mark phase complete in checkpoint state:
- `Critique — documented all issues, checked spec coverage for outside-in behavior`

## Fix Plan

- Add bounded smoke tests in each missing family test home that:
  - parse the extracted command via `Cli::try_parse_from(...)`
  - call the family-owned runtime entrypoint directly
  - assert the documented fail-fast validation or pagination guardrail before any backend work
- Keep the fixes local to the owning modules and avoid widening visibility or changing the shipped CLI contract.

Mark phase complete in checkpoint state:
- `Fix plan — all issues have fixes`

## Repair

### Fixes Applied

- Added direct owner-local smoke coverage for the extracted runtime entrypoints:
  - `src/cli/article/tests.rs:81`
  - `src/cli/disease/mod.rs:167`
  - `src/cli/pathway/mod.rs:191`
  - `src/cli/protein/mod.rs:107`
  - `src/cli/pgx/mod.rs:87`
  - `src/cli/gwas/mod.rs:73`
  - `src/cli/phenotype/mod.rs:49`
  - `src/cli/trial/mod.rs:458`
  - `src/cli/system/mod.rs:298`
- Collateral repair during validation:
  - Corrected the expected limit-range assertions in the new `pgx`, `gwas`, and `phenotype` tests to `1..=50`, which is the actual runtime contract enforced by those entities.

### Post-Fix Collateral Damage Scan

- Dead code: none introduced; all new tests exercise live entrypoints.
- Unused imports/variables: fixed import sets where the new parser-based tests required `Parser`, `Commands`, or `SearchEntity`.
- Resource cleanup conflicts: none; no cleanup logic changed.
- Stale error messages: corrected the three limit-range assertions to match the actual error text.
- Shadowing: none introduced.

### Validation

- `cargo test cli --lib` passed.
- `cargo test --lib && cargo clippy --lib --tests -- -D warnings` passed.
- `cargo fmt` applied after the test additions.

### Residual Concerns

- None. No out-of-scope issues were identified that warranted a separate issue file.

Mark phase complete in checkpoint state:
- `Repair — fixes applied, wrote code-review-log.md`

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| 1 | missing-test | no | `.march/design-final.md` required targeted owner-local smoke tests for the extracted family runtimes, but direct dispatch proofs were missing for `article`, `disease`, `pathway`, `protein`, `pgx`, `gwas`, `phenotype`, `trial`, and `system`. |
| 2 | collateral-damage | yes | The first review-added `pgx`, `gwas`, and `phenotype` smoke tests asserted the wrong limit range (`1..=100`); validation exposed the mismatch and the assertions were corrected to the shipped `1..=50` contract. |
