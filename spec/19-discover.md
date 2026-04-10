# Discover

`discover` is the free-text entrypoint for concept resolution before the user
knows which typed BioMCP command to run. These checks validate the approved
examples against stable structural markers and suggestion contracts.

| Section | Command focus | Why it matters |
|---|---|---|
| Gene Alias | `discover ERBB1` | Confirms alias resolution and gene suggestion |
| Drug Brand Name | `discover Keytruda` | Confirms brand-name normalization to generic drug |
| Symptom Query | `discover "chest pain"` | Confirms symptom-safe suggestions and MedlinePlus overlay |
| HPO Symptom Bridge | `discover "developmental delay"` | Confirms HPO-backed symptom queries suggest phenotype search first |
| Treatment Query | `discover "what drugs treat myasthenia gravis"` | Confirms treatment intent leads with structured indication search |
| Disease Symptoms | `discover "symptoms of Marfan syndrome"` | Confirms disease-linked symptom routing prefers phenotypes |
| Gene + Disease | `discover "BRAF melanoma"` | Confirms combined orientation queries prefer `search all` |
| Ambiguous Query | `discover diabetes` | Confirms ambiguity guidance is explicit |
| Pathway Query | `discover "MAPK signaling"` | Confirms pathway-oriented suggestion generation |
| Underspecified Variant | `discover V600E` | Confirms the command avoids false gene certainty |
| OLS4-only Mode | `env -u UMLS_API_KEY discover BRCA1` | Confirms truthful degradation without UMLS |
| JSON Metadata | `--json discover ERBB1` | Confirms discover-specific `_meta` contract |
| UMLS Crosswalks | `--json discover "cystic fibrosis"` | Confirms optional clinical crosswalk enrichment |

## Gene Alias

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" discover ERBB1)"
echo "$out" | mustmatch like "# Discover: ERBB1"
echo "$out" | mustmatch like '- **EGFR** (`HGNC:3236`)'
echo "$out" | mustmatch like "biomcp get gene EGFR"
```

## Drug Brand Name

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" discover Keytruda)"
echo "$out" | mustmatch like "# Discover: Keytruda"
echo "$out" | mustmatch like "pembrolizumab"
echo "$out" | mustmatch like "biomcp get drug \"pembrolizumab\""
```

## Symptom Query

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" discover "chest pain")"
echo "$out" | mustmatch like "## Plain Language"
echo "$out" | mustmatch like "MedlinePlus"
echo "$out" | mustmatch like "biomcp search disease -q \"chest pain\" --limit 10"
echo "$out" | mustmatch like "biomcp search trial -c \"chest pain\" --limit 5"
echo "$out" | mustmatch like "biomcp search article -k \"chest pain\" --limit 5"
```

## HPO Symptom Bridge

When `discover` finds an HPO-backed symptom concept, it should surface the
normalized `HP:` identifier and make phenotype ranking the first next step.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" discover "developmental delay")"
echo "$out" | mustmatch like "### Symptom"
echo "$out" | mustmatch like '**Global developmental delay** (`HP:0001263`)'
echo "$out" | mustmatch like $'## Suggested Commands\n- `biomcp search phenotype "HP:0001263"`'
```

## Treatment Query

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" --json discover "what drugs treat myasthenia gravis")"
echo "$out" | jq -e '._meta.next_commands[0] | ascii_downcase == "biomcp search drug --indication \"myasthenia gravis\" --limit 5"' > /dev/null
```

## Disease Symptoms

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" --json discover "symptoms of Marfan syndrome")"
echo "$out" | jq -e '._meta.next_commands[0] == "biomcp get disease MONDO:0007947 phenotypes"' > /dev/null
```

## Gene + Disease

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" --json discover "BRAF melanoma")"
echo "$out" | jq -e '._meta.next_commands[0] == "biomcp search all --gene BRAF --disease \"melanoma\""' > /dev/null
```

## Ambiguous Query

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" discover diabetes)"
echo "$out" | mustmatch like "## Concepts"
echo "$out" | mustmatch like "### Disease"
echo "$out" | mustmatch like $'## Suggested Commands\n- `biomcp search disease -q "diabetes" --limit 10`'
echo "$out" | mustmatch like "1. **diabetes mellitus**"
echo "$out" | mustmatch '/\n2\. \*\*.+\*\*/'
echo "$out" | mustmatch not like "## Plain Language"
```

## Pathway Query

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" discover "MAPK signaling")"
echo "$out" | mustmatch like "### Pathway"
echo "$out" | mustmatch like "biomcp search pathway -q \"MAPK signaling\" --limit 5"
```

## Underspecified Variant

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" discover V600E)"
echo "$out" | mustmatch like "### Variant"
echo "$out" | mustmatch not like "biomcp get gene "
echo "$out" | mustmatch not like "## Plain Language"
```

## OLS4-only Mode

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$(env -u UMLS_API_KEY "$bin" discover BRCA1)"
echo "$out" | mustmatch like "# Discover: BRCA1"
echo "$out" | mustmatch like "UMLS enrichment unavailable"
```

## JSON Metadata

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" --json discover ERBB1)"
echo "$out" | mustmatch like '"concepts": ['
echo "$out" | jq -e '._meta.next_commands | type == "array" and length > 0' > /dev/null
echo "$out" | jq -e 'has("next_commands") | not' > /dev/null
echo "$out" | mustmatch like '"section_sources": ['
echo "$out" | mustmatch like '"discovery_sources": ['
echo "$out" | mustmatch like '"evidence_urls": ['
```

## UMLS Crosswalks

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" --json discover "cystic fibrosis")"
echo "$out" | grep -Eq '"(ICD10CM|SNOMEDCT|RXNORM)"'
```
