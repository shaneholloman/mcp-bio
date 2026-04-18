# Diagnostic Functional Note

The `diagnostic` entity is a source-aware local-runtime surface over two
diagnostic bundles:

- NCBI Genetic Testing Registry (GTR) for gene-centric genetic tests
- WHO Prequalified IVD for infectious-disease diagnostic products

## Scope

- `search diagnostic --source <gtr|who-ivd|all> --gene|--disease|--type|--manufacturer`
- `get diagnostic <diagnostic_id> [genes|conditions|methods|all]`
- `biomcp gtr sync`
- `biomcp who-ivd sync`
- full `biomcp health` readiness for the GTR and WHO IVD local bundles

Out of scope in this slice:

- FDA device overlays
- a new `--source` flag on `get diagnostic`
- cross-entity diagnostic helper commands
- live GTR or WHO IVD API calls beyond local refresh
- persistent processed caches
- any third diagnostic source

## Source lifecycle

BioMCP treats both diagnostic sources as local-runtime inputs, parallel to EMA,
WHO Prequalification, and CDC CVX/MVX.

The GTR runtime root is `BIOMCP_GTR_DIR` or the default platform data
directory. A valid GTR root requires both:

- `test_version.gz`
- `test_condition_gene.txt`

Sync must validate both files before replacing either one. A partial refresh is
considered invalid because diagnostic search/detail joins both files.

The WHO IVD runtime root is `BIOMCP_WHO_IVD_DIR` or the default platform data
directory. A valid WHO IVD root requires:

- `who_ivd.csv`

WHO IVD refresh uses the WHO CSV header contract and replaces the local file
atomically only after the required headers are validated.

## Search contract

Diagnostic search is filter-only and conjunctive, with source-aware matching:

- GTR: `--gene` exact match over joined gene names, `--disease` substring over
  joined condition names, `--type` exact equality on GTR test type, and
  `--manufacturer` substring over manufacturer/lab labels
- WHO IVD: `--disease` substring over `Pathogen/Disease/Marker`, `--type`
  exact match over `Assay Format`, and `--manufacturer` substring over
  `Manufacturer name`

Result ordering is deterministic: normalized test name ascending, then
accession ascending after the source-specific match sets are merged. Pagination
applies only after the global merge. Exact totals remain available for
single-source pages; mixed-source `--source all` pages do not claim an exact
combined total.

Explicit `--source who-ivd --gene ...` is invalid and should return a recovery
hint. The default `--source all` route keeps gene-only searches valid by
skipping the WHO IVD leg.

## Get contract

`get diagnostic <id>` always returns the summary card. Source resolution is
implicit from the identifier: GTR accession regex first, WHO IVD exact product
code lookup second.

Optional public sections remain:

- `genes`
- `conditions`
- `methods`
- `all`

Section support is source-aware:

- GTR supports `genes`, `conditions`, and `methods`
- WHO IVD supports `conditions`

`all` expands only to the sections supported by the resolved source. JSON keeps
the same progressive-disclosure contract by omitting unrequested sections and
preserving requested empty sections as `[]`. WHO IVD cards add source-native
summary fields such as target/marker, regulatory version, and prequalification
year instead of forcing GTR-only detail labels.

## MCP boundary

`search diagnostic` and `get diagnostic` remain MCP-safe because they stay
read-only. `biomcp gtr sync` and `biomcp who-ivd sync` remain CLI-only because
they mutate local runtime roots.
