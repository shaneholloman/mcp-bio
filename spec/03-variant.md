# Variant Queries

Variant workflows in BioMCP unify clinical significance, population frequency, and helper pivots to trials and literature. This file validates both broad search behavior and focused retrieval of BRAF V600E context. Assertions emphasize headings, columns, and known durable identifiers.

| Section | Command focus | Why it matters |
|---|---|---|
| Gene-level search | `search variant -g BRAF` | Confirms variant table rendering |
| Protein filter | `search variant -g BRAF --hgvsp V600E` | Confirms targeted query narrowing |
| Detail card | `get variant "BRAF V600E"` | Confirms consolidated variant facts |
| ClinVar section | `get variant ... clinvar` | Confirms clinical interpretation expansion |
| Population section | `get variant ... population` | Confirms frequency context expansion |
| Trial helper | `variant trials ...` | Confirms mutation-centric trial lookup |
| Article helper | `variant articles ...` | Confirms mutation-centric literature lookup |

## Searching by Gene

Gene-scoped variant search is the broad intake step before applying mutation-level filters. The output should include a stable column header set and explicit query echo.

```bash
out="$(biomcp search variant -g BRAF --limit 3)"
echo "$out" | mustmatch like "| ID | Gene | Protein | Legacy Name |"
echo "$out" | mustmatch like "Query: gene=BRAF"
```

## Search JSON Next Commands

Non-empty variant search JSON should expose executable follow-up commands for
the top hit and the broader variant command surface.

```bash
json_out="$(biomcp --json search variant -g BRAF --limit 3)"
echo "$json_out" | mustmatch like '"next_commands":'
echo "$json_out" | jq -e '._meta.next_commands[0] | test("^biomcp get variant .+$")' > /dev/null
echo "$json_out" | jq -e '._meta.next_commands | any(. == "biomcp list variant")' > /dev/null
```

## Finding a Specific Variant

Adding `--hgvsp` should constrain the result set to a precise protein change. We check for the query marker and V600E appearance in rows.

```bash
out="$(biomcp search variant -g BRAF --hgvsp V600E --limit 3)"
echo "$out" | mustmatch like "hgvsp=V600E"
echo "$out" | mustmatch like "| chr7:g.140453136A>T | BRAF | p.V600E |"
```

## Long-Form Protein Filter

Long-form `--hgvsp` input should normalize into the same typed protein filter as
the short form instead of being passed through as raw text.

```bash
out="$(biomcp search variant -g BRAF --hgvsp p.Val600Glu --limit 3)"
echo "$out" | mustmatch like "Query: gene=BRAF, hgvsp=V600E"
echo "$out" | mustmatch like "hgvsp=V600E"
echo "$out" | mustmatch like "| chr7:g.140453136A>T | BRAF | p.V600E |"
```

## Residue Alias Search

Gene-scoped residue aliases should stay on the variant path instead of falling
through to condition text. The query echo should describe the dedicated alias
search honestly.

```bash
out="$(biomcp search variant "PTPN22 620W" --limit 5)"
echo "$out" | mustmatch like "gene=PTPN22"
echo "$out" | mustmatch like "residue_alias=620W"
```

## Residue Alias Search with Gene Flag

A residue alias supplied as a positional token alongside `--gene` should use the
same dedicated alias search path as the two-token positional form.

```bash
out="$(biomcp search variant -g PTPN22 620W --limit 5)"
echo "$out" | mustmatch like "gene=PTPN22"
echo "$out" | mustmatch like "residue_alias=620W"
```

## Protein Shorthand with Gene Context

Standalone protein shorthand becomes safe once the gene is already supplied in a
flag. The query echo should show the normal exact protein filter rather than an
ambiguous gene search.

```bash
out="$(biomcp search variant -g PTPN22 R620W --limit 5)"
echo "$out" | mustmatch like "gene=PTPN22"
echo "$out" | mustmatch like "hgvsp=R620W"
```

## Standalone Protein Shorthand Guidance

Without gene context, a shorthand like `R620W` is too ambiguous for automatic
typed search. BioMCP should return variant-specific next commands rather than
silently rewriting the query into a gene or condition search.

