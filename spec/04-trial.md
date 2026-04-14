# Trial Queries

Trial search in BioMCP supports condition-first exploration with clinical filters for status, phase, and mutation-centric discovery. This file validates search cards plus sectioned trial retrieval for one known NCT record. Assertions remain structural so they are resilient to changing trial inventories.

| Section | Command focus | Why it matters |
|---|---|---|
| Condition search | `search trial -c melanoma` | Confirms baseline trial retrieval |
| Status filter | `search trial ... -s recruiting` | Confirms recruitment filtering |
| Phase filter | `search trial ... -p 3` | Confirms phase filtering |
| NCI source search | `search trial --source nci` | Documents keyed and auth-gated NCI behavior |
| Mutation search | `search trial ... --mutation G12D` | Confirms mutation search query echo and table shape |
| Trial detail | `get trial NCT02576665` | Confirms trial card structure |
| Eligibility section | `get trial ... eligibility` | Confirms criteria expansion |
| Locations section | `get trial ... locations` | Confirms site listing expansion |
| Trial help and list docs | `search trial --help`, `list trial` | Documents source-specific NCI notes |

## Searching by Condition

Condition-based search is the default entrypoint for clinical trial discovery. The output should include trial table columns and the condition query echo.

```bash
out="$(biomcp search trial -c melanoma --limit 3)"
echo "$out" | mustmatch like "|NCT ID|Title|Status|Phase|Conditions|"
echo "$out" | mustmatch like "condition=melanoma"
```

## Filtering by Status

Recruitment status is often the first triage filter for study feasibility. We assert on explicit status echo and unchanged table schema.

```bash
out="$(biomcp search trial -c melanoma -s recruiting --limit 3)"
echo "$out" | mustmatch like "status=recruiting"
echo "$out" | mustmatch like "|NCT ID|Title|Status|Phase|Conditions|"
```

## Age Filter Count Stability

Age-only count-only search must stay on the cheap CTGov total path regardless
of display limit. This guards the regression where age was incorrectly grouped
with expensive detail-verified post-filters and `Total:` changed with `--limit`.

```bash
t10="$("$(git rev-parse --show-toplevel)/target/release/biomcp" search trial -c melanoma -s recruiting --age 51 --limit 10 --count-only | sed -n 's/^Total: \([0-9]*\).*/\1/p')"
t20="$("$(git rev-parse --show-toplevel)/target/release/biomcp" search trial -c melanoma -s recruiting --age 51 --limit 20 --count-only | sed -n 's/^Total: \([0-9]*\).*/\1/p')"
t50="$("$(git rev-parse --show-toplevel)/target/release/biomcp" search trial -c melanoma -s recruiting --age 51 --limit 50 --count-only | sed -n 's/^Total: \([0-9]*\).*/\1/p')"
test -n "$t10"
test "$t10" = "$t20"
test "$t20" = "$t50"
```

## Age-Only Count Approximation Signal

When age is the only non-API filter, `--count-only` must signal that the total
comes from the upstream CTGov fast-count path and is only approximate after
BioMCP's client-side age post-filter.

```bash timeout=180
out="$("$(git rev-parse --show-toplevel)/target/release/biomcp" search trial -c melanoma -s recruiting --age 51 --count-only)"
echo "$out" | mustmatch '/^Total: [0-9]+/'
echo "$out" | mustmatch like "(approximate, age post-filtered)"

json_out="$("$(git rev-parse --show-toplevel)/target/release/biomcp" search trial -c melanoma -s recruiting --age 51 --count-only --json)"
echo "$json_out" | jq -e '.total | type == "number"' > /dev/null
echo "$json_out" | mustmatch like '"approximate": true'
```

## Expensive Count Traversal Cap

When facility or eligibility verification forces the expensive detail-fetch
path, `--count-only` must stop at the traversal cap and report an unknown total
instead of a misleading lower bound.

Contract note:
Text output is `Total: unknown (traversal limit reached)` and JSON output is
`{"total": null}` when the traversal cap is hit. This is regression-covered in
the CTGov trial unit tests (`src/entities/trial/search/ctgov/tests.rs`) because
a real live-spec query broad enough to exhaust the cap would require
large-scale per-study detail fetches and is not stable enough for the
executable spec suite.

## Fractional Age Filter

Fractional year input matters because ClinicalTrials.gov eligibility often uses
months for pediatric studies. This regression guards the truncation bug that
rejected `--age 0.5`.

