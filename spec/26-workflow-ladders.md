# Workflow Ladders

Workflow ladders are sidecar-backed JSON metadata for first-call HATEOAS. They
are not markdown output and they are not new MCP resources. `_meta.next_commands`
remains the dynamic one-hop follow-up list; `_meta.workflow` and
`_meta.ladder[]` name one static multi-step worked-example path.

| Section | Command focus | Why it matters |
|---|---|---|
| Treatment lookup | `search drug --indication ... --json` | Structured indication search can emit `treatment-lookup` |
| Article follow-up | `get article ... --json` | Annotated articles can emit `article-follow-up` |
| Variant pathogenicity | `get variant ... --json` | ClinVar-backed variants can emit `variant-pathogenicity` |
| Mechanism pathway | `get gene ... --json` | Reactome-backed genes can emit `mechanism-pathway` without widening the visible default card |
| Pharmacogene cumulative | `get drug warfarin --json` | Drugs with three or more CPIC genes can emit `pharmacogene-cumulative` |
| Disease chooser | `search disease ... --json` | Disease search emits at most one workflow and mutation catalog has priority over trial recruitment |
| Probe degradation | JSON first calls | Optional probe failures omit workflow metadata rather than failing the primary response |

## Treatment Lookup

Structured indication search should keep the drug region envelope while adding
the workflow ladder when the result page is non-empty.

```bash
out="$(biomcp search drug --indication "myasthenia gravis" --limit 5 --json)"
echo "$out" | jq -e '.region == "us"' > /dev/null
echo "$out" | jq -e '._meta.workflow == "treatment-lookup"' > /dev/null
echo "$out" | jq -e '._meta.ladder[0].command == "biomcp search drug --indication \"myasthenia gravis\" --limit 5"' > /dev/null
echo "$out" | jq -e '._meta.next_commands | any(. == "biomcp list drug")' > /dev/null
```

## Article Follow-up

An article with PubTator-linked entities should include the article follow-up
ladder. The static ladder remains the playbook path even when the input PMID is
the same as the playbook seed.

```bash
out="$(biomcp get article 22663011 --json)"
echo "$out" | jq -e '._meta.workflow == "article-follow-up"' > /dev/null
echo "$out" | jq -e '._meta.ladder[0].command == "biomcp get article 22663011 annotations"' > /dev/null
echo "$out" | jq -e '._meta.next_commands | index("biomcp article entities 22663011") != null' > /dev/null
```

## Variant Pathogenicity

Default variant JSON should be able to emit the ladder when the pre-strip
ClinVar signal exists.

```bash
out="$(biomcp get variant "BRAF V600E" --json)"
echo "$out" | jq -e '._meta.workflow == "variant-pathogenicity"' > /dev/null
echo "$out" | jq -e '._meta.ladder | length == 4' > /dev/null
echo "$out" | jq -e '._meta.ladder[0].command == "biomcp get variant \"BRAF V600E\" clinvar predictions population"' > /dev/null
```

## Mechanism Pathway

Gene JSON can emit a mechanism-pathway ladder from a bounded Reactome probe
without forcing the default `pathways` section into the visible payload.

```bash
out="$(biomcp get gene ABL1 --json)"
echo "$out" | jq -e '._meta.workflow == "mechanism-pathway"' > /dev/null
echo "$out" | jq -e 'has("pathways") | not' > /dev/null
echo "$out" | jq -e '._meta.ladder[2].command == "biomcp get gene ABL1 pathways protein"' > /dev/null
```

## Pharmacogene Cumulative

Warfarin should cross the CPIC distinct-gene threshold and aspirin should not.

```bash
warfarin="$(biomcp get drug warfarin --json)"
echo "$warfarin" | jq -e '._meta.workflow == "pharmacogene-cumulative"' > /dev/null
echo "$warfarin" | jq -e '._meta.ladder[0].command == "biomcp search pgx -d warfarin --limit 10"' > /dev/null
echo "$warfarin" | jq -e '._meta.next_commands | index("biomcp search pgx -d warfarin") != null' > /dev/null

aspirin="$(biomcp get drug aspirin --json)"
echo "$aspirin" | jq -e '._meta | has("workflow") | not' > /dev/null
echo "$aspirin" | jq -e '._meta | has("ladder") | not' > /dev/null
```

## Disease Workflow Chooser

Disease search emits at most one workflow. Mutation catalog wins when the top
disease also has recruiting trials and at least three pathogenic variants.

```bash
out="$(biomcp search disease tuberculosis --limit 5 --json)"
echo "$out" | jq -e '._meta.workflow == "mutation-catalog"' > /dev/null
echo "$out" | jq -e '._meta.ladder[0].command == "biomcp get gene PLN"' > /dev/null
echo "$out" | jq -e '([._meta.ladder[] | select(.command | contains("search trial"))] | length) == 0' > /dev/null
```

## Trial Recruitment

If the mutation threshold is not met but recruiting trials exist, disease search
can emit the trial-recruitment ladder.

```bash
out="$(biomcp search disease "tick-borne encephalitis" --limit 5 --json)"
echo "$out" | jq -e '._meta.workflow == "trial-recruitment"' > /dev/null
echo "$out" | jq -e '._meta.ladder[2].command == "biomcp search trial -c \"tick-borne encephalitis\" --status recruiting --limit 5"' > /dev/null
```
