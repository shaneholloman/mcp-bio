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
