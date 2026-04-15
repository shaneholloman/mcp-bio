# Code Review Log - Ticket 207

## Critique

Reviewed the ticket and implementation inputs:

- Read `.march/design-draft.md`, `.march/design-final.md`, `.march/code-log.md`, and the full `git diff main..HEAD`.
- Reviewed commit history for the code step (`eab1009c`, `e68aafe1`).
- Re-ran the relevant local gates:
  - `cargo audit -q`
  - `cargo test --lib`
  - `cargo build --release --locked`
  - `make spec-pr`
- Re-checked the current semver-lane heads with dry runs:
  - `cargo update -p tar --dry-run`
  - `cargo update -p rustls-webpki --dry-run`
  - `cargo update -p rand@0.9.4 --dry-run`
  - `cargo update -p rand@0.10.1 --dry-run`
  - `cargo update -p rand@0.8.5 --dry-run`

### Design Completeness Audit

Design items traced to the implementation and proof surface:

- Targeted lockfile updates only:
  - matched by `Cargo.lock` updates for `tar 0.4.45`, `rustls-webpki 0.103.12`, `rand 0.9.4`, and `rand 0.10.1`
  - confirmed no `Cargo.toml` edit was needed
- Warning reduction where patch updates exist:
  - matched by the `rand` lane updates in `Cargo.lock`
  - confirmed `cargo audit -q` now leaves only `bincode`, `instant`, `rustls-pemfile`, and `rand 0.8.5`
- Self-update proof gap closure:
  - matched by new tests in `src/cli/update.rs`
  - traced to `extract_binary_from_targz_returns_matching_binary_bytes`
  - traced to `extract_binary_from_targz_rejects_empty_binary`
  - traced to `extract_binary_from_targz_reports_missing_binary_as_not_found`
- Existing PMC OA proof:
  - matched by existing tests in `src/sources/pmc_oa.rs`
- Existing cBioPortal proof:
  - implementation already had direct archive-install tests in `src/sources/cbioportal_download.rs`
  - this surfaced one documentation defect: the design artifact pointed at `spec/13-study.md` for archive extraction proof, but that spec uses fixture-backed installed-study data and does not exercise download/extraction
- Execution order:
  - confirmed from `.march/code-log.md` and commit order that the unit-test addition landed before the lockfile update
  - no user-facing docs/help/spec change was required because the public contract did not change

### Test-Design Traceability

Proof matrix coverage after review:

- `cargo audit -q` proves the `tar` advisories are cleared.
- `cargo audit -q` proves the `rustls-webpki` advisory is cleared.
- `cargo audit -q` proves warning reduction and the exact residual warning set.
- `src/cli/update.rs` unit tests prove the self-update tar.gz extraction helper still works.
- `src/sources/pmc_oa.rs` tests prove PMC OA tar extraction still works.
- `src/sources/cbioportal_download.rs` tests prove cBioPortal archive download/install still works:
  - `download_study_installs_archive_into_root`
  - `download_study_rejects_entries_outside_expected_top_level_directory`
- `make spec-pr` proves the PR-blocking user-visible study/spec lanes still stay green, including `spec/13-study.md` for the installed-study CLI surface.

### Implementation Quality Review

- No shipped-code defects were found in the `Cargo.lock` or `src/cli/update.rs` diff after reviewing behavior, assertions, and error classification.
- The new update tests assert behavior, not just reachability:
  - happy path checks exact extracted bytes
  - failure paths check the exact error variant and message/suggestion contract
- Security review found no new injection, traversal, auth, or secret-handling risks in the changed surface.
- Duplication review found adjacent local tar/gzip helpers in other archive tests, but no existing shared helper that the new `src/cli/update.rs` tests should obviously reuse.

## Fix Plan

Fix the stale internal proof mapping so the design artifact matches the actual executable proof surface:

1. Update `.march/design-final.md` to cite `src/sources/cbioportal_download.rs` lib tests as the cBioPortal archive proof.
2. Clarify that `spec/13-study.md` remains the installed-study CLI proof, not archive download/extraction proof.

## Repair

Applied the fix:

- Updated `.march/design-final.md` to replace the incorrect `spec/13-study.md` archive-proof mapping with the existing `src/sources/cbioportal_download.rs` lib tests.
- Clarified the spec-coverage notes so the study spec and archive-install tests are described accurately.

### Post-Fix Collateral Scan

The fix was documentation-only. Re-read the edited section for:

- stale references: fixed
- dead code / unused symbols: not applicable
- resource cleanup conflicts: not applicable
- stale error messages: not applicable
- shadowed variables: not applicable

No collateral issues were introduced.

## Verification

- `cargo audit -q`: exit `0`; residual warnings are `bincode`, `instant`, `rustls-pemfile`, and `rand 0.8.5`
- `cargo test --lib`: passed (`1491 passed`)
- `cargo build --release --locked`: passed
- `make spec-pr`: passed (`222 passed, 4 skipped`; `99 passed, 2 skipped`)
- `cargo update --dry-run` checks: all targeted crates reported `Locking 0 packages to latest Rust 1.93.1 compatible versions`

## Residual Concerns

- `rand 0.8.5` still warns in `cargo audit -q`; the current lockfile lane has no newer patch release.
- Verify should continue treating cBioPortal archive extraction proof as the lib tests in `src/sources/cbioportal_download.rs`, not as `spec/13-study.md`.
- No out-of-scope follow-up issue was filed from this review.

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| 1 | stale-doc | no | `.march/design-final.md` incorrectly cited `spec/13-study.md` as proof of the cBioPortal archive extraction path; the real executable proof is the existing lib tests in `src/sources/cbioportal_download.rs`. |
