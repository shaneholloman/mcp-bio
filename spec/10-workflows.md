# Skill Workflows

BioMCP now ships a concise overview plus an embedded worked-example catalog.
This file validates the layered skill behavior: canonical prompt rendering,
catalog listing, opening numbered or slugged examples, and install byte parity.

| Section | Command focus | Why it matters |
|---|---|---|
| Skill overview | `biomcp skill` | Confirms the overview is routing-first and concise |
| Canonical render | `biomcp skill render` | Confirms the scriptable prompt surface matches the overview |
| Install parity | `biomcp skill install <dir> --force` | Confirms installed `SKILL.md` is byte-identical to redirected render stdout |
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
echo "$out" | mustmatch like "## How-to reference"
echo "$out" | mustmatch like "## Anti-patterns"
echo "$out" | mustmatch not like "../docs/"
echo "$out" | mustmatch not like ".md)"
echo "$out" | mustmatch like 'biomcp search article -k "<query>" --type review --limit 5'
echo "$out" | mustmatch like "Never do more than 3 article searches for one question"
echo "$out" | mustmatch like "ClinicalTrials.gov usually does not index nicknames"
echo "$out" | mustmatch like "## Output and evidence rules"
echo "$out" | mustmatch like "## Answer commitment"
echo "$out" | mustmatch like "If one command already answers the question, stop searching and answer"
echo "$out" | mustmatch like "biomcp get drug nivolumab regulatory"
echo "$out" | mustmatch like "If 3+ searches keep returning relevant papers"
echo "$out" | mustmatch like 'Run `biomcp skill list` for worked examples'
```

## Canonical Prompt Render

`biomcp skill render` is the canonical agent-facing prompt surface. It should
match the default overview byte-for-byte when both are redirected from the CLI,
and it should not contain repo-relative markdown links that installed agents
cannot open.

```bash
tmp="$(mktemp -d)"
biomcp skill > "$tmp/overview.md"
biomcp skill render > "$tmp/render.md"
cmp "$tmp/overview.md" "$tmp/render.md"
cat "$tmp/render.md" | mustmatch like "## Routing rules"
cat "$tmp/render.md" | mustmatch like "## How-to reference"
cat "$tmp/render.md" | mustmatch not like "../docs/"
cat "$tmp/render.md" | mustmatch not like ".md)"
```

## Render Matches Install

The installed `SKILL.md` must be byte-identical to redirected canonical render
stdout, while the rest of the embedded skill tree still installs.

```bash
tmp="$(mktemp -d)"
agent="$tmp/agent"
biomcp skill render > "$tmp/rendered.md"
biomcp skill install "$agent" --force
cmp "$tmp/rendered.md" "$agent/skills/biomcp/SKILL.md"
test -d "$agent/skills/biomcp/use-cases"
cat "$agent/skills/biomcp/SKILL.md" | mustmatch like "## How-to reference"
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
echo "$out" | mustmatch not like "variant-to-treatment"
echo "$out" | mustmatch not like "drug-investigation"
echo "$out" | mustmatch not like "gene-function-lookup"
echo "$out" | mustmatch not like "trial-searching"
echo "$out" | mustmatch not like "literature-synthesis"
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
