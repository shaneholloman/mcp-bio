# CLI Reference

BioMCP provides one command family with entity-oriented subcommands.

## Global options

- `--json`: return structured JSON output
- `--no-cache`: bypass HTTP cache for the current command

`--json` normally returns structured output, but `biomcp cache path` is a plain-text exception. `biomcp cache stats`, `biomcp cache clean`, and `biomcp cache clear` respect `--json` on success. `biomcp cache clear` still refuses non-TTY destructive runs with plain stderr unless you pass `--yes`.

## Core command patterns

```text
biomcp search <entity> [filters]
biomcp get <entity> <id> [section...]
```

Section names are positional trailing arguments after `<id>`.

## Evidence metadata

`get` responses include outbound evidence links in markdown output where available.
In JSON mode, links are exposed under `_meta.evidence_urls` and can include
Ensembl, OMIM, NCBI Gene, and UniProt URLs. Section-level provenance is exposed
under `_meta.section_sources`.

## Top-level commands

```text
biomcp search ...
biomcp get ...
biomcp discover <query>
biomcp enrich <GENE1,GENE2,...> [--limit N]
biomcp batch <entity> <id1,id2,...> [--sections ...] [--source ...]
biomcp chart [type]
biomcp cache path
biomcp cache stats
biomcp cache clean [--max-age <duration>] [--max-size <size>] [--dry-run]
biomcp cache clear [--yes]
biomcp ema sync
biomcp who sync
biomcp health [--apis-only]
biomcp list [entity]
biomcp study list
biomcp study download [--list] [<study_id>]
biomcp study filter --study <id> [--mutated <symbol>] [--amplified <symbol>] [--deleted <symbol>] [--expression-above <gene:threshold>] [--expression-below <gene:threshold>] [--cancer-type <type>]
biomcp study query --study <id> --gene <symbol> --type <mutations|cna|expression>
biomcp study cohort --study <id> --gene <symbol>
biomcp study survival --study <id> --gene <symbol> [--endpoint <os|dfs|pfs|dss>]
biomcp study compare --study <id> --gene <symbol> --type <expression|mutations> --target <symbol>
biomcp study co-occurrence --study <id> --genes <g1,g2,...>
biomcp skill
biomcp skill install [dir]
biomcp skill list                 # list embedded worked examples
biomcp mcp
biomcp serve
biomcp serve-http [--host 127.0.0.1] [--port 8080]
biomcp update [--check]
biomcp uninstall
biomcp version
```

Worked examples are also addressable directly:

```text
biomcp skill 01
biomcp skill article-follow-up
```

`biomcp health --apis-only` is the upstream inventory smoke test. Full
`biomcp health` also reports local readiness rows such as EMA local data,
WHO Prequalification local data, cache dir status, and cache-limit warnings
when the managed HTTP cache is over size or below the configured disk-free
floor.

`biomcp cache path` is a local-CLI-only operator command. It prints the managed
HTTP cache path as plain text and ignores the global `--json` flag.

`biomcp cache stats` is the companion local-CLI operator command. It reports the
resolved cache path, total blob inventory, referenced blob bytes used for
enforcement, orphan count, age range, and the resolved cache limits including
`min_disk_free`; under `--json`, it returns the same contract as a JSON object.

`biomcp cache clean [--max-age <duration>] [--max-size <size>] [--dry-run]`
is the targeted maintenance command for the same cache family. It always removes
orphan blobs, can optionally evict entries older than a duration or LRU-evict to
a byte target, and keeps the same structured report under `--json`.

`biomcp cache clear [--yes]` is the destructive sibling for the same managed
HTTP cache tree. It wipes `<resolved cache_root>/http` completely, never touches
the sibling `downloads/` directory, prompts for confirmation when stdin is a
TTY, and refuses non-interactive runs with plain stderr unless you pass
`--yes`. Successful `--json` output uses `{ "bytes_freed": <number|null>,
"entries_removed": <number> }`.

## Search command families

## Discover

```bash
biomcp discover ERBB1
biomcp discover "chest pain"
biomcp discover "developmental delay"
biomcp --json discover diabetes
```

Use `discover` when the user starts with free text rather than a known entity
type. Markdown output groups resolved concepts by type and suggests concrete
follow-up BioMCP commands. JSON adds `_meta.discovery_sources` alongside the
standard `_meta.next_commands` and `_meta.section_sources` metadata.
Symptom-first queries that resolve to HPO concepts can suggest
`biomcp search phenotype "HP:..."` as the first follow-up.

### All (cross-entity)

```bash
biomcp search all --gene BRAF --disease melanoma
biomcp search all --gene BRAF --counts-only
biomcp search all --keyword "immunotherapy resistance" --since 2024-01-01
biomcp search all --gene BRAF --debug-plan
```

See also: [Search All Workflow](../how-to/search-all-workflow.md)

