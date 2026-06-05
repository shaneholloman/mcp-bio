# Pathway Queries

Pathway search and detail calls are where BioMCP has to normalize the same
biological idea across KEGG, Reactome, and WikiPathways without hiding
source-specific limits. These batch-B canaries keep alias handling, ranking,
default cards, and rejection guidance honest.

## Long-Form Alias Normalization

Long-form pathway wording should still land on the canonical pathway the user
asked for instead of drifting into nearby but unrelated pathway names.

```bash
../../tools/biomcp-ci search pathway 'mitogen activated protein kinase signaling pathway' --limit 3 | mustmatch like '# Pathways: mitogen activated protein kinase signaling pathway
| KEGG | hsa04010 | MAPK signaling pathway |'
```

## Query-Required Guidance

An empty pathway search should fail with a recoverable instruction rather than
printing a blank result table.

```bash
../../tools/biomcp-ci search pathway 2>&1 | mustmatch like 'Query is required.
biomcp search pathway -q "MAPK signaling"'
```

## Exact-Title Ranking

When the user already knows the exact pathway title, that exact-title row should
stay visible at the top of the small result set.

```bash
../../tools/biomcp-ci search pathway 'MAPK signaling pathway' --limit 3 | mustmatch like '| Source | ID | Name |'
../../tools/biomcp-ci search pathway 'MAPK signaling pathway' --limit 3 \
  | awk '/^\| Source \| ID \| Name \|/{getline; getline; print; exit}' \
  | mustmatch like '| KEGG | hsa04010 | MAPK signaling pathway |'
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
