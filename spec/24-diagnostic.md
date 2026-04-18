# Diagnostic Multi-Source Local Data

Diagnostic commands now surface source-native test inventory from both the NCBI
Genetic Testing Registry (GTR) and the WHO Prequalified IVD CSV. This file
locks down the operator-facing local-data readiness contract plus the public
multi-source search/get surfaces for the `diagnostic` entity.

| Section | Command focus | Why it matters |
|---|---|---|
| Local health readiness | `biomcp health` | Confirms both local diagnostic bundles appear as readable readiness rows |
| Default gene search | `search diagnostic --gene BRCA1` | Confirms gene-first workflows still route through GTR under default `--source all` |
| Explicit WHO gene validation | `search diagnostic --gene BRCA1 --source who-ivd` | Confirms WHO rejects unsupported gene-only search with a recovery hint |
| WHO disease search | `search diagnostic --disease HIV --source who-ivd` | Confirms WHO disease search returns source-aware rows from the local CSV |
| Mixed-source search | `search diagnostic --disease ma --source all` | Confirms merged pages preserve per-row provenance and avoid claiming an exact combined total |
| GTR conjunctive filters | `search diagnostic --gene EGFR --type molecular --source gtr` | Confirms deterministic GTR filter behavior remains intact |
| Search JSON follow-ups | `--json search diagnostic --disease HIV --source who-ivd` | Confirms WHO search JSON exposes shell-safe quoted follow-up commands |
| GTR detail card | `get diagnostic GTR...` | Confirms existing GTR progressive-disclosure behavior remains intact |
| WHO detail card | `get diagnostic "<who_code>"` | Confirms WHO summary/detail behavior, section limits, and quoted next steps |
| WHO `all` expansion | `get diagnostic "<who_code>" all` | Confirms WHO `all` expands only to the source-supported section set |
| WHO JSON follow-ups | `--json get diagnostic "<who_code>" conditions` | Confirms WHO JSON keeps quoted `_meta.next_commands` and source-aware `section_sources` |

## Local Health Readiness

Full `biomcp health` should expose local GTR readiness and local WHO IVD
readiness separately from the API-only inventory so operators can confirm
diagnostic prerequisites before debugging search or get output.

```bash
bash fixtures/setup-gtr-spec-fixture.sh "$PWD"
bash fixtures/setup-who-ivd-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-gtr-env"
. "$PWD/.cache/spec-who-ivd-env"
out="$(biomcp health)"
echo "$out" | mustmatch like "GTR local data ($BIOMCP_GTR_DIR)"
echo "$out" | mustmatch like "| GTR local data ($BIOMCP_GTR_DIR) | configured |"
echo "$out" | mustmatch like "WHO IVD local data ($BIOMCP_WHO_IVD_DIR)"
echo "$out" | mustmatch like "| WHO IVD local data ($BIOMCP_WHO_IVD_DIR) | configured |"
```

## Default Gene Search

Gene-first diagnostic search should remain valid under the new default
`--source all` route, return GTR rows only, and expose the new source-aware
search table.

```bash
bash fixtures/setup-gtr-spec-fixture.sh "$PWD"
bash fixtures/setup-who-ivd-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-gtr-env"
. "$PWD/.cache/spec-who-ivd-env"
out="$(biomcp search diagnostic --gene BRCA1 --limit 5)"
echo "$out" | mustmatch like "# Diagnostic tests: gene=BRCA1"
echo "$out" | mustmatch like "|Accession|Name|Type|Manufacturer / Lab|Source|Genes|Conditions|"
echo "$out" | mustmatch like "GTR000000001.1"
echo "$out" | mustmatch like "NCBI Genetic Testing Registry"
echo "$out" | mustmatch not like "WHO Prequalified IVD"
echo "$out" | mustmatch like 'Use `biomcp get diagnostic GTR000000001.1` for details.'
```

## Explicit WHO Gene Validation

An explicit WHO-only gene search must fail fast because WHO IVD is not a
gene-capable diagnostic source.

```bash
bash fixtures/setup-who-ivd-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-who-ivd-env"
out="$(biomcp search diagnostic --gene BRCA1 --source who-ivd 2>&1 || true)"
echo "$out" | mustmatch like "Error: Invalid argument"
echo "$out" | mustmatch like "WHO IVD does not support --gene"
echo "$out" | mustmatch like "use --source gtr or omit --source for gene-first diagnostic searches"
```

## WHO Disease Search

WHO disease-name search should return source-aware rows from the local CSV and
keep follow-up commands shell-safe when the product code contains spaces.