### Gene

```bash
biomcp search gene BRAF --limit 10 --offset 0
```

### Disease

```bash
biomcp search disease -q melanoma --source mondo --limit 10 --offset 0
```

### PGx

```bash
biomcp search pgx -g CYP2D6 --limit 10
biomcp search pgx -d warfarin --limit 10
```

### Phenotype (Monarch semsim)

```bash
biomcp search phenotype "HP:0001250 HP:0001263" --limit 10
biomcp search phenotype "seizure, developmental delay" --limit 10
```

### GWAS

```bash
biomcp search gwas -g TCF7L2 --limit 10
biomcp search gwas --trait "type 2 diabetes" --limit 10
```

### Article

```bash
biomcp search article -g BRAF -d melanoma --since 2024-01-01 --limit 5 --offset 0
biomcp --json search article -g BRAF --debug-plan --limit 5
```

### Trial

```bash
biomcp search trial -c melanoma --status recruiting --source ctgov --limit 5 --offset 0
```

### Variant

```bash
biomcp search variant -g BRAF --hgvsp V600E --limit 5 --offset 0
```

### Drug

```bash
biomcp search drug -q "kinase inhibitor" --limit 5 --offset 0
biomcp search drug Keytruda --limit 5
biomcp search drug Keytruda --region eu --limit 5
biomcp search drug trastuzumab --region who --limit 5
biomcp search drug --indication malaria --region who --limit 5
```

### Pathway

```bash
biomcp search pathway -q "MAPK signaling" --limit 5 --offset 0
biomcp search pathway -q "Pathways in cancer" --limit 5 --offset 0
```

### Protein

```bash
biomcp search protein -q kinase --limit 5 --offset 0
biomcp search protein -q kinase --all-species --limit 5
```

### Adverse event

```bash
biomcp search adverse-event --drug pembrolizumab --serious --limit 5 --offset 0
biomcp search adverse-event --type device --manufacturer Medtronic --limit 5
biomcp search adverse-event --type device --product-code PQP --limit 5
```

## Get command families

### Gene

```bash
biomcp get gene BRAF
biomcp get gene BRAF pathways ontology diseases protein
biomcp get gene BRAF go interactions civic expression hpa druggability clingen constraint
biomcp get gene ERBB2 funding
biomcp get gene BRAF all
```

`funding` stays opt-in and is not included in `biomcp get gene <symbol> all`.

### Disease

```bash
biomcp get disease melanoma
biomcp get disease MONDO:0005105 genes phenotypes
biomcp get disease MONDO:0005105 variants models
biomcp get disease MONDO:0005105 pathways prevalence civic survival
biomcp get disease "chronic myeloid leukemia" funding
biomcp get disease "chronic myeloid leukemia" survival
biomcp get disease MONDO:0005105 all
```

`funding` stays opt-in and is not included in `biomcp get disease <name_or_id> all`.

### PGx

```bash
biomcp get pgx CYP2D6
biomcp get pgx codeine recommendations frequencies
biomcp get pgx warfarin annotations
```

### Article

```bash
biomcp get article 22663011
biomcp get article 22663011 fulltext
biomcp get article 22663011 tldr
biomcp article batch 22663011 24200969
```

`S2_API_KEY` is optional. With it, BioMCP sends authenticated Semantic Scholar
requests at 1 req/sec for `search article`, `get article`, `get article ... tldr`,
`article batch`, and the explicit `article citations|references|recommendations`
helpers. Without it, those same paths use the shared unauthenticated pool at
1 req/2sec.

### Trial

```bash
biomcp get trial NCT02576665
biomcp get trial NCT02576665 eligibility
```

### Variant

```bash
biomcp get variant "BRAF V600E"
biomcp get variant "BRAF V600E" predict
biomcp get variant rs7903146 gwas
```

### Drug

```bash
biomcp get drug pembrolizumab
biomcp get drug trastuzumab regulatory --region who
biomcp get drug Keytruda regulatory --region eu
biomcp get drug Ozempic safety --region eu
biomcp get drug carboplatin shortage
```

Omitting `--region` on a plain name/alias `search drug` checks U.S., EU, and
WHO data. If you omit `--region` while using structured filters such as
`--target` or `--indication`, BioMCP stays on the U.S. MyChem path. Explicit
`--region who` filters structured U.S. hits through WHO Prequalification.
Explicit `--region eu` or `--region all` with structured filters still errors.
For `get drug`, use `--region` only with `regulatory`, `safety`, `shortage`, or
`all`; WHO currently supports `regulatory` and `all`, while `approvals` stays
U.S.-only.

### Pathway

```bash
biomcp get pathway R-HSA-5673001
biomcp get pathway R-HSA-5673001 genes
biomcp get pathway hsa05200
biomcp get pathway hsa05200 genes
```

