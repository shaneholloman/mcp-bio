# Disease Queries

Disease commands normalize labels to ontology-backed identifiers and provide cross-entity pivots. This file validates melanoma-centric workflows plus canonical MONDO-ID disease-gene paths for somatic and germline coverage. Assertions focus on stable schema and identifier markers rather than dynamic counts.

| Section | Command focus | Why it matters |
|---|---|---|
| Disease search | `search disease melanoma` | Confirms disease normalization output |
| Disease detail | `get disease melanoma` | Confirms canonical disease card |
| Disease survival | `get disease "chronic myeloid leukemia" survival` | Confirms SEER-backed disease survival rendering |
| Disease funding | `get disease "chronic myeloid leukemia" funding` | Confirms NIH Reporter funding contract |
| Non-cancer funding | `get disease "Marfan syndrome" funding` | Confirms funding coverage is not cancer-specific |
| Funding stays opt-in | `get disease "chronic myeloid leukemia" all` | Confirms `all` still excludes NIH Reporter funding |
| Disease genes | `get disease melanoma genes` | Confirms association section rendering |
| Sparse phenotype guidance | `get disease MONDO:0100605 phenotypes` | Confirms truthful completeness note and review follow-up |
| Disease phenotypes keep gene pivot | `get disease "Duchenne muscular dystrophy" phenotypes` | Confirms phenotype-only output still keeps the existing disease-to-genes pivot |
| Disease to trials | `disease trials melanoma` | Confirms trial helper path |
| Disease to articles | `disease articles melanoma` | Confirms literature helper path |
| Disease to drugs | `disease drugs melanoma` | Confirms treatment helper path |

## Searching by Name

Search should return ontology-backed disease rows and canonical MONDO identifiers. We assert the markdown table schema, then confirm the direct-hit JSON shape stays free of fallback metadata.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" search disease melanoma --limit 3)"
echo "$out" | mustmatch like "| ID | Name | Synonyms |"
echo "$out" | mustmatch like "MONDO:0005105"
json="$("$bin" --json search disease melanoma --limit 1)"
echo "$json" | jq -e '.count == 1' > /dev/null
echo "$json" | jq -e '.results[0].id == "MONDO:0005105"' > /dev/null
echo "$json" | jq -e '._meta.next_commands[0] | test("^biomcp get disease .+$")' > /dev/null
echo "$json" | jq -e '._meta.next_commands | any(. == "biomcp list disease")' > /dev/null
echo "$json" | jq -e '._meta | has("fallback_used") | not' > /dev/null
echo "$json" | jq -e '.results[0] | has("resolved_via") | not' > /dev/null
echo "$json" | jq -e '.results[0] | has("source_id") | not' > /dev/null
```

## Disease Search Discover Fallback

When direct MyDisease search returns zero rows for a disease that is only
recoverable through discover plus xref crosswalk, search should return the
canonical disease row with provenance instead of stopping at "No diseases found".
If the live discover dependency is unavailable, the CLI should degrade to an
explicit discover hint and an empty JSON result rather than fabricating fallback
provenance. The same section proves both paths.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" search disease "Arnold Chiari syndrome")"
echo "$out" | mustmatch like "# Diseases: Arnold Chiari syndrome"
json="$("$bin" --json search disease "Arnold Chiari syndrome")"
if printf '%s\n' "$out" | grep -q 'Resolved via discover + crosswalk'; then
  echo "$out" | mustmatch like "Resolved via discover + crosswalk"
  echo "$out" | mustmatch like "| ID | Name | Resolved via | Source ID |"
  echo "$out" | mustmatch like "MONDO:0000115"
  echo "$out" | mustmatch like "Arnold Chiari Malformation"
  echo "$out" | mustmatch like "MESH crosswalk"
  echo "$out" | mustmatch like "MESH:D001139"
  echo "$json" | jq -e '.count >= 1' > /dev/null
  echo "$json" | jq -e '.results[0].id == "MONDO:0000115"' > /dev/null
  echo "$json" | jq -e '.results[0].resolved_via == "MESH crosswalk"' > /dev/null
  echo "$json" | jq -e '.results[0].source_id == "MESH:D001139"' > /dev/null
  echo "$json" | jq -e '._meta.next_commands[0] | test("^biomcp get disease .+$")' > /dev/null
  echo "$json" | jq -e '._meta.next_commands | any(. == "biomcp list disease")' > /dev/null
  echo "$json" | jq -e '._meta.fallback_used == true' > /dev/null
else
  echo "$out" | mustmatch like "No diseases found matching 'Arnold Chiari syndrome'"
  echo "$out" | mustmatch like 'Try: biomcp discover "Arnold Chiari syndrome"'
  echo "$json" | jq -e '.count == 0' > /dev/null
  echo "$json" | jq -e '.results == []' > /dev/null
  echo "$json" | jq -e 'has("_meta") | not' > /dev/null
fi
```

