## Review Scope

- Read `.march/ticket.md`, `.march/design-draft.md`, `.march/design-final.md`, and `.march/code-log.md`
- Rebased onto `main` with `GIT_EDITOR=true git rebase main`
- Reviewed the full branch diff with `git diff main..HEAD` and the branch commits `c2c2aa7`, `93ac89f`, `aa3c236`, `45c8b4c`, and `dc04e7c`
- Re-ran the proof surface used by this ticket:
  - `cargo test cli::variant --lib`
  - `cargo test cli::trial --lib`
  - `cargo test cli::system --lib`
  - `cargo test cli::tests::facade --lib`
  - `cargo test cli::tests::next_commands --lib`
  - `cargo test cli::tests::outcome --lib`
  - `cargo test cli --lib`
  - `cargo fmt --check`
  - `cargo clippy --lib --tests -- -D warnings`
  - `make check < /dev/null`
  - `cargo build --release --locked`
  - `make spec`

## Critique

### Design Completeness Audit

- `src/cli/mod.rs` is a thin facade. `rg -n '^(const|fn|async fn|struct|enum|type) ' src/cli/mod.rs` returns no matches, and `wc -l src/cli/mod.rs` reports `62`.
- `src/cli/shared.rs` exists and owns the shared helpers called out in the design: CLI construction, query normalization, alias fallback, pagination helpers, and shared JSON rendering.
- The family-local helper moves all landed in the owning dispatch modules:
  - `src/cli/article/dispatch.rs`
  - `src/cli/disease/dispatch.rs`
  - `src/cli/drug/dispatch.rs`
  - `src/cli/gene/dispatch.rs`
  - `src/cli/pathway/dispatch.rs`
  - `src/cli/study/dispatch.rs`
  - `src/cli/system/dispatch.rs`
  - `src/cli/trial/dispatch.rs`
  - `src/cli/variant/dispatch.rs`
- Every family listed in the design now declares `#[cfg(test)] mod tests;`, and the expected sidecar test files exist under `src/cli/*/tests.rs`.
- `src/cli/tests/` owns the remaining cross-family CLI tests via `facade`, `outcome`, `next_commands_validity`, and `next_commands_json_property`.
- `src/cli/test_support.rs` remains the shared CLI unit-test helper module.
- No shipped help/spec/doc contract changed in this ticket. That matches the design intent for a structural-only move, and the fresh locked release build plus `make spec` confirmed the user-visible contract still holds.

Finding:

- Design item "no CLI file exceeds the cap" has no ticket-scoped matching change and is not true for this repository even on `main`. The proof command in `.march/design-final.md` uses `rg --files src/cli -g '*.rs' | xargs wc -l`, but that still reports unrelated pre-existing files over 700 lines, including:
  - `src/cli/search_all.rs` at `2829`
  - `src/cli/health.rs` at `2513`
  - `src/cli/benchmark/run.rs` at `1352`
  - `src/cli/list.rs` at `1074`
  - `src/cli/skill.rs` at `840`
  - `src/cli/benchmark/score.rs` at `837`
  - `src/cli/cache.rs` at `817`
- The moved ticket surface does satisfy the cap, including `src/cli/mod.rs` (`62`), `src/cli/shared.rs` (`328`), `src/cli/variant/dispatch.rs` (`678`), `src/cli/trial/tests.rs` (`550`), and the split cross-family sidecars.

### Test-Design Traceability

- The proof-matrix test homes all exist and match the intended ownership:
  - Variant relocation: `src/cli/variant/tests.rs`
  - Trial relocation: `src/cli/trial/tests.rs`
  - System/runtime/cache relocation: `src/cli/system/tests.rs` and `src/cli/tests/facade/*.rs`
  - Cross-family next-command and output contracts: `src/cli/tests/next_commands_validity.rs`, `src/cli/tests/next_commands_json_property/*`, and `src/cli/tests/outcome.rs`
- The targeted proof commands all passed on this branch.
- The tests are asserting behavior rather than only compilation. Examples:
  - `src/cli/tests/facade/help.rs` checks hidden runtime globals and top-level help text directly.
  - `src/cli/tests/facade/cache.rs` checks plain-text vs JSON cache behavior and CLI-only wording.
  - `src/cli/article/tests.rs`, `src/cli/drug/tests.rs`, `src/cli/disease/tests.rs`, `src/cli/trial/tests.rs`, and `src/cli/variant/tests.rs` assert concrete JSON fields, error text, and parser/runtime guardrails for the moved helpers.
- The existing spec files named in the proof matrix all remain the outside-in contract, and the full `make spec` run passed against a freshly built release binary.

Finding:

- No missing design-required test or weak moved-surface assertion was found. The traceability audit passed.

### Quality Review

- Implementation quality: the move follows adjacent Rust module patterns, keeps helper ownership local, and does not widen visibility beyond what the sidecar tests need.
- Duplication: no new abstraction duplicates an existing repo helper. `shared.rs` consolidates previously root-local helpers instead of creating a second implementation.
- Security: no new untrusted input flow into file paths, shell commands, or queries was introduced. The cache/MCP local-boundary behavior remained green in both Rust tests and `spec/15-mcp-runtime.md` plus `spec/22-cache.md`.
- Performance: this is structural movement only. I did not find a new runtime-path regression or avoidable algorithmic cost in the touched code.

Mark phase complete in checkpoint state:

- `Critique — documented all issues, checked spec coverage for outside-in behavior`

## Fix Plan

- No implementation repair is needed in the moved CLI code. The code move, tests, and runtime/spec gates are green.
- The only defect from this review is the stale/overbroad design proof item for the global 700-line check. That is outside ticket 185's touched surface, so the repair for this step is:
  - document it here
  - file a follow-up issue under `~/workspace/planning/biomcp/issues/`

Mark phase complete in checkpoint state:

- `Fix plan — all issues have fixes`

## Repair

- No `src/` code changes were required during review.
- Wrote this review log.
- Filed `/home/ian/workspace/planning/biomcp/issues/185-cli-line-cap-proof-command-too-broad.md` for the out-of-scope proof/design mismatch.

### Post-Fix Collateral Damage Scan

- No code fix was applied in this review, so there was no new branch, import, cleanup path, error message, or variable shadowing risk introduced by the review itself.

### Validation

- All targeted Rust proof commands passed.
- `cargo test cli --lib`, `cargo fmt --check`, and `cargo clippy --lib --tests -- -D warnings` passed.
- `make check < /dev/null` passed.
- `cargo build --release --locked` passed.
- `make spec` passed against the freshly built release binary.

### Residual Concerns

- Verify should treat the global 700-line proof mismatch as a design artifact issue, not as a regression in the ticket 185 implementation.

Mark phase complete in checkpoint state:

- `Repair — fixes applied, wrote code-review-log.md`

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| 1 | stale-doc | yes | `.march/design-final.md` proves "no CLI file exceeds the cap" with `rg --files src/cli -g '*.rs' | xargs wc -l`, but that command still fails on unrelated pre-existing files on `main`; ticket 185 only repaired the moved facade/sidecar surface. |
