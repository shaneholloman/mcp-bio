## Executive Summary

Article fulltext is architecturally stuck in one `detail.rs` JATS/XML waterfall:
the renderer hardcodes `PMC OA`, the cache key cannot distinguish extractor
families, the spike's HTML converter is GPL-blocked, and the existing
Semantic Scholar PDF metadata is never used for recovery. The target keeps the
current production JATS renderer, extracts fulltext resolution into an
article-local module, adds truthful source provenance and source-aware cache
keys, introduces an explicit `--pdf` last-resort path for open-access PDFs, and
adds a narrow PMC article-page HTML fallback with a permissive converter. The
path is three independently shippable tickets.

## Survey Issues Addressed

- **Issue 1: `html2md` is GPL-blocked**
  - Ticket 255 adds the repo `cargo deny` allowlist gate.
  - Ticket 257 ships a permissive HTML dependency set and keeps `html2md` out of
    the production graph.
- **Issue 2: JATS extractor duplication is a false problem**
  - Ticket 255 keeps `src/transform/article/jats.rs` as the canonical renderer
    and builds the resolver around it instead of importing the spike JATS path.
- **Issue 3: No HTML source exists in the production fulltext pipeline**
  - Ticket 257 adds one concrete HTML source: the PMCID-derived PMC article
    page, fetched only after XML misses.
- **Issue 4: PDF bytes must be fetched from somewhere**
  - Ticket 256 adds an explicit `--pdf` path that uses
    `semantic_scholar.open_access_pdf.url` only after XML misses.
- **Issue 5: Hardcoded `## Full Text (PMC OA)` template label**
  - Ticket 255 adds `ArticleFulltextSource` and makes markdown/provenance use the
    actual winning source label.
- **Issue 6: `FULLTEXT_CACHE_VERSION` is extraction-algorithm-coupled**
  - Ticket 255 makes the cache key extractor-family-specific so JATS/HTML/PDF
    outputs cannot collide.

## Target Architecture

### Article Fulltext Resolver Boundary

- **Current:** `src/entities/article/detail.rs` owns article identity
  resolution, Semantic Scholar enrichment, the XML waterfall, the JATS render
  worker, the cache key, and fulltext note generation in one function.
- **Target:** `src/entities/article/fulltext.rs` owns XML/HTML/PDF acquisition,
  fallback order, bounded fetches, and source-aware cache/provenance. The
  article facade keeps identity resolution and delegates fulltext work to the
  new module.
- **Key changes:** add `ArticleFulltextKind`, `ArticleFulltextSource`, and
  `Article.full_text_source`; move the current XML waterfall and save/cache
  logic out of `detail.rs`; make `templates/article.md.j2`,
  `src/render/markdown/article.rs`, and `src/render/provenance.rs` render the
  actual source instead of hardcoded `PMC OA`; add repo-root `deny.toml` and
  `cargo deny check licenses` to the lint gate.
- **Invariants:** production `src/transform/article/jats.rs` remains the only
  shipped JATS renderer; XML stays the default fulltext path; `Saved to:` stays
  the CLI contract; cache keys include the extractor family.

### Opt-In PDF Fallback

- **Current:** `semantic_scholar.open_access_pdf.url` is displayed in the article
  card but never used for fulltext extraction.
- **Target:** `biomcp get article <id> fulltext --pdf` enables one explicit
  last-resort PDF fetch after XML (and later HTML) failure, using the existing
  Semantic Scholar open-access URL.
- **Key changes:** add `--pdf` to `src/cli/article/mod.rs`; introduce
  `ArticleGetOptions { allow_pdf }`; add `src/transform/article/pdf.rs`; fetch
  PDF bytes from the article fulltext resolver, not `src/sources/semantic_scholar.rs`;
  use `unpdf` with a 12-page bound; label successful output as `Semantic Scholar PDF`.
- **Invariants:** default `get article <id> fulltext` does not attempt PDF;
  `pdf_oxide` stays out of the first production path; PDF is only tried when
  the user opts in and a PDF URL exists.

### PMC HTML Fallback

- **Current:** no production HTML source is wired, and the spike's HTML path
  depends on GPL `html2md`.
- **Target:** when XML misses and a PMCID exists, BioMCP derives
  `https://pmc.ncbi.nlm.nih.gov/articles/{pmcid}/`, fetches the PMC article
  page, and renders it through a permissive HTML stack before the opt-in PDF
  branch.
- **Key changes:** add `src/transform/article/html.rs`; ship `readability-rust`
  plus an audited permissive HTML-to-Markdown converter; add bounded PMC HTML
  fetch logic in `src/entities/article/fulltext.rs`; commit focused HTML
  fixtures for PMC plus two non-production spike pages as converter-quality
  guards.
- **Invariants:** HTML remains an internal fallback, not a new section name;
  PMCID-derived PMC pages are the only initial production HTML source; arbitrary
  publisher/preprint HTML remains out of scope for this migration.

## Ticket Sequence

