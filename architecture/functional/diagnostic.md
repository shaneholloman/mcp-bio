# Diagnostic Functional Note

The `diagnostic` entity is a source-native view over the NCBI Genetic Testing
Registry (GTR) bulk exports. This slice does not attempt a multi-source
diagnostic overlay. It ships a local-runtime GTR backbone with deterministic
search and progressive-disclosure detail.

## Scope

- `search diagnostic --gene|--disease|--type|--manufacturer`
- `get diagnostic <gtr_accession> [genes|conditions|methods|all]`
- `biomcp gtr sync`
- full `biomcp health` readiness for the GTR local bundle

Out of scope in this slice:

- FDA device overlays
- WHO IVD overlays
- cross-entity diagnostic helper commands
- live GTR API calls
- persistent processed caches

## Source lifecycle

BioMCP treats GTR as a local-runtime source, parallel to EMA, WHO
Prequalification, and CDC CVX/MVX. The runtime root is `BIOMCP_GTR_DIR` or the
default platform data directory. A valid GTR root requires both:

- `test_version.gz`
- `test_condition_gene.txt`

Sync must validate both files before replacing either one. A partial refresh is
considered invalid because diagnostic search/detail joins both files.

## Search contract

Diagnostic search is filter-only and conjunctive:

- `--gene`: case-insensitive exact match over joined gene names
- `--disease`: case-insensitive substring over joined condition names
- `--type`: case-insensitive exact equality on GTR test type
- `--manufacturer`: case-insensitive substring over manufacturer/lab labels

Result ordering is deterministic: normalized test name ascending, then
accession ascending.

## Get contract

`get diagnostic <id>` always returns the summary card. Optional sections are:

- `genes`
- `conditions`
- `methods`
- `all`

`all` expands to `genes`, `conditions`, and `methods`. JSON keeps the same
progressive-disclosure contract by omitting unrequested sections and preserving
requested empty sections as `[]`.

## MCP boundary

`search diagnostic` and `get diagnostic` remain MCP-safe because they stay
read-only. `biomcp gtr sync` remains CLI-only because it mutates the local
runtime root.