## Disease Search Discover Fallback Synonym

Alternate user wording should still recover the same disease through the
discover fallback path.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" search disease "Chiari malformation")"
echo "$out" | mustmatch like "Resolved via discover + crosswalk"
echo "$out" | mustmatch like "MONDO:0000115"
echo "$out" | mustmatch like "Chiari malformation"
```

## Disease Search Discover Fallback for T-PLL

The fallback should also recover sparse hematologic disease labels that are
missing from the direct MONDO/DOID-backed text index.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" search disease "T-cell prolymphocytic leukemia")"
echo "$out" | mustmatch like "Resolved via discover + crosswalk"
echo "$out" | mustmatch like "MONDO:0019468"
echo "$out" | mustmatch like "T-cell prolymphocytic leukemia"
```

## Disease Search Offset Hint

Paginating past the available fallback rows should keep the raw query in the
discover hint instead of leaking the display summary with `offset=...`.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" search disease "Arnold Chiari syndrome" --offset 5)"
echo "$out" | mustmatch like 'Try: biomcp discover "Arnold Chiari syndrome"'
echo "$out" | mustmatch not like 'Try: biomcp discover "Arnold Chiari syndrome, offset=5"'
```

## Disease Search Fallback Miss

If discover also fails to produce a crosswalkable disease concept, the command
should keep the existing empty-state message and discover hint.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" search disease "nonexistent disease xyz")"
echo "$out" | mustmatch like "No diseases found matching 'nonexistent disease xyz'."
echo "$out" | mustmatch like 'Try: biomcp discover "nonexistent disease xyz"'
echo "$out" | mustmatch not like "Resolved via discover + crosswalk"
```

## Disease Search No Fallback

Operators should be able to disable the discover recovery path for
performance-sensitive or scripting usage.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" search disease "Arnold Chiari syndrome" --no-fallback)"
echo "$out" | mustmatch like "No diseases found matching 'Arnold Chiari syndrome'."
echo "$out" | mustmatch like 'Try: biomcp discover "Arnold Chiari syndrome"'
echo "$out" | mustmatch not like "Resolved via discover + crosswalk"
```

## Getting Disease Details

The disease detail card should resolve the query label to a normalized concept. This check targets heading and canonical ID line.

```bash
out="$(biomcp get disease melanoma)"
echo "$out" | mustmatch like "# melanoma"
echo "$out" | mustmatch like "ID: MONDO:0005105"
echo "$out" | mustmatch like "Genes (Open Targets): CDKN2A (OT"
echo "$out" | mustmatch like $'More:\n  biomcp get disease MONDO:0005105 genes   - associated genes\n  biomcp get disease MONDO:0005105 pathways   - pathways from associated genes\n  biomcp get disease MONDO:0005105 phenotypes   - HPO phenotype annotations\n  biomcp get disease MONDO:0005105 survival   - SEER Explorer cancer survival rates\n  biomcp get disease MONDO:0005105 funding   - NIH Reporter grant support'
json="$(biomcp --json get disease melanoma)"
echo "$json" | jq -e '._meta.next_commands[:5] == [
  "biomcp get disease MONDO:0005105 genes",
  "biomcp get disease MONDO:0005105 pathways",
  "biomcp get disease MONDO:0005105 phenotypes",
  "biomcp get disease MONDO:0005105 survival",
  "biomcp get disease MONDO:0005105 funding"
]' > /dev/null
```

## Disease Survival

The survival section should expose SEER Explorer output for a mapped cancer
without turning survival into a standalone entity. We assert the section
heading, source metadata, compact table shape, and JSON field contract rather
than pinning numeric rates.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" get disease "chronic myeloid leukemia" survival)"
echo "$out" | mustmatch like "## Survival (SEER Explorer)"
echo "$out" | mustmatch like "site code 97"
echo "$out" | mustmatch like "All Ages · All Races / Ethnicities"
echo "$out" | mustmatch like "All Races / Ethnicities"
echo "$out" | mustmatch like "| Sex | Latest observed year | 5-year relative survival | 95% CI | Cases | Latest modeled |"
echo "$out" | mustmatch like "Both Sexes"
json="$("$bin" --json get disease "chronic myeloid leukemia" survival)"
echo "$json" | jq -e '.survival.site_code == 97' > /dev/null
echo "$json" | jq -e '.survival.site_label | contains("CML")' > /dev/null
echo "$json" | jq -e '.survival.series | length >= 1' > /dev/null
echo "$json" | jq -e '.survival.series[0].points | length >= 1' > /dev/null
echo "$json" | jq -e '.survival_note == null' > /dev/null
```

