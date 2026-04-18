# BioMCP Overview

BioMCP is a single-binary CLI for querying biomedical sources with one command grammar. This overview confirms the binary identity, upstream API reachability, and high-level command map. The checks in this file focus on stable interface markers rather than volatile data payloads.

| Section | Command focus | Why it matters |
|---|---|---|
| Version | `biomcp version` | Confirms binary identity and semantic versioning |
| Health check | `biomcp health --apis-only` | Confirms per-source connectivity and excluded key-gated sources |
| Command reference | `biomcp list` | Confirms core entities are discoverable |
| Entity help | `biomcp list gene` | Confirms contextual filter/helper guidance |
| Article routing | `biomcp list article` | Confirms topic-vs-review-vs-follow-up guidance |

## Version

Version output is the fastest smoke test because it exercises local binary startup without touching network sources. The assertion checks both product name and a semantic version pattern.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
out="$("$bin" version)"
echo "$out" | mustmatch '/^biomcp [0-9]+\.[0-9]+\.[0-9]+/'
```

## Health Check

The API-only health command reports one row per live upstream provider plus explicit excluded rows for key-gated sources. Full `biomcp health` adds local readiness rows such as EMA local data, WHO Prequalification local data, CDC CVX/MVX local data, GTR local data, cache dir, and cache-limit warnings. We assert on the API-only table header and the explicit status summary here because those are stable formatting markers for the upstream inventory contract.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
out="$(env -u NCI_API_KEY -u ONCOKB_TOKEN -u DISGENET_API_KEY -u ALPHAGENOME_API_KEY -u S2_API_KEY -u UMLS_API_KEY "$bin" health --apis-only)"
echo "$out" | mustmatch like "| API | Status | Latency |"
echo "$out" | mustmatch like "| LitSense2 |"
echo "$out" | mustmatch like "| NIH Reporter |"
echo "$out" | mustmatch like "| SEER Explorer |"
echo "$out" | mustmatch not like "EMA local data ("
echo "$out" | mustmatch not like "WHO Prequalification local data ("
echo "$out" | mustmatch not like "CDC CVX/MVX local data ("
echo "$out" | mustmatch not like "GTR local data ("
echo "$out" | mustmatch not like "Cache dir ("
echo "$out" | mustmatch not like "Cache limits"
echo "$out" | mustmatch not like "(key:"
echo "$out" | mustmatch '/Status: [0-9]+ ok, [0-9]+ error, [0-9]+ excluded/'

json_out="$(env -u NCI_API_KEY -u ONCOKB_TOKEN -u DISGENET_API_KEY -u ALPHAGENOME_API_KEY -u S2_API_KEY -u UMLS_API_KEY "$bin" --json health --apis-only)"
echo "$json_out" | jq -e 'all(.rows[]; (.status | type) == "string")' > /dev/null
echo "$json_out" | jq -e 'all(.rows[]; ((.status | contains("(key:")) | not))' > /dev/null
echo "$json_out" | jq -e 'all(.rows[]; (.api | startswith("EMA local data (") | not))' > /dev/null
echo "$json_out" | jq -e 'all(.rows[]; (.api | startswith("WHO Prequalification local data (") | not))' > /dev/null
echo "$json_out" | jq -e 'all(.rows[]; (.api | startswith("CDC CVX/MVX local data (") | not))' > /dev/null
echo "$json_out" | jq -e 'all(.rows[]; (.api | startswith("GTR local data (") | not))' > /dev/null
echo "$json_out" | jq -e 'all(.rows[]; (.api | startswith("Cache dir (") | not))' > /dev/null
echo "$json_out" | jq -e 'all(.rows[]; .api != "Cache limits")' > /dev/null
echo "$json_out" | jq -e 'any(.rows[]; .api == "LitSense2")' > /dev/null
echo "$json_out" | jq -e 'any(.rows[]; .api == "NIH Reporter")' > /dev/null
echo "$json_out" | jq -e 'any(.rows[]; .api == "SEER Explorer")' > /dev/null
echo "$json_out" | jq -e 'any(.rows[]; .api == "OncoKB" and .status == "excluded (set ONCOKB_TOKEN)" and .key_configured == false)' > /dev/null
echo "$json_out" | jq -e 'any(.rows[]; .api == "MyGene" and ((has("key_configured")) | not))' > /dev/null
```

## Command Reference