```bash timeout=180
out="$("$(git rev-parse --show-toplevel)/target/release/biomcp" search trial --age 0.5 --count-only)"
echo "$out" | mustmatch '/^Total: [0-9]+/'
echo "$out" | grep -qE "^Total: [0-9]+"
```

## Filtering by Phase

Trial phase helps separate exploratory from confirmatory evidence. The phase-specific query marker should be present with the standard trial table.

```bash
out="$(biomcp search trial -c melanoma -p 3 --limit 3)"
echo "$out" | mustmatch like "Query: condition=melanoma, phase=3"
echo "$out" | mustmatch like "|NCT ID|Title|Status|Phase|Conditions|"
```

## Combined Phase 1 and 2 Search

The `1/2` shorthand should preserve the raw query echo while broadening to the combined ClinicalTrials.gov phase bucket. This regression guards the overly narrow early-phase mapping bug.

```bash
out="$("$(git rev-parse --show-toplevel)/target/release/biomcp" search trial -c melanoma --phase 1/2 --limit 3)"
echo "$out" | mustmatch like "Query: condition=melanoma, phase=1/2"
echo "$out" | mustmatch like "|NCT ID|Title|Status|Phase|Conditions|"
```

## Mutation Search

Mutation-centric search must surface trials where the mutation term appears in title, summary, or keywords, not only in eligibility criteria text. This regression guards against the G12D undercounting bug.

```bash
out="$(biomcp search trial -c "pancreatic cancer" --mutation "G12D" --phase 3)"
echo "$out" | mustmatch like "mutation=G12D"
echo "$out" | mustmatch like "|NCT ID|Title|Status|Phase|Conditions|"
```

## Intervention Code Punctuation Normalization

Intervention code searches should normalize the confirmed space-delimited drug-code pattern before dispatch. The CLI should still echo the user query while returning the standard trial table.

```bash
out="$("$(git rev-parse --show-toplevel)/target/release/biomcp" search trial -c "pancreatic cancer" --intervention "HRS 4642" --limit 1)"
echo "$out" | mustmatch like "intervention=HRS 4642"
echo "$out" | mustmatch like "|NCT ID|Title|Status|Phase|Conditions|"
```

## Zero-Result Positional Hint

Free-text trial nicknames on the positional ClinicalTrials.gov path should
explain the upstream limitation and point the operator at the two intended
recovery moves.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" search trial "CodeBreaK 300")"
echo "$out" | mustmatch like "No trials found matching the filters."
echo "$out" | mustmatch like "ClinicalTrials.gov does not index trial nicknames."
echo "$out" | mustmatch like 'biomcp search trial -i "<drug>" -c "<condition>"'
echo "$out" | mustmatch like 'biomcp search article "CodeBreaK 300" to find the NCT ID'
```

## Getting Trial Details

`get trial` provides protocol identity and key metadata in a compact card. We assert on stable NCT heading and status field marker.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" get trial NCT02576665)"
echo "$out" | mustmatch like "# NCT02576665"
echo "$out" | mustmatch like "Status: TERMINATED | Phase: PHASE1 | Study Type: INTERVENTIONAL"
echo "$out" | mustmatch like "biomcp get trial NCT02576665 outcomes   - endpoint measures and time frames"
echo "$out" | mustmatch like "biomcp get trial NCT02576665 references   - linked publications and PMID citations"
echo "$out" | mustmatch like "biomcp get trial NCT02576665 arms   - study arms and assigned interventions"
echo "$out" | mustmatch like 'biomcp search article --drug "Toca 511" -q "NCT02576665 A Study of Toca'
echo "$out" | mustmatch like "find publications or conference reports from this completed/terminated trial"
results_line="$(printf '%s\n' "$out" | grep -n 'biomcp search article --drug "Toca 511"' | head -n1 | cut -d: -f1)"
disease_line="$(printf '%s\n' "$out" | grep -n 'biomcp search disease --query "Colorectal Cancer"' | head -n1 | cut -d: -f1)"
test -n "$results_line"
test -n "$disease_line"
test "$results_line" -lt "$disease_line"
```

## Eligibility Section

Eligibility content captures inclusion and exclusion criteria needed for screening workflows. The test targets the section heading and a canonical inclusion label.

```bash
out="$(biomcp get trial NCT02576665 eligibility)"
echo "$out" | mustmatch like "## Eligibility"
echo "$out" | mustmatch like "Inclusion Criteria"
```

## Locations Section

