# Skill Workflows

BioMCP now ships a concise overview plus an embedded worked-example catalog. This
file validates the layered skill behavior: overview, catalog listing, and opening
numbered or slugged examples through the existing `show_use_case()` path.

| Section | Command focus | Why it matters |
|---|---|---|
| Skill overview | `biomcp skill` | Confirms the overview is routing-first and concise |
| List worked examples | `biomcp skill list` | Confirms the embedded catalog is populated |
| Open numeric example | `biomcp skill 01` | Confirms numbered use-cases still resolve |
| Open slug example | `biomcp skill article-follow-up` | Confirms slug lookups open the expected markdown |

## Skill Overview

The overview should teach routing rules, teach when to stop searching and
answer from supported evidence, and then point the user to the worked examples
instead of inlining every workflow.

```bash
out="$(biomcp skill)"
echo "$out" | mustmatch like "## Routing rules"
echo "$out" | mustmatch like "## Section reference"
echo "$out" | mustmatch like "## Cross-entity pivot rules"
echo "$out" | mustmatch like "## Output and evidence rules"
echo "$out" | mustmatch like "## Answer commitment"
echo "$out" | mustmatch like "If one command already answers the question, stop searching and answer"
echo "$out" | mustmatch like "biomcp get drug nivolumab regulatory"
echo "$out" | mustmatch like "If 3+ searches keep returning relevant papers"
echo "$out" | mustmatch like 'Run `biomcp skill list` for worked examples'
```

## Listing Skills

`biomcp skill list` should now render the embedded worked-example catalog.

```bash
out="$(biomcp skill list)"
echo "$out" | mustmatch like "# BioMCP Worked Examples"
echo "$out" | mustmatch like "01 treatment-lookup"
echo "$out" | mustmatch like "02 symptom-phenotype"
echo "$out" | mustmatch like "03 gene-disease-orientation"
echo "$out" | mustmatch like "04 article-follow-up"
```

## Viewing a Skill by Number

Numeric addressing should open the numbered worked example through the existing
loader and show executable commands, not a not-found error.

```bash
out="$(biomcp skill 01)"
echo "$out" | mustmatch like "# Pattern: Treatment / approved-drug lookup"
echo "$out" | mustmatch like 'biomcp search drug --indication "myasthenia gravis" --limit 5'
echo "$out" | mustmatch like "biomcp get drug pyridostigmine"
```

## Viewing a Skill by Slug

Slug addressing should open the matching worked example and preserve the
citation/recommendation workflow commands.

```bash
out="$(biomcp skill article-follow-up)"
echo "$out" | mustmatch like "# Pattern: Article follow-up via citations and recommendations"
echo "$out" | mustmatch like "biomcp article citations 22663011 --limit 5"
echo "$out" | mustmatch like "biomcp article recommendations 22663011 --limit 5"
```
