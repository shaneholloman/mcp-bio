# How to: find articles

This guide shows practical literature-search patterns.

## Broad start

```bash
biomcp search article -g BRAF --limit 10
```

`search article` always works without credentials. BioMCP keeps
`sort=relevance` as the default, but the effective ranking mode depends on the
query: keyword-bearing searches default to hybrid scoring, while entity-only
searches default to lexical directness. LitSense2 joins keyword-bearing
federated searches, and the Semantic Scholar leg is still eligible whenever the
filter set is compatible. `S2_API_KEY` upgrades those Semantic Scholar
requests to authenticated quota; without it, BioMCP uses the shared pool.

## Search PubMed directly

```bash
biomcp search article -g BRAF --source pubmed --limit 5
```

## Add disease context

```bash
biomcp search article -g BRAF -d melanoma --limit 10
```

## Tune semantic versus lexical balance

```bash
biomcp search article -k "Hirschsprung disease ganglion cells" --ranking-mode hybrid --weight-semantic 0.5 --weight-lexical 0.2 --limit 5
```

Use `--ranking-mode lexical` to force the old directness comparator on a
keyword query, `--ranking-mode semantic` to sort by LitSense2 score first, or
`--weight-*` flags to retune the default hybrid formula
`0.4*semantic + 0.3*lexical + 0.2*citations + 0.1*position`.

## Constrain by date

```bash
biomcp search article -g BRAF --since 2024-01-01 --limit 10
```

## Exclude preprints when supported

```bash
biomcp search article -g BRAF --since 2024-01-01 --no-preprints --limit 10
```

## Pull the full-text section

```bash
biomcp get article 22663011 fulltext
```

## Fetch several shortlisted papers at once

```bash
biomcp article batch 22663011 24200969 39073865
```

Use `article batch` after search when you already know the candidate PMIDs or
DOIs and want compact title/journal/year/entity cards before opening one paper
in full detail. The helper preserves input order and still works when
`S2_API_KEY` is unset.

## Use `--type` carefully

```bash
biomcp search article -g BRAF --type review --limit 5
```

`--type` on the default `--source all` route uses Europe PMC + PubMed when the
other selected filters are PubMed-compatible. If you also need
`--open-access` or `--no-preprints`, PubMed drops out and the search collapses
to Europe PMC-only with an explicit note. Use `--source pubmed` when you want
PubMed's own MeSH/title/abstract search directly and do not need those
PubMed-incompatible filters.

## Inspect the ranking rationale in JSON

```bash
env -u S2_API_KEY biomcp --json search article -g BRAF --limit 3
```

Look for `semantic_scholar_enabled`, row-level `matched_sources`, and
`ranking` metadata to see why a paper ranked where it did. Hybrid rows expose
normalized semantic, lexical, citation, and source-position components plus the
composite score; lexical rows preserve the existing directness metadata.

## Inspect the executed search plan

Markdown:

```bash
env -u S2_API_KEY biomcp search article -g BRAF --debug-plan --limit 3
```

JSON / MCP-friendly text output:

```bash
env -u S2_API_KEY biomcp --json search article -g BRAF --debug-plan --limit 3
```

`--debug-plan` adds a top-level `debug_plan` payload in JSON and prepends the
same payload as a fenced JSON block in markdown. Request JSON+plan for MCP
callers with `--json --debug-plan`.

## Follow-up pattern

After identifying key papers, pivot to trials or variants:

```bash
biomcp search trial -c melanoma --mutation "BRAF V600E" --limit 5
biomcp search variant -g BRAF --limit 5
```
