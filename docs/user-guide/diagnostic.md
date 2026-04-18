# Diagnostic

Use diagnostic commands when you need source-native genetic test inventory from
the NCBI Genetic Testing Registry (GTR). The diagnostic surface is local-runtime
data: BioMCP syncs the GTR bulk bundle on first use, then searches and renders
summary/detail cards from the local files.

## Search diagnostic tests

Gene-first search:

```bash
biomcp search diagnostic --gene BRCA1 --limit 5
biomcp search diagnostic --gene EGFR --type Clinical --limit 5
```

Disease-first search:

```bash
biomcp search diagnostic --disease melanoma --limit 5
```

Manufacturer or lab narrowing:

```bash
biomcp search diagnostic --gene BRCA1 --manufacturer Tempus --limit 5
```

Diagnostic search is filter-only. At least one of `--gene`, `--disease`,
`--type`, or `--manufacturer` is required. All provided filters are
conjunctive, `--limit` must stay within `1..=50`, and result ordering is
deterministic: normalized test name ascending, then accession ascending.
`--type` values come directly from the current GTR export and may vary across
releases; recent live bundles use labels such as `Clinical` and `Research`.

## Get a diagnostic record

```bash
biomcp get diagnostic GTR000000001.1
```

Default output returns the summary card only. The base card keeps concise
metadata such as test type, manufacturer or laboratory, institution, country,
CLIA number, statuses, and method categories.

## Request diagnostic sections

Supported sections: `genes`, `conditions`, `methods`, `all`.

```bash
biomcp get diagnostic GTR000000001.1 genes
biomcp get diagnostic GTR000000001.1 conditions
biomcp get diagnostic GTR000000001.1 methods
biomcp get diagnostic GTR000000001.1 all
```

`all` expands to `genes`, `conditions`, and `methods`. `summary` is always
included and is not a separate section token.

Markdown hides unrequested sections entirely. When a requested section is
empty, BioMCP still renders the heading with a truthful empty-state note such
as `No genes listed in GTR.`. JSON keeps the same distinction: unrequested
sections are omitted, while requested empty sections serialize as `[]`.

## JSON metadata

Non-empty `search diagnostic --json` responses include `_meta.next_commands`.
The first follow-up drills the top result with `biomcp get diagnostic <id>`,
and `biomcp list diagnostic` is always present.

`get diagnostic --json` also includes `_meta.next_commands` and
`_meta.section_sources`. Follow-up commands are section-aware, so requesting
`genes` suppresses the redundant `genes` follow-up but still suggests the
remaining sections.

## GTR local data setup

Diagnostic commands read GTR local data from `BIOMCP_GTR_DIR` first, then the
platform data directory (`~/.local/share/biomcp/gtr` on typical Linux systems).
On first use, BioMCP auto-downloads both required GTR files into that root and
refreshes stale data after 7 days.

Required files:

- `test_version.gz`
- `test_condition_gene.txt`

Confirm local GTR readiness with full health output:

```bash
biomcp health
```

Force-refresh GTR local data manually:

```bash
biomcp gtr sync
```

If you need to override the default path:

```bash
export BIOMCP_GTR_DIR="/path/to/gtr"
biomcp health
```

Manual preseed remains supported for offline or controlled environments. A
complete GTR root must contain both required files listed above.

GTR row meanings:

- `configured`: `BIOMCP_GTR_DIR` is set and both GTR files are present
- `configured (stale)`: `BIOMCP_GTR_DIR` is set and complete, but one or more GTR files are older than the 7-day refresh window
- `available (default path)`: the default platform data directory contains a complete GTR root
- `available (default path, stale)`: the default platform data directory contains both GTR files, but one or more are older than the 7-day refresh window
- `not configured`: no complete GTR root is installed at the default path yet
- `error (missing: ...)`: the GTR directory exists but is missing one or more required files