```bash
bash fixtures/setup-who-ivd-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-who-ivd-env"
out="$(biomcp search diagnostic --disease HIV --source who-ivd --limit 5)"
echo "$out" | mustmatch like "# Diagnostic tests: disease=HIV"
echo "$out" | mustmatch like "|Accession|Name|Type|Manufacturer / Lab|Source|Genes|Conditions|"
echo "$out" | mustmatch like "ITPW02232- TC40"
echo "$out" | mustmatch like "WHO Prequalified IVD"
echo "$out" | mustmatch like "HIV"
echo "$out" | mustmatch like 'Use `biomcp get diagnostic "ITPW02232- TC40"` for details.'
```

## Mixed-Source Search

When both WHO IVD and GTR contribute rows, merged search pages should preserve
row provenance and avoid claiming an exact combined total.

```bash
bash fixtures/setup-gtr-spec-fixture.sh "$PWD"
bash fixtures/setup-who-ivd-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-gtr-env"
. "$PWD/.cache/spec-who-ivd-env"
out="$(biomcp search diagnostic --disease ma --source all --limit 10)"
echo "$out" | mustmatch like "# Diagnostic tests: disease=ma"
echo "$out" | mustmatch like "Found 2 diagnostic tests"
echo "$out" | mustmatch not like "(of"
echo "$out" | mustmatch like "NCBI Genetic Testing Registry"
echo "$out" | mustmatch like "WHO Prequalified IVD"
```

## Search JSON Follow-ups

<!-- mustmatch-lint: skip -->

JSON WHO search output should include shell-safe quoted `_meta.next_commands`
so agents can drill the top WHO result without reparsing markdown.

```bash
bash fixtures/setup-who-ivd-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-who-ivd-env"
json_out="$(biomcp --json search diagnostic --disease HIV --source who-ivd --limit 1)"
echo "$json_out" | jq -e '.[0].source == "who-ivd"' > /dev/null
echo "$json_out" | jq -e '._meta.next_commands | any(. == "biomcp get diagnostic \"ITPW02232- TC40\"")' > /dev/null
echo "$json_out" | jq -e '._meta.next_commands | any(. == "biomcp list diagnostic")' > /dev/null
```

## GTR Conjunctive Filters

Diagnostic search stays filter-only, conjunctive, and deterministic for the GTR
leg. A gene plus type filter should still render the stable table contract
while carrying the new source column.

```bash
bash fixtures/setup-gtr-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-gtr-env"
out="$(biomcp search diagnostic --gene EGFR --type molecular --source gtr --limit 5)"
echo "$out" | mustmatch like "# Diagnostic tests: gene=EGFR, type=molecular, source=gtr"
echo "$out" | mustmatch like "|Accession|Name|Type|Manufacturer / Lab|Source|Genes|Conditions|"
echo "$out" | mustmatch like "GTR000000002.1"
echo "$out" | mustmatch like "NCBI Genetic Testing Registry"
echo "$out" | mustmatch not like "GTR000000001.1"
```

## GTR Detail Card

The base `get diagnostic` command should return a summary card by default, while
explicit sections reveal genes, conditions, and methods for GTR records.

```bash
bash fixtures/setup-gtr-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-gtr-env"
summary_out="$(biomcp get diagnostic GTR000000001.1)"
echo "$summary_out" | mustmatch like "# Diagnostic: GTR000000001.1"
echo "$summary_out" | mustmatch like "Source: NCBI Genetic Testing Registry"
echo "$summary_out" | mustmatch like "Method Categories: Molecular genetics"
echo "$summary_out" | mustmatch not like "## Genes"
echo "$summary_out" | mustmatch not like "## Conditions"
echo "$summary_out" | mustmatch not like "## Methods"

detail_out="$(biomcp get diagnostic GTR000000001.1 genes conditions methods)"
echo "$detail_out" | mustmatch like "# Diagnostic: GTR000000001.1"
echo "$detail_out" | mustmatch like "BRCA1, BARD1"
echo "$detail_out" | mustmatch like "## Conditions"
echo "$detail_out" | mustmatch like "## Methods"
```

## WHO Detail Card

WHO detail cards should render the WHO source label and WHO-native summary
fields without inventing unsupported GTR-only sections.