```bash
status=0
out="$(biomcp search variant R620W 2>&1)" || status=$?
test "${status}" -eq 1
echo "$out" | mustmatch like "without gene context"
echo "$out" | mustmatch like "biomcp search variant --hgvsp R620W --limit 10"
echo "$out" | mustmatch like "biomcp discover R620W"
```

## Getting Variant Details

The default variant card should include both human-readable and identifier-centric fields. We assert on a stable rsID and pathogenicity marker.

```bash
out="$(biomcp get variant "BRAF V600E")"
echo "$out" | mustmatch like "rs113488022"
echo "$out" | mustmatch like "Significance: Pathogenic"
```

## Legacy Name Search for Stop-Gain

PLN stop-gain search results should show the compact legacy label alongside the
existing HGVS protein value.

```bash
out="$(biomcp search variant -g PLN --hgvsp L39X --limit 3)"
echo "$out" | mustmatch like "| ID | Gene | Protein | Legacy Name |"
echo "$out" | mustmatch like "| chr6:g.118880200T>G | PLN | p.L39X | PLN L39stop |"
```

## Legacy Name Search for Missense

Legacy-name derivation should also work for non-stop aliases without changing
the current HGVS field.

```bash
out="$(biomcp search variant -g PLN --hgvsp R25C --limit 3)"
echo "$out" | mustmatch like "| chr6:g.118880157C>T | PLN | p.R25C | PLN R25C |"
```

## Long-Form Exact Variant Details

Long-form exact input should resolve through the same exact variant path as the
equivalent short protein change.

```bash
out="$(biomcp get variant "BRAF p.Val600Glu")"
echo "$out" | mustmatch like "rs113488022"
echo "$out" | mustmatch like "Significance: Pathogenic"
```

## Legacy Name in Detail Card

The detail card should keep the existing HGVS protein line and add the compact
legacy label directly below it for ClinVar-backed PLN hits.

```bash
out="$(biomcp get variant 'chr6:g.118880200T>G')"
echo "$out" | mustmatch like "Protein: p.L39X"
echo "$out" | mustmatch like "Legacy Name: PLN L39stop"
```

## ClinVar Section

ClinVar-focused expansion is used for clinical interpretation and evidence traceability. The check targets section heading and variant metadata marker.

```bash
out="$(biomcp get variant "BRAF V600E" clinvar)"
echo "$out" | mustmatch like "## ClinVar"
echo "$out" | mustmatch like "Variant ID:"
```

## ClinVar Compact Disease Anchor

ClinVar output should expose the top ranked disease aggregate directly so agents can read the leading condition without re-parsing the full list.

```bash
out="$(biomcp --json get variant "BRAF V600E" clinvar)"
echo "$out" | jq -e '.top_disease.condition | type == "string"' > /dev/null
echo "$out" | jq -e '.top_disease.reports | type == "number"' > /dev/null
```

## Legacy Name JSON Contract

JSON output should expose the compact normalized legacy label instead of a raw
upstream ClinVar title.

```bash
out="$(biomcp --json get variant 'chr6:g.118880200T>G')"
echo "$out" | jq -e '.legacy_name == "PLN L39stop"' > /dev/null
```

## Population Frequencies

Population frequency context helps distinguish rare versus common variation. We assert on population section labeling and gnomAD field presence.

```bash
out="$(biomcp get variant "BRAF V600E" population)"
echo "$out" | mustmatch like "## Population"
echo "$out" | mustmatch like "## Population (gnomAD via MyVariant.info)"
```

## Population Compact Fields

Population JSON should expose both a stable raw field and a compact percentage string so agents can answer frequency questions without table parsing.

```bash
out="$(biomcp --json get variant "BRAF V600E" population)"
echo "$out" | jq -e '.allele_frequency_raw | type == "number"' > /dev/null
echo "$out" | jq -e '.allele_frequency_percent | type == "string"' > /dev/null
```

## Population Compact Markdown

The markdown population line should keep the raw AF and append the compact percentage inline in the same section.

```bash
out="$(biomcp get variant "BRAF V600E" population)"
echo "$out" | mustmatch '/gnomAD AF: .*%/'
```

## GWAS Supporting PMIDs

GWAS JSON should expose ordered supporting PMIDs as a dedicated array without requiring agents to traverse the full GWAS rows.