### Protein

```bash
biomcp get protein P15056
biomcp get protein P15056 domains interactions
biomcp get protein P15056 complexes
```

### Adverse event

```bash
biomcp get adverse-event 10222779
biomcp get adverse-event 10222779 reactions outcomes
biomcp get adverse-event 10222779 concomitant guidance all
```

## Enrichment

```bash
biomcp enrich BRAF,KRAS,NRAS --limit 10
biomcp enrich BRAF,KRAS,NRAS --limit 10 --json
```

## Batch mode

Batch is limited to 10 IDs per command.

```bash
biomcp batch gene BRAF,TP53
biomcp batch gene BRAF,TP53 --sections pathways,interactions
biomcp batch trial NCT02576665,NCT03715933 --source nci
biomcp batch variant "BRAF V600E","KRAS G12D" --json
```

## MCP mode

- `biomcp serve` runs the stdio MCP server.
- `biomcp serve-http` runs the MCP Streamable HTTP server.
- Streamable HTTP clients connect to `/mcp`.
- Probe routes: `/health`, `/readyz`, and `/`.
- `biomcp serve-sse` remains available only as a hidden compatibility command that points users back to `biomcp serve-http`.

See also: `docs/reference/mcp-server.md`.

## Helper command families

```bash
biomcp variant trials "BRAF V600E"
biomcp variant articles "BRAF V600E"
biomcp variant oncokb "BRAF V600E"
biomcp drug adverse-events pembrolizumab
biomcp drug trials pembrolizumab
biomcp disease trials melanoma
biomcp disease drugs melanoma
biomcp disease articles "Lynch syndrome"
biomcp gene trials BRAF
biomcp gene drugs BRAF
biomcp gene articles BRCA1
biomcp gene pathways BRAF
biomcp pathway drugs R-HSA-5673001
biomcp pathway drugs hsa05200
biomcp pathway articles R-HSA-5673001
biomcp pathway trials R-HSA-5673001
biomcp protein structures P15056
biomcp article entities 22663011
biomcp article citations 22663011 --limit 3
biomcp article references 22663011 --limit 3
biomcp article recommendations 22663011 --limit 3
```

## Chart reference

Use `biomcp chart` to list chart families and `biomcp chart <type>` for the
embedded help page for one chart type.

```bash
biomcp chart
biomcp chart violin
```

## Local study analytics

`study` is BioMCP's local cBioPortal analytics family for downloaded
cBioPortal-style datasets. Unlike the 12 remote entity commands, `study`
operates on files in your local study root instead of querying remote APIs for
each request.

Use `BIOMCP_STUDY_DIR` when you want an explicit study root for reproducible
downloads and examples; if it is unset, BioMCP falls back to its default study
root. `biomcp study download --list` shows downloadable IDs, and
`biomcp study download <study_id>` installs a study into that local root.

| Use this | When |
|----------|------|
| `biomcp search/get/<entity>` | You want live API-backed discovery or detail across the 12 remote entity commands |
| `biomcp study download` | You need to fetch a cBioPortal-style study dataset into your local study root |
| `biomcp study ...` analytics commands | You already have local study files and want cohort, query, survival, compare, or co-occurrence analysis |

### Study command examples

```bash
biomcp study list
biomcp study download --list
biomcp study download msk_impact_2017
biomcp study query --study msk_impact_2017 --gene TP53 --type mutations
biomcp study query --study msk_impact_2017 --gene TP53 --type mutations --chart bar --theme dark --palette wong -o docs/blog/images/tp53-mutation-bar.svg
biomcp study filter --study brca_tcga_pan_can_atlas_2018 --mutated TP53 --amplified ERBB2 --expression-above ERBB2:1.5
biomcp study cohort --study brca_tcga_pan_can_atlas_2018 --gene TP53
biomcp study survival --study brca_tcga_pan_can_atlas_2018 --gene TP53 --endpoint os
biomcp study compare --study brca_tcga_pan_can_atlas_2018 --gene TP53 --type expression --target ERBB2
biomcp study compare --study brca_tcga_pan_can_atlas_2018 --gene TP53 --type mutations --target PIK3CA
biomcp study co-occurrence --study msk_impact_2017 --genes TP53,KRAS
```

### Dataset requirements

- `study list` shows locally available studies.
- `study download` fetches remote datasets into the local study root.
- `study filter` intersects mutation, CNA, expression, and clinical filters.
- `study query` supports `mutations`, `cna`, and `expression` per-gene summaries.
- `study cohort`, `study survival`, and `study compare` require `data_mutations.txt` and `data_clinical_sample.txt`.
- `study survival` also requires `data_clinical_patient.txt` with canonical `{ENDPOINT}_STATUS` and `{ENDPOINT}_MONTHS` columns.
- Expression workflows require a supported expression matrix file.