## Disease Survival No-Data Note

Non-cancer or unmapped diseases should keep the command successful and surface
the stable SEER no-data note instead of failing or guessing at a cancer site.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" get disease "Marfan syndrome" survival)"
echo "$out" | mustmatch like "## Survival (SEER Explorer)"
echo "$out" | mustmatch like "SEER survival data not available for this condition."
json="$("$bin" --json get disease "Marfan syndrome" survival)"
echo "$json" | jq -e '.survival == null' > /dev/null
echo "$json" | jq -e '.survival_note == "SEER survival data not available for this condition."' > /dev/null
```

## Disease Survival Hodgkin Mapping

Common disease wording that differs from the upstream ontology label should
still resolve to the intended cancer site instead of falling through to a
different lymphoma subtype or the no-data note.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" get disease "Hodgkin lymphoma" survival)"
echo "$out" | mustmatch like "## Survival (SEER Explorer)"
echo "$out" | mustmatch like "Hodgkin Lymphoma (site code 83)"
echo "$out" | mustmatch like "Both Sexes"
json="$("$bin" --json get disease "Hodgkin lymphoma" survival)"
echo "$json" | jq -e '.id == "MONDO:0004952"' > /dev/null
echo "$json" | jq -e '.survival.site_code == 83' > /dev/null
echo "$json" | jq -e '.survival_note == null' > /dev/null
```

## Disease Funding

The disease funding section should expose NIH Reporter grant rows with stable
column structure, a truthful summary line, and JSON provenance.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" get disease "chronic myeloid leukemia" funding)"
echo "$out" | mustmatch like "## Funding (NIH Reporter)"
echo "$out" | mustmatch like "| Project | PI | Organization | FY | Amount |"
echo "$out" | mustmatch '/Showing top [0-9]+ unique grants from [0-9]+ matching NIH project-year records across FY20[0-9]{2}-FY20[0-9]{2}\./'
echo "$out" | mustmatch '/\| .* \| .* \| .* \| 20[0-9]{2} \| \$[0-9,]+ \|/'
json="$("$bin" --json get disease "chronic myeloid leukemia" funding)"
echo "$json" | jq -e '.funding.query == "chronic myeloid leukemia"' > /dev/null
echo "$json" | jq -e '(.funding.fiscal_years | length) == 5' > /dev/null
echo "$json" | jq -e '.funding.grants | length > 0' > /dev/null
echo "$json" | jq -e '(.funding.grants[0].award_amount | type) == "number"' > /dev/null
echo "$json" | jq -e '(.funding.grants[0].project_num | type) == "string"' > /dev/null
echo "$json" | jq -e '.funding_note == null' > /dev/null
echo "$json" | jq -e 'any(._meta.section_sources[]; .key == "funding" and (.sources | index("NIH Reporter")))' > /dev/null
```

## Disease Funding Beyond Cancer

Funding coverage should not be limited to oncology. Marfan syndrome is the
regression anchor because it exercises a non-cancer disease with live NIH
Reporter funding.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" get disease "Marfan syndrome" funding)"
echo "$out" | mustmatch like "## Funding (NIH Reporter)"
echo "$out" | mustmatch like "| Project | PI | Organization | FY | Amount |"
json="$("$bin" --json get disease "Marfan syndrome" funding)"
echo "$json" | jq -e '.funding.query == "Marfan syndrome"' > /dev/null
echo "$json" | jq -e '.funding.grants | length > 0' > /dev/null
echo "$json" | jq -e '.funding_note == null' > /dev/null
```

## Disease Funding Stays Opt-In

