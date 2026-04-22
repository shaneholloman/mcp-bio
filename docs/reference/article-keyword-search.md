# Article Keyword Search

This reference documents how `-k/--keyword` behaves and when to pair it with
typed article filters.

## When `--keyword` should stand alone

Use keyword-only article search when the question does not start with a known
gene, disease, or drug anchor.

- Unknown-entity questions: search the evidence first instead of inventing a
  typed `-g`, `-d`, or `--drug` value.
- Dataset or method questions: keep the search free-text and add
  `--type review` when you want synthesis papers or surveys.

Examples:

```bash
biomcp search article -k '"cafe-au-lait spots" neurofibromas disease' --type review --limit 5
biomcp search article -k "TCGA mutation analysis dataset" --type review --limit 5
```

On the default `--source all` route, adding `-k/--keyword` also brings LitSense2
into compatible federated searches and makes the default relevance mode
`hybrid`.
That semantic-aware path uses the LitSense2-derived semantic signal; rows
without LitSense2 provenance contribute `semantic=0`.
BioMCP caps each federated source's contribution after deduplication and before
ranking. Default: 40% of `--limit` on federated pools with at least three
surviving primary sources. Rows count against their primary source after
deduplication. Use `--max-per-source <N>` to override that cap, use
`--max-per-source 0` for the default cap explicitly, and set it equal to
`--limit` to disable capping.

## Exact entity suggestions from keyword-only search

If the whole normalized keyword exactly matches a gene, drug, or disease
vocabulary label or exact alias, article search may add a direct typed follow-up
command. The same command appears in markdown `See also` and in JSON
`_meta.next_commands`; JSON also includes an article-local
`_meta.suggestions[]` object with `command`, `reason`, and `sections`.

Examples:

```bash
biomcp search article -k BRAF --limit 5
biomcp search article -k imatinib --limit 5
biomcp search article -k melanoma --limit 5
```

The exact check is for the whole keyword, not any token inside the phrase.
`BRAF V600E` does not suggest `biomcp get gene BRAF`, and `lung cancer
immunotherapy` does not invent a single disease card. Searches that already
include `-g/--gene`, `-d/--disease`, or `--drug` also suppress these direct
entity suggestions because the typed anchor was chosen explicitly.

## Session loop-breaker suggestions

Use `--session <token>` when a caller may issue multiple keyword-only article
searches for one task and wants JSON guidance if the wording starts to loop.
The token is a local correlation label, not an authentication token or secret.
Use short non-identifying labels such as `lit-review-1`; do not include PHI,
credentials, email addresses, or user identifiers.

Example:

```bash
biomcp --json search article -k "Oncotype DX review" --session lit-review-1 --limit 5
biomcp --json search article -k "Oncotype DX DCIS" --session lit-review-1 --limit 5
```

BioMCP compares consecutive successful article keyword searches with the same
session token. It lowercases the keyword, removes common search filler words,
and triggers when the post-stopword term-set Jaccard overlap is at least 60%.
The session baseline expires after 10 minutes and stores only the last keyword
terms plus up to 20 PMIDs from the previous result page under the local cache
root.

Loop-breaker guidance appears only in JSON `_meta.suggestions[]`; default
markdown output is unchanged. Exact entity suggestions keep `sections`.
Loop-breaker suggestions omit `sections` and are ordered by fallback strategy:

1. `biomcp article batch ...` for the previous search's top PMIDs, when any
   were available.
2. `biomcp discover "<topic>"` to map the current topic to structured
   biomedical entities.
3. A date-narrowed `biomcp search article ... --year-min ... --year-max ...`
   retry derived from the current result page when such a retry is available.

Calls without `--session`, first calls in a new session, disjoint keyword
changes, expired session state, or keywords that normalize to no meaningful
terms do not emit loop-breaker suggestions.

## When `--keyword` should be combined with typed filters

If the gene, disease, or drug is already known, keep that anchor in a typed
flag and use `-k` for mechanisms, phenotypes, outcomes, datasets, and other
free-text concepts.

Example:

```bash
biomcp search article --drug amiodarone -k "photosensitivity mechanism" --limit 5
```

Without `-k`, typed-only searches stay on the compatible federated lexical
route.

## Do not invent typed flags for unknown entities

If the question is "which disease causes this phenotype?" or "which drug causes
this effect?", do not guess a disease or drug name just to fill `-d` or
`--drug`. Start with keyword-only search, then rerun with a typed flag once the
first page reveals the likely anchor. If you need BioMCP to resolve the entity
before you search, use `biomcp discover "<question>"`.

## Keyword behavior

`--keyword` (`-k`) is treated as escaped free text and no longer auto-quotes
whitespace-containing values.

This allows multi-word keyword retrieval such as:

```bash
biomcp search article -k "large language model clinical trials" --limit 5
```

## Phrase behavior for entity filters

Entity-oriented filters retain phrase quoting behavior:

- `--gene`
- `--disease`
- `--drug`
- `--author`

Example:

```bash
biomcp search article -g "BRAF V600E" --author "Jane Doe" --limit 5
```

## Combined filters

Filters can also be combined with typed anchors and other article controls:

```bash
biomcp search article --drug amiodarone -k "photosensitivity mechanism" --limit 5
```
