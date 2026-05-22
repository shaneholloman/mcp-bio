# Explore — 369 Score variant-agent source APIs

## Spike Question

Which requested variant-agent source APIs are worth exposing in BioMCP as read-only public service/API proxies, versus leaving to the agent or rejecting, given current BioMCP capabilities and product boundaries?

Success means a source-by-source feasibility matrix, clear BioMCP vs agent-owned boundaries, repo-path evidence for current capabilities/gaps, auth/terms flags, and no more than 3–5 concrete follow-up tickets. This spike made no production code changes.

## Prior Art Summary

### Existing variant implementation

Repo evidence:
- `src/cli/variant/mod.rs` and `src/cli/variant/dispatch.rs` expose `search variant`, `get variant`, `variant trials`, `variant articles`, and token-gated `variant oncokb`.
- `src/entities/variant/resolution.rs` accepts exact `get` IDs only as rsID, genomic HGVS `chrN:g.posRef>Alt`, or gene + protein (`BRAF V600E`, `BRAF p.Val600Glu`). Transcript HGVS such as `NM_000248.3:c.135del` is deliberately unsupported today.
- `src/entities/variant/get.rs` uses `MyVariantClient` as the base resolver and gates optional sections: `clinvar`, `population`, `conservation`, `cosmic`, `cgi`, `civic`, `cbioportal`, `gwas`, `predict`.
- `src/entities/variant/search/mod.rs` and `src/sources/myvariant.rs` implement MyVariant Lucene filters over dbNSFP, ClinVar, gnomAD, COSMIC, CIViC, and related fields.
- `src/sources/gnomad.rs` is gene-constraint only, not exact allele lookup.
- `src/sources/clingen.rs` is gene validity/dosage only, not ClinGen Allele Registry.
- `src/sources/civic.rs`, `src/sources/cbioportal.rs`, and `src/sources/oncokb.rs` already cover selected curated sources as opt-in/direct helpers.
- `src/render/provenance.rs` emits section-level provenance; variant population is labeled `gnomAD via MyVariant.info`.
- `src/cli/list/molecular.rs` and `src/cli/health/catalog.rs` provide prose list/health readiness, but not machine-readable service capability discovery.

Design decisions to reuse:
- Strict input contracts and guardrail messages rather than local biological normalization.
- Fast MyVariant default path; slow/risky enrichments behind explicit sections/helpers.
- Source-labelled output and no-store caching for authenticated sources.

Design decisions to change/adapt:
- Add upstream normalization service proxies rather than teaching BioMCP to infer transcript/genomic/protein equivalence itself.
- Add direct gnomAD exact allele lookup only after normalized coordinate input is available; keep MyVariant gnomAD as a labeled fallback.
- Preserve COSMIC as indirect-only/rejected for direct integration.

### Existing article/literature implementation

Repo evidence:
- `src/cli/article/mod.rs` and `src/cli/article/dispatch.rs` expose `search article`, `get article`, `article entities`, `article batch`, `citations`, `references`, and `recommendations`.
- `src/entities/article/search.rs`, `backends.rs`, and `enrichment.rs` federate PubTator3, Europe PMC, PubMed, Semantic Scholar, and keyword-gated LitSense2 with source status and ranking metadata.
- `src/entities/article/detail.rs` accepts PMID, PMCID, and DOI; current DOI resolution depends on Europe PMC.
- `src/entities/article/fulltext.rs` already has a full-text ladder: NCBI ID Converter, Europe PMC PMC XML, NCBI EFetch PMC XML, PMC OA archive XML, Europe PMC MED XML, PMC HTML, and optional Semantic Scholar PDF only with `--pdf`.
- `src/sources/{pubmed,europepmc,pubtator,litsense2,semantic_scholar,ncbi_idconv,ncbi_efetch,pmc_oa}.rs` provide the existing source clients.
- There is no Crossref, OpenAlex, or Unpaywall client and no structured `article access` status aggregator.

Design decisions to reuse:
- Service-provided text snippets/annotations are in scope; local grep and PDF conversion/search workflow remains agent-owned.
- Optional source degradation is additive and visible via source status.
- Full text retrieval is public-source only and saves artifacts instead of inlining long bodies.

## Approaches Tried

Result files are committed under `architecture/experiments/369-score-variant-agent-source-apis/results/`; scripts are under `architecture/experiments/369-score-variant-agent-source-apis/scripts/`.

### 1. Current BioMCP baseline probes

Script: `scripts/probe_current_biomcp.py`  
Results: `results/current_biomcp_baseline.json`

Measured existing commands against the motivating examples.