The command index is the human entry point for discovery. It should now open with a routing table that teaches which command to start with before the grammar reference.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
out="$("$bin" list)"
echo "$out" | mustmatch like "# BioMCP Command Reference"
echo "$out" | mustmatch like "## When to Use What"
echo "$out" | mustmatch like "search drug --indication \"<disease>\""
echo "$out" | mustmatch like "discover \"<free text>\""
echo "$out" | mustmatch like "search all --gene BRAF --disease melanoma"
echo "$out" | mustmatch like "- diagnostic"
echo "$out" | mustmatch like "Turn a literature question into article filters"
echo "$out" | mustmatch like "article citations <id>"
echo "$out" | mustmatch like "batch <entity> <id1,id2,...>"
echo "$out" | mustmatch like "enrich <GENE1,GENE2,...>"
echo "$out" | mustmatch not like "## Query formulation"
echo "$out" | mustmatch not like "photosensitivity mechanism"
echo "$out" | mustmatch like '- `cache path` - print the managed HTTP cache directory `<resolved cache_root>/http`; output stays plain text and ignores `--json`'
echo "$out" | mustmatch like '- `cache stats` - show HTTP cache statistics (total blob inventory, referenced blob bytes, age range, resolved limits including min disk free); supports `--json` for machine-readable output'
echo "$out" | mustmatch like '- `cache clean [--max-age <duration>] [--max-size <size>] [--dry-run]` - remove orphan blobs and optionally age- or size-evict the HTTP cache; supports `--json` for machine-readable output'
echo "$out" | mustmatch like '- `cache clear [--yes]` - destructively wipe `<resolved cache_root>/http`; never touches `downloads/`; supports `--json` on success and requires a TTY unless `--yes` is passed'
echo "$out" | mustmatch like '- `discover <query>`'
echo "$out" | mustmatch like '- `cvx sync`'
echo "$out" | mustmatch like '- `ema sync`'
echo "$out" | mustmatch like '- `gtr sync`'
echo "$out" | mustmatch like '- `who sync`'
echo "$out" | mustmatch like $'## Entities\n\n- gene\n- variant\n- article\n- trial'
```

## Entity Help

Entity-specific help should expose both filter syntax and cross-entity helpers. These cues are important for users who need to move from orientation to targeted execution quickly.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
out="$("$bin" list gene)"
echo "$out" | mustmatch like "## Search filters"
echo "$out" | mustmatch like "## Helpers"
echo "$out" | mustmatch like "## When to use this surface"
echo "$out" | mustmatch like 'Use `get gene <symbol>` for the default card'
```

## Batch Help

`biomcp batch --help` should include concrete examples for article, gene,
trial, and variant workflows together with the batch limits and the
cross-reference back to the batch command reference.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
out="$("$bin" batch --help)"
echo "$out" | mustmatch '/EXAMPLES/'
echo "$out" | mustmatch like "biomcp batch article"
echo "$out" | mustmatch like "biomcp batch gene"
echo "$out" | mustmatch like "biomcp batch trial"
echo "$out" | mustmatch like "biomcp batch variant"
echo "$out" | mustmatch like "Batch accepts up to 10 IDs per call."
echo "$out" | mustmatch like "Each call must use a single entity type."
echo "$out" | mustmatch like "See also: biomcp list batch"
```

## Article Routing Help

`biomcp list article` should explain how to turn a literature question into
typed article filters: known anchors go in `-g/-d/--drug`, free-text concepts
go in `-k`, unknown-entity questions stay keyword-first, review questions can
add `--type review`, and strong papers still pivot to citations or
recommendations.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
out="$("$bin" list article)"
echo "$out" | mustmatch like "## When to use this surface"
echo "$out" | mustmatch like "## Query formulation"
echo "$out" | mustmatch like "Use keyword search to scan a topic before you know the entities."
echo "$out" | mustmatch like "Known gene/disease/drug already identified"
echo "$out" | mustmatch like "Keyword-only topic, dataset, or method question"
echo "$out" | mustmatch like 'Do not invent `-g/-d/--drug`; stay keyword-first or start with `discover`'
echo "$out" | mustmatch like 'Prefer `--type review`'
echo "$out" | mustmatch like "biomcp search article -g BRAF --limit 5"
echo "$out" | mustmatch like "biomcp search article -g TP53 -k \"apoptosis gene regulation\" --limit 5"
echo "$out" | mustmatch like "biomcp search article --drug amiodarone -k \"photosensitivity mechanism\" --limit 5"
echo "$out" | mustmatch like "biomcp search article -k '\"cafe-au-lait spots\" neurofibromas disease' --type review --limit 5"
echo "$out" | mustmatch like "biomcp search article -k \"TCGA mutation analysis dataset\" --type review --limit 5"
echo "$out" | mustmatch like "--ranking-mode <lexical|semantic|hybrid>"
echo "$out" | mustmatch like "--max-per-source <N>"
echo "$out" | mustmatch like "Cap each federated source's contribution after deduplication and before ranking."
echo "$out" | mustmatch like 'Default: 40% of `--limit` on federated pools with at least three surviving primary sources.'
echo "$out" | mustmatch like "keyword-bearing article queries default to hybrid"
echo "$out" | mustmatch like "article citations <id>"
echo "$out" | mustmatch like "article recommendations <id>"
```
