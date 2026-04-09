# BioMCP Command Reference

BioMCP connects to PubMed, ClinicalTrials.gov, ClinVar, gnomAD, OncoKB, Reactome,
KEGG, UniProt, PharmGKB, CPIC, OpenFDA, Monarch Initiative, GWAS Catalog, and more.
One command grammar covers all entities.

## Quickstart

New to BioMCP? Try:

- `skill install` - install BioMCP skill guidance to your agent
- `get gene BRAF` - look up a gene
- `get variant "BRAF V600E"` - annotate a variant
- `discover "chest pain"` - resolve free text before choosing an entity
- `search trial -c melanoma` - find clinical trials
- `search all --gene BRAF --disease melanoma` - cross-entity summary card

## When to Use What

| You want to know... | Start with |
|---|---|
| What drugs treat a disease | `search drug --indication "<disease>" --limit 5` |
| Symptoms or phenotypes of a disease | `get disease <name_or_id> phenotypes` |
| What a gene does | `get gene <symbol>` |
| Tissue expression or localization of a gene product | `get gene <symbol> hpa` or `get gene <symbol> protein` |
| Drug safety or adverse events | `drug adverse-events <name>` or `get drug <name> safety` |
| Review literature that synthesizes a topic | `search article -k "<query>" --type review --limit 5` |
| Follow one article into related evidence | `article citations <id> --limit 5` or `article recommendations <id> --limit 5` |
| I know the entities but not the next pivot | `search all --gene BRAF --disease melanoma` |
| I only have free text and need routing | `discover "<free text>"` |
| The same sections for several entities | `batch <entity> <id1,id2,...> --sections <s1,s2,...>` |
| Enriched pathways or functions for a gene set | `enrich <GENE1,GENE2,...>` |

## Entities

- gene
- variant
- article
- trial
- drug
- disease
- phenotype
- pgx
- gwas
- pathway
- protein
- study
- adverse-event

## Patterns

- `search <entity> [query|filters]` - find entities
- `discover <query>` - resolve free-text concepts into typed follow-up commands
- `search all [slot filters]` - curated multi-entity orientation (`--gene/--variant/--disease/--drug/--keyword`)
- `search trial [filters]` - trial search is filter-only
- `get <entity> <id> [section...]` - fetch by identifier with optional sections
- `get drug <name> regulatory|safety|shortage [--region <us|eu|all>]` - region-aware U.S./EU drug context
- `get trial <nct_id> locations --offset <N> --limit <N>` - page trial locations
- `enrich <GENE1,GENE2,...>` - gene-set enrichment via g:Profiler
- `batch <entity> <id1,id2,...>` - parallel get operations
- `study list|download|top-mutated|filter|query|co-occurrence|cohort|survival|compare` - local cBioPortal study analytics

## Filter Highlights

- `search variant ... --review-status --population --revel-min --gerp-min --tumor-site --condition --impact --lof --has --missing --therapy`
- `search adverse-event ... --date-from --date-to --suspect-only --sex --age-min --age-max --reporter --count`
- `search gene ... --region --pathway --go` (use GO IDs like `GO:0004672`; search output includes Coordinates/UniProt/OMIM)
- `search protein ... --reviewed --disease --existence` (default reviewed mode)
- `search trial ... --mutation --criteria --study-type --has-results --date-from --date-to`
- `search article ... --date-from --date-to --journal --source <all, pubtator, europepmc, pubmed, litsense2>`
- `search drug ... --region <us|eu|all>` (omitting `--region` checks both U.S. and EU for plain name/alias lookups; omitted structured filters stay U.S.-only; explicit `eu|all` with structured filters errors)

## Helpers

- `variant trials <id> --source <ctgov|nci> --limit <N> --offset <N>`
- `variant articles <id>`
- `drug trials <name>`
- `drug adverse-events <name>`
- `disease trials <name>`
- `disease articles <name>`
- `disease drugs <name>`
- `article entities <pmid> --limit <N>`
- `article citations <id> --limit <N>` (optional auth; shared pool without `S2_API_KEY`)
- `article references <id> --limit <N>` (optional auth; shared pool without `S2_API_KEY`)
- `article recommendations <id> [<id>...] [--negative <id>...] --limit <N>` (optional auth; shared pool without `S2_API_KEY`)
- `gene trials|drugs|articles <symbol>`
- `gene pathways <symbol> --limit <N> --offset <N>`
- `pathway drugs|articles|trials <id>`
- `protein structures <accession> --limit <N> --offset <N>`
- `study list`
- `study download [--list] [<study_id>]`
- `study top-mutated --study <id> [--limit <N>]`
- `study filter --study <id> [--mutated <symbol>] [--amplified <symbol>] [--deleted <symbol>] [--expression-above <gene:threshold>] [--expression-below <gene:threshold>] [--cancer-type <type>]`
- `study query --study <id> --gene <symbol> --type <mutations|cna|expression>`
- `study cohort --study <id> --gene <symbol>`
- `study survival --study <id> --gene <symbol> [--endpoint <os|dfs|pfs|dss>]`
- `study compare --study <id> --gene <symbol> --type <expression|mutations> --target <symbol>`
- `study co-occurrence --study <id> --genes <g1,g2,...>`
- `search phenotype \"HP:... HP:...\"`
- `search gwas -g <gene> | --trait <text>`

## Best-Effort Searches

Best-effort helpers search free-text fields (for example, eligibility criteria,
indication text, and abstracts) rather than strict structured identifiers.
Results depend on source document wording and may vary across sources.

## Deployment Notes

- Set `NCBI_API_KEY` to increase NCBI request throughput for article annotation/full-text paths.
- Set `S2_API_KEY` for authenticated Semantic Scholar requests at 1 req/sec; without it, BioMCP uses the shared pool at 1 req/2sec.
- `search article` defaults to PubTator3 + Europe PMC + PubMed when the filter set is compatible; LitSense2 joins keyword-driven federated searches, and Semantic Scholar is still automatic on compatible queries.
- `search article --source litsense2` requires `-k/--keyword` (or a positional query) and does not support `--type` or `--open-access`.
- EU drug commands auto-download the EMA human-medicines JSON feeds on first use into the default data dir or `BIOMCP_EMA_DIR`, then refresh stale files after 72 hours.
- Run `ema sync` to force-refresh the EMA local data feeds.
- Use `biomcp health --apis-only` for upstream/API checks and full `biomcp health` for local EMA/cache readiness plus cache-limit warnings.
- In multi-worker environments, run one shared `biomcp serve-http` process so workers share one Streamable HTTP `/mcp` endpoint and one limiter budget.

## Ops

- `cache path` - print the managed HTTP cache directory `<resolved cache_root>/http`; output stays plain text and ignores `--json`
- `cache stats` - show HTTP cache statistics (total blob inventory, referenced blob bytes, age range, resolved limits including min disk free); supports `--json` for machine-readable output
- `cache clean [--max-age <duration>] [--max-size <size>] [--dry-run]` - remove orphan blobs and optionally age- or size-evict the HTTP cache; supports `--json` for machine-readable output
- `cache clear [--yes]` - destructively wipe `<resolved cache_root>/http`; never touches `downloads/`; supports `--json` on success and requires a TTY unless `--yes` is passed
- `ema sync`
- `update [--check]`
- `uninstall`
- `health [--apis-only]`
- `version`

Run `biomcp list <entity>` for entity-specific examples.
