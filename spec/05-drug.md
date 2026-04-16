# Drug and Safety Queries

Drug commands connect mechanism and target context with trial and adverse-event pivots. This file checks both core drug retrieval and OpenFDA-backed safety summaries. Assertions use durable headings and table columns instead of volatile report content.

| Section | Command focus | Why it matters |
|---|---|---|
| EMA health readiness | `biomcp health` | Confirms the local EMA batch is surfaced as an operator-readable readiness row |
| Drug search | `search drug pembrolizumab --region us` | Confirms stable U.S. name-based lookup |
| Indication miss framing | `search drug --indication "Marfan syndrome"` | Confirms zero structured hits are explained as regulatory evidence |
| Drug detail | `get drug pembrolizumab` | Confirms mechanism/target card |
| Sparse drug guidance | `get drug orteronel` | Confirms article-search follow-up for investigational cards |
| Targets section | `get drug ... targets` | Confirms progressive disclosure |
| Trial helper | `drug trials pembrolizumab` | Confirms intervention-based trial pivot |
| Adverse-event helper | `drug adverse-events pembrolizumab` | Confirms FAERS-backed safety signal pivot without CTGov fallback noise |
| Adverse-event search | `search adverse-event -d ibuprofen` | Confirms direct safety search |

## EMA Health Readiness

Full `biomcp health` should expose local EMA readiness separately from the API-only inventory so operators can confirm EU drug prerequisites before debugging query output.

```bash
bash fixtures/setup-ema-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-ema-env"
out="$(biomcp health)"
echo "$out" | mustmatch like "EMA local data ($BIOMCP_EMA_DIR)"
echo "$out" | mustmatch like "| EMA local data ($BIOMCP_EMA_DIR) | configured |"
echo "$out" | mustmatch '/\| Cache dir \(.+\) \| ok \| [0-9]+ms \| - \|/'
```

## WHO Health Readiness

Full `biomcp health` should also expose WHO local readiness separately from the
API-only inventory so operators can confirm WHO drug prerequisites before
debugging query output.

```bash
bash fixtures/setup-who-pq-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-who-pq-env"
out="$(biomcp health)"
echo "$out" | mustmatch like "WHO Prequalification local data ($BIOMCP_WHO_DIR)"
echo "$out" | mustmatch like "| WHO Prequalification local data ($BIOMCP_WHO_DIR) | configured |"
```

## Searching by Name

Name-first search is the stable PR-gate coverage for generic U.S. lookup
without the EMA local-data dependency. This section runs with
`BIOMCP_EMA_DIR` unset and fresh XDG roots so a regression back to EMA
auto-sync is visible immediately. The later EMA-seeded sections cover the
default U.S.+EU no-flag path and the explicit EU/all-region variants.

```bash
tmp_data="$(mktemp -d)"
tmp_cache="$(mktemp -d)"
err="$(mktemp)"
out="$(env -u BIOMCP_EMA_DIR XDG_DATA_HOME="$tmp_data" XDG_CACHE_HOME="$tmp_cache" biomcp search drug pembrolizumab --region us --limit 3 2>"$err")"
echo "$out" | mustmatch like "# Drugs: pembrolizumab"
echo "$out" | mustmatch like "|Name|Mechanism|Target|"
cat "$err" | mustmatch not like "Downloading EMA data"
test ! -d "$tmp_data/biomcp/ema"
```

## Drug Search JSON Next Commands

Non-empty drug search JSON should expose machine-readable follow-up commands
for the preferred top hit and the full drug command surface.

```bash
json_out="$(biomcp --json search drug pembrolizumab --region us --limit 3)"
echo "$json_out" | mustmatch like '"next_commands":'
echo "$json_out" | jq -e '._meta.next_commands[0] | test("^biomcp get drug .+$")' > /dev/null
echo "$json_out" | jq -e '._meta.next_commands | any(. == "biomcp list drug")' > /dev/null
```

## Brand Name Get Fallback

