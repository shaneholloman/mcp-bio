# Diagnostic and GTR Local Data

Diagnostic commands surface source-native test inventory from the NCBI Genetic
Testing Registry (GTR). This file locks down the operator-facing GTR readiness
contract plus the public search/get surfaces for the new `diagnostic` entity.

| Section | Command focus | Why it matters |
|---|---|---|
| GTR health readiness | `biomcp health` | Confirms the local GTR bundle appears as a readable readiness row |
| Search by gene | `search diagnostic --gene BRCA1` | Confirms the gene-first "what test exists?" workflow |
| Search by disease | `search diagnostic --disease melanoma` | Confirms disease-name matching over joined condition names |
| Conjunctive filters | `search diagnostic --gene EGFR --type molecular` | Confirms deterministic filter-only search |
| Detail card | `get diagnostic <id>` | Confirms progressive-disclosure summary plus sectioned detail |
| JSON follow-ups | `--json get diagnostic <id>` | Confirms `_meta.next_commands` and section provenance |

## GTR Health Readiness

Full `biomcp health` should expose local GTR readiness separately from the
API-only inventory so operators can confirm diagnostic prerequisites before
debugging search or get output.

```bash
bash fixtures/setup-gtr-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-gtr-env"
out="$(biomcp health)"
echo "$out" | mustmatch like "GTR local data ($BIOMCP_GTR_DIR)"
echo "$out" | mustmatch like "| GTR local data ($BIOMCP_GTR_DIR) | configured |"
```

## Search by Gene

Gene-first diagnostic search should return a stable markdown heading, a table,
and a next-step hint to inspect one diagnostic card in more detail.

```bash
bash fixtures/setup-gtr-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-gtr-env"
out="$(biomcp search diagnostic --gene BRCA1 --limit 5)"
echo "$out" | mustmatch like "# Diagnostic tests: gene=BRCA1"
echo "$out" | mustmatch like "|Accession|Name|Type|Manufacturer / Lab|Genes|Conditions|"
echo "$out" | mustmatch like 'Use `biomcp get diagnostic'
```

## Search by Disease

Disease-name search should match joined condition names from the local GTR
bundle instead of relying on identifier-only condition fields.

```bash
bash fixtures/setup-gtr-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-gtr-env"
out="$(biomcp search diagnostic --disease melanoma --limit 5)"
echo "$out" | mustmatch like "# Diagnostic tests: disease=melanoma"
echo "$out" | mustmatch like "Cutaneous melanoma"
```

## Conjunctive Filters

Diagnostic search is filter-only, conjunctive, and deterministic. A gene plus
type filter should still render the same stable table contract.

```bash
bash fixtures/setup-gtr-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-gtr-env"
out="$(biomcp search diagnostic --gene EGFR --type molecular --limit 5)"
echo "$out" | mustmatch like "# Diagnostic tests: gene=EGFR, type=molecular"
echo "$out" | mustmatch like "|Accession|Name|Type|Manufacturer / Lab|Genes|Conditions|"
```

## Detail Card

The base `get diagnostic` command should return a summary card by default, while
explicit sections reveal genes, conditions, and methods.

```bash
bash fixtures/setup-gtr-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-gtr-env"
out="$(biomcp get diagnostic GTR000000001.1 genes conditions methods)"
echo "$out" | mustmatch like "# Diagnostic: GTR000000001.1"
echo "$out" | mustmatch like "BRCA1, BARD1"
echo "$out" | mustmatch like "## Conditions"
echo "$out" | mustmatch like "## Methods"
```

## JSON Follow-ups

JSON detail output should keep section-aware follow-up commands and section
provenance so agents can drill the remaining sections deterministically.

```bash
bash fixtures/setup-gtr-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-gtr-env"
json_out="$(biomcp --json get diagnostic GTR000000001.1 genes)"
echo "$json_out" | mustmatch like '"accession": "GTR000000001.1"'
echo "$json_out" | jq -e '._meta.next_commands | any(. == "biomcp get diagnostic GTR000000001.1 conditions")' > /dev/null
echo "$json_out" | jq -e '._meta.next_commands | any(. == "biomcp get diagnostic GTR000000001.1 methods")' > /dev/null
echo "$json_out" | jq -e '._meta.next_commands | any(. == "biomcp list diagnostic")' > /dev/null
echo "$json_out" | jq -e '._meta.section_sources | type == "array"' > /dev/null
```
