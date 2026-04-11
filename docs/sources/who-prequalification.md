---
title: "WHO Prequalification MCP Tool for Global Drug Access | BioMCP"
description: "Use BioMCP to search WHO Prequalification-backed drug records in BioMCP and retrieve global regulatory context through the local WHO batch."
---

# WHO Prequalification

WHO Prequalification matters when you need global regulatory context for medicines used across procurement and access programs outside the U.S./EU-only lens. It is the right page for questions about whether a medicine appears in the WHO finished-pharmaceutical-products list and for comparing that status with other regional drug views.

In BioMCP, WHO Prequalification is a local-runtime source for drug name/alias lookups and regional regulatory sections rather than a live per-request API surface. BioMCP auto-downloads the finished-pharmaceutical-products CSV into `BIOMCP_WHO_DIR` or the default data directory on first use, supports `--region who|all`, lets structured `--region who` searches filter structured U.S. hits through WHO prequalification, and exposes `biomcp who sync` when you want a forced refresh.

## What BioMCP exposes

| Command | What BioMCP gets from this source | Integration note |
|---|---|---|
| `search drug <name> --region who` | WHO-prequalified drug matches by name or alias | Uses the local WHO batch for WHO-only name/alias lookups |
| `search drug <name> --region all` | Combined U.S., EU, and WHO name/alias search | Adds WHO rows to the split all-region output |
| `search drug --indication <disease> --region who` | WHO-prequalified rows filtered from structured U.S. search hits | Keeps MyChem structured semantics and narrows results through the WHO batch |
| `get drug <name> regulatory --region who|all` | WHO or combined regulatory context | WHO-backed regional regulatory section |

## Example commands

```bash
biomcp search drug trastuzumab --region who --limit 5
```

Returns WHO-prequalified trastuzumab rows from the local WHO dataset.

```bash
biomcp search drug --indication malaria --region who --limit 5
```

Filters structured U.S. malaria hits through the local WHO prequalification batch.

```bash
biomcp get drug trastuzumab regulatory --region who
```

Returns WHO-backed regulatory context for the WHO region.

```bash
biomcp get drug imatinib regulatory --region who
```

Returns the truthful WHO empty state when the drug is not WHO-prequalified.

```bash
biomcp who sync
```

Refreshes the local WHO batch without waiting for the next automatic sync.

## API access

No BioMCP API key required. BioMCP auto-downloads the WHO finished-pharmaceutical-products CSV into `BIOMCP_WHO_DIR` or the default data directory on first use.

## Official source

[WHO Prequalification](https://extranet.who.int/prequal/medicines/prequalified/finished-pharmaceutical-products/export?page&_format=csv) is the official WHO CSV export behind BioMCP's WHO drug context.

## Related docs

- [Drug](../user-guide/drug.md)
- [Data Sources](../reference/data-sources.md)
- [Troubleshooting](../troubleshooting.md)
