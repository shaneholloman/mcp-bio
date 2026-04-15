# Pharmacogenomics Queries

PGx commands surface CPIC and PharmGKB-aligned interaction context for gene-drug decision support. This file validates core query modes and recommendation views around CYP2D6 and warfarin. Assertions prioritize durable section headings and table schemas.

| Section | Command focus | Why it matters |
|---|---|---|
| Gene search | `search pgx -g CYP2D6` | Confirms gene-centric PGx search |
| Drug search | `search pgx -d warfarin` | Confirms drug-centric PGx search |
| PGx detail | `get pgx CYP2D6` | Confirms expanded interaction card |
| CPIC level filter | `search pgx ... --cpic-level A` | Confirms evidence-level filtering |
| Recommendations | `get pgx CYP2D6 recommendations` | Confirms recommendation table output |
| Population frequencies | `get pgx DPYD frequencies` | Confirms optional frequency fields render without template errors |

## Searching by Gene

Gene-centric PGx search should return interaction rows with CPIC context and guideline labels. We assert on heading context and canonical table columns.

```bash
out="$(biomcp search pgx -g CYP2D6 --limit 3)"
echo "$out" | mustmatch like "# PGx Search: gene=CYP2D6"
echo "$out" | mustmatch like "| Gene | Drug | CPIC Level | PGx Testing | Guideline |"
```

## Searching by Drug

Drug-centric PGx search is useful for medication review and genotype-aware prescribing. We check the drug heading and a case-insensitive warfarin match.

```bash
out="$(biomcp search pgx -d warfarin --limit 3)"
echo "$out" | mustmatch like "# PGx Search: drug=warfarin"
echo "$out" | mustmatch '/warfarin/i'
```

## Search JSON Next Commands

Non-empty PGx search JSON should expose the best detail pivot and the full PGx
command-family reference in `_meta.next_commands`.

```bash
json_out="$(biomcp --json search pgx -g CYP2D6 --limit 3)"
echo "$json_out" | mustmatch like '"next_commands":'
echo "$json_out" | jq -e '._meta.next_commands[0] | test("^biomcp get pgx .+$")' > /dev/null
echo "$json_out" | jq -e '._meta.next_commands | any(. == "biomcp list pgx")' > /dev/null
```

## Getting PGx Details

`get pgx` aggregates affected drugs and recommendation context for the query anchor. The assertions verify card heading and affected-drug summary marker.

```bash
out="$(biomcp get pgx CYP2D6)"
echo "$out" | mustmatch like "# PGx: CYP2D6"
echo "$out" | mustmatch like "Affected Drugs:"
```

## Filtering by CPIC Level

CPIC filtering narrows results to specific evidence tiers in prescribing workflows. The query echo and gene row token should both be present.

```bash
out="$(biomcp search pgx -g CYP2D6 --cpic-level A --limit 3)"
echo "$out" | mustmatch like "cpic_level=A"
echo "$out" | mustmatch like "| CYP2D6 |"
```

## PGx Recommendations

Recommendation view should expose a compact, repeatable table for downstream review. We assert on section heading and recommendation table columns.

```bash
out="$(biomcp get pgx CYP2D6 recommendations)"
echo "$out" | mustmatch like "# CYP2D6 - recommendations"
echo "$out" | mustmatch like "| Drug | Gene | CPIC Level | PGx Testing |"
```

## Population Frequencies

Population-frequency tables should render placeholder cells when CPIC omits
optional frequency metadata instead of failing in the template layer.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" get pgx DPYD frequencies)"
echo "$out" | mustmatch like "# DPYD - frequencies"
echo "$out" | mustmatch like "## Population Frequencies (CPIC)"
echo "$out" | mustmatch like "| Gene | Allele | Population | Frequency | Subjects |"
echo "$out" | mustmatch not like "Template error"
```
