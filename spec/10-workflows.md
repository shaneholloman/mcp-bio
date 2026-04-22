# Skill Workflows

BioMCP now ships a concise overview plus an embedded worked-example catalog.
This file validates the layered skill behavior: canonical prompt rendering,
catalog listing, opening numbered or slugged examples, and install byte parity.

| Section | Command focus | Why it matters |
|---|---|---|
| Skill overview | `biomcp skill` | Confirms the overview is routing-first and concise |
| Canonical render | `biomcp skill render` | Confirms the scriptable prompt surface matches the overview |
| Install parity | `biomcp skill install <dir> --force` | Confirms installed `SKILL.md` is byte-identical to redirected render stdout |
| Workflow ladder sidecars | `biomcp skill install <dir> --force` | Confirms schema and sidecar assets install with the skill tree |
| List worked examples | `biomcp skill list` | Confirms the embedded catalog is populated |
| Open numeric example | `biomcp skill 01` | Confirms numbered use-cases still resolve |
| Open slug example | `biomcp skill variant-pathogenicity` | Confirms slug lookups open the expected markdown |

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

## Workflow Ladder Sidecars Install

Sidecar-backed ladders are installed as data assets next to the markdown
playbooks. They must not appear in the human-facing skill catalog, but agents
can inspect them on disk after `biomcp skill install`.

```bash
tmp="$(mktemp -d)"
agent="$tmp/agent"
biomcp skill install "$agent" --force
test -f "$agent/skills/biomcp/schemas/workflow-ladder.schema.json"
for slug in treatment-lookup article-follow-up variant-pathogenicity trial-recruitment mechanism-pathway pharmacogene-cumulative mutation-catalog; do
  test -f "$agent/skills/biomcp/use-cases/$slug.ladder.json"
done
out="$(biomcp skill list)"
echo "$out" | mustmatch not like ".ladder.json"
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
echo "$out" | mustmatch like "05 variant-pathogenicity"
echo "$out" | mustmatch like "06 drug-regulatory"
echo "$out" | mustmatch like "07 gene-function-localization"
echo "$out" | mustmatch like "08 mechanism-pathway"
echo "$out" | mustmatch like "09 trial-recruitment"
echo "$out" | mustmatch like "10 pharmacogene-cumulative"
echo "$out" | mustmatch like "11 disease-locus-mapping"
echo "$out" | mustmatch like "12 cellular-process-regulation"
echo "$out" | mustmatch like "13 mutation-catalog"
echo "$out" | mustmatch like "14 syndrome-disambiguation"
echo "$out" | mustmatch like "15 negative-evidence"
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

out="$(biomcp skill 05)"
echo "$out" | mustmatch like "# Pattern: Variant pathogenicity evidence"
echo "$out" | mustmatch like 'biomcp get variant "BRAF V600E" clinvar predictions population'
echo "$out" | mustmatch like 'biomcp variant trials "BRAF V600E" --limit 5'
```

## Viewing a Skill by Slug

Slug addressing should open the matching worked example and preserve the
variant evidence workflow commands.

```bash
out="$(biomcp skill variant-pathogenicity)"
echo "$out" | mustmatch like "# Pattern: Variant pathogenicity evidence"
echo "$out" | mustmatch like 'biomcp get variant "BRAF V600E" civic cgi'
echo "$out" | mustmatch like 'biomcp variant articles "BRAF V600E" --limit 5'
```