| Probe | Result |
|---|---|
| `search variant MYD88 S219C` | Success, 1 row, top ID `chr3:g.38182032C>G`, ~868 ms |
| `search variant ERBB2 D277Y` | Success, 2 rows, top ID `chr17:g.37866662G>T`, ~814 ms |
| `search variant KLHL6 L65P` | Success, 1 row, top ID `chr3:g.183273248A>G`, ~827 ms |
| `get variant rs148924291 population` | Success, `KLHL6`, `p.L65P`, `gnomad_af=0`, provenance `gnomAD via MyVariant.info`, ~830 ms |
| `get variant NM_000248.3:c.135del` | Fails fast with unsupported variant format, ~14 ms |
| `search article -g MYD88 -k S219C` | Success, 3 rows, source status includes Semantic Scholar authenticated/ok, ~3.46 s |
| `get article 36053490 annotations` | Success, PubTator annotations and DOI `10.1002/hon.3073`, ~1.72 s |
| `get article 29695787 fulltext` | Success, saved full text via `NCBI EFetch PMC XML`, ~1.72 s |
| `get article 10.1200/JCO.2018.36.15_suppl.e24316` | Not found through current Europe PMC DOI route, ~951 ms |
| `list variant/article --json` | Commands only; no service/input capability metadata |

Conclusion: existing BioMCP already covers many article and MyVariant-backed variant needs, but gaps are exactly where the ticket predicted: transcript-HGVS normalization, direct exact allele population, DOI/conference metadata, article access status, and machine-readable service capability discovery.

### 2. Normalization/population API probes

Script: `scripts/probe_external_apis.py`  
Results: `results/external_api_probes.json`

| Service | Probe outcome | Implication |
|---|---|---|
| Mutalyzer | `NM_000248.3:c.135del` returned normalized cDNA and protein `NP_000239.1:p.(Asn46ThrfsTer4)` in ~484 ms | Strong first normalization proxy candidate |
| VariantValidator | MITF and ERBB2 transcript HGVS both returned GRCh38 loci plus explicit transcript-version warnings in ~530–604 ms | Strong first normalization proxy candidate, especially for mismatch warnings |
| NCBI Variation/SPDI | SPDI probe returned JSON with explicit reference mismatch warning in ~196 ms | Useful exact-allele primitive, but input must already be SPDI-like |
| ClinGen Allele Registry | MITF HGVS returned equivalent titles/alleles in ~204 ms but `@id` was blank-node `_:` rather than a clear stable CAid | Possible, needs endpoint/response-shape review |
| MyVariant.info | MYD88 S219C and KLHL6 rs148924291 resolved to genomic IDs/rsIDs; KLHL6 carried gnomAD fields | Already useful fallback; provenance must remain explicit |
| Ensembl VEP REST | RefSeq transcript HGVS probe failed with clear 400 | Useful only after genomic coordinate or supported transcript form is known |
| gnomAD direct GraphQL | guessed MYD88 exact allele returned `Variant not found` with HTTP 200 | Valuable but gated by normalization and exact build/allele contract |

Conclusion: first build slice should be Mutalyzer + VariantValidator. SPDI/ClinGen/gnomAD should follow once service capability discovery and normalized coordinate handling exist.

### 3. Literature/access metadata API probes

Script: `scripts/probe_external_apis.py`  
Results: `results/external_api_probes.json`

| Service | Probe outcome | Implication |
|---|---|---|
| PubMed | `MYD88 S219C` returned 7 PMIDs, first `36053490`, ~258 ms | Existing BioMCP PubMed route is valuable |
| Europe PMC | `MYD88 S219C` returned 49 hits, ~601 ms | Existing metadata route is valuable but ranks broader results differently |
| PubTator3 | PMID `36053490` annotations returned, ~142 ms | Existing annotation proxy is strong |
| LitSense2 | `KLHL6 L65P` returned 100 sentence hits, first PMID `29695787`, ~2.97 s | Good service-provided snippet/match candidate; keep source-labelled |
| Semantic Scholar | PMID `29967253` metadata returned, ~183 ms | Useful optional graph/access metadata; terms/rate gated |
| Crossref | ASCO DOI `10.1200/JCO.2018.36.15_suppl.e24316` returned title and DOI landing page, ~97 ms | Strong first DOI/conference metadata proxy |
| OpenAlex | Same ASCO DOI returned metadata and OA status `closed`, ~177 ms | Strong Crossref complement for OA/citation metadata |
| Unpaywall | ERBB2 CCR DOI returned `is_oa=false`, `oa_status=closed`, ~151 ms | Strong access-status component |

Conclusion: add Crossref/OpenAlex DOI metadata and Unpaywall/OpenAlex access status. Do not duplicate the existing fulltext ladder; expose structured availability/acquisition status around it.

### 4. Scored source matrix and boundary synthesis

Script: `scripts/synthesize_feasibility_matrix.py`  
Results: `results/source_feasibility_matrix.json`