`funding` should stay explicit so `get disease <name_or_id> all` does not
render an NIH Reporter section the user did not request.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" get disease "chronic myeloid leukemia" all)"
echo "$out" | mustmatch like "## Survival (SEER Explorer)"
echo "$out" | mustmatch not like "## Funding (NIH Reporter)"
json="$("$bin" --json get disease "chronic myeloid leukemia" all)"
echo "$json" | jq -e '.survival != null or .survival_note != null' > /dev/null
echo "$json" | jq -e '.funding == null and .funding_note == null' > /dev/null
```

## Disease Crosswalk Identifier Resolution

Crosswalkable identifiers such as MeSH should resolve through MyDisease xrefs
and return the same canonical disease card path as the free-text lookup.

```bash
mesh_id="$(biomcp --json get disease melanoma | jq -r '.xrefs.MeSH')"
test -n "$mesh_id"
test "$mesh_id" != "null"
out="$(biomcp get disease "MESH:${mesh_id}")"
echo "$out" | mustmatch like "# melanoma"
echo "$out" | mustmatch like "ID: MONDO:0005105"
```

## Full Disease Definitions

Disease detail output should preserve the full curated definition text so characterization clauses remain available without falling back to phenotype dumps.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" get disease MONDO:0100605)"
echo "$out" | mustmatch like "hypogonadotropic hypogonadism"
echo "$out" | mustmatch like "neurodevelopmental delay or regression"
echo "$out" | mustmatch not like "It is characterized by the association of…"
```

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" get disease MONDO:0017799)"
echo "$out" | mustmatch like "pleural effusion, ascites and non-malignant ovarian neoplasm"
echo "$out" | mustmatch like "surgical resection of the ovarian mass"
echo "$out" | mustmatch not like "Prognosis is favorable following…"
```

## Disease Genes

Associated-gene expansion is central for translating phenotype-level queries into molecular follow-up. We assert on section heading and table structure.

```bash
out="$(biomcp get disease melanoma genes)"
echo "$out" | mustmatch like "## Associated Genes"
echo "$out" | mustmatch like "| Gene | Relationship | Source | OpenTargets |"
echo "$out" | mustmatch '/overall [0-9.]+/'
```

## Canonical CLL Disease Genes

Canonical MONDO IDs should surface OpenTargets-contributed cancer genes directly in the disease-gene table rather than only in the compact summary.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" get disease MONDO:0003864 genes)"
echo "$out" | mustmatch like "## Associated Genes"
echo "$out" | mustmatch like "| Gene | Relationship | Source | OpenTargets |"
echo "$out" | mustmatch like "| TP53 | associated with disease |"
echo "$out" | mustmatch like "| ATM | associated with disease |"
echo "$out" | mustmatch like "| NOTCH1 | associated with disease |"
echo "$out" | mustmatch like "OpenTargets"
echo "$out" | mustmatch '/overall [0-9.]+/'
```

## Canonical T-PLL Disease Genes

Sparse canonical MONDO cards should recover a human-readable disease label and associated genes through the OLS4-to-OpenTargets path.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" get disease MONDO:0019468 genes)"
echo "$out" | mustmatch like "# T-cell prolymphocytic leukemia"
echo "$out" | mustmatch like "## Associated Genes"
echo "$out" | mustmatch like "| ATM | associated with disease |"
echo "$out" | mustmatch like "| JAK3 | associated with disease |"
echo "$out" | mustmatch like "| STAT5B | associated with disease |"
echo "$out" | mustmatch like "OpenTargets"
```

## Canonical Parkinson Disease Genes

Germline-oriented diseases should still render a populated genes table with stable Parkinson anchors.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" get disease MONDO:0005180 genes)"
echo "$out" | mustmatch like "## Associated Genes"
echo "$out" | mustmatch like "| SNCA | causes |"
```

## Canonical CMT1A Disease Genes