Site listings support geographic feasibility checks and referral planning. The output should include the locations heading and a stable column schema.

```bash
out="$(biomcp get trial NCT02576665 locations --limit 3)"
echo "$out" | mustmatch like "## Locations"
echo "$out" | mustmatch like "| Facility | City | Country | Status | Contact |"
```

## Trial Help Explains Special Filter Semantics

The trial help output should explain the three non-obvious filter behaviors that
otherwise surprise operators: facility text-search versus geo-verify cost,
ClinicalTrials.gov's combined `1/2` phase label, and `--sex all` meaning no sex
restriction.

```bash
out="$(biomcp search trial --help)"
echo "$out" | mustmatch like "text-search mode"
echo "$out" | mustmatch like "geo-verify mode"
echo "$out" | mustmatch like "materially more expensive"
echo "$out" | mustmatch like "Canonical CLI forms: NA, 1, 1/2, 2, 3, 4."
echo "$out" | mustmatch like "Accepted aliases: EARLY_PHASE1, PHASE1, PHASE2, PHASE3, PHASE4."
echo "$out" | mustmatch like "combined Phase 1/Phase 2 label"
echo "$out" | mustmatch like "not Phase 1 OR Phase 2"
echo "$out" | mustmatch like "no sex restriction"
echo "$out" | tr '\n' ' ' | mustmatch like "age-only CTGov searches report an approximate upstream total"
```

## NCI Source Search

NCI CTS keeps the shared BioMCP trial flags but depends on the current CTS
contract and, in normal live use, an `NCI_API_KEY`. This section documents both
the keyed success path and the explicit auth guidance path so the spec remains
executable on keyed and unkeyed machines.

```bash
bin="${BIOMCP_BIN:-biomcp}"
if out="$("$bin" search trial --source nci -c melanoma --limit 3 2>&1)"; then
  echo "$out" | mustmatch like "source=nci"
  echo "$out" | mustmatch like "|NCT ID|Title|Status|Phase|Conditions|"
else
  echo "$out" | mustmatch '/(NCI_API_KEY|auth|HTTP 401|unauthorized)/i'
fi
```

## NCI Terminated Status Search

The normalized `terminated` filter must map to the live CTS status value that
still returns melanoma trials. This proof accepts the existing auth guidance on
unkeyed machines, but when `NCI_API_KEY` is present it must return at least one
NCI result row.

```bash
bin="${BIOMCP_BIN:-biomcp}"
if out="$("$bin" search trial --source nci -c melanoma -s terminated --limit 1 2>&1)"; then
  echo "$out" | mustmatch like "source=nci"
  echo "$out" | mustmatch like "status=terminated"
  echo "$out" | mustmatch like "|NCT ID|Title|Status|Phase|Conditions|"
  echo "$out" | mustmatch '/\|NCT[0-9]{8}\|/'
else
  echo "$out" | mustmatch '/(NCI_API_KEY|auth|HTTP 401|unauthorized)/i'
fi
```

## Trial Help Documents NCI Source Semantics

The shared `search trial` help should call out which semantics stay CTGov-
specific and which are remapped for NCI. This keeps operators from assuming the
CTGov status and phase vocabulary is universal.

```bash
out="$(biomcp search trial --help)"
echo "$out" | mustmatch like "Source-specific notes"
echo "$out" | mustmatch like "grounds to an NCI disease ID when available"
echo "$out" | mustmatch like "one mapped status at a time"
echo "$out" | mustmatch like "maps to CTS \`I_II\`"
echo "$out" | mustmatch like "\`early_phase1\` is not supported on \`--source nci\`"
echo "$out" | mustmatch like "sites.org_coordinates"
echo "$out" | mustmatch like "no separate NCI keyword flag"
```

## Trial List Documents NCI Filters

`biomcp list trial` is the compact operator reference. It should summarize the
NCI-specific disease grounding, status, phase, and geo rules without inventing
new flags.

```bash
out="$(biomcp list trial)"
echo "$out" | mustmatch like "## NCI source notes"
echo "$out" | mustmatch like "NCI disease ID"
echo "$out" | mustmatch like "one normalized status at a time"
echo "$out" | mustmatch like "\`--source nci --phase 1/2\` maps to CTS \`I_II\`"
echo "$out" | mustmatch like "\`--phase early_phase1\` is not supported"
echo "$out" | mustmatch like "sites.org_coordinates"
echo "$out" | mustmatch like "no separate NCI keyword flag"
```