Brand-only names should transparently reuse the plain drug-search fallback when
direct `get drug` lookup misses but the name resolves to one canonical drug.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" get drug XIPERE)"
echo "$out" | mustmatch like "# triamcinolone acetonide"
echo "$out" | mustmatch not like "Error: drug 'XIPERE' not found."
echo "$out" | mustmatch not like "Did you mean:"
```

## Search Help Shows Region Defaults

The inline help should advertise the no-flag cross-region default while keeping
the structured-filter exception explicit.

```bash
out="$(biomcp search drug --help)"
echo "$out" | mustmatch like "When to use:"
echo "$out" | mustmatch like "when you know the drug or brand name"
echo "$out" | mustmatch like "--indication, --target, or --mechanism"
echo "$out" | mustmatch '/\[default: all\]/'
echo "$out" | mustmatch like "Omitting --region on a plain name/alias search checks U.S., EU, and WHO data."
echo "$out" | mustmatch like "If you omit --region while using structured filters such as --target or --indication, BioMCP stays on the U.S. MyChem path."
echo "$out" | mustmatch like "Explicit --region who filters structured U.S. hits through WHO Prequalification."
```

## Structured Indication Misses Are Informative

When a structured indication query finds no U.S. regulatory match, the output should frame that absence as evidence about the regulatory surface rather than a generic failure.

```bash
out="$(biomcp search drug --indication 'Marfan syndrome' --region us --limit 3)"
echo "$out" | mustmatch like "U.S. regulatory data"
echo "$out" | mustmatch like "This absence is informative"
echo "$out" | mustmatch like 'biomcp search article -k "Marfan syndrome treatment" --type review --limit 5'
echo "$out" | mustmatch not like $'No drugs found\n\nShowing 0 of 0 results.'
```

## Getting Drug Details

`get drug` expands mechanism, targets, indications, and key metadata. We assert on the normalized heading and a stable metadata/section marker.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" get drug pembrolizumab)"
echo "$out" | mustmatch like "# pembrolizumab"
echo "$out" | mustmatch like "DrugBank ID: DB09037"
echo "$out" | mustmatch like "## Targets"
echo "$out" | mustmatch like "biomcp get drug pembrolizumab label   - approved-indication and FDA label detail beyond the base card"
echo "$out" | mustmatch like "biomcp get drug pembrolizumab regulatory   - approval and supplement history; use only if the base card lacks approval context"
echo "$out" | mustmatch like "biomcp get drug pembrolizumab safety   - regulatory safety detail"
echo "$out" | mustmatch like "post-marketing signal"
```

## Sparse Drug Cards Suggest Literature Follow-Up

Investigational or sparse label cards should point the user to review literature for indication context instead of pretending the structured card is complete.

```bash
out="$(biomcp get drug orteronel)"
echo "$out" | mustmatch like "biomcp search article --drug orteronel --type review --limit 5"
echo "$out" | mustmatch like "indication context"
```

## Drug Indications

Indications are sourced from OpenTargets and should render user-facing stage labels instead of leaking GraphQL failures or raw field names. This checks the repaired indication path without binding the spec to a particular disease row.

```bash
out="$(biomcp get drug pembrolizumab indications)"
echo "$out" | mustmatch like "## Indications (Open Targets)"
echo "$out" | mustmatch not like "Cannot query field"
echo "$out" | mustmatch '/\((Approved|Phase [0-9](\/[0-9])?|Early Phase 1)\)/'
```

## Compact FDA Label Summary

Default `label` mode should render a compact approved-indications summary and
keep the verbose FDA subsections behind `--raw`. The same compact contract
should hold for JSON output.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" get drug pembrolizumab label)"
echo "$out" | mustmatch like "## FDA Label"
echo "$out" | mustmatch like "### Approved Indications"
echo "$out" | mustmatch like "- Melanoma"
echo "$out" | mustmatch like "Non-Small Cell Lung Cancer"
echo "$out" | mustmatch like "Triple-Negative Breast Cancer"
echo "$out" | mustmatch like 'Use `--raw` for the full truncated FDA label text.'
echo "$out" | mustmatch not like "who: are not eligible"
echo "$out" | mustmatch not like "adults with locally advanced unresectable"
echo "$out" | mustmatch not like "### Warnings and Precautions"
echo "$out" | mustmatch not like "### Dosage and Administration"
json="$("$bin" --json get drug pembrolizumab label)"
echo "$json" | jq -e '.label.indication_summary | type == "array" and length > 5' > /dev/null
echo "$json" | jq -e '.label.indications == null' > /dev/null
echo "$json" | jq -e '.label.warnings == null' > /dev/null
echo "$json" | jq -e '.label.dosage == null' > /dev/null
```

## Compact FDA Label Summary Lists All Approved Section 1 Indications

The compact summary should surface every approved indication block from label
section 1, including multi-indication labels such as thalidomide.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" get drug thalidomide label)"
echo "$out" | mustmatch like "### Approved Indications"
echo "$out" | mustmatch like "Multiple Myeloma"
echo "$out" | mustmatch like "Erythema Nodosum Leprosum"
json="$("$bin" --json get drug thalidomide label)"
echo "$json" | jq -e '[.label.indication_summary[].name] | any(test("multiple myeloma"; "i"))' > /dev/null
echo "$json" | jq -e '[.label.indication_summary[].name] | any(test("erythema nodosum leprosum"; "i"))' > /dev/null
```

