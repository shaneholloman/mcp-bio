# Code Review Log

## Critique

- Read `.march/design-draft.md`, `.march/design-final.md`, `.march/code-log.md`, `.march/ticket.md`, and `git diff main..HEAD`.
- Re-ran the required local gates independently: `make check < /dev/null` and `make spec < /dev/null`.
- Design completeness audit:
  - Mapped the final design's runtime, docs, health, and proof-matrix items to the SEER source, disease entity/render/provenance, health/help surfaces, and source-doc inventory files.
  - Found two design-listed inventory surfaces with no matching code change: `docs/index.md` and `architecture/functional/overview.md`.
  - Confirmed from `.march/code-log.md` that help/docs/spec surfaces were updated before runtime implementation, which matches the mandatory code-step order.
- Test-design traceability audit:
  - Found no outside-in proof for the acceptance criterion that `biomcp get disease "Hodgkin lymphoma" survival` resolves to site `83`.
  - Found no regression proof for the SEER-unavailable note path required by the truthful-unavailable contract.
- Runtime/code review findings:
  - `biomcp get disease "Hodgkin lymphoma" survival` could resolve a weak contains-only non-Hodgkin hit and degrade to `SEER survival data not available for this condition.` instead of returning site `83`.
  - Canonical fallback misses during disease resolution could emit noisy warnings instead of degrading quietly to "no matching fallback row".
  - `make spec` exposed a real follow-up defect in `spec/21-cross-entity-see-also.md::Oncology Study Local Match`: the study matcher only handled exact or contiguous labels, so resolved names like `breast carcinoma` missed `Breast Invasive Carcinoma` and incorrectly fell back to `biomcp study download --list`.

## Fixes Applied

- Updated the missing design-listed inventory/help surfaces:
  - `docs/index.md`
  - `architecture/functional/overview.md`
  These now list `SEER Explorer` on the disease surface and use a survival example command.
- Repaired disease survival resolution in `src/entities/disease.rs`:
  - added Hodgkin alias query variants (`hodgkins lymphoma`, `hodgkin disease`)
  - scored direct candidates across all resolver query variants
  - rejected weak contains-only direct matches below a direct-match threshold so non-Hodgkin hits do not win a Hodgkin query
  - treated canonical fallback `NotFound` as `Ok(None)` so CML fallback misses do not leak warnings
  - added regression tests for alias expansion, weak-match rejection, unavailable-note degradation, and quiet canonical fallback misses
- Added the missing outside-in proof in `spec/07-disease.md` for Hodgkin survival mapping to site `83`.
- Repaired oncology study follow-up matching in `src/render/markdown.rs`:
  - added token-subset label matching so non-contiguous clinical labels can still resolve local study hints
  - added a regression test covering `breast carcinoma` against `Breast Invasive Carcinoma`

## Post-Fix Collateral Scan

- After the disease resolver changes, rechecked for dead code, unused imports/variables, stale error handling, and shadowing via `cargo clippy`, targeted tests, and live probes. No collateral issues remained.
- After the study-matcher change, rechecked the touched render path for stale fallback text, dead branches, and over-broad matching via targeted render tests and the failing spec slice. No new collateral issues remained.

## Verification

- Focused proofs:
  - `cargo test resolver_queries_adds_hodgkin_alias_variants -- --nocapture`
  - `cargo test scored_best_candidate_for_queries_prefers_hodgkin_alias_over_non_hodgkin_contains_match -- --nocapture`
  - `cargo test resolve_fallback_row_ignores_not_found_canonical_ids -- --nocapture`
  - `cargo test add_survival_section_sets_unavailable_note_when_catalog_fails -- --nocapture`
  - `cargo test related_disease_oncology -- --nocapture`
  - `uv run --extra dev sh -c 'PATH="$(pwd)/target/release:$PATH" pytest spec/21-cross-entity-see-also.md -k "Oncology and Study" --mustmatch-lang bash --mustmatch-timeout 120 -v'`
- Live-path checks:
  - `target/release/biomcp --json get disease "Hodgkin lymphoma" survival` returned site `83` with no `survival_note`
  - `target/release/biomcp get disease "chronic myeloid leukemia" survival` no longer emitted the fallback-resolution warning
- Docs/source-contract checks:
  - `uv run pytest tests/test_source_pages_docs_contract.py tests/test_source_licensing_docs_contract.py tests/test_documentation_consistency_audit_contract.py tests/test_upstream_planning_analysis_docs.py -v`
  - `uv run mkdocs build --strict`
- Full gates:
  - `make check < /dev/null` — passed
  - `make spec < /dev/null` — passed (`331 passed, 6 skipped`)

## Residual Concerns

- No remaining blocking defects found in scope.
- Verify should still expect normal live-provider drift risk from MyDisease and SEER Explorer because both integrations depend on external data/services; the repaired tests now cover the concrete failure modes seen during review.

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| 1 | stale-doc | no | Design-listed source inventory surfaces `docs/index.md` and `architecture/functional/overview.md` had no matching SEER update |
| 2 | missing-test | yes | Design acceptance/proof required an outside-in Hodgkin survival mapping proof for site `83`, but no matching spec assertion existed |
| 3 | missing-test | yes | The truthful-unavailable SEER note path had no regression test |
| 4 | validation-gap | no | Direct disease resolution accepted weak contains-only non-Hodgkin matches for a Hodgkin query, producing a false no-data survival result |
| 5 | error-classification | no | Canonical fallback `NotFound` surfaced as warning-producing failure instead of benign no-row degradation during disease fallback resolution |
| 6 | validation-gap | no | Oncology study matching only handled exact/contiguous labels, so `breast carcinoma` missed `Breast Invasive Carcinoma` and violated the study follow-up contract |