Narrow Mendelian diseases should keep their focused Monarch-style signal instead of regressing into unrelated OT noise.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" get disease MONDO:0007309 genes)"
echo "$out" | mustmatch like "## Associated Genes"
echo "$out" | mustmatch like "| PMP22 | causes |"
```

## Disease Top Variant Summary

Variant expansions should expose the top-ranked disease-to-variant anchor directly in both JSON and markdown, while keeping the full table intact.

```bash
out="$(biomcp --json get disease melanoma variants)"
echo "$out" | jq -e '.top_variant.variant | type == "string"' > /dev/null
echo "$out" | jq -e '.top_variant.source | type == "string"' > /dev/null
echo "$out" | jq -e '.top_variant.evidence_count | type == "number"' > /dev/null
```

```bash
out="$(biomcp get disease melanoma variants)"
echo "$out" | mustmatch like "## Variants"
echo "$out" | mustmatch like "Top Variant:"
```

## Disease to Trials

Disease helper commands should map directly into trial search with condition context retained. The check asserts query echo and trial columns.

```bash
out="$(biomcp disease trials melanoma --limit 3)"
echo "$out" | mustmatch like "condition=melanoma"
echo "$out" | mustmatch like "|NCT ID|Title|Status|Phase|Conditions|"
```

## Disease to Articles

Disease-linked literature retrieval supports rapid evidence triage. Assertions check heading context and the article table schema.

```bash
out="$(biomcp disease articles melanoma --limit 3)"
echo "$out" | mustmatch like "# Articles: disease=melanoma"
echo "$out" | mustmatch like "| PMID | Title |"
```

## Disease to Drugs

Disease-to-drug pivoting provides treatment-oriented context when starting from diagnosis. The output should include indication heading and compact drug table.

```bash
out="$(biomcp disease drugs melanoma --limit 3)"
echo "$out" | mustmatch like "# Drugs: indication=melanoma"
echo "$out" | mustmatch like "|Name|Mechanism|Target|"
```

## Sparse Phenotype Coverage Notes

When phenotype rows are present but limited, BioMCP should say the section is source-backed and may be incomplete for the full disease presentation, then suggest a review-literature follow-up.

```bash
out="$(biomcp get disease MONDO:0100605 phenotypes)"
echo "$out" | mustmatch like "source-backed"
echo "$out" | mustmatch like "may be incomplete for the full disease presentation"
echo "$out" | mustmatch like 'biomcp search article -d "4H leukodystrophy" --type review --limit 5'
```

## Disease Phenotype Key Features

Section-only phenotype output should distinguish the classic disease summary from the comprehensive HPO table.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" get disease MONDO:0008222 phenotypes)"
echo "$out" | mustmatch like "### Key Features"
echo "$out" | mustmatch '/periodic muscle paralysis/i'
echo "$out" | mustmatch '/QT interval/i'
echo "$out" | mustmatch like "source-backed"
```

## Disease Phenotype Key Features JSON

Structured disease output should expose the same compact summary as `key_features`.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" --json get disease MONDO:0008222 phenotypes)"
echo "$out" | jq -e '.key_features | length >= 3' > /dev/null
echo "$out" | jq -e '.key_features | any(test("periodic muscle paralysis"; "i"))' > /dev/null
```

## Disease Phenotypes Keep Gene Pivot

Phenotype-only disease output should keep the existing `More:` block that
starts with the disease-to-genes pivot instead of adding a duplicate phenotype-
specific tip.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" get disease "Duchenne muscular dystrophy" phenotypes)"
printf '%s\n' "$out" | grep -q '^More:$'
echo "$out" | mustmatch '/biomcp get disease .+ genes/'
echo "$out" | mustmatch not like "Tip: For genotype-phenotype correlations:"
```

## Exact Disease Ranking

Exact disease labels should be reranked to the front of the returned page even when upstream ordering is noisy. This regression checks that the canonical colorectal cancer node appears in the surfaced result set.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" search disease "colorectal cancer" --limit 10)"
echo "$out" | mustmatch like "| ID | Name | Synonyms |"
echo "$out" | mustmatch like "| MONDO:0024331 | colorectal carcinoma |"
```

## Disease DisGeNET Associations

DisGeNET scored disease-gene associations require `DISGENET_API_KEY`. The section heading and table schema are stable invariants; individual scores and row counts vary by API tier.

```bash
status=0
out="$(biomcp get disease melanoma disgenet 2>&1)" || status=$?
if [ "$status" -eq 0 ] && ! printf '%s\n' "$out" | grep -qi '403 Forbidden'; then
  echo "$out" | mustmatch like "## DisGeNET"
  echo "$out" | mustmatch like "| Gene | Entrez ID | Score | PMIDs | Trials | EL | EI |"
else
  echo "$out" | mustmatch '/(403 Forbidden|forbidden|DISGENET_API_KEY|Unauthorized)/'
fi
```

```bash
status=0
out="$(biomcp get disease melanoma disgenet --json 2>&1)" || status=$?
if [ "$status" -eq 0 ] && ! printf '%s\n' "$out" | grep -qi '403 Forbidden'; then
  echo "$out" | jq -e '.disgenet.associations | length > 0' > /dev/null
else
  echo "$out" | mustmatch '/(403 Forbidden|forbidden|DISGENET_API_KEY|Unauthorized)/'
fi
```