## Raw FDA Label Output

Raw label mode should preserve the current truncated FDA subsections when the
operator explicitly asks for them. The same raw opt-in should hold for JSON
output.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" get drug pembrolizumab label --raw)"
echo "$out" | mustmatch like "### Indications and Usage"
echo "$out" | mustmatch like "### Warnings and Precautions"
echo "$out" | mustmatch like "### Dosage and Administration"
echo "$out" | mustmatch not like "### Approved Indications"
json="$("$bin" --json get drug pembrolizumab label --raw)"
echo "$json" | jq -e '.label.indication_summary | type == "array" and length > 0' > /dev/null
echo "$json" | jq -e '.label.indications | type == "string"' > /dev/null
echo "$json" | jq -e '.label.warnings | type == "string"' > /dev/null
echo "$json" | jq -e '.label.dosage | type == "string"' > /dev/null
```

## Get Drug Help Surfaces Supported Sections

The inline help should agree with `biomcp list drug` and the implementation for
supported typed sections, including the regional EMA additions.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" get drug --help)"
echo "$out" | mustmatch like "Sections to include (label, regulatory, safety, shortage, targets, indications, interactions, civic, approvals, all)"
echo "$out" | mustmatch like "Data region for regional sections"
echo "$out" | mustmatch like "--region <REGION>"
echo "$out" | mustmatch '/Preserve raw FDA label subsections when used with .*label.*all/'
echo "$out" | mustmatch like "biomcp get drug pembrolizumab approvals"
echo "$out" | mustmatch like "biomcp get drug pembrolizumab label --raw"
echo "$out" | mustmatch like "biomcp get drug trastuzumab regulatory --region who"
```

## Drug List Documents Region Grammar

