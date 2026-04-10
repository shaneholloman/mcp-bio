# Cross-Entity See Also

This spec locks the approved cross-entity next-step hints that should teach the
typed BioMCP surfaces directly from normal output. Agents learn the right next
call from output context, not from proactively reading help — so every card and
empty-state must surface the structured path directly.

| Section | Command focus | Why it matters |
|---|---|---|
| Drug to PGx | `get drug warfarin` | Teaches the structured PGx surface from a drug card |
| Gene to PGx | `get gene TP53` | Teaches the PGx search from a gene card |
| Disease to Drug | `get disease melanoma` | Teaches indication-oriented drug search from a disease card |
| Variant Search Follow-up | `search variant -g SCN5A --condition "Brugada"` | Restores HATEOAS follow-up commands on variant result pages |
| Gene More ordering | `get gene NANOG` | Keeps `ontology` at equal prominence in follow-up sections |
| Oncology study local match | `get disease "breast cancer" genes` | Prefers executable `study top-mutated` when a local study exists |
| Oncology study fallback | `get disease melanoma genes` | Falls back to `study download --list` when no local study can be chosen |
| Disease zero-result discover | `search disease definitelynotarealdisease` | Teaches `discover` when disease search is empty |
| Drug zero-result discover | `search drug definitelynotarealdrugname --region us` | Teaches `discover` when drug search is empty |
| Article curated pivots | `get article 22663011` | Promotes executable entity pivots ahead of citation chasing |
| Completed trial results guidance | `get trial NCT02576665` | Promotes result-oriented follow-up before generic condition pivots |

## Drug to PGx

Drug cards should advertise the typed PGx search directly in normal markdown
output so agents can pivot without guessing the command shape.

```bash
out="$(biomcp get drug warfarin)"
echo "$out" | mustmatch like "biomcp search pgx -d warfarin"
echo "$out" | mustmatch like "pharmacogenomics interactions"
```

The JSON contract should expose the same next command in `_meta.next_commands`.

```bash
out="$(biomcp --json get drug warfarin)"
echo "$out" | jq -e '._meta.next_commands | index("biomcp search pgx -d warfarin") != null' > /dev/null
```

## Gene to PGx

Gene cards should point to the PGx search in both markdown and JSON because the
same hint powers agentic follow-up planning across renderers.

```bash
out="$(biomcp get gene TP53)"
echo "$out" | mustmatch like "biomcp search pgx -g TP53"
echo "$out" | mustmatch like "pharmacogenomics interactions"

braf_out="$(biomcp get gene BRAF)"
echo "$braf_out" | mustmatch like "biomcp search pgx -g BRAF"
if echo "$braf_out" | grep -F "biomcp get pgx BRAF" >/dev/null; then
  echo "unexpected stale gene->pgx command" >&2
  exit 1
fi
```

```bash
out="$(biomcp --json get gene TP53)"
echo "$out" | jq -e '._meta.next_commands | index("biomcp search pgx -g TP53") != null' > /dev/null

pgx_out="$(biomcp search pgx -g TP53 --limit 3)"
echo "$pgx_out" | mustmatch like "No PGx interactions found."
echo "$pgx_out" | mustmatch like "# PGx Search: gene=TP53"
```

## Disease to Drug

Disease cards should point to typed indication search so the follow-up command
returns treatment-oriented drug results instead of name matches.

```bash
out="$(biomcp get disease melanoma)"
echo "$out" | mustmatch like 'biomcp search drug --indication "melanoma"'
echo "$out" | mustmatch like "treatment options for this condition"
if echo "$out" | grep -F "biomcp search drug melanoma" >/dev/null; then
  echo "unexpected positional disease->drug command" >&2
  exit 1
fi
```

```bash
out="$(biomcp --json get disease melanoma)"
echo "$out" | jq -e '._meta.next_commands | index("biomcp search drug --indication \"melanoma\"") != null' > /dev/null

drug_out="$(biomcp search drug --indication "melanoma" --limit 5)"
echo "$drug_out" | mustmatch like "# Drugs: indication=melanoma"
echo "$drug_out" | mustmatch like "pembrolizumab"
```

## Variant Search Follow-up

Variant search result pages should expose the next executable detail pivot plus
the gene and condition context that produced the list.