Scores are 1–5 where 5 is best/lower-risk for BioMCP.

| Source | Area | Classification | Score |
|---|---|---:|---:|
| Mutalyzer | normalization | good BioMCP proxy candidate | 4.44 |
| VariantValidator | normalization | good BioMCP proxy candidate | 4.78 |
| NCBI Variation/SPDI | normalization | possible but gated | 4.22 |
| ClinGen Allele Registry | normalization | possible but gated | 4.11 |
| MyVariant.info | variant annotation/normalization fallback | good BioMCP proxy candidate | 4.56 |
| Ensembl VEP REST | annotation/normalization | possible but gated | 4.22 |
| gnomAD direct API | population | possible but gated | 3.89 |
| MyVariant.info gnomAD fallback | population | good BioMCP proxy candidate | 4.44 |
| PubMed/PMC/NCBI E-utilities | literature/access | good BioMCP proxy candidate | 4.67 |
| Europe PMC | literature/access | good BioMCP proxy candidate | 4.89 |
| PubTator3 | literature/text-mining | good BioMCP proxy candidate | 4.67 |
| LitSense2 | literature/text-mining | good BioMCP proxy candidate | 4.33 |
| Semantic Scholar | literature/graph/access metadata | possible but gated | 3.89 |
| Crossref | DOI/conference metadata | good BioMCP proxy candidate | 4.89 |
| OpenAlex | DOI/OA/citation metadata | good BioMCP proxy candidate | 4.78 |
| Unpaywall | access status | good BioMCP proxy candidate | 4.56 |
| CIViC | curated variant evidence | good BioMCP proxy candidate | 4.44 |
| ClinVar | curated variant evidence | possible but gated | 4.22 |
| cBioPortal | curated/cohort variant context | possible but gated | 3.67 |
| OncoKB | curated oncogenic/actionability | possible but gated | 3.44 |
| COSMIC | curated somatic variant evidence | reject/default-exclude | 1.78 |

## Decision

Promote a small follow-up set, ordered by product leverage and implementation risk:

1. **Add service capability discovery for source/input/status metadata.**  
   First slice: `biomcp list services --json` and selected `service <name> capabilities --json` for existing article/variant sources. This lets agents discover supported input forms before guessing.

2. **Add public variant normalization proxies.**  
   First slice: Mutalyzer + VariantValidator for transcript HGVS with typed statuses (`success`, `invalid_input`, `unsupported_notation`, `not_found`, `service_error`) and warnings. Preserve input and service output separately.

3. **Add DOI/conference metadata and article access-status proxies.**  
   First slice: Crossref + OpenAlex DOI lookup for conference abstracts and Unpaywall/OpenAlex access status. The ASCO DOI probe is the canary because current BioMCP misses it but both Crossref/OpenAlex find it.

4. **Add direct exact-allele population proxy only after normalization exists.**  
   First slice: gnomAD exact variant ID lookup with explicit `found`, `not_found`, `not_queryable`, and `service_error`; keep MyVariant gnomAD fields as a clearly labeled fallback.

Do not build workflow/interpretation features. Do not add direct COSMIC. Keep OncoKB explicit/token-gated and terms-sensitive.

## Boundary Classification

| Request | Owner/classification | Rationale |
|---|---|---|
| Messy report/entity cleanup | agent-owned | Agents parse prose and decide aliases to try. |
| Regex/pattern validation | BioMCP input contract | Use only for supported service input shapes and guardrail errors. |
| Biological normalization/equivalence | BioMCP only via upstream services | Proxy Mutalyzer/VariantValidator/SPDI/ClinGen; do not invent equivalence. |
| Source lookup/provenance/status | BioMCP | This is the core read-only service-proxy job. |
| Local PDF/Word/Markdown search | agent-owned | Agents use local tools after BioMCP retrieves allowed text/metadata. |
| Classification/oncogenicity/actionability | agent-owned | BioMCP returns source records, not clinical interpretation. |
| COSMIC direct integration | reject/default-exclude | Planning docs exclude it for licensing; keep indirect-only caution. |

## Outcome

**promote**

## Risks for Exploit

- Upstream API contracts may shift; exploit tickets must pin response shapes with fixtures and source docs.
- Variant normalization services can disagree. BioMCP should return source-specific results side-by-side, not collapse them into one asserted truth.
- gnomAD direct lookup depends on exact build/coordinate/allele IDs; normalization must precede it or `not_found` will be misleading.
- Unpaywall needs a real contact email policy before shipping.
- Crossref/OpenAlex metadata can identify abstracts without abstract body text; output must say `metadata_only` when full text/body is unavailable.
- OncoKB and Semantic Scholar remain terms/rate-sensitive; keep auth/status explicit and avoid making them mandatory.
- COSMIC must remain excluded except indirect provenance from aggregator payloads.