`biomcp list drug` is the concise grammar contract for region-aware drug
sections and the MCP help mirror. The list output should continue to document
the same regional section grammar that `get drug --help` exposes.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" list drug)"
echo "$out" | mustmatch like "get drug <name> label [--raw]"
echo "$out" | mustmatch like "get drug <name> regulatory [--region <us|eu|who|all>]"
echo "$out" | mustmatch like "get drug <name> safety [--region <us|eu|all>]"
echo "$out" | mustmatch like "get drug <name> shortage [--region <us|eu|all>]"
echo "$out" | mustmatch like "get drug <name> all [--region <us|eu|who|all>]"
```

## Compact Approval Fields

Drug JSON should expose additive approval aliases and a compact summary so approval questions do not require parsing the base card prose.

```bash
out="$(biomcp --json get drug pembrolizumab)"
echo "$out" | mustmatch like '"approval_date"'
echo "$out" | jq -e '.approval_date | type == "string"' > /dev/null
echo "$out" | jq -e '.approval_date_raw | type == "string"' > /dev/null
echo "$out" | jq -e '.approval_date == .approval_date_raw' > /dev/null
echo "$out" | jq -e '.approval_date_display | type == "string"' > /dev/null
echo "$out" | jq -e '.approval_summary | type == "string"' > /dev/null
```

## Human-Friendly Approval Date

The drug card should render the human-friendly display date in the base header instead of only the raw ISO string.

```bash
out="$(biomcp get drug pembrolizumab)"
echo "$out" | mustmatch '/FDA Approved.*[A-Z][a-z]+ [0-9]{1,2}, [0-9]{4}/'
```

## Drug Targets

Target-only expansion is useful when the workflow is gene-centric. This check ensures the section heading and expected target token are present.

```bash
out="$(biomcp get drug pembrolizumab targets)"
echo "$out" | mustmatch like "## Targets"
echo "$out" | mustmatch like $'## Targets (ChEMBL / Open Targets)\nPDCD1'
echo "$out" | mustmatch not like "Family:"
echo "$out" | mustmatch not like "Members:"
```

## Drug Target Family Members Stay Visible

When the displayed targets resolve to a well-known family, the base card should
still surface the concrete family members in the main target list.

```bash
out="$(biomcp get drug olaparib)"
echo "$out" | mustmatch like "## Targets"
echo "$out" | mustmatch like "PARP1, PARP2, PARP3"
```

## Drug Target Family JSON

The additive JSON contract should preserve the existing targets list while exposing the family summary when available.

```bash
out="$(biomcp --json get drug olaparib)"
echo "$out" | jq -e '.target_family == "PARP"' >/dev/null
echo "$out" | jq -e '(.targets | index("PARP1")) and (.targets | index("PARP2")) and (.targets | index("PARP3"))' >/dev/null
echo "$out" | jq -e 'if has("target_family_name") then (.target_family_name | type) == "string" else true end' >/dev/null
```

## Drug Target Family JSON Omission

Single-target drugs should keep the existing JSON shape and omit the additive family fields entirely.

```bash
out="$(biomcp --json get drug pembrolizumab)"
echo "$out" | jq -e 'has("target_family") | not' >/dev/null
echo "$out" | jq -e 'has("target_family_name") | not' >/dev/null
```

## Mixed Drug Targets Stay Flat

Drugs with unrelated targets should keep the plain target list without a misleading family summary.

```bash
out="$(biomcp get drug imatinib)"
echo "$out" | mustmatch like "## Targets"
echo "$out" | mustmatch like "ABL1, DDR1, DDR2, BCR, KIT, PDGFRB"
echo "$out" | mustmatch not like "Family:"
echo "$out" | mustmatch not like "Members:"
```

## Drug Variant Targets

Variant-specific therapy targets should render separately from the generic ChEMBL/Open Targets list so the source labels stay truthful while still surfacing matchable CIViC context.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" get drug rindopepimut targets)"
echo "$out" | mustmatch like "## Targets (ChEMBL / Open Targets)"
echo "$out" | mustmatch like "Variant Targets (CIViC): EGFRvIII"
```

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" --json get drug rindopepimut targets)"
echo "$out" | jq -e '
  (.variant_targets | index("EGFRvIII"))
  and any(._meta.section_sources[]; .key == "variant_targets" and (.sources | index("CIViC")))
' > /dev/null
```

## Drug Interactions With Public Label Text

The public MyChem payload does not reliably expose structured DrugBank interaction rows, so BioMCP should render OpenFDA label text when it exists instead of claiming no interactions are known.

```bash
out="$(biomcp get drug Warfarin interactions)"
echo "$out" | mustmatch like "## Interactions"
echo "$out" | mustmatch like "DRUG INTERACTIONS"
echo "$out" | mustmatch not like "No known drug-drug interactions found."
```

## Drug Interactions Truthful Fallback

When public label text is also unavailable, the interactions section must say so explicitly rather than implying the drug has no interactions.

```bash
out="$(biomcp get drug pembrolizumab interactions)"
echo "$out" | mustmatch like "## Interactions"
echo "$out" | mustmatch like "Interaction details not available from public sources."
echo "$out" | mustmatch not like "No known drug-drug interactions found."
```

## Drug to Trials

Drug trial helper on the default CTGov path should inherit intervention alias
expansion, surface the matched alias, and keep the strict-literal opt-out
visible in the query echo.

```bash timeout=180
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" drug trials daraxonrasib --limit 10)"
strict="$("$bin" drug trials daraxonrasib --no-alias-expand --limit 10)"
echo "$out" | mustmatch like "intervention=daraxonrasib"
echo "$out" | mustmatch like "Matched Intervention"
echo "$out" | mustmatch like "|RMC-6236|"
echo "$strict" | mustmatch like "alias_expand=off"
```

## Drug Trials Help Documents Alias Expansion

The helper help text should document that the CTGov path inherits trial alias
expansion and exposes the strict-literal opt-out flag.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" drug trials --help)"
list_out="$("$bin" list drug)"
echo "$out" | mustmatch like "--no-alias-expand"
echo "$out" | mustmatch like "inherits intervention alias expansion"
echo "$out" | mustmatch like "Matched Intervention"
echo "$out" | mustmatch like "matched_intervention_label"
echo "$list_out" | mustmatch like "drug trials <name> [--no-alias-expand]"
echo "$list_out" | mustmatch like "inherits CTGov intervention alias expansion"
```

## Drug to Adverse Events

