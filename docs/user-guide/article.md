# Article

Use article commands for literature retrieval by disease, gene, drug, and identifier.

## Typical article workflow

1. search a topic,
2. choose an identifier,
3. retrieve default summary,
4. request full text or annotations only when needed.

## Search articles

By gene and disease:

```bash
biomcp search article -g BRAF -d melanoma --limit 5
```

By keyword:

```bash
biomcp search article -k "immunotherapy resistance" --limit 5
```

Tune keyword-bearing relevance:

```bash
biomcp search article -k "Hirschsprung disease ganglion cells" --ranking-mode hybrid --weight-semantic 0.5 --weight-lexical 0.2 --limit 5
```

By date:

```bash
biomcp search article -g BRAF --since 2024-01-01 --limit 5
```

By year range:

```bash
biomcp search article -k "BRAF melanoma" --year-min 2000 --year-max 2013 --limit 5
```

Exclude preprints when supported by source metadata:

```bash
biomcp search article -g BRAF --since 2024-01-01 --no-preprints --limit 5
```

## Query formulation

Turn a natural-language literature question into two parts:

- Put a known gene, disease, or drug in `-g/--gene`, `-d/--disease`, or `--drug`.
- Put mechanisms, phenotypes, outcomes, datasets, and other free-text concepts in `-k/--keyword`.
- If the question is asking which gene, disease, or drug fits the evidence and you do not know the entity yet, do not guess a typed flag. Start with keyword-only article search or run `biomcp discover "<question>"` first.
- Use `--type review` for synthesis questions, list-style questions, and dataset surveys.

Known anchor only:

```bash
biomcp search article -g BRAF --limit 5
```

Known anchor plus mechanism or process:

```bash
biomcp search article -g TP53 -k "apoptosis gene regulation" --limit 5
```

Unknown-entity disease-identification question:

```bash
biomcp search article -k '"cafe-au-lait spots" neurofibromas disease' --type review --limit 5
```

Known drug plus mechanism:

```bash
biomcp search article --drug amiodarone -k "photosensitivity mechanism" --limit 5
```

Dataset or method question:

```bash
biomcp search article -k "TCGA mutation analysis dataset" --type review --limit 5
```

### Multi-source federation

Article search fans out to PubTator3, Europe PMC, and PubMed by default when
the filter set is compatible. Known gene, disease, and drug anchors
participate in that typed route. When a non-empty keyword is present, BioMCP
also adds LitSense2 to the federated route. Semantic Scholar can still join
the same query when the filter set is compatible. BioMCP merges duplicates
across PMID, PMCID, and DOI where possible. `S2_API_KEY` upgrades the Semantic
Scholar leg to authenticated requests at 1 req/sec; without it, BioMCP uses
the shared unauthenticated pool at 1 req/2sec. Search results are still
deduplicated by PMID when BioMCP can resolve one.

Default `--sort relevance` is mode-aware:

- Keyword-bearing queries default to `--ranking-mode hybrid`, using
  `0.4*semantic + 0.3*lexical + 0.2*citations + 0.1*position` with the
  LitSense2-derived semantic signal.
- Entity-only queries default to `--ranking-mode lexical`, preserving the
  existing calibrated PubMed rescue plus lexical directness comparator.
- `--ranking-mode semantic` sorts the LitSense2-derived semantic signal first
  and falls back to the lexical comparator for deterministic ties.
- Rows without LitSense2 provenance contribute `ranking.semantic_score = 0`
  in semantic-aware ranking modes.
- `--weight-semantic`, `--weight-lexical`, `--weight-citations`, and
  `--weight-position` retune the hybrid formula.

Markdown preserves the merged rank order, and JSON includes row-level
`matched_sources`, `ranking`, `citation_count`, and
`influential_citation_count`.

Use `--source <all, pubtator, europepmc, pubmed, litsense2>` to select one
backend or keep the default federated search.
BioMCP caps each federated source's contribution after deduplication and before
ranking. Default: 40% of `--limit` on federated pools with at least three
surviving primary sources. Rows count against their primary source after
deduplication. Use `--max-per-source <N>` to override that cap, use
`--max-per-source 0` for the default cap explicitly, and set it equal to
`--limit` to disable capping.
Default article search excludes confirmed retractions unless you pass
`--include-retracted`. Sources that do not expose retraction metadata still
participate in the search, and JSON search rows keep the tri-state contract:
`"is_retracted": true`, `false`, or `null`.
`--type`, `--open-access`, and `--no-preprints` are backend-compatibility
constraints rather than universal filters across every article source.
`--type` on `--source all` uses Europe PMC + PubMed when `--open-access` and
`--no-preprints` are both absent. If you add `--open-access` or
`--no-preprints`, PubMed becomes ineligible and BioMCP surfaces the Europe
PMC-only note in markdown, JSON, and debug-plan output instead of silently
pretending the filter applies across every source.

