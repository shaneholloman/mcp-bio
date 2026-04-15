# Code Review Log — Ticket 210

Date: 2026-04-15
Ticket: `210-add-study-cli-help-descriptions-and-regression-test`

## Phase 1 — Critique

### Design Completeness Audit

I traced the `design-final.md` help-contract items to the actual change set:

| Design area | Matching code/spec change | Result |
|---|---|---|
| All nine `study` subcommands get one-line help text | `src/cli/study/mod.rs` variant doc comments; `src/cli/study/tests/help.rs::study_help_lists_descriptions_for_all_subcommands`; `spec/13-study.md::Study Help` | Implemented |
| Key flag help for `query`, `filter`, `compare`, `co-occurrence`, `survival`, `download`, `top-mutated`, and `cohort` | `src/cli/study/mod.rs` field doc comments; targeted assertions in `src/cli/study/tests/help.rs`; executable assertions in `spec/13-study.md` | Implemented |
| Canonical values and accepted aliases for `query --type`, `compare --type`, and `survival --endpoint` | `src/cli/study/mod.rs`; `src/cli/study/tests/help.rs`; `spec/13-study.md::Study Help` | Implemented |
| New Rust regression module and module wire-up | `src/cli/study/tests/help.rs`; `src/cli/study/tests.rs` | Implemented |
| Executable spec coverage for the changed help contract | `spec/13-study.md` intro row + `## Study Help` section | Implemented |
| Contract-first execution order | `.march/code-log.md` step order: spec, then Rust tests, then runtime help strings | Implemented |

External docs already matched the intended wording closely enough that no extra doc edits were required:

- `docs/reference/quick-reference.md`
- `docs/user-guide/cli-reference.md`

I also verified `.march/code-log.md` updated spec/tests before the runtime clap help strings.

### Test-Design Traceability

The original implementation had the runtime help text, but several proofs were weaker than the acceptance contract:

1. `study_survival_help_describes_endpoint_values_and_aliases` and the study spec did not assert the `--study` help text required by acceptance criterion 4.
2. `study_compare_help_describes_type_and_target` and the study spec did not fully pin the `--study` / `--gene` help contract required by acceptance criterion 5.
3. `study_co_occurrence_help_describes_gene_list_contract` and the study spec did not assert the `--study` help text required by acceptance criterion 6.
4. `study_download_help_describes_list_and_study_id` and the study spec only checked generic wording, not the positional `study_id` contract / `required unless --list` requirement from acceptance criterion 7.

### Other Quality Checks

- Security: no untrusted-input flow changes, shell execution changes, path handling changes, or data exposure regressions were introduced by this ticket.
- Duplication: no unnecessary helper or abstraction was introduced; the new help tests follow the existing `Cli::command()` / `write_long_help()` pattern used elsewhere in the repo.
- Implementation quality: the runtime help text in `src/cli/study/mod.rs` matched the design copy and left parsing/dispatch behavior unchanged, which was correct for the ticket scope.

## Phase 2 — Fix Plan

1. Tighten the Rust help regression tests so the changed acceptance criteria are explicitly asserted on stable help phrases.
2. Tighten `spec/13-study.md` so the executable outside-in spec covers the same missing study/help/positional requirement details.
3. Re-run targeted study proofs and the repo's focused validation profile.

## Phase 3 — Repair

### Fixes Applied

- Updated `src/cli/study/tests/help.rs` to assert:
  - `cBioPortal study ID` in `survival`, `compare`, and `co-occurrence` help
  - `[STUDY_ID]` plus `required unless --list` in `download` help
- Updated `spec/13-study.md` to assert:
  - `--study <STUDY>` and `cBioPortal study ID` for `survival`
  - `--study <STUDY>`, `--gene <GENE>`, `cBioPortal study ID`, and the gene description for `compare`
  - `--study <STUDY>` and `cBioPortal study ID` for `co-occurrence`
  - `required unless --list` for `download`

### Post-Fix Collateral Damage Scan

After each edit I checked the touched surface for:

- dead code or orphaned imports: none introduced
- resource cleanup conflicts: not applicable; no cleanup logic changed
- stale error/help text: none introduced; assertions were aligned to the actual stable clap rendering
- shadowed variables: none introduced

### Validation

Passed:

- `cargo test --lib cli::study::tests::help -- --nocapture`
- `cargo test --lib short_help_hides_chart_flags_but_long_help_shows_them`
- `XDG_CACHE_HOME="$PWD/.cache" uv run --extra dev sh -c 'PATH="$PWD/target/release:$PATH" RUST_LOG=error pytest spec/13-study.md --mustmatch-lang bash --mustmatch-timeout 60 -v'`
- `cargo test --lib && cargo clippy --lib --tests -- -D warnings`

No out-of-scope follow-up issue was filed.

## Residual Concerns

None. The remaining help assertions now rely on stable phrases/tokens rather than clap spacing, so the proof surface is materially stronger without being brittle.

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| 1 | weak-assertion | no | Survival help proofs omitted the `--study` help contract required by the design acceptance criteria |
| 2 | weak-assertion | no | Compare help proofs omitted part of the `--study` / `--gene` help contract required by the design acceptance criteria |
| 3 | weak-assertion | no | Co-occurrence help proofs omitted the `--study` help contract required by the design acceptance criteria |
| 4 | weak-assertion | no | Download help proofs omitted the positional `study_id` requirement text (`required unless --list`) required by the design acceptance criteria |