This helper links a therapy directly to adverse-event reporting data. For drugs with
FAERS coverage, the existing report table remains the primary contract and should not
grow a ClinicalTrials.gov fallback section.

```bash
out="$(biomcp drug adverse-events pembrolizumab --limit 3)"
echo "$out" | mustmatch like "# Adverse Events: drug=pembrolizumab"
echo "$out" | mustmatch like "|Report ID|Drug|Reactions|Serious|"
echo "$out" | mustmatch not like "Trial-Reported Adverse Events (ClinicalTrials.gov)"
```

## Drug Adverse Events CTGov Fallback (fixture)

When FAERS returns 404, the helper should say why FAERS is empty for
investigational or newly approved drugs, then append a clearly labeled
ClinicalTrials.gov trial-results section.

```bash
bin="${BIOMCP_BIN:-biomcp}"
bash fixtures/setup-drug-ae-fallback-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-drug-ae-fallback-env"
out="$("$bin" drug adverse-events daraxonrasib --limit 5)"
echo "$out" | mustmatch like "Drug not found in FAERS. FAERS is a post-marketing database"
echo "$out" | mustmatch like "## Trial-Reported Adverse Events (ClinicalTrials.gov)"
echo "$out" | mustmatch like "| Rash | 2 |"
```

## Drug Adverse Events CTGov Fallback JSON (fixture)

The JSON contract keeps the FAERS-shaped top-level fields but marks the 404 branch
explicitly and emits aggregated trial adverse-event terms with source-aware naming.

```bash
bin="${BIOMCP_BIN:-biomcp}"
bash fixtures/setup-drug-ae-fallback-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-drug-ae-fallback-env"
json_out="$("$bin" --json drug adverse-events daraxonrasib --limit 5)"
echo "$json_out" | mustmatch like '"faers_not_found": true'
echo "$json_out" | mustmatch like '"trial_adverse_events":'
echo "$json_out" | jq -e '.faers_not_found == true' > /dev/null
echo "$json_out" | jq -e '.trial_adverse_events | length >= 3' > /dev/null
echo "$json_out" | jq -e '.trial_adverse_events[0].term == "Rash"' > /dev/null
```

## Drug Adverse Events CTGov Fallback Empty (fixture)

If FAERS is 404 and ClinicalTrials.gov has no posted adverse-event terms, the helper
should stay truthful about both sources and omit the fallback table instead of
inventing empty rows.

```bash
bin="${BIOMCP_BIN:-biomcp}"
bash fixtures/setup-drug-ae-fallback-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-drug-ae-fallback-env"
out="$("$bin" drug adverse-events ctgov-empty --limit 5)"
echo "$out" | mustmatch like "Drug not found in FAERS. FAERS is a post-marketing database"
echo "$out" | mustmatch like "ClinicalTrials.gov did not return posted trial adverse events"
echo "$out" | mustmatch not like "## Trial-Reported Adverse Events (ClinicalTrials.gov)"
```

## Drug Adverse Events FAERS Empty Does Not Fallback (fixture)

A FAERS 200 response with an empty result set is different from a FAERS 404. This
branch should explain that the drug matched FAERS indexing but no events matched,
and it must not query or render ClinicalTrials.gov fallback content.

```bash
bin="${BIOMCP_BIN:-biomcp}"
bash fixtures/setup-drug-ae-fallback-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-drug-ae-fallback-env"
out="$("$bin" drug adverse-events faers-empty --limit 5)"
echo "$out" | mustmatch like "Drug found in FAERS, but no events matched your filters"
echo "$out" | mustmatch not like "Trial-Reported Adverse Events (ClinicalTrials.gov)"
echo "$out" | mustmatch not like "| Rash | 2 |"
```

## Search Adverse Event Disambiguation (fixture)

`search adverse-event` text should distinguish FAERS 404 from FAERS 200+empty without
ever triggering or rendering ClinicalTrials.gov fallback content.

```bash
bin="${BIOMCP_BIN:-biomcp}"
bash fixtures/setup-drug-ae-fallback-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-drug-ae-fallback-env"
out_404="$("$bin" search adverse-event -d daraxonrasib --limit 5)"
out_empty="$("$bin" search adverse-event -d faers-empty --limit 5)"
echo "$out_404" | mustmatch like "Drug not found in FAERS. FAERS is a post-marketing database"
echo "$out_404" | mustmatch not like "Trial-Reported Adverse Events (ClinicalTrials.gov)"
echo "$out_empty" | mustmatch like "Drug found in FAERS, but no events matched your filters"
echo "$out_empty" | mustmatch not like "Trial-Reported Adverse Events (ClinicalTrials.gov)"
```

