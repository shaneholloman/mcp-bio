# Explore — Collect article fulltext miss fixtures for BioC renderer gate

## Spike Question

Can we identify and commit a small, representative set of PMIDs/PMCIDs/DOIs where BioMCP's current JATS/XML + PMC HTML fulltext ladder is missing, degraded, or lacks useful structure, while NCBI BioC/PubTator materially helps enough to justify a BioC-to-Markdown renderer gate?

Success means either a bounded fixture set of current-ladder miss/degradation cases with source mappings, request shapes, observed source kinds, license/reuse evidence, and insufficiency rationale — or a clear artifact that says no convincing cases were found. No BioMCP runtime behavior changes are in scope.

## Prior Art Summary

Ticket 381 already measured candidate source-first article Markdown sources. It found:

- Current Europe PMC/PMC JATS XML covered all sampled PMC/OA fulltext cases.
- NCBI BioC PMC matched that coverage and provided passage/license metadata, but did not add coverage.
- PubTator3 BioC JSON returned title/abstract plus annotations, not observed fulltext.
- PMC OA manifests strengthened license/retraction/package provenance, not rendering coverage.
- S2ORC belongs outside BioMCP runtime.

Current implementation review confirmed the production ladder remains:

1. NCBI ID Converter PMCID bridge.
2. Europe PMC PMC XML.
3. NCBI EFetch PMC XML.
4. PMC OA Archive XML.
5. Europe PMC MED XML.
6. PMC HTML.
7. Semantic Scholar PDF only with explicit `--pdf`.

Design decisions to preserve: structured XML/HTML before PDF, PDF opt-in, truthful license/reuse warnings, fixture-backed routine proof, and no PubTator-as-fulltext claim unless data proves it.

## Approaches Tried

Artifacts:

- Script: `architecture/experiments/384-collect-article-fulltext-miss-fixtures-for-bioc-renderer-gate/scripts/collect_bioc_miss_candidates.py`
- Detailed compact JSON: `architecture/experiments/384-collect-article-fulltext-miss-fixtures-for-bioc-renderer-gate/results/bioc_miss_candidate_probe.json`
- Matrix CSV: `architecture/experiments/384-collect-article-fulltext-miss-fixtures-for-bioc-renderer-gate/results/bioc_miss_candidate_matrix.csv`

The JSON records, for every candidate source call, the source ID mapping, request URL/query, source kind, status/content type, parse/fulltext signal, quality counts or BioC passage counts, license/reuse evidence when present, and a current-insufficiency classification. Full article bodies and archives were not committed.

### A. Prior-art regression/control set

Cases: `27083046`/`PMC4878868`, `17299597`/`PMC1790863`, `41807883`/`PMC12976322`, and non-PMC control `22663011`.

Measurements:

| Case | Current best | BioC fulltext | PubTator fulltext | Result |
|---|---|---:|---:|---|
| prior JATS verified | JATS/XML | yes | no | BioC coverage-equivalent, not a miss/degradation fixture. |
| prior BioC sample | JATS/XML | yes | no | BioC coverage-equivalent. |
| prior table article | JATS/XML | yes | no | Current XML already had table structure. |
| non-PMC control | miss | no | no | PubTator only title/abstract annotations. |

### B. Europe PMC non-OA / HTML-only candidate search

Query: `SRC:PMC AND OPEN_ACCESS:N`, first 4 unique records.

Measurements:

- Current best observed source: PMC HTML for all 4.
- NCBI BioC fulltext: 0/4.
- PubTator fulltext: not applicable; no PMID in those PMC-only records.
- Material BioC wins: 0/4.

Interpretation: this approach found the desired current-ladder degradation class (XML/OA archive absent, HTML present), but BioC did not supply fulltext for those cases.

### C. Non-OA MED fulltext search

Query: `SRC:MED AND HAS_FT:Y AND OPEN_ACCESS:N`, first 4 unique records.

Measurements:

- Current best observed source: PMC HTML for 3/4, miss for 1/4.
- NCBI BioC fulltext: 0/4.
- PubTator fulltext: 0/4; each response had only `title` and `abstract` passages, with annotations.
- Material BioC wins: 0/4.

Interpretation: current XML/HTML insufficiency existed, but BioC did not repair it. PubTator remained annotation enrichment, not fulltext.

### D. Open-access BioC-equivalence search

Query: `SRC:PMC AND HAS_FT:Y AND OPEN_ACCESS:Y`, first 4 unique records.

Measurements:

| Case | Current XML sections | Current XML tables | BioC passages | BioC license | Result |
|---|---:|---:|---:|---|---|
| `PMC13193167` | 9 | 4 | 137 | CC BY-NC-ND | Equivalent coverage; current JATS already structured. |
| `PMC13193163` | 16 | 0 | 77 | CC BY | Equivalent coverage; no current miss. |
| `PMC13189089` | 10 | 3 | 102 | CC BY | Equivalent coverage; current JATS already had tables. |
| `PMC13189218` | 5 | 5 | 120 | CC BY | Equivalent coverage; current JATS already had tables. |

Interpretation: BioC license/passage metadata is useful, but this search did not find a BioC-only renderer acceptance fixture. Current JATS/XML remained the better default source for structure.

## Decision

**NO-GO for an immediate BioC-to-Markdown renderer ticket from this evidence.**

Across 16 bounded cases:

- Material BioC wins over current XML/HTML: **0/16**.
- Current best observed source was structured JATS/XML for 7 cases, PMC HTML for 7 cases, and miss for 2 cases.
- NCBI BioC fulltext was available for 7 cases, and every one already had current structured JATS/XML coverage.
- Current XML/HTML insufficiency was observed in 9 cases, but BioC supplied fulltext for none of them.
- PubTator3 supplied fulltext for 0 cases; observed PMID responses were title/abstract plus annotations only.

Therefore there is no fixture-backed acceptance set for a renderer gate. Adding a renderer or source rung now would repeat ticket 381's unsupported default-ladder expansion risk.

Recommended source-order constraint if future evidence appears: BioC must stay out of the default ladder unless a committed fixture proves it wins after existing XML rungs and before/against PMC HTML for that exact degradation class. PubTator must remain annotation enrichment unless a documented endpoint returns fulltext in fixture-backed tests.

## Outcome

**kill** — kill the immediate BioC renderer/source-rung follow-up. Keep the collected compact candidate matrix as negative evidence. Reopen only if a specific PMID/PMCID/DOI is found where current XML/HTML misses or materially degrades and BioC provides fulltext with useful structure/license evidence.

## Risks for Exploit

- The sample is intentionally bounded; a broader live harvest could still find rare BioC-only wins.
- PMC HTML was treated as available by content type/status and compact excerpt, not full renderer-quality scoring.
- PMC OA archive XML was inferred from manifest tgz availability rather than downloading archives during this negative search.
- Live-source behavior can change; these results are spike evidence, not routine gate assertions.
- BioC passage structure differs from JATS. If a future concrete fixture wins, renderer work still needs its own deterministic fixture suite and source-order tests.
