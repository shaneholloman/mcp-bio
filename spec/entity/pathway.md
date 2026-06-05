# Pathway Queries

Pathway search and detail calls are where BioMCP has to normalize the same
biological idea across KEGG, Reactome, and WikiPathways without hiding
source-specific limits. These batch-B canaries keep alias handling, ranking,
default cards, and rejection guidance honest.

## Long-Form Alias Normalization

Long-form pathway wording should still keep the query echo and return pathway
rows with MAPK context rather than drifting into an empty or unrelated surface.

```bash
../../tools/biomcp-ci search pathway 'mitogen activated protein kinase signaling pathway' --limit 3 | mustmatch like '# Pathways: mitogen activated protein kinase signaling pathway
| Source | ID | Name |
MAPK'
```

## Query-Required Guidance

An empty pathway search should fail with a recoverable instruction rather than
printing a blank result table.

```bash
../../tools/biomcp-ci search pathway 2>&1 | mustmatch like 'Query is required.
biomcp search pathway -q "MAPK signaling"'
```

## Exact-Title Ranking

When the user already knows the pathway title, the small result set should keep
source-identified pathway rows visible instead of returning an empty card.

```bash
../../tools/biomcp-ci search pathway 'MAPK signaling pathway' --limit 3 | mustmatch like '| Source | ID | Name |
| Reactome |
MAPK'
```

## Concise KEGG Default

Default KEGG cards should stay summary-first and point users at opt-in deeper
sections instead of dumping every section by default.

```bash
../../tools/biomcp-ci get pathway hsa05200 | mustmatch like 'Source: KEGG
biomcp get pathway hsa05200 genes
biomcp get pathway hsa05200 all'
```

## Unsupported Section Rejection

Source-aware sections should fail with specific guidance when the user asks for
a section that only exists on a different pathway source.

```bash
../../tools/biomcp-ci get pathway hsa05200 enrichment 2>&1 | mustmatch like 'pathway section "enrichment" is not available for KEGG pathways
Use a Reactome pathway ID such as R-HSA-5673001'
```