| # | ID | Name | Addresses | Dependencies | Priority | Status |
|---|-----|------|-----------|-------------|----------|--------|
| 1 | 255 | Add article fulltext resolver boundary and license gate | Issues 1, 2, 5, 6 foundation | none | 8 | ready |
| 2 | 256 | Add opt-in PDF fallback for article fulltext | Issue 4, plus source-aware CLI/cache/provenance follow-through | 255-add-article-fulltext-resolver-boundary-and-license-gate | 8 | draft |
| 3 | 257 | Add PMC HTML fallback for article fulltext | Issues 1 and 3 HTML path | 256-add-opt-in-pdf-fallback-for-article-fulltext | 5 | draft |

## Doc Updates

- Added `architecture/technical/article-fulltext-markdown.md`.
  - Documents the current survey problems.
  - Defines the target article fulltext resolver, JATS/HTML/PDF boundaries,
    PMCID-derived PMC HTML fallback, explicit PDF policy, license posture,
    regression fixtures, invariants, and ticket decomposition.
- Rewrote `.march/blueprint.md` for ticket 250 with the target architecture,
  actual ticket IDs, statuses, assumptions, and risk framing.
- No team-specific `planning/biomcp/planning/quality-bar.md`, strategy, or
  frontier doc exists in this checkout, so alignment was checked against
  `architecture/functional/overview.md` and
  `architecture/technical/source-integration.md` instead.

## Assumptions

- **Assumption:** The existing production JATS renderer is strictly better than
  the spike JATS path and should remain canonical.
  - **Basis:** The survey compared `src/transform/article/jats.rs` and the spike
    crate and found production has stronger reference and `xref` handling.
  - **Validation:** Ticket 255 preserves the current JATS test/spec contract
    while extracting the resolver boundary.
  - **Fallback:** If the refactor exposes a missing helper, copy only the needed
    spike logic into production tests or helpers, not a second JATS renderer.

- **Assumption:** PDF should stay explicit behind `--pdf`.
  - **Basis:** The spike proved bounded PDF fallback is useful, but it is slower
    and noisier than XML; the repo's progressive-disclosure contract prefers
    opt-in expensive sections and modifiers.
  - **Validation:** Ticket 256 adds the named modifier and proves the default
    fulltext path remains unchanged.
  - **Fallback:** If Ian or a lead wants automatic PDF fallback later, make that
    a follow-up ticket that updates help, specs, latency expectations, and
    source-order tests explicitly.

- **Assumption:** PMCID-derived PMC article pages are the right first HTML source.
  - **Basis:** The spike already validated a PMC article page fixture, and PMCID
    is existing article metadata that deterministically maps to one PMC page URL.
  - **Validation:** Ticket 257 adds the PMC fixture path plus bounded fetch
    checks and keeps the non-production HTML fixtures green.
  - **Fallback:** If PMC article pages prove too unstable, keep the HTML module
    fixture-backed only and leave runtime HTML disabled while a narrower source
    ticket is designed.

- **Assumption:** `unpdf` is sufficient as the first shipped PDF engine.
  - **Basis:** The spike results favored `unpdf` on two of the three PDF quality
    cases and it keeps the initial production dependency surface smaller.
  - **Validation:** Ticket 256 proves the article-grade PDF fixture and keeps the
    additional spike PDFs as regression guards.
  - **Fallback:** If the article-grade fixture or live recovery path fails, add a
    separate follow-up ticket for `pdf_oxide` or engine arbitration instead of
    broadening ticket 256.

- **Assumption:** Adding `cargo deny` to the lint gate is acceptable repo
  overhead for this migration.
  - **Basis:** License compliance is a hard blocker for the HTML/PDF crates, and
    the foundation ticket can land the allowlist before adding new dependencies.
  - **Validation:** Ticket 255 makes the allowlist executable in the repo lint
    gate and documents the local tool requirement.
  - **Fallback:** If local-tool friction is too high, keep `deny.toml` and move
    the enforcement step to CI while preserving the same allowlist and ticket
    scopes.

## Risk Assessment

- Refactoring the current XML path into `src/entities/article/fulltext.rs` can
  accidentally change the waterfall order or note text. Ticket 255 should keep
  focused detail tests on the existing order and saved-file contract before
  touching HTML/PDF work.
- `--pdf` adds a named modifier to an article `get` path that currently uses a
  positional sections list. Ticket 256 must prove the modifier is parsed as a
  flag, not swallowed as another section token.
- PDF URLs point at third-party content and can be slow or large. The target
  keeps PDF opt-in, bounded, and clearly labeled so the default path remains
  safe.
- PMC article pages can change shape more often than PMC XML. Ticket 257 should
  keep committed HTML fixtures and treat non-PMC pages as converter-quality
  tests rather than production sources.
- `cargo deny` can fail on local environments that do not yet have `cargo-deny`
  installed. Ticket 255 should pair the gate with explicit setup docs instead of
  a silent skip.

Rollback strategy:

- Ticket 255 can be reverted independently: the old `detail.rs` XML path is
  fully recoverable and no new extractor dependencies are added yet.
- Ticket 256 can be reverted while keeping the fulltext resolver boundary,
  truthful labels, cache keys, and license gate from ticket 255.
- Ticket 257 can be reverted independently if PMC HTML fallback proves noisy;
  the explicit PDF path from ticket 256 remains available for users who opt in.

## Open Questions

- None block tickets 255-257. The remaining expansion questions are post-target:
  whether to add non-PMC HTML sources after the PMC path lands, and whether a
  second PDF engine ever justifies the extra dependency surface.
