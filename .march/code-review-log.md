# Code Review Log — ticket 383 Add article fulltext provenance and quality flags

## Critique Summary

Reviewed `.march/ticket.md`, `.march/design-draft.md`, `.march/design-final.md`, `.march/code-log.md`, the full diff against `main`, and the changed Rust/docs/spec surfaces.

### Design Completeness Audit

Initial implementation covered the primary manifest contract: `Article.full_text_manifest`, JATS/HTML/PDF resolver population, Europe PMC license/open-access threading, JATS quality flags, deterministic fixture/spec assertions, and docs updates.

One design-completeness defect was found and fixed: `design-final.md` required PMC OA archive/package provenance when that rung wins, and required retraction provenance only when backed by reliable source fields. The implementation did not expose PMC OA archive manifest metadata and emitted `retracted: false` even when no publication-type/retraction field was present.

### Test-Design Traceability

Forward traceability passed. Every proof-matrix row has a landed assertion/support change:

- JATS/XML manifest assertions in `spec/surface/cli.md` and `spec/entity/article.md`.
- PMC HTML unknown-license manifest assertions in both spec files.
- Explicit PDF fallback manifest assertions in both spec files.
- Fixture support for JATS sections/tables/references, Europe PMC OA/license metadata, unknown-license HTML, and Semantic Scholar PDF license.
- Existing saved-file/PDF opt-in assertions remain in `spec/entity/article.md`.

Reverse traceability passed. The only new shipped-contract assertions are the design-approved manifest assertions and fixture support. No shipped-contract assertions were removed, relaxed, or invented by the code step.

### Edit Discipline Audit

Estimated minimal size was the additive manifest structs, resolver metadata threading, JATS quality helper, source metadata deserialization, fixture/spec additions, and docs. Actual changes stayed on the design-named surfaces. The review repair added only the missing PMC OA/retraction provenance threading and a PMCID trim on the new manifest identifier. No over-edit defect remains.

### Quality Checks

- **Implementation quality:** Follows existing resolver/source-client conventions and preserves resolver order and legacy `full_text_source` fields.
- **Test quality:** Specs assert user-observable JSON contract through deterministic fixtures, not implementation internals.
- **Performance:** JATS quality extraction is one cheap parse; HTML/PDF stay conservative; no new routine live calls or heavyweight dependencies.
- **Data completeness:** Fixed PMC OA package provenance and reliable-only retraction provenance. License/reuse unknowns remain explicit.
- **Security:** Manifest serializes trimmed upstream identifiers/licenses/URLs as data only; no API keys, headers, fixture temp roots, or secret-derived values are emitted.
- **Duplication:** Searched manifest/quality/reuse/package/provenance terms; no pre-existing article fulltext manifest helper existed.

## Repairs Applied

- Added `PmcOaArchiveManifest` and `get_full_text_xml_with_manifest` so PMC OA archive winners can carry package URL, optional OA license, and optional OA retraction metadata into `full_text_manifest.provenance`/`reuse`.
- Threaded PMC OA manifest metadata through the XML winner path without changing resolver order.
- Changed Europe PMC detail retraction threading to `None` when no publication-type/retraction evidence exists, avoiding an overconfident `retracted: false` manifest field.
- Trimmed the HTML PMCID manifest identifier before serialization.
- Split the previously uncommitted code implementation into `build: add article fulltext manifest implementation`, leaving the review repair as a separate commit as required.

## Validation Rerun

- `cargo check --all-targets` — passed.
- `cargo test -p biomcp-cli sources::pmc_oa::tests -- --nocapture` — 4 passed.
- `make spec-contracts` — 55 passed.
- `PATH="$PWD/target/release:$PATH" BIOMCP_BIN="$PWD/target/release/biomcp" uv run --no-sync pytest spec/entity/article.md --mustmatch-lang bash --mustmatch-timeout 180 -v` — 8 passed.
- `cargo test --lib && cargo clippy --lib --tests -- -D warnings` — 2007 tests passed; clippy passed.
- `make check` — passed (`cargo nextest`: 2086 passed, 1 skipped; `pytest tests/`: 251 passed; `mkdocs build --strict`: passed).
- `git diff --check` — passed.

Note: one `make spec-contracts` attempt timed out after I had manually started the article fixture without its normal test-block cleanup trap, leaving the fixture lock held. I killed that manual fixture process and reran the lane successfully.

## Residual Concerns

None requiring a follow-up issue.

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| 1 | stale-doc | no | Design/docs promised PMC OA package/retraction provenance when available, but runtime discarded the OA archive manifest after extracting XML. Fixed by exposing and threading `PmcOaArchiveManifest` into `full_text_manifest`. |
| 2 | stale-doc | no | Design said `provenance.retracted` is populated only from reliable source fields, but runtime emitted `retracted: false` even when Europe PMC supplied no publication-type/retraction field. Fixed by keeping detail retraction provenance absent unless source evidence exists. |
