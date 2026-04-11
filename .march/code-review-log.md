# Code Review Log

## Critique

- Read `.march/ticket.md`, `.march/design-draft.md`, `.march/design-final.md`, `.march/code-log.md`, and `git diff main..HEAD`.
- Re-ran the required gates independently before repairs: `make check < /dev/null` and `make spec < /dev/null`.
- Design completeness audit:
  - Mapped every `Needs change`, acceptance criterion, and proof-matrix row in `.march/design-final.md` to runtime code, docs, tests, specs, and contract checks.
  - Found stale contract docs: `docs/user-guide/drug.md` did not document the new stale EMA/WHO local-data statuses, and `architecture/technical/source-integration.md` still described EMA as the only local runtime source and omitted the WHO `--apis-only` exclusion and stale-state contract.
  - Found missing WHO proof coverage: no unit coverage for the WHO health status matrix (`configured`, `available`, `configured (stale)`, `available (default path, stale)`, `not configured`, missing-file error), no WHO render/provenance/search-JSON proof, no explicit MCP-description assertion that `who sync` stays CLI-only, and no outside-in spec proof that `get drug ... safety|shortage --region who` rejects while `get drug ... all --region who` remains valid.
  - Found runtime/contract drift: `src/sources/who_pq.rs` still advertised `WHO_PQ_SIZE_HINT = "~1 MB"` instead of the designed/live `~134 KB`.
  - Found a lifecycle proof gap: `tests/who_pq_auto_sync.rs` did not cover missing-file re-download on the next WHO search.
  - Confirmed from `.march/code-log.md` that docs/help/spec surfaces were updated before runtime edits, which matches the required execution order, but the surfaces above were still incomplete and therefore blocking.
- Test-design traceability audit:
  - Verified the existing WHO proof rows for plain-name search, structured search, sync lifecycle, parser normalization, source inventory/licensing, and source-guide nav.
  - Missing traceability rows were the WHO health-state matrix, WHO render/provenance/search-JSON proof, MCP read-only description proof, and outside-in WHO-only section validation/all-section behavior.

## Fixes Applied

- Repaired stale docs and contract tests:
  - Updated `docs/user-guide/drug.md` to document stale EMA and WHO local-data statuses.
  - Updated `architecture/technical/source-integration.md` to describe EMA and WHO as local runtime sources, document `BIOMCP_WHO_DIR`, and state that `biomcp health --apis-only` excludes both local rows.
  - Updated `tests/test_upstream_planning_analysis_docs.py` so the planning/doc contract now enforces the new WHO local-runtime wording and stale-status help text.
- Repaired WHO runtime and proof coverage:
  - Corrected `src/sources/who_pq.rs` to use the designed/live `WHO_PQ_SIZE_HINT` value of `~134 KB`.
  - Added WHO health tests in `src/cli/health.rs` for default-path available/stale, configured available/stale, not configured, and missing-file error outcomes.
  - Added the missing WHO auto-sync recovery proof in `tests/who_pq_auto_sync.rs` for missing-file re-download on the next search.
  - Refactored `src/entities/drug.rs` structured WHO paging through a helper so the stop-after-one-extra-row and exact-total-on-exhaustion behaviors are directly testable, and added validation tests for `safety|shortage --region who` rejection plus `all --region who` acceptance.
  - Added WHO render, provenance, and search-JSON regression tests in `src/render/markdown.rs`, `src/render/provenance.rs`, and `src/cli/mod.rs`.
  - Added an MCP contract assertion in `tests/test_mcp_contract.py` that `who sync` remains absent from the read-only MCP description.
  - Added outside-in WHO specs in `spec/05-drug.md` for unsupported WHO safety/shortage sections and supported WHO `all` output.
- Repaired post-fix gate defects:
  - Fixed the new WHO spec block to use a non-trivial `mustmatch like` literal so `check-quality-ratchet` passes.
  - Stabilized the NCI trial fallback tests in `src/entities/trial.rs` by extracting an injected-client helper, moving the affected tests off shared environment variables, and exposing test-only constructors in `src/sources/mydisease.rs` and `src/sources/nci_cts.rs`.
  - Pinned `spec/22-cache.md` to `min_disk_free = "1B"` in the cache-warning fixture so it deterministically tests the `max_size` warning path instead of inheriting the host filesystem's ambient disk-floor state.

## Post-Fix Collateral Scan

- After the WHO doc/runtime/test repairs, rechecked the touched files for dead code, unused imports, stale error messages, cleanup conflicts, and shadowing with `cargo fmt`, `cargo clippy`, focused tests, and the full gates. No dead code or stale error paths remained in the repaired WHO paths.
- The first repaired `make check` rerun exposed two collateral issues:
  - `spec/05-drug.md` introduced a too-short `mustmatch like` literal that failed `check-quality-ratchet`.
  - The full suite exposed an env-coupled NCI trial fallback test that was stable in isolation but flaky under `make check`.
- The first repaired `make spec` rerun exposed one additional environment-sensitive spec defect:
  - `spec/22-cache.md::Cache Health Warning` relied on the default 10% disk floor, so on a host already below that threshold the warm-up search auto-cleaned the cache before the warning assertion ran.
- All three collateral issues were fixed and re-verified before the final gate reruns.

## Verification

- Focused proofs:
  - `cargo test who_ -- --nocapture`
  - `uv run pytest tests/test_upstream_planning_analysis_docs.py tests/test_mcp_contract.py -q`
  - `cargo test nci_search_page_ -- --nocapture`
  - `cargo test nci_status_mapping_uses_documented_single_value_filters -- --nocapture`
- Full gates:
  - `make check < /dev/null` — passed after repairs (`1439` Rust tests green plus integration/doc tests)
  - `make spec < /dev/null` — passed after repairs (`338 passed, 6 skipped`)

## Residual Concerns

- No remaining blocking defects found in scope.
- No out-of-scope follow-up issues were filed from this review pass.

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| 1 | stale-doc | no | `docs/user-guide/drug.md` and `architecture/technical/source-integration.md` did not reflect the shipped WHO/EMA stale local-data health contract |
| 2 | missing-test | yes | The WHO health proof-matrix row required configured/default/stale/not-configured/error coverage, but that unit coverage was missing |
| 3 | missing-test | yes | WHO render/provenance/search-JSON proof required by the design was not present in the test suite |
| 4 | missing-test | yes | The WHO read-only contract lacked an explicit `who sync` MCP exclusion assertion, and the drug specs lacked outside-in WHO safety/shortage rejection and WHO `all` behavior coverage |
| 5 | stale-doc | no | `WHO_PQ_SIZE_HINT` drifted from the designed/live upstream contract (`~1 MB` vs `~134 KB`) |
| 6 | missing-test | yes | WHO auto-sync lifecycle coverage omitted the missing-file re-download scenario |
| 7 | weak-assertion | yes | The new WHO spec used a too-short `mustmatch like` literal and failed the quality ratchet |
| 8 | collateral-damage | no | Full-suite reruns exposed env-coupled NCI trial fallback tests that were flaky under `make check`; they were stabilized with injected test clients |
| 9 | collateral-damage | no | `spec/22-cache.md::Cache Health Warning` depended on the host's ambient disk-floor state and could self-clean before asserting the intended `max_size` warning path |
