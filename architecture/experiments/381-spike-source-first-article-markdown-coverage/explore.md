# Explore — Spike source-first article Markdown coverage

## Spike Question

Which additional free/open, non-PDF article sources materially improve BioMCP's source-first Markdown coverage and quality beyond the current JATS/XML and PMC HTML ladder, without adding heavyweight PDF dependencies or routine live-source fragility?

Success means a measured coverage/quality/provenance matrix, a recommendation for production BioMCP rungs vs Vault handoffs vs no-go sources, and follow-up work shaped around fixtures/request contracts rather than routine live checks.

## Prior Art Summary

BioMCP already has the right source-first skeleton:

1. NCBI ID Converter bridges PMID/DOI to PMCID when needed.
2. Fulltext resolution tries Europe PMC PMC XML, NCBI EFetch PMC XML, PMC OA archive XML, Europe PMC MED XML, PMC HTML, then Semantic Scholar PDF only with explicit `--pdf`.
3. Saved fulltext artifacts expose `full_text_path`, `full_text_source { kind, label, source }`, and a coarse miss note. Markdown shows `## Full Text (<label>)` plus `Saved to:` rather than inlining the article body.
4. The JATS renderer preserves title, abstract, section headings, paragraphs, figures/captions, lists, simple tables, citations/links, and references. HTML uses readability + `htmd` and is less structured.
5. PubTator3 is already a BioMCP client for search/annotations and has `export_biocjson(pmid)`, but today it hydrates article cards/annotations, not fulltext artifacts.
6. Source licensing docs already warn that fulltext reuse depends on provider and article-level terms; runtime does not enforce article licenses.

Design decisions to reuse: stable source labels/provenance, structured sources before PDF, PDF as opt-in last rung, bounded body/archive reads, fixture-backed routine proof, and truthful degradation.

## Approaches Tried

Measured with:

- Script: `architecture/experiments/381-spike-source-first-article-markdown-coverage/scripts/source_coverage_probe.py`
- Raw JSON: `architecture/experiments/381-spike-source-first-article-markdown-coverage/results/source_coverage_probe.json`
- Compact CSV: `architecture/experiments/381-spike-source-first-article-markdown-coverage/results/source_coverage_matrix.csv`

Small set:

| Case | Resolved IDs | Why |
|---|---|---|
| `jats_verified_pmid` | PMID `27083046`, PMCID `PMC4878868` | Existing JATS verification article. |
| `ncbi_bioc_sample` | PMID `17299597`, PMCID `PMC1790863` | Official NCBI BioC PMC sample. |
| `non_pmc_control` | PMID `22663011`, no PMCID | Prior no-PMCID/PDF-control article. |
| `title_lookup_only` | title query resolved to PMID `41807883`, PMCID `PMC12976322` | Exercises title-derived lookup and a table-containing article. |

### A. Current Europe PMC XML baseline

For the three PMCID/OA cases, Europe PMC `fullTextXML` by PMCID succeeded and preserved the key JATS quality signals:

| Case | Sections | Paragraphs | Tables | References |
|---|---:|---:|---:|---:|
| `27083046` / `PMC4878868` | 21 | 212 | 0 | 48 |
| `17299597` / `PMC1790863` | 29 | 75 | 0 | 33 |
| title lookup / `PMC12976322` | 41 | 145 | 3 | 404 |

Europe PMC core metadata also exposed `isOpenAccess`, `license`, `fullTextIdList`, and `fullTextUrlList` for the OA cases.

### B. NCBI BioC PMC REST

Endpoint shape tested: `pmcoa.cgi/BioC_json/{PMID-or-PMCID}/unicode`.

Results:

- Covered the same three PMCID/OA cases as current Europe PMC XML.
- Accepted both PMID and PMCID for those cases.
- Returned structured passages (`paragraph`, `title_*`, `abstract`, `fig_caption`, `ref`, and `table` for the table-containing case).
- Returned license data (`CC BY` plus license prose).
- Returned no entity annotations in this API output.
- Did not cover the non-PMC control.

Comparison to prior art: BioC did not increase small-set fulltext coverage beyond the existing PMCID-driven XML ladder. Its value is a possible fallback renderer and clearer license/passage metadata, not a proven new first rung.

### C. PubTator3 BioC JSON export

Endpoint shape tested from existing BioMCP client: `/publications/export/biocjson?pmids=...`.

Results:

- Covered all four PMIDs, including the non-PMC control.
- Returned title + abstract passages and entity annotations.
- Did not return full text in the observed endpoint.
- `pmcids=` was rejected (`pmids is a mandatory parameter`) by the tested PubTator3 endpoint; legacy `pubtator-api` behaved the same in a quick check.

Comparison to prior art: This reuses existing client capability, but remains an annotation/abstract enrichment source rather than a fulltext Markdown rung.

### D. PMC OA manifest / package metadata

Endpoint shape tested: `oa.fcgi?id=<PMCID>`.

Results:

- Covered all three PMCID/OA cases.
- Exposed `license=CC BY`, `retracted=no`, citation, and tgz archive link.
- Did not itself add a new renderer; it strengthens provenance and batch harvesting.

Comparison to prior art: BioMCP already uses PMC OA tgz/XML as one XML rung. The follow-up opportunity is surfacing manifest fields and quality/reuse flags, not adding another content path.

### E. Europe PMC metadata alternatives

Europe PMC `resultType=core` search was useful for every case:

- PMID and title lookup resolved IDs.
- OA records carried `license`, `isOpenAccess`, `fullTextIdList`, and `fullTextUrlList`.
- The non-PMC control had `isOpenAccess=N` and no fulltext coverage.

This is the lowest-risk production-adjacent win: keep the existing content ladder, but add fixture-backed provenance/quality/license flags from metadata already reachable through BioMCP's Europe PMC client.

### F. S2ORC / Semantic Scholar dataset JSON

Lightweight dataset metadata confirmed `s2orc` and `s2orc_v2` exist in the latest release (`2026-05-21`) and are full-body JSON datasets from open-access PDFs with ODC-BY dataset terms plus article-level OA info.

Fit:

- BioMCP runtime: poor. This is a bulk dataset/API-key download shape, not an on-demand per-article source rung.
- Vault/batch: possible if Ian accepts storage, licensing, and update policy.

## Decision

Winner for immediate BioMCP production follow-up: **pivot to provenance/quality flags over the existing source ladder**, not a new fulltext rung yet.

Rationale from measured data:

- No tested source increased fulltext coverage beyond the current XML ladder on the small set.
- NCBI BioC PMC is the best fallback candidate, but in this sample it duplicated Europe/PMC XML coverage while losing JATS-specific richness unless a new BioC renderer is built.
- PubTator3 BioC JSON adds valuable entity annotations for title/abstract, but it is not a fulltext source in the tested endpoint.
- Europe PMC core metadata and PMC OA manifests add concrete provenance/reuse fields now: license, OA status, fulltext URLs/IDs, retraction, package link.
- S2ORC belongs outside BioMCP runtime.

Recommended follow-up sequence:

1. **Build: article fulltext provenance/quality flags from existing metadata.** Fixture-backed only. Surface fields such as source kind, has_sections, has_tables, has_references, license_present, license/reuse warning, OA/retracted state when known.
2. **Spike/build: BioC renderer only after collecting fixtures where BioC succeeds and current XML/HTML misses or degrades.** If no such fixtures are found, keep BioC out of the ladder.
3. **Build: PubTator annotation manifest enrichment, not fulltext.** If useful for Vault, expose title/abstract entity annotation coverage beside saved fulltext provenance.
4. **Architect/Vault: S2ORC batch ingestion handoff.** Keep bulk dataset download/indexing outside BioMCP runtime.

Routine-gate strategy: future build tickets should use request-contract fixtures and renderer fixtures. Live broad-source checks should stay in release/operator smoke lanes and should not become default PR/spec gates.

## Outcome

**pivot**

Do not promote a direct "add BioC/PubTator fulltext rung" exploit from this spike alone. Promote a smaller provenance/quality-flags slice first; treat BioC as a conditional fallback candidate needing miss/degradation fixtures.

## Risks for Exploit

- Small sample may miss articles where BioC succeeds and Europe/EFetch/PMC OA XML fails.
- BioC passage types are not JATS; a renderer needs its own fixture suite and quality contract.
- License fields vary by provider and article. BioMCP should report context/warnings, not certify reuse safety.
- PubTator endpoint semantics may differ across old/new APIs; production should pin request contracts before relying on any export shape.
- Table/reference quality flags must be computed from rendered/parsed structure, not just inferred from source names.
- Live-source behavior is time-sensitive; do not encode these live measurements as routine assertions.
