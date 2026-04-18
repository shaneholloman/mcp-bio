# Diagnostic

Use diagnostic commands when you need source-native diagnostic inventory from
BioMCP's local-runtime diagnostic surface. The entity now merges two local
sources:

- NCBI Genetic Testing Registry (GTR) for gene-centric genetic tests
- WHO Prequalified IVD for infectious-disease diagnostics

## Search diagnostic tests

Gene-first GTR search:

```bash
biomcp search diagnostic --gene BRCA1 --limit 5
biomcp search diagnostic --gene EGFR --type Clinical --source gtr --limit 5
```

WHO IVD infectious-disease search:

```bash
biomcp search diagnostic --disease HIV --source who-ivd --limit 5
biomcp search diagnostic --disease tuberculosis --source who-ivd --limit 5
```

Default `--source all` keeps GTR gene workflows valid and adds WHO IVD when the
filters can match it:

```bash
biomcp search diagnostic --disease malaria --source all --limit 5
biomcp search diagnostic --manufacturer InTec --source all --limit 5
```

Diagnostic search is filter-only. At least one of `--gene`, `--disease`,
`--type`, or `--manufacturer` is required. All provided filters are
conjunctive, `--limit` must stay within `1..=50`, and result ordering is
deterministic: normalized test name ascending, then accession ascending after
the source-specific match sets are merged.

`--source` accepts `gtr`, `who-ivd`, or `all` (default). GTR remains the
gene-capable source. WHO IVD supports `--disease`, `--type`, and
`--manufacturer`, but explicit `--source who-ivd --gene ...` is invalid and
returns a source-aware recovery hint. On the default `--source all` route,
gene-only searches stay valid because the WHO IVD leg is skipped.

## Get a diagnostic record

```bash
biomcp get diagnostic GTR000000001.1
biomcp get diagnostic "ITPW02232- TC40"
```

Default output returns the summary card only. The base card keeps concise
metadata such as source label, test type, manufacturer, and the source-native
summary fields for the resolved record. GTR cards keep laboratory, institution,
country, CLIA number, statuses, and method categories. WHO IVD cards add
target/marker, regulatory version, and prequalification year.

## Request diagnostic sections

Supported public section tokens remain `genes`, `conditions`, `methods`, and
`all`, but support is resolved per source.

```bash
biomcp get diagnostic GTR000000001.1 genes
biomcp get diagnostic GTR000000001.1 conditions
biomcp get diagnostic GTR000000001.1 methods
biomcp get diagnostic GTR000000001.1 all
biomcp get diagnostic "ITPW02232- TC40" conditions
biomcp get diagnostic "ITPW02232- TC40" all
```

Source-aware section support:

- GTR: `genes`, `conditions`, `methods`
- WHO IVD: `conditions`

`all` expands only to the sections supported by the resolved source. `summary`
is always included and is not a separate section token.

Markdown hides unrequested sections entirely. When a requested section is
empty, BioMCP still renders the heading with a truthful empty-state note. JSON
keeps the same distinction: unrequested sections are omitted, while requested
empty sections serialize as `[]`. WHO IVD `genes` and `methods` are not empty
sections; they are unsupported requests and fail before any data fetch.

## JSON metadata

Non-empty `search diagnostic --json` responses include `_meta.next_commands`.
The first follow-up drills the top result with `biomcp get diagnostic <id>`,
and `biomcp list diagnostic` is always present. Search rows also preserve the
source (`gtr` or `who-ivd`) so merged result pages stay truthful.

`get diagnostic --json` also includes `_meta.next_commands` and
`_meta.section_sources`. Follow-up commands are section-aware, so requesting
`genes` suppresses the redundant `genes` follow-up but still suggests the
remaining supported sections. WHO product codes can contain spaces, so
follow-up commands quote them automatically.

## Local data setup

Diagnostic commands read GTR local data from `BIOMCP_GTR_DIR` and WHO IVD local
data from `BIOMCP_WHO_IVD_DIR` first, then the platform data directory. On
first use, BioMCP auto-downloads the required files into those roots and
refreshes stale data automatically.

Required GTR files:

- `test_version.gz`
- `test_condition_gene.txt`

Required WHO IVD files:

- `who_ivd.csv`

Confirm local diagnostic readiness with full health output:

```bash
biomcp health
```

Force-refresh the local data manually:

```bash
biomcp gtr sync
biomcp who-ivd sync
```

If you need to override the default path:

```bash
export BIOMCP_GTR_DIR="/path/to/gtr"
export BIOMCP_WHO_IVD_DIR="/path/to/who-ivd"
biomcp health
```

Manual preseed remains supported for offline or controlled environments. A
complete GTR root must contain both GTR files listed above, and a complete WHO
IVD root must contain `who_ivd.csv`. Full `biomcp health` reports separate
`GTR local data (...)` and `WHO IVD local data (...)` rows so operators can see
which bundle is missing, stale, or explicitly configured.