## Adverse Event Search

Direct adverse-event search is useful for safety reconnaissance independent of drug metadata. We verify the heading and stable summary marker.

```bash
out="$(biomcp search adverse-event -d ibuprofen --limit 3)"
echo "$out" | mustmatch like "# Adverse Events: drug=ibuprofen"
echo "$out" | mustmatch like "Total reports (OpenFDA)"
```

## FAERS JSON Next Commands

Non-empty adverse-event search JSON should expose machine-readable follow-up
commands for the top report and the command-family reference.

```bash
json_out="$(biomcp --json search adverse-event -d ibuprofen --limit 3)"
echo "$json_out" | mustmatch like '"next_commands":'
echo "$json_out" | jq -e '._meta.next_commands[0] | test("^biomcp get adverse-event .+$")' > /dev/null
echo "$json_out" | jq -e '._meta.next_commands | any(. == "biomcp list adverse-event")' > /dev/null
```

## Drug List Documents Adverse Event Fallback

The list surface should document the repaired empty-state wording and the JSON fields
that identify when ClinicalTrials.gov contributed fallback data.

```bash
bin="${BIOMCP_BIN:-biomcp}"
list_out="$("$bin" list drug)"
echo "$list_out" | mustmatch like '`drug adverse-events <name>` - checks FAERS first'
echo "$list_out" | mustmatch like "falls back to ClinicalTrials.gov trial-reported adverse events only on FAERS 404"
echo "$list_out" | mustmatch like "faers_not_found"
echo "$list_out" | mustmatch like "trial_adverse_events"
```

## Brand Name Search Uses Exact Match Ranking