```bash
bash fixtures/setup-who-ivd-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-who-ivd-env"
summary_out="$(biomcp get diagnostic 'ITPW02232- TC40')"
echo "$summary_out" | mustmatch like "# Diagnostic: ITPW02232- TC40"
echo "$summary_out" | mustmatch like "Source: WHO Prequalified IVD"
echo "$summary_out" | mustmatch like "Assay Format: Immunochromatographic (lateral flow)"
echo "$summary_out" | mustmatch like "Manufacturer: InTec Products, Inc."
echo "$summary_out" | mustmatch like "Target / Marker: HIV"
echo "$summary_out" | mustmatch like "Regulatory Version: Rest-of-World"
echo "$summary_out" | mustmatch like "Prequalification Year: 2019"
echo "$summary_out" | mustmatch not like "## Conditions"
echo "$summary_out" | mustmatch like 'biomcp get diagnostic "ITPW02232- TC40" conditions'

conditions_out="$(biomcp get diagnostic 'ITPW02232- TC40' conditions)"
echo "$conditions_out" | mustmatch like "# Diagnostic: ITPW02232- TC40"
echo "$conditions_out" | mustmatch like "## Conditions"
echo "$conditions_out" | mustmatch like "HIV"
echo "$conditions_out" | mustmatch not like "## Genes"
echo "$conditions_out" | mustmatch not like "## Methods"
```

## WHO `all` Expansion

WHO cards should treat `all` as a source-aware shorthand for the sections WHO
actually supports rather than inventing empty GTR-style sections.

```bash
bash fixtures/setup-who-ivd-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-who-ivd-env"
all_out="$(biomcp get diagnostic 'ITPW02232- TC40' all)"
echo "$all_out" | mustmatch like "# Diagnostic: ITPW02232- TC40"
echo "$all_out" | mustmatch like "## Conditions"
echo "$all_out" | mustmatch like "HIV"
echo "$all_out" | mustmatch not like "## Genes"
echo "$all_out" | mustmatch not like "## Methods"
```

## WHO Unsupported Sections

WHO detail cards only support `conditions`; `genes` and `methods` must fail
with source-aware unsupported-section errors.

```bash
bash fixtures/setup-who-ivd-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-who-ivd-env"
genes_out="$(biomcp get diagnostic 'ITPW02232- TC40' genes 2>&1 || true)"
echo "$genes_out" | mustmatch like "Error: Invalid argument"
echo "$genes_out" | mustmatch like 'Section `genes` is not available for WHO Prequalified IVD diagnostic records'
echo "$genes_out" | mustmatch like 'Try `biomcp get diagnostic "ITPW02232- TC40" conditions`'

methods_out="$(biomcp get diagnostic 'ITPW02232- TC40' methods 2>&1 || true)"
echo "$methods_out" | mustmatch like "Error: Invalid argument"
echo "$methods_out" | mustmatch like 'Section `methods` is not available for WHO Prequalified IVD diagnostic records'
echo "$methods_out" | mustmatch like 'Try `biomcp get diagnostic "ITPW02232- TC40" conditions`'
```

## WHO JSON Follow-ups

JSON WHO detail output should keep quoted follow-up commands and source-aware
section provenance while omitting unsupported sections from `_meta`.

```bash
bash fixtures/setup-who-ivd-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-who-ivd-env"
json_out="$(biomcp --json get diagnostic 'ITPW02232- TC40' conditions)"
echo "$json_out" | mustmatch like '"accession": "ITPW02232- TC40"'
echo "$json_out" | jq -e 'has("conditions")' > /dev/null
echo "$json_out" | jq -e 'has("genes") | not' > /dev/null
echo "$json_out" | jq -e 'has("methods") | not' > /dev/null
echo "$json_out" | jq -e '._meta.next_commands | any(. == "biomcp get diagnostic \"ITPW02232- TC40\"")' > /dev/null
echo "$json_out" | jq -e '._meta.next_commands | any(. == "biomcp list diagnostic")' > /dev/null
echo "$json_out" | jq -e '._meta.section_sources | any(.key == "summary" and (.sources | any(. == "WHO Prequalified IVD")))' > /dev/null
echo "$json_out" | jq -e '._meta.section_sources | any(.key == "conditions" and (.sources | any(. == "WHO Prequalified IVD")))' > /dev/null
echo "$json_out" | jq -e '._meta.section_sources | all(.key != "genes")' > /dev/null
echo "$json_out" | jq -e '._meta.section_sources | all(.key != "methods")' > /dev/null
```

## Filter Validation

Diagnostic search without any filters should still fail fast with the
documented validation error instead of attempting an empty full-bundle scan.

```bash
bash fixtures/setup-gtr-spec-fixture.sh "$PWD"
bash fixtures/setup-who-ivd-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-gtr-env"
. "$PWD/.cache/spec-who-ivd-env"
out="$(biomcp search diagnostic 2>&1 || true)"
echo "$out" | mustmatch like "Error: Invalid argument: diagnostic search requires at least one of --gene, --disease, --type, or --manufacturer"
```