To search a single backend:

```bash
biomcp search article -g BRAF --source pubtator --limit 5
biomcp search article -g BRAF --source europepmc --limit 5
biomcp search article -g BRAF --source pubmed --limit 5
```

To force a tighter federated balance:

```bash
biomcp search article -k "Kartagener syndrome ciliopathy" --limit 50 --max-per-source 10
```

## Get an article

Supported IDs are PMID (digits only), PMCID (e.g., PMC9984800), and DOI
(e.g., 10.1056/NEJMoa1203421). Publisher PIIs (e.g., `S1535610826000103`) are not
indexed by PubMed or Europe PMC and cannot be resolved.

```bash
biomcp get article 22663011
```

Default article output can include an optional Semantic Scholar section with
TLDR text, influence counts, and open-access PDF metadata when that paper
resolves in Semantic Scholar. `S2_API_KEY` makes those requests authenticated;
without it, BioMCP uses the shared pool. `search article --source` now supports
`all`, `pubtator`, `europepmc`, `pubmed`, and `litsense2`; Semantic Scholar
remains an automatic compatible leg rather than a directly selectable backend.

## Request specific sections

Full text section:

```bash
biomcp get article 22663011 fulltext
```

This prints a local `Saved to:` path for cached full-text Markdown when PMC
full text is available. The saved Markdown preserves JATS section structure
and renders the bibliography under `## References` when the source XML
includes `<ref-list>`.

Annotation section:

```bash
biomcp get article 22663011 annotations
```

Semantic Scholar TLDR section:

```bash
biomcp get article 22663011 tldr
```

## Helper commands

```bash
biomcp article entities 22663011   # extract annotated entities via PubTator
biomcp article batch 22663011 24200969          # compact multi-article summary cards
biomcp article citations 22663011 --limit 3         # Semantic Scholar citation graph
biomcp article references 22663011 --limit 3        # Semantic Scholar reference graph
biomcp article recommendations 22663011 --limit 3   # Semantic Scholar related papers
```

`article batch` works without `S2_API_KEY` and echoes the original
`requested_id` together with resolved PMID/PMCID/DOI fields. When Semantic
Scholar data is available, the batch helper can add optional TLDR and citation
metadata. `S2_API_KEY` makes that enrichment authenticated and more reliable.
Use `article batch` as the default follow-up after `search article` when you
already have several shortlisted PMIDs or DOIs.

The Semantic Scholar graph helpers also work without `S2_API_KEY`, but they use
the shared pool and can fail fast on HTTP 429 with guidance to set the key for
a dedicated rate limit. Citations usually work broadly; references and
recommendations can be sparse or empty for paywalled papers because of
publisher elision in the Semantic Scholar graph.

## Caching behavior

Downloaded content is stored in the BioMCP cache directory.
This avoids repeated large payload downloads during iterative workflows.

## JSON mode

```bash
biomcp --json get article 22663011
biomcp --json search article -g BRAF --limit 3
biomcp --json article batch 22663011 24200969
```

JSON article responses include `_meta.next_commands` and `_meta.section_sources`,
so article workflows can promote the next likely pivots and preserve section
provenance without scraping markdown. JSON `search article` responses also echo
`query`, `sort`, `semantic_scholar_enabled`, and row-level ranking/provenance
metadata. In relevance mode, ranking metadata now includes the effective mode
plus normalized lexical, citation, and position components; semantic-aware
rows expose `ranking.semantic_score` as the LitSense2-derived signal and use
`0` when LitSense2 did not match. Hybrid rows also include the composite
score. JSON `article batch` responses are a bare array of compact cards so
callers can map results back to the original input order.

## Practical tips

- Start with narrow `--limit` values.
- Add a disease term when gene-only search is too broad.
- Use section requests to avoid oversized responses.
- Use `biomcp get article <id> tldr` when you want only the optional Semantic Scholar section.

## Related guides

- [Gene](gene.md)
- [Trial](trial.md)
- [How to find articles](../how-to/find-articles.md)