Brand-only MyChem hits should still render search rows with a usable canonical
name. The OpenFDA rescue path should prefer the exact Keytruda label over the
newer KEYTRUDA QLEX combo label and respect the requested limit/total text.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" search drug Keytruda --region us --limit 1)"
echo "$out" | mustmatch like "# Drugs: Keytruda"
echo "$out" | mustmatch like "Found 1 drug"
echo "$out" | mustmatch like "|Name|Mechanism|Target|"
echo "$out" | mustmatch like "pembrolizumab"
echo "$out" | mustmatch not like "pembrolizumab and berahyaluronidase alfa-pmph"
```

## EMA Search Region

The EMA human-medicine fixture should support EU-only search rows with the EMA
product number and authorization status while still honoring existing drug
normalization.

```bash
bash fixtures/setup-ema-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-ema-env"
out="$(biomcp search drug Keytruda --region eu --limit 5)"
echo "$out" | mustmatch like "# Drugs: Keytruda"
echo "$out" | mustmatch like "|Name|Active Substance|EMA Number|Status|"
echo "$out" | mustmatch like "|Keytruda|pembrolizumab|EMEA/H/C/003820|Authorised|"
echo "$out" | mustmatch like "pembrolizumab"
echo "$out" | mustmatch like "EMEA/H/C/003820"
echo "$out" | mustmatch like "Authorised"
```

## EMA Influenza Vaccine Search

EMA search should also match cleaned therapeutic-indication text so natural
queries such as "influenza vaccine" surface the seeded influenza products even
when the phrase does not appear in the medicine name.

```bash
bash fixtures/setup-ema-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-ema-env"
out="$(biomcp search drug --region ema -q 'influenza vaccine' --limit 5)"
echo "$out" | mustmatch like "# Drugs: influenza vaccine"
echo "$out" | mustmatch like "|Flucelvax Tetra|"
echo "$out" | mustmatch like "|Fluad Tetra|"
```

## WHO Search Region

The WHO fixture should support WHO-only search rows with the WHO reference
number, listing basis, and normalized prequalification date.

```bash
bash fixtures/setup-who-pq-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-who-pq-env"
out="$(biomcp search drug trastuzumab --region who --limit 5)"
echo "$out" | mustmatch like "# Drugs: trastuzumab"
echo "$out" | mustmatch like "|INN|Therapeutic Area|Dosage Form|Applicant|WHO Ref|Listing Basis|Date|"
echo "$out" | mustmatch like "Trastuzumab"
echo "$out" | mustmatch like "Trastuzumab|Oncology"
echo "$out" | mustmatch like "Samsung Bioepis NL B.V.|BT-ON001"
echo "$out" | mustmatch like "Prequalification - Abridged"
echo "$out" | mustmatch like "2019-12-18"
```

## WHO Structured Search Region

Structured WHO search should keep the structured U.S. drug-search semantics and
filter the candidate hits through the WHO prequalification batch.

```bash
bash fixtures/setup-who-pq-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-who-pq-env"
out="$(biomcp search drug --indication malaria --region who --limit 5)"
echo "$out" | mustmatch like "|INN|Therapeutic Area|Dosage Form|Applicant|WHO Ref|Listing Basis|Date|"
echo "$out" | mustmatch like "Artemether/Lumefantrine"
echo "$out" | mustmatch like "Artemether/Lumefantrine|Malaria"
echo "$out" | mustmatch like "Novartis Pharma AG|MA026"
```

## Default Drug Search Covers US, EU, and WHO

Omitting `--region` on a plain name query should render the same split
U.S./EU/WHO layout as the explicit all-regions mode.

```bash
bash fixtures/setup-ema-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-ema-env"
bash fixtures/setup-who-pq-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-who-pq-env"
out="$(biomcp search drug Keytruda --limit 5)"
echo "$out" | mustmatch like "# Drugs: Keytruda"
echo "$out" | mustmatch like "## US (MyChem.info / OpenFDA)"
echo "$out" | mustmatch like "## EU (EMA)"
echo "$out" | mustmatch like "## WHO (WHO Prequalification)"
echo "$out" | mustmatch like "EMEA/H/C/003820"
```

## All-Region Search Covers WHO

`--region all` should render separate labeled U.S., EU, and WHO result blocks
instead of flattening them into one unlabeled table.

```bash
bash fixtures/setup-ema-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-ema-env"
bash fixtures/setup-who-pq-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-who-pq-env"
out="$(biomcp search drug Keytruda --region all --limit 5)"
echo "$out" | mustmatch like "# Drugs: Keytruda"
echo "$out" | mustmatch like "## US (MyChem.info / OpenFDA)"
echo "$out" | mustmatch like "## EU (EMA)"
echo "$out" | mustmatch like "## WHO (WHO Prequalification)"
echo "$out" | mustmatch like "EMEA/H/C/003820"
```

## EMA Regulatory Section

The EU regulatory section should anchor on the EMA medicine row and show recent
post-authorisation activity.

```bash
bash fixtures/setup-ema-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-ema-env"
out="$(biomcp get drug Keytruda regulatory --region eu)"
echo "$out" | mustmatch like "## Regulatory (EU"
echo "$out" | mustmatch like "EMEA/H/C/003820"
echo "$out" | mustmatch like "Authorised"
echo "$out" | mustmatch like "27/02/2026"
```

## EMA Alias Regulatory Section

`--region ema` should be accepted as an alias for the canonical `eu` region and
render the repaired EMA regulatory data, including the marketing-authorisation
date and authorized indication text.

```bash
bash fixtures/setup-ema-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-ema-env"
out="$(biomcp get drug Dupixent regulatory --region ema)"
echo "$out" | mustmatch like "## Regulatory (EU"
echo "$out" | mustmatch like "EMEA/H/C/004390"
echo "$out" | mustmatch like "26/09/2017"
echo "$out" | mustmatch like "### Authorized indications"
echo "$out" | mustmatch like "atopic dermatitis"
```

## Default Drug Regulatory Covers US And EU

Omitting `--region` on the direct regulatory path should surface the combined
U.S. and EU regulatory blocks while keeping the regular no-flag `get drug ...
all` behavior unchanged.

```bash
bash fixtures/setup-ema-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-ema-env"
bash fixtures/setup-who-pq-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-who-pq-env"
out="$(biomcp get drug nivolumab regulatory)"
echo "$out" | mustmatch like "## Regulatory (US - Drugs@FDA)"
echo "$out" | mustmatch like "## Regulatory (EU - EMA)"
echo "$out" | mustmatch like "EMEA/H/C/003985"
```

## WHO Regulatory Section

The WHO regulatory section should anchor on WHO medicine rows and show the WHO
reference, listing basis, and normalized prequalification date.

```bash
bash fixtures/setup-who-pq-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-who-pq-env"
out="$(biomcp get drug trastuzumab regulatory --region who)"
echo "$out" | mustmatch like "## Regulatory (WHO Prequalification)"
echo "$out" | mustmatch like "| WHO Ref | Presentation |"
echo "$out" | mustmatch like "Presentation"
echo "$out" | mustmatch like "Prequalification Date"
echo "$out" | mustmatch like "| BT-ON001 | Trastuzumab Powder"
echo "$out" | mustmatch like "2019-12-18"
```

## WHO Regulatory Empty State

When a drug is not WHO-prequalified, the WHO regulatory path should return the
truthful empty-state copy instead of an error.

```bash
bash fixtures/setup-who-pq-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-who-pq-env"
out="$(biomcp get drug imatinib regulatory --region who)"
echo "$out" | mustmatch like "Not WHO-prequalified"
```

## WHO Unsupported Sections Reject Fast

WHO regional data is regulatory-only. Explicit `safety` and `shortage`
requests with `--region who` should fail before any data fetch.

```bash
out="$(biomcp get drug trastuzumab safety --region who 2>&1 || true)"
echo "$out" | mustmatch like "Error: Invalid argument: WHO regional data currently supports regulatory only"

