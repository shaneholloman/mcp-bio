# Discover

`discover` is the free-text entrypoint for concept resolution before the user
knows which typed BioMCP command to run. These checks validate the approved
examples against stable structural markers and suggestion contracts.

| Section | Command focus | Why it matters |
|---|---|---|
| Gene Alias | `discover ERBB1` | Confirms alias resolution and gene suggestion |
| Drug Brand Name | `discover Keytruda` | Confirms brand-name normalization to generic drug |
| Symptom Query | `discover "chest pain"` | Confirms symptom-safe suggestions and MedlinePlus overlay |
| HPO-backed Symptom Concepts | `discover diabetes` | Confirms normalized `HP:` symptom concepts surface in discover results when OLS4 returns them |
| No Entity Resolved | `discover qzvxxptl` | Confirms zero-result discover output now suggests review-style article search in markdown and JSON |
| Treatment Query | `discover "what drugs treat myasthenia gravis"` | Confirms treatment intent leads with structured indication search |
| Disease Symptoms | `discover "symptoms of Marfan syndrome"` | Confirms disease-linked symptom routing prefers phenotypes |
| Gene + Disease | `discover "BRAF melanoma"` | Confirms combined orientation queries prefer `search all` |
| Gene Topic To Article Search | `discover "CTCF cohesin"` | Confirms gene-plus-topic queries can pivot into gene-filtered article search |
| Ambiguous Query | `discover diabetes` | Confirms ambiguity guidance is explicit |
| Pathway Query | `discover "MAPK signaling"` | Confirms pathway-oriented suggestion generation |
| Underspecified Variant | `discover R620W` | Confirms low-confidence variant routing keeps article guidance without false gene certainty |
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

## HPO-backed Symptom Concepts

When `discover` finds symptom concepts with normalized `HP:` identifiers, it
should surface those IDs directly in the concept list even when disease concepts
still rank above them.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" discover diabetes)"
echo "$out" | mustmatch like "### Symptom"
echo "$out" | mustmatch like '**Diabetes insipidus** (`HP:0000873`)'
echo "$out" | mustmatch like '**Diabetes mellitus** (`HP:0000819`)'
```

## No Entity Resolved

When `discover` finds no biomedical concepts, it should not dead-end. Instead,
it should point the user toward review-style article search in both markdown and
JSON output. `qzvxxptl` is a stable zero-result probe in the current live data.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" discover qzvxxptl)"
echo "$out" | mustmatch like 'No biomedical entities resolved. Try: biomcp search article -k qzvxxptl --type review --limit 5'
echo "$out" | mustmatch like 'biomcp search article -k qzvxxptl --type review --limit 5'

json_out="$("$bin" --json discover qzvxxptl)"
echo "$json_out" | jq -e '.notes | any(. == "No biomedical entities resolved. Try: biomcp search article -k qzvxxptl --type review --limit 5")' > /dev/null
echo "$json_out" | jq -e '._meta.next_commands[0] == "biomcp search article -k qzvxxptl --type review --limit 5"' > /dev/null
```

## Treatment Query

Treatment-oriented natural-language queries should surface the direct drug-search
follow-up in JSON mode so agents can pivot into a concrete therapy search.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" --json discover "what drugs treat myasthenia gravis")"
echo "$out" | mustmatch like 'biomcp search drug --indication \"myasthenia gravis\" --limit 5'
echo "$out" | jq -e '._meta.next_commands[0] | ascii_downcase == "biomcp search drug --indication \"myasthenia gravis\" --limit 5"' > /dev/null
```

## Disease Symptoms

Symptom-style prompts should route to the phenotype slice of the resolved disease
card instead of falling back to a generic search.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" --json discover "symptoms of Marfan syndrome")"
echo "$out" | mustmatch like '"biomcp get disease MONDO:0007947 phenotypes"'
echo "$out" | jq -e '._meta.next_commands[0] == "biomcp get disease MONDO:0007947 phenotypes"' > /dev/null
```

## Gene + Disease

Mixed gene-and-disease queries should keep both concepts in the suggested
follow-up rather than collapsing to a single-entity lookup.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" --json discover "BRAF melanoma")"
echo "$out" | mustmatch like 'biomcp search all --gene BRAF --disease \"melanoma\"'
echo "$out" | jq -e '._meta.next_commands[0] == "biomcp search all --gene BRAF --disease \"melanoma\""' > /dev/null
```

## Gene Topic To Article Search

When `discover` resolves an unambiguous gene and there is still a meaningful
topic after removing the gene token, the command should suggest a gene-filtered
article search. Gene-function wording should preserve the topic when present
and fall back to the gene-only article search when no topic remains.

```bash
bin="${BIOMCP_BIN:-biomcp}"

out="$("$bin" discover "CTCF cohesin")"
echo "$out" | mustmatch like "biomcp get gene CTCF"
echo "$out" | mustmatch like 'biomcp search article -g CTCF -k cohesin --limit 5'

json_out="$("$bin" --json discover "CTCF cohesin")"
echo "$json_out" | jq -e '._meta.next_commands | any(. == "biomcp search article -g CTCF -k cohesin --limit 5")' > /dev/null
echo "$json_out" | jq -e '._meta.suggestions | any(. == "biomcp search article -g CTCF -k cohesin --limit 5")' > /dev/null

function_out="$("$bin" discover "CTCF function cohesin")"
echo "$function_out" | mustmatch like 'biomcp search article -g CTCF -k cohesin --limit 5'

fallback_out="$("$bin" discover "what does CTCF do")"
echo "$fallback_out" | mustmatch like 'biomcp search article -g CTCF --limit 5'
echo "$fallback_out" | mustmatch not like 'biomcp search article -g CTCF -k "" --limit 5'
```

## Ambiguous Query

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" discover diabetes)"
echo "$out" | mustmatch like "## Concepts"
echo "$out" | mustmatch like "### Disease"
echo "$out" | mustmatch like "biomcp search disease -q diabetes --limit 10"
echo "$out" | mustmatch like "1. **diabetes mellitus**"
echo "$out" | mustmatch '/\n2\. \*\*.+\*\*/'
echo "$out" | mustmatch like "## Suggested Commands"
```

## Pathway Query

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" discover "MAPK signaling")"
echo "$out" | mustmatch like "### Pathway"
echo "$out" | mustmatch like "biomcp search pathway -q \"MAPK signaling\" --limit 5"
```

## Underspecified Variant

Low-confidence fallback concepts should keep their existing routing while still
surfacing a broader-results article-search hint. `R620W` currently resolves to
a label-only variant without a canonical ID, which keeps this live proof stable.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" discover R620W)"
echo "$out" | mustmatch like "### Variant"
echo "$out" | mustmatch not like "biomcp get gene "
echo "$out" | mustmatch not like "## Plain Language"
echo "$out" | mustmatch like "For broader results: biomcp search article -k R620W"
echo "$out" | mustmatch like 'biomcp search article -k "R620W" --limit 5'
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
echo "$out" | mustmatch '/"(ICD10CM|SNOMEDCT|RXNORM)"/'
```
