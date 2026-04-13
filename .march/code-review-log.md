# Code Review Log — Ticket 186

## Critique

### Design completeness audit

- `RUN.md` now documents the narrowed pre-commit contract in `## Pre-Merge Checks`, including the enforced commands, the explicit "does not run" list, the heavier manual proofs, and the `git commit --no-verify` escape hatch.
- `architecture/technical/overview.md` now names `.march/validation-profiles.toml` as the source of record, lists all five reserved profiles, maps the current build-flow usage, and explains why `full-blocking` uses `make spec-pr` while `full-contracts` stays declared-but-unassigned.
- `.march/validation-profiles.toml` exists, is git-tracked, declares exactly the five reserved profiles, and records `# observed ...` timing comments above each table.
- `tests/test_validation_profile_contract.py` covers the tracked-file and TOML contract for the new profile file.
- `tests/test_upstream_planning_analysis_docs.py` covers the runbook and architecture-doc contract for the new validation-tier and hook documentation.
- The shared hook change has no hunk in `git diff main..HEAD` because the hook lives at the git-resolved shared path outside the tracked worktree: `/home/ian/workspace/repos/biomcp/.git/hooks/pre-commit`. I verified that file directly; this matches the design constraint and was intentional.

### Test-design traceability

- Proof-matrix coverage is present for the repo-owned contract surface:
  - runbook hook contract doc: `tests/test_upstream_planning_analysis_docs.py`
  - architecture validation-tier doc: `tests/test_upstream_planning_analysis_docs.py`
  - validation profile file tracking, shape, commands, and timing comments: `tests/test_validation_profile_contract.py`
  - shared hook path/content/behavior: direct structural proofs against `git rev-parse --git-path hooks/pre-commit`
- No `spec/*.md` change was required. The ticket changes repo tooling and docs, not CLI behavior, so the existing Python contract-test pattern is the correct outside-in proof surface for this change.

### Finding

1. `tests/test_upstream_planning_analysis_docs.py` did not fully encode the design's current build-flow mapping. The new test only checked for selected step names and command substrings, so it would not fail if the `01-design` / `02-design-review` "no profile" text drifted or if a profile row paired the wrong command with the wrong current-use column.

## Fixes

- Strengthened `test_validation_profile_and_hook_contract_docs_are_pinned` to require:
  - the explicit `01-design` / `02-design-review` no-profile language
  - the exact five profile rows, including each command and current-use column
- Post-fix collateral scan:
  - no new dead code or unreachable branches
  - no unused imports or variables
  - no stale error messages or shadowed names introduced by the test change

## Verification

- `uv run pytest tests/test_upstream_planning_analysis_docs.py tests/test_validation_profile_contract.py tests/test_directory_submission_contract.py::test_repo_cleanup_removes_local_artifacts_and_deleted_dirs_from_git tests/test_public_install_docs_contract.py::test_ticketed_blog_install_blocks_put_curl_before_uv_and_pip tests/test_documentation_consistency_audit_contract.py::test_blog_try_it_and_install_copy_are_consistent -q`
- `hook_path="$(git rev-parse --git-path hooks/pre-commit)"; test -f "$hook_path"`
- `hook_path="$(git rev-parse --git-path hooks/pre-commit)"; rg -n "cargo fmt --check|cargo clippy --lib --tests -- -D warnings" "$hook_path" && ! rg -n "cargo test" "$hook_path"`
- `hook_path="$(git rev-parse --git-path hooks/pre-commit)"; bash "$hook_path"`
- `sh -c 'cargo test --lib && cargo clippy --lib --tests -- -D warnings'`

The review intentionally did not rerun `make check && make spec-pr && make test-contracts`; the code-step save point already recorded that full runtime gate, and this review step was instructed to avoid rerunning it.

## Residual Concerns

- The shared hook remains a local/shared-git artifact rather than a tracked repo file. Verify should re-check the resolved hook path if the shared workspace changed after this review.
- No out-of-scope follow-up issues were filed from this review.

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| 1 | weak-assertion | no | The overview doc-contract test only checked loose substrings, so it would not catch drift in the explicit no-profile steps or the exact profile-to-command-to-usage mapping required by the design. |