out="$(biomcp get drug trastuzumab shortage --region who 2>&1 || true)"
echo "$out" | mustmatch like "Error: Invalid argument: WHO regional data currently supports regulatory only"
```

## WHO All Section Keeps Only Supported Regional Blocks

`get drug <name> all --region who` should stay valid, keep the normal
nonregional sections, render WHO regulatory data, and omit unsupported WHO
safety/shortage regional blocks.

```bash
bash spec/fixtures/setup-who-pq-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-who-pq-env"
```

```bash
bash fixtures/setup-who-pq-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-who-pq-env"
out="$(biomcp get drug trastuzumab all --region who)"
echo "$out" | mustmatch like "## Regulatory (WHO Prequalification)"
echo "$out" | mustmatch like "| BT-ON001 | Trastuzumab Powder"
echo "$out" | mustmatch not like "## Safety ("
echo "$out" | mustmatch not like "## Shortage ("
```

## EMA Safety Truthful Empty Sections

The EU safety surface should render DHPC matches and keep referrals/PSUSAs
truthful when the EMA batch has no matching rows.

```bash
bash fixtures/setup-ema-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-ema-env"
out="$(biomcp get drug Ozempic safety --region eu)"
echo "$out" | mustmatch like "## Safety (EU"
echo "$out" | mustmatch like "| Medicine | Type | Outcome | First Published | Last Updated |"
echo "$out" | mustmatch like "Medicine shortage"
echo "$out" | mustmatch like "### Referrals"
echo "$out" | mustmatch like "No data found (EMA)"
echo "$out" | mustmatch like "### PSUSAs"
echo "$out" | mustmatch like "No data found (EMA)"
```

## EMA Shortage Section

EU shortage output should expose the EMA shortage status, alternatives flag,
and update date from the local batch.

```bash
bash fixtures/setup-ema-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-ema-env"
out="$(biomcp get drug Ozempic shortage --region eu)"
echo "$out" | mustmatch like "## Shortage (EU"
echo "$out" | mustmatch '/Resolved.*13\/01\/2026/'
echo "$out" | mustmatch '/Yes.*13\/01\/2026/'
echo "$out" | mustmatch like "13/01/2026"
```

## Mechanism Filter Finds Purine Analog Drugs

The mechanism filter should surface purine analogs even when the upstream text
labels only expose the ATC class or a non-purine NDC pharmacology class.

```bash
out="$(biomcp search drug --mechanism purine --limit 10)"
echo "$out" | mustmatch like "pentostatin"
echo "$out" | mustmatch like "nelarabine"
echo "$out" | mustmatch like "cladribine"
echo "$out" | mustmatch like "clofarabine"
echo "$out" | mustmatch like "fludarabine"
```

## Leukemia Query Keeps Purine Analogs Reachable

Combining indication and mechanism filters should still keep the expected
purine analog leukemia drugs visible.

```bash
out="$(biomcp search drug --indication leukemia --mechanism purine --limit 10)"
echo "$out" | mustmatch like "pentostatin"
echo "$out" | mustmatch like "nelarabine"
echo "$out" | mustmatch like "cladribine"
```

## Deoxycoformycin Resolves To Pentostatin

The alias lookup already works today and should stay covered by executable
proof so future normalization changes do not break it.

```bash
out="$(biomcp search drug deoxycoformycin --limit 5)"
echo "$out" | mustmatch like "pentostatin"
```