```bash
out="$(biomcp --json get variant rs7903146 gwas)"
echo "$out" | jq -e '(.supporting_pmids | type == "array") or (.gwas_unavailable_reason | type == "string")' > /dev/null
```

## Get with Residue Alias Guidance

`get variant` remains exact-only, so residue aliases should return recovery
guidance that keeps the user on the variant search path.

```bash
status=0
out="$(biomcp get variant "PTPN22 620W" 2>&1)" || status=$?
test "${status}" -eq 1
echo "$out" | mustmatch like "BioMCP could not map 'PTPN22 620W' to an exact variant."
echo "$out" | mustmatch like "biomcp search variant \"PTPN22 620W\" --limit 10"
echo "$out" | mustmatch like "biomcp search variant -g PTPN22 --limit 10"
```

## JSON Guidance Metadata

JSON error output for variant shorthand should expose the same high-level
contract as the alias fallback flow: structured `_meta.alias_resolution`,
ordered `_meta.next_commands`, and a non-zero exit.

```bash
status=0
out="$(biomcp --json get variant R620W)" || status=$?
test "${status}" -eq 1
echo "$out" | mustmatch like '"alias_resolution": {'
echo "$out" | mustmatch like '"kind": "protein_change_only"'
echo "$out" | mustmatch like '"requested_entity": "variant"'
echo "$out" | mustmatch like '"next_commands": ['
echo "$out" | mustmatch like '"biomcp discover R620W"'
```

## GWAS Search JSON Next Commands

GWAS search JSON should expose the top-variant follow-up directly so agents can
pivot from association search into the canonical variant card.

```bash
json_out="$(biomcp --json search gwas -g TCF7L2 --limit 5)"
echo "$json_out" | mustmatch like '"next_commands":'
echo "$json_out" | jq -e '._meta.next_commands[0] | test("^biomcp get variant .+$")' > /dev/null
echo "$json_out" | jq -e '._meta.next_commands | any(. == "biomcp list gwas")' > /dev/null
```

## Variant to Trials

A mutation-to-trial pivot is central for translational use cases. The trial output should include the expected table schema and mutation query echo.

```bash
out="$(biomcp variant trials "BRAF V600E" --limit 3)"
echo "$out" | mustmatch like "|NCT ID|Title|Status|Phase|Conditions|"
echo "$out" | mustmatch like "Query: mutation=BRAF V600E"
```

## Variant to Articles

Literature helper commands should retain both gene and keyword context in the heading. We verify header context and article table columns.

```bash
out="$(biomcp variant articles "BRAF V600E" --limit 3)"
echo "$out" | mustmatch like "# Articles: gene=BRAF, keyword=V600E"
echo "$out" | mustmatch like "| PMID | Title |"
```

## Searching by rsID

rsID positional queries should normalize to an exact rsID search instead of falling back to gene text. The query echo and returned BRAF row guard the rsID normalization fix.

```bash
out="$(biomcp search variant rs113488022 --limit 5)"
echo "$out" | mustmatch like "Query: rsid=rs113488022"
echo "$out" | mustmatch like "| chr7:g.140453136A>T | BRAF | p.V600E |"
```

## Searching by c.HGVS

Gene plus c.HGVS shorthand should map to exact gene and coding-change filters. This regression checks the query echo and BRAF recall.

```bash
out="$(biomcp search variant "BRAF c.1799T>A" --limit 5)"
echo "$out" | mustmatch like "Query: gene=BRAF, hgvsc=c.1799T>A"
echo "$out" | mustmatch like "hgvsc=c.1799T>A"
echo "$out" | mustmatch like "| chr7:g.140453136A>T | BRAF |"
```

## Exon Deletion Phrase Search

Confirmed exon-deletion phrases should resolve to a gene-scoped consequence search rather than generic condition text. The query echo should show the normalized consequence filter.

```bash
out="$(biomcp search variant "EGFR Exon 19 Deletion" --limit 5)"
echo "$out" | mustmatch like "Query: gene=EGFR, consequence=inframe_deletion"
echo "$out" | mustmatch like "consequence=inframe_deletion"
echo "$out" | mustmatch like "| ID | Gene | Protein | Legacy Name | Significance |"
```