```bash
out="$(biomcp search variant -g SCN5A --condition "Brugada" --limit 3)"
echo "$out" | mustmatch like $'See also:\n  biomcp get variant '
echo "$out" | mustmatch like "biomcp get variant "
echo "$out" | mustmatch like "biomcp get gene SCN5A"
echo "$out" | mustmatch like "biomcp search disease --query Brugada"
```

## Gene More Ordering

This ticket should not demote `ontology`; the default gene card still needs the
top follow-up trio to stay `pathways`, `ontology`, and `diseases`.

```bash
out="$(biomcp get gene NANOG)"
echo "$out" | mustmatch like $'More:\n  biomcp get gene NANOG pathways'
echo "$out" | mustmatch like "biomcp get gene NANOG pathways"
echo "$out" | mustmatch like "biomcp get gene NANOG ontology"
echo "$out" | mustmatch like "biomcp get gene NANOG diseases"
```

## Oncology Study Local Match

When oncology context and a matching local study are both present, the disease
card should suggest the executable `study top-mutated` command.

```bash
bash fixtures/setup-study-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-study-env"
out="$(biomcp get disease "breast cancer" genes)"
echo "$out" | mustmatch like "biomcp study top-mutated --study brca_tcga_pan_can_atlas_2018"
echo "$out" | mustmatch like "mutation frequency ranking"
```

## Oncology Study Fallback

When there is no usable local study match, the disease card should still teach
the next structured step by falling back to the study catalog.

```bash
empty_root="$(mktemp -d)"
out="$(BIOMCP_STUDY_DIR="$empty_root" biomcp get disease melanoma genes)"
echo "$out" | mustmatch like "biomcp study download --list"
echo "$out" | mustmatch like "browse downloadable cancer genomics studies"
rm -rf "$empty_root"
```

## Disease Zero-Result Discover

Empty disease searches should redirect users to `discover` with the original
query preserved in the suggested command.

```bash
out="$(biomcp search disease definitelynotarealdisease --limit 3)"
echo "$out" | mustmatch like "Try: biomcp discover definitelynotarealdisease"
echo "$out" | mustmatch like "resolve abbreviations and synonyms"
```

## Drug Zero-Result Discover

Empty drug searches should do the same, nudging users toward `discover` when a
trial code or alias is more likely than a canonical drug name match.

```bash
out="$(biomcp search drug definitelynotarealdrugname --region us --limit 3)"
echo "$out" | mustmatch like "Try: biomcp discover definitelynotarealdrugname"
echo "$out" | mustmatch like "resolve drug trial codes and aliases"
```

## Article Curated Pivots

Article cards should keep the explicit entity-helper escape hatch first, then
promote executable typed pivots before citation-chain expansion.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" get article 22663011)"
echo "$out" | mustmatch like "biomcp article entities 22663011"
echo "$out" | mustmatch like "biomcp search gene -q BRAF"
echo "$out" | mustmatch like "biomcp article citations 22663011 --limit 3"
if echo "$out" | grep -F "biomcp get gene serine-threonine protein kinase" >/dev/null; then
  echo "unexpected stale raw article gene command" >&2
  exit 1
fi
entities_line="$(printf '%s\n' "$out" | grep -n 'biomcp article entities 22663011' | head -n1 | cut -d: -f1)"
gene_line="$(printf '%s\n' "$out" | grep -n 'biomcp search gene -q BRAF' | head -n1 | cut -d: -f1)"
citations_line="$(printf '%s\n' "$out" | grep -n 'biomcp article citations 22663011 --limit 3' | head -n1 | cut -d: -f1)"
test -n "$entities_line"
test -n "$gene_line"
test -n "$citations_line"
test "$entities_line" -lt "$gene_line"
test "$gene_line" -lt "$citations_line"
```

## Completed Trial Results Guidance

Completed or terminated trial cards should point to likely result publications
before the generic disease/article/trial pivots for the condition.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" get trial NCT02576665)"
echo "$out" | mustmatch like 'biomcp search article --drug "Toca 511" -q "NCT02576665 A Study of Toca'
echo "$out" | mustmatch like "find publications or conference reports from this completed/terminated trial"
results_line="$(printf '%s\n' "$out" | grep -n 'biomcp search article --drug "Toca 511"' | head -n1 | cut -d: -f1)"
disease_line="$(printf '%s\n' "$out" | grep -n 'biomcp search disease --query "Colorectal Cancer"' | head -n1 | cut -d: -f1)"
test -n "$results_line"
test -n "$disease_line"
test "$results_line" -lt "$disease_line"
```
