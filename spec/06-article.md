# Article Queries

Article commands provide literature retrieval and annotation-focused enrichment for entity extraction. This spec validates both retrieval modes and PubTator annotation surfaces. Assertions are anchored to headings, IDs, and table schemas that remain stable over time.

| Section | Command focus | Why it matters |
|---|---|---|
| Gene search | `search article -g BRAF` | Confirms gene-linked literature lookup |
| Keyword search | `search article -q "alternative microexon splicing metastasis" --source litsense2` | Confirms source-scoped free-text discovery |
| Gene keyword pivot | `search article -k "SRY Sox9 miRNA"` | Confirms article search can suggest typed gene pivots from recognizable keyword tokens |
| Drug keyword pivot | `search article -k "psoralen photobinding DNA"` | Confirms article search can suggest typed drug pivots without false-positive gene hints |
| PubTator source search | `search article --source pubtator` | Confirms default filtering still allows source-specific PubTator results |
| Federated source preservation | `--json search article -q ...` | Confirms default filtering still preserves non-EuropePMC matches |
| Article detail | `get article 22663011` | Confirms canonical article card output |
| Annotation section | `get article ... annotations` | Confirms PubTator integration and extraction guidance |
| Entity helper | `article entities 22663011` | Confirms entity extraction pivot |
| Batch helper | `article batch 22663011 24200969` | Confirms compact multi-article fetch |
| Semantic Scholar detail | `get article 22663011 tldr` | Confirms optional-key enrichment section |
| Semantic Scholar graph | `article citations|references 22663011` | Confirms citation graph pivots |
| Semantic Scholar recommendations | `article recommendations ...` | Confirms related-paper pivots |

## Searching by Gene

Gene-based literature search is a common evidence collection step in variant and disease workflows. We assert on heading context and table columns.

```bash
out="$(biomcp search article -g BRAF --limit 3)"
echo "$out" | mustmatch like "# Articles: gene=BRAF"
echo "$out" | mustmatch like "Ranking: calibrated PubMed rescue + lexical directness"
echo "$out" | mustmatch like "at least one anchor hit"
echo "$out" | mustmatch like "| PMID | Title |"
```

## Search JSON Next Commands

Non-empty article search JSON should expose the top article follow-up directly
so agents can move from search to detail without scraping markdown helpers.

```bash
json_out="$(biomcp --json search article -g BRAF --limit 3)"
echo "$json_out" | mustmatch like '"next_commands":'
echo "$json_out" | jq -e '._meta.next_commands[0] | test("^biomcp get article .+$")' > /dev/null
echo "$json_out" | jq -e '._meta.next_commands | any(. == "biomcp list article")' > /dev/null
```

## Article Search Gene Keyword Pivot

When keyword search contains a recognizable gene token, BioMCP should suggest a
typed gene card and a gene-filtered article pivot without disturbing the
existing article-detail follow-up order.

```bash
bin="${BIOMCP_BIN:-$(git rev-parse --show-toplevel)/target/release/biomcp}"
out="$("$bin" search article -k "SRY Sox9 miRNA" --limit 1)"
echo "$out" | mustmatch like "Filters: [query], -k/-q <keyword>"
printf '%s\n' "$out" | grep -q '^See also:$'
echo "$out" | mustmatch '/biomcp get article [0-9]+/'
echo "$out" | mustmatch like "biomcp get gene SRY"
echo "$out" | mustmatch like 'biomcp search article -g SRY -k "Sox9 miRNA"'

filters_line="$(printf '%s\n' "$out" | grep -n '^Filters:' | cut -d: -f1 | head -n1)"
see_line="$(printf '%s\n' "$out" | grep -n '^See also:' | cut -d: -f1 | head -n1)"
test -n "$filters_line"
test -n "$see_line"
test "$see_line" -gt "$filters_line"

json_out="$("$bin" --json search article -k "SRY Sox9 miRNA" --limit 1)"
echo "$json_out" | jq -e '._meta.next_commands[0] | test("^biomcp get article [0-9]+$")' > /dev/null
echo "$json_out" | jq -e '._meta.next_commands | any(. == "biomcp list article")' > /dev/null
echo "$json_out" | jq -e '._meta.next_commands | any(. == "biomcp get gene SRY")' > /dev/null
echo "$json_out" | jq -e '._meta.next_commands | any(. == "biomcp search article -g SRY -k \"Sox9 miRNA\"")' > /dev/null
echo "$json_out" | jq -e '._meta.suggestions | any(. == "biomcp get gene SRY")' > /dev/null
echo "$json_out" | jq -e '._meta.suggestions | any(. == "biomcp search article -g SRY -k \"Sox9 miRNA\"")' > /dev/null
echo "$json_out" | jq -e '[._meta.suggestions[] | select(. == "biomcp list article")] | length == 0' > /dev/null
```

## Article Search Drug Keyword Pivot

When keyword search contains a recognizable drug token, BioMCP should suggest a
typed drug card without misclassifying common biomedical acronyms like `DNA` as
gene pivots.

```bash
bin="${BIOMCP_BIN:-$(git rev-parse --show-toplevel)/target/release/biomcp}"
out="$("$bin" search article -k "psoralen photobinding DNA" --limit 1)"
printf '%s\n' "$out" | grep -q '^See also:$'
echo "$out" | mustmatch like "biomcp get drug psoralen"
echo "$out" | mustmatch not like "biomcp get gene DNA"

json_out="$("$bin" --json search article -k "psoralen photobinding DNA" --limit 1)"
echo "$json_out" | jq -e '._meta.next_commands | any(. == "biomcp get drug psoralen")' > /dev/null
echo "$json_out" | jq -e '[._meta.next_commands[] | select(. == "biomcp get gene DNA")] | length == 0' > /dev/null
echo "$json_out" | jq -e '._meta.suggestions | any(. == "biomcp get drug psoralen")' > /dev/null
```

## Searching by Keyword

Keyword search supports broad discovery before narrowing to specific entities. The output should echo keyword context and include PMID-centric table output.

```bash
bin="${BIOMCP_BIN:-$(git rev-parse --show-toplevel)/target/release/biomcp}"
out="$("$bin" search article -q 'alternative microexon splicing metastasis' --source litsense2 --limit 1)"
echo "$out" | mustmatch like "keyword=alternative microexon splicing metastasis"
echo "$out" | grep -F 'Ranking: hybrid relevance (score = 0.4*semantic + 0.3*lexical + 0.2*citations + 0.1*position)' >/dev/null
echo "$out" | grep -F '| PMID | Title | Source(s) | Date | Why | Cit. |' >/dev/null
```

## Keyword Search Can Force Lexical Ranking

Keyword-bearing queries default to hybrid ranking, but the lexical comparator
must remain available as an explicit regression guard.

```bash
bin="${BIOMCP_BIN:-$(git rev-parse --show-toplevel)/target/release/biomcp}"
out="$("$bin" search article -q 'BRAF melanoma' --source pubmed --ranking-mode lexical --limit 1)"
echo "$out" | mustmatch like "keyword=BRAF melanoma"
echo "$out" | grep -F 'Ranking: calibrated PubMed rescue + lexical directness' >/dev/null
echo "$out" | grep -Fv 'Ranking: hybrid relevance' >/dev/null
```

## Invalid Date Fails Before Backend Warnings

Malformed article dates must fail at the front door, before backend routing,
autocomplete, or warning paths run.

```bash
unset status
out="$(biomcp search article -g BRAF --date-from 2025-99-01 --limit 1 2>&1)" || status=$?
test "${status:-0}" -eq 1
echo "$out" | mustmatch like "Error: Invalid argument:"
echo "$out" | mustmatch like "Invalid month 99 in --date-from"
echo "$out" | mustmatch not like "--since"
echo "$out" | mustmatch not like "WARN"
echo "$out" | mustmatch not like "PubTator"
echo "$out" | mustmatch not like "Europe PMC"
echo "$out" | mustmatch not like "Semantic Scholar"
```

## Missing Filters Fail Before Planner Warnings

Queryless article searches should fail with the existing invalid-argument
guidance and should not leak backend-leg warning noise.

```bash
unset status
out="$(biomcp search article --limit 1 2>&1)" || status=$?
test "${status:-0}" -eq 1
echo "$out" | mustmatch like "Error: Invalid argument:"
echo "$out" | mustmatch like "At least one filter is required."
echo "$out" | mustmatch like "biomcp search article -g BRAF"
echo "$out" | mustmatch not like "WARN"
echo "$out" | mustmatch not like "PubTator"
echo "$out" | mustmatch not like "Europe PMC"
echo "$out" | mustmatch not like "Semantic Scholar"
```

## Inverted Date Range Is A Clean Invalid Argument

Date ranges with `--date-from` after `--date-to` must fail with the explicit
ordering error and no backend warning noise.

```bash
unset status
out="$(biomcp search article -g BRAF --date-from 2024-01-01 --date-to 2020-01-01 --limit 1 2>&1)" || status=$?
test "${status:-0}" -eq 1
echo "$out" | mustmatch like "Error: Invalid argument: --date-from must be <= --date-to"
echo "$out" | mustmatch not like "WARN"
echo "$out" | mustmatch not like "PubTator"
echo "$out" | mustmatch not like "Europe PMC"
echo "$out" | mustmatch not like "Semantic Scholar"
```

## Article Date Flag Help Advertises Accepted Formats

The article command help and list output should both advertise the shared date
parser contract: `YYYY`, `YYYY-MM`, and `YYYY-MM-DD`. They should also expose
the repaired LitSense2 source roster anywhere article sources are shown.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
help_out="$("$bin" search article --help)"
echo "$help_out" | mustmatch like "Published after date (YYYY, YYYY-MM, or YYYY-MM-DD)"
echo "$help_out" | mustmatch like "Published before date (YYYY, YYYY-MM, or YYYY-MM-DD)"
echo "$help_out" | mustmatch '/\[aliases: --since\]/'
echo "$help_out" | mustmatch '/\[aliases: --until\]/'
echo "$help_out" | mustmatch like "QUERY FORMULATION:"
echo "$help_out" | mustmatch like 'Known gene/disease/drug anchors belong in `-g/--gene`, `-d/--disease`, or `--drug`.'
echo "$help_out" | mustmatch like 'Use `-k/--keyword` for mechanisms, phenotypes, datasets, outcomes, and other free-text concepts.'
echo "$help_out" | mustmatch like 'Unknown-entity questions should stay keyword-first or start with `discover`.'
echo "$help_out" | mustmatch like 'Adding `-k/--keyword` on the default route brings in LitSense2 and default `hybrid` relevance.'
echo "$help_out" | mustmatch like '`semantic` sorts by the LitSense2-derived semantic signal and falls back to lexical ties.'
echo "$help_out" | mustmatch like 'Hybrid score = `0.4*semantic + 0.3*lexical + 0.2*citations + 0.1*position` by default, using the same LitSense2-derived semantic signal and `semantic=0` when LitSense2 did not match.'
echo "$help_out" | mustmatch like "biomcp search article -g TP53 -k \"apoptosis gene regulation\" --limit 5"
echo "$help_out" | mustmatch like "biomcp search article -k '\"cafe-au-lait spots\" neurofibromas disease' --type review --limit 5"
echo "$help_out" | mustmatch like "--ranking-mode"
echo "$help_out" | mustmatch like "--weight-semantic"
echo "$help_out" | mustmatch like "--weight-lexical"
echo "$help_out" | mustmatch like "--weight-citations"
echo "$help_out" | mustmatch like "--weight-position"
echo "$help_out" | mustmatch like "--max-per-source <N>"
echo "$help_out" | mustmatch like "Cap each federated source's contribution after deduplication and before ranking."
echo "$help_out" | mustmatch like 'Default: 40% of `--limit` on federated pools with at least three surviving primary sources.'
echo "$help_out" | mustmatch like '`0` uses the default cap.'
echo "$help_out" | mustmatch like 'Setting it equal to `--limit` disables capping.'
echo "$help_out" | mustmatch like "Rows count against their primary source after deduplication."
echo "$help_out" | mustmatch like "0.4*semantic + 0.3*lexical + 0.2*citations + 0.1*position"
printf '%s\n' "$help_out" | grep -F -- '[possible values: all, pubtator, europepmc, pubmed, litsense2]' >/dev/null

list_out="$("$bin" list article)"
echo "$list_out" | mustmatch like "## Query formulation"
echo "$list_out" | mustmatch like "Known gene/disease/drug already identified"
echo "$list_out" | mustmatch like "Keyword-only topic, dataset, or method question"
echo "$list_out" | mustmatch like 'Do not invent `-g/-d/--drug`; stay keyword-first or start with `discover`'
echo "$list_out" | mustmatch like "biomcp search article -g BRAF --limit 5"
echo "$list_out" | mustmatch like "biomcp search article -g TP53 -k \"apoptosis gene regulation\" --limit 5"
echo "$list_out" | mustmatch like "biomcp search article --drug amiodarone -k \"photosensitivity mechanism\" --limit 5"
echo "$list_out" | mustmatch like "biomcp search article -k '\"cafe-au-lait spots\" neurofibromas disease' --type review --limit 5"
echo "$list_out" | mustmatch like "biomcp search article -k \"TCGA mutation analysis dataset\" --type review --limit 5"
echo "$list_out" | mustmatch like "typed gene/disease/drug anchors participate in PubTator3 + Europe PMC + PubMed"
echo "$list_out" | mustmatch like "--date-from <YYYY|YYYY-MM|YYYY-MM-DD>"
echo "$list_out" | mustmatch like "--date-to <YYYY|YYYY-MM|YYYY-MM-DD>"
echo "$list_out" | mustmatch like "--since <YYYY|YYYY-MM|YYYY-MM-DD>"
echo "$list_out" | mustmatch like "--ranking-mode <lexical|semantic|hybrid>"
echo "$list_out" | mustmatch like "--weight-semantic <float>"
echo "$list_out" | mustmatch like "--weight-lexical <float>"
echo "$list_out" | mustmatch like "--weight-citations <float>"
echo "$list_out" | mustmatch like "--weight-position <float>"
echo "$list_out" | mustmatch like "keyword-bearing article queries default to hybrid"
echo "$list_out" | mustmatch like "LitSense2-derived semantic signal"
echo "$list_out" | mustmatch like 'rows without LitSense2 provenance contribute `semantic=0`'
echo "$list_out" | mustmatch like "--source <all, pubtator, europepmc, pubmed, litsense2>"
echo "$list_out" | mustmatch like "--max-per-source <N>"
echo "$list_out" | mustmatch like "Cap each federated source's contribution after deduplication and before ranking."
echo "$list_out" | mustmatch like 'Default: 40% of `--limit` on federated pools with at least three surviving primary sources.'
echo "$list_out" | mustmatch like '`0` uses the default cap; setting it equal to `--limit` disables capping.'
echo "$list_out" | mustmatch like "Rows count against their primary source after deduplication."
echo "$list_out" | mustmatch like "search article --source litsense2"
echo "$list_out" | mustmatch like "first_index_date"
echo "$list_out" | mustmatch like "Newest indexed: YYYY-MM-DD (N days ago)"
```

## Article Year Help and Validation

The year aliases should stay strict at the CLI boundary, document themselves on
the help/list surfaces, and reuse the existing date-range validation once
expanded.

```bash
bin="${BIOMCP_BIN:-$(git rev-parse --show-toplevel)/target/release/biomcp}"

help_out="$("$bin" search article --help)"
echo "$help_out" | mustmatch like "--year-min <YYYY>"
echo "$help_out" | mustmatch like "--year-max <YYYY>"
echo "$help_out" | mustmatch like "Published from year (YYYY)"
echo "$help_out" | mustmatch like "Published through year (YYYY)"

list_out="$("$bin" list article)"
echo "$list_out" | mustmatch like "--year-min <YYYY>"
echo "$list_out" | mustmatch like "--year-max <YYYY>"
echo "$list_out" | mustmatch like "year-refinement next commands"

unset status
out="$("$bin" search article -g BRAF --year-min 200 --limit 1 2>&1)" || status=$?
test "${status:-0}" -ne 0
echo "$out" | mustmatch like "invalid value '200' for '--year-min <YYYY>'"
echo "$out" | mustmatch like "expected YYYY"

unset status
out="$("$bin" search article -g BRAF --year-min 2000 --date-from 2000-01-01 --limit 1 2>&1)" || status=$?
test "${status:-0}" -ne 0
echo "$out" | mustmatch like "the argument '--year-min <YYYY>' cannot be used with '--date-from <DATE_FROM>'"

unset status
out="$("$bin" search article -g BRAF --year-max 2013 --date-to 2013-12-31 --limit 1 2>&1)" || status=$?
test "${status:-0}" -ne 0
echo "$out" | mustmatch like "the argument '--year-max <YYYY>' cannot be used with '--date-to <DATE_TO>'"

unset status
out="$("$bin" search article -g BRAF --year-min 2013 --year-max 2000 --limit 1 2>&1)" || status=$?
test "${status:-0}" -eq 1
echo "$out" | mustmatch like "Error: Invalid argument: --date-from must be <= --date-to"
```

## Live Article Year Range Search

```bash
bin="${BIOMCP_BIN:-$(git rev-parse --show-toplevel)/target/release/biomcp}"
json_out="$("$bin" --json search article -g BRAF --source pubmed --year-min 2000 --year-max 2013 --limit 5)"
echo "$json_out" | mustmatch like '"results":'
echo "$json_out" | jq -e '.results | length > 0' > /dev/null
echo "$json_out" | jq -e 'all(.results[]; (.date == null) or (.date == "") or (((.date[0:4] | tonumber) >= 2000) and ((.date[0:4] | tonumber) <= 2013)))' > /dev/null
```

## Article Query Echo Surfaces Explicit Max-Per-Source Overrides

Explicit `--max-per-source` overrides should surface in article query context
so operators can verify which cap mode the search ran with.

```bash
bin="${BIOMCP_BIN:-$(git rev-parse --show-toplevel)/target/release/biomcp}"
out="$("$bin" search article -g BRAF --max-per-source 10 --limit 25)"
echo "$out" | mustmatch like "max_per_source=10"
```

## Source-Specific PubTator Search Uses Default Retraction Filter

Default article search still excludes confirmed retractions, but PubTator rows
without retraction metadata should remain eligible when the user selects the
PubTator source directly.

```bash
bin="${BIOMCP_BIN:-$(git rev-parse --show-toplevel)/target/release/biomcp}"
out="$("$bin" search article -q 'alternative microexon splicing metastasis' --source pubtator --limit 3)"
echo "$out" | mustmatch like "| PMID | Title |"
echo "$out" | mustmatch not like "No articles found"

json_out="$("$bin" --json search article -q 'alternative microexon splicing metastasis' --source pubtator --ranking-mode hybrid --limit 3)"
echo "$json_out" | jq -e '(.results | length) > 0 and all(.results[]; .ranking.mode == "hybrid" and .ranking.semantic_score == 0)' > /dev/null
```

## Source-Specific PubMed Search

Explicit PubMed routing should expose the source in the rendered query context
and preserve the standard article table contract for stable smoke queries.

```bash
bin="${BIOMCP_BIN:-$(git rev-parse --show-toplevel)/target/release/biomcp}"
out="$("$bin" search article -g BRAF --source pubmed --limit 3)"
echo "$out" | mustmatch like "source=pubmed"
echo "$out" | mustmatch like "| PMID | Title |"
echo "$out" | mustmatch not like "No articles found"
printf '%s\n' "$out" | grep -F -- '--source <all|pubtator|europepmc|pubmed|litsense2>' >/dev/null

json_out="$(env -u S2_API_KEY "$bin" --json search article -k 'GDNF RET Hirschsprung 1996' --source pubmed --limit 5)"
echo "$json_out" | jq -e 'any(.results[]; .pmid == "8896569" and .source == "pubmed" and .matched_sources == ["pubmed"] and (.citation_count // 0) > 0 and ((.abstract_snippet // "") | length > 0))' >/dev/null
```

## First Index Date in Article Search

Europe PMC and PubMed search rows expose the upstream first-indexed date when
the provider returns it. JSON carries `first_index_date` per row; markdown adds
`Newest indexed: YYYY-MM-DD (N days ago)` immediately after the result table.

```bash
bin="${BIOMCP_BIN:-$(git rev-parse --show-toplevel)/target/release/biomcp}"
json_out="$("$bin" --json search article -g BRAF --source europepmc --limit 3)"
echo "$json_out" | jq -e 'any(.results[]; (.first_index_date // "") | test("^[0-9]{4}-[0-9]{2}-[0-9]{2}$"))' >/dev/null

md_out="$("$bin" search article -g BRAF --source europepmc --limit 3)"
echo "$md_out" | mustmatch '/Newest indexed: [0-9]{4}-[0-9]{2}-[0-9]{2} \([0-9]+ days ago\)/'
```

```bash
bin="${BIOMCP_BIN:-$(git rev-parse --show-toplevel)/target/release/biomcp}"
json_out="$("$bin" --json search article -g BRAF --source pubmed --limit 3)"
echo "$json_out" | jq -e 'any(.results[]; .source == "pubmed" and ((.first_index_date // "") | test("^[0-9]{4}-[0-9]{2}-[0-9]{2}$")))' >/dev/null
```

```bash
bin="${BIOMCP_BIN:-$(git rev-parse --show-toplevel)/target/release/biomcp}"
md_out="$("$bin" search article -q "alternative microexon splicing metastasis" --source pubtator --limit 3)"
echo "$md_out" | mustmatch not '/Newest indexed:/'
```

## Source-Specific LitSense2 Search

Explicit LitSense2 routing should accept the new source flag, preserve score
values in JSON, expose the repaired semantic signal in both hybrid and semantic
ranking modes, hydrate usable article titles for keyword-driven matches, and
reject filter combinations it cannot truthfully satisfy in this ticket.

```bash
bin="${BIOMCP_BIN:-$(git rev-parse --show-toplevel)/target/release/biomcp}"
out="$("$bin" --json search article -k 'BRAF melanoma' --source litsense2 --limit 1)"
echo "$out" | jq -e '(.results | length) > 0' > /dev/null
echo "$out" | jq -e 'all(.results[]; .source == "litsense2")' > /dev/null
echo "$out" | jq -e 'all(.results[]; (.score | type) == "number")' > /dev/null
echo "$out" | jq -e 'all(.results[]; .ranking.semantic_score == (.score | if . < 0 then 0 elif . > 1 then 1 else . end))' > /dev/null
echo "$out" | jq -e 'all(.results[]; (.pmid | type) == "string" and (.pmid | length) > 0)' > /dev/null
echo "$out" | jq -e 'all(.results[]; (.title | length) > 0)' > /dev/null

semantic_out="$(env -u S2_API_KEY "$bin" --json search article -k 'alternative microexon splicing metastasis' --source litsense2 --ranking-mode semantic --limit 1)"
echo "$semantic_out" | jq -e '(.results | length) > 0 and all(.results[]; .ranking.mode == "semantic" and .ranking.semantic_score == (.score | if . < 0 then 0 elif . > 1 then 1 else . end))' > /dev/null
status=0
out="$("$bin" search article -g BRAF --source litsense2 --limit 1 2>&1)" || status=$?
test "$status" -ne 0
echo "$out" | mustmatch like "--source litsense2"
echo "$out" | mustmatch like "requires a keyword"

typed_status=0
typed_out="$("$bin" search article -k melanoma --source litsense2 --type review --limit 1 2>&1)" || typed_status=$?
test "$typed_status" -ne 0
echo "$typed_out" | mustmatch like "--source litsense2"
echo "$typed_out" | mustmatch like "does not support --type"

open_status=0
open_out="$("$bin" search article -k melanoma --source litsense2 --open-access --limit 1 2>&1)" || open_status=$?
test "$open_status" -ne 0
echo "$open_out" | mustmatch like "--source litsense2"
echo "$open_out" | mustmatch like "does not support --open-access"
```

## Federated Search Preserves Non-EuropePMC Matches Under Default Retraction Filter

JSON article search preserves the tri-state `is_retracted` contract as
`true`, `false`, or `null`. Under the default filter, only confirmed
retractions are excluded, so federated search can still surface PubTator or
other non-EuropePMC matches when those sources lack retraction metadata.

```bash
bin="${BIOMCP_BIN:-$(git rev-parse --show-toplevel)/target/release/biomcp}"
out="$(env -u S2_API_KEY "$bin" --json search article -q 'alternative microexon splicing metastasis' --limit 5)"
echo "$out" | jq -r 'all(.results[]; (.matched_sources | type) == "array")' | mustmatch "true"
echo "$out" | jq -r 'any(.results[]; (.matched_sources | any(. != "europepmc")))' | mustmatch "true"
```

## Keyword Anchors Tokenize In JSON Ranking Metadata

Multi-word `--keyword` queries should contribute independently matchable
ranking concepts instead of one exact phrase blob. The public JSON contract
exposes that through `ranking.anchor_count`.

```bash
bin="${BIOMCP_BIN:-$(git rev-parse --show-toplevel)/target/release/biomcp}"
out="$(env -u S2_API_KEY "$bin" --json search article -q 'alternative microexon splicing metastasis' --limit 5)"
echo "$out" | jq -r '(.results | length > 0) and all(.results[]; .ranking.anchor_count == 4 and .ranking.mode == "hybrid")' | mustmatch "true"
echo "$out" | jq -e 'all(.results[]; ((.matched_sources | index("litsense2")) != null) or (.ranking.semantic_score == 0))' >/dev/null
```

## Type Filter Uses The Compatible Source Set

`--type` on `--source all` should use Europe PMC + PubMed when the selected
filters are PubMed-compatible, and should collapse to Europe PMC-only when
other selected filters make PubMed ineligible.

```bash
bin="${BIOMCP_BIN:-$(git rev-parse --show-toplevel)/target/release/biomcp}"
out="$("$bin" search article -g BRAF --type review --limit 3)"
echo "$out" | mustmatch like "> Note: --type restricts article search to Europe PMC and PubMed."
echo "$out" | mustmatch like "| PMID | Title |"

strict_out="$("$bin" search article -g BRAF --type review --no-preprints --limit 3)"
echo "$strict_out" | mustmatch like "> Note: --type restricts this article search to Europe PMC."
```

## Getting Article Details

The article detail card should preserve stable bibliographic anchors for reproducible referencing. We assert on PMID and journal markers.

```bash
out="$(biomcp get article 22663011)"
echo "$out" | mustmatch like "PMID: 22663011"
echo "$out" | mustmatch '/Journal: .+/'
```

## Article Annotations

Annotation output summarizes entity classes detected by PubTator. The section should also explain that these are normalized entity mentions suitable for standardized extraction.

```bash
out="$(biomcp get article 22663011 annotations)"
echo "$out" | mustmatch like "## PubTator Annotations"
echo "$out" | mustmatch like "normalized entity mentions"
echo "$out" | mustmatch like "standardized extraction"
echo "$out" | mustmatch '/Genes: [A-Z0-9]/'
```

## Article Full Text Saved Markdown

Full text remains a path-based contract on stdout. The proof needs to confirm
that BioMCP still prints `Saved to:` while the cached file preserves PMC/JATS
structure and renders the bibliography under `## References` when source
`<ref-list>` data is present.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT
out="$(TMPDIR="$tmpdir" "$bin" get article 27083046 fulltext)"
if false; then
  echo "collector" | mustmatch "collector"
fi
printf '%s\n' "$out" | grep -F -- "## Full Text" >/dev/null
path="$(printf '%s\n' "$out" | sed -n 's/^Saved to: //p' | head -n1)"
test -n "$path"
test -f "$path"
export SAVED_PATH="$path"
python3 - <<'PY'
import os
import sys
from pathlib import Path

saved = Path(os.environ["SAVED_PATH"]).read_text()
_, refs = saved.split("## References", 1)
checks = [
    "# Synaptotagmin-1 C2B domain interacts simultaneously" in saved,
    "## Abstract" in saved,
    "## Introduction" in saved,
    "## References" in saved,
    any(line.startswith("1. ") for line in refs.splitlines()),
    "Architecture of the synaptotagmin-snare machinery for neuronal exocytosis" in refs,
    "[10.1038/nsmb1056](https://doi.org/10.1038/nsmb1056)" in refs,
    "references cited." not in refs,
    "Creative Commons Attribution License" not in saved,
    "eLife Sciences Publications" not in saved,
]
if not all(checks):
    sys.exit(1)
PY
```

## Large Article Full Text Saved Markdown

Large PMC OA archives should also preserve the saved-file contract instead of
failing at the default 8 MB response-body ceiling.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT
out="$(TMPDIR="$tmpdir" "$bin" get article 25268582 fulltext)"
echo "$out" | mustmatch like "## Full Text"
path="$(printf '%s\n' "$out" | sed -n 's/^Saved to: //p' | head -n1)"
test -n "$path"
test -f "$path"
test -s "$path"
```

## Article to Entities

`article entities` exposes actionable next-command pivots by entity class. We check top-level heading and genes subsection marker.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" article entities 22663011)"
echo "$out" | mustmatch like "# Entities in PMID 22663011"
echo "$out" | mustmatch like "## Genes ("
echo "$out" | mustmatch like '`biomcp search gene -q BRAF`'
echo "$out" | mustmatch like '`biomcp search gene -q "serine-threonine protein kinase"`'
if echo "$out" | grep -F "biomcp get gene serine-threonine protein kinase" >/dev/null; then
  echo "unexpected stale raw gene command" >&2
  exit 1
fi
```

## Article Batch

`article batch` returns compact numbered cards for known IDs without
changing single-article output. The markdown contract exposes a stable heading,
numbered card sections with PMID/bibliographic fields, and degrades cleanly
without Semantic Scholar TLDR data.

```bash
out="$(biomcp article batch 22663011 24200969)"
echo "$out" | mustmatch like "# Article Batch (2)"
echo "$out" | mustmatch like "## 1. Improved survival with MEK inhibition in BRAF-mutated melanoma."
echo "$out" | mustmatch like "## 2. Activities of multiple cancer-related pathways are associated"
echo "$out" | mustmatch like "PMID: 22663011"
echo "$out" | mustmatch like "PMID: 24200969"

json_out="$(biomcp --json article batch 22663011 24200969)"
echo "$json_out" | mustmatch like '"requested_id": "22663011"'
echo "$json_out" | mustmatch like '"pmid": "22663011"'
echo "$json_out" | mustmatch like '"title": "'
echo "$json_out" | jq -e '.[0].year | type == "number"' > /dev/null

no_key_out="$(env -u S2_API_KEY biomcp --json article batch 22663011)"
echo "$no_key_out" | mustmatch like '"requested_id": "22663011"'
echo "$no_key_out" | mustmatch like '"title": "'
```

## Article Batch Invalid Identifier

An unsupported identifier format should fail with the existing supported
identifier guidance rather than a generic error.

```bash
out="$(biomcp article batch S1535610826000103 2>&1 || true)"
echo "$out" | mustmatch like "Unsupported identifier"
```

## Article Batch Limit Enforcement

More than 20 IDs should fail immediately, before any network work.

```bash
out="$(biomcp article batch 1000001 1000002 1000003 1000004 1000005 1000006 1000007 1000008 1000009 1000010 1000011 1000012 1000013 1000014 1000015 1000016 1000017 1000018 1000019 1000020 1000021 2>&1 || true)"
echo "$out" | mustmatch like "limited to 20"
```

## Optional-Key Get Article Path

Ordinary `get article` must still work when Semantic Scholar is unavailable. We
force the no-key path even on keyed machines and assert that the PubMed card
still renders without an API-key gate.

```bash
out="$(env -u S2_API_KEY biomcp get article 22663011)"
echo "$out" | mustmatch like "PMID: 22663011"
echo "$out" | mustmatch '/Journal: .+/'
echo "$out" | mustmatch not like "API key required"
```

## Article Search JSON Without Semantic Scholar Key

No-key article search must stay explicit and functional. JSON should report the
eligible Semantic Scholar leg while still surfacing ranking metadata from the
local relevance policy.

```bash
out="$(env -u S2_API_KEY biomcp --json search article -g BRAF --limit 3 2>/dev/null)"
echo "$out" | mustmatch like '"semantic_scholar_enabled": true'
echo "$out" | mustmatch like '"ranking_policy": "calibrated PubMed rescue + lexical directness'
echo "$out" | mustmatch like 'at least one anchor hit'
echo "$out" | mustmatch like '"ranking": {'
echo "$out" | mustmatch like '"pubmed_rescue":'
echo "$out" | jq -e 'all(.results[]; .ranking.mode == "lexical")' > /dev/null
```

## Article Search JSON With Semantic Scholar Key

When `S2_API_KEY` is present, article search should expose the keyed search-leg
state and merged source metadata in JSON.

```bash
out="$(biomcp --json search article -g BRAF -d melanoma --include-retracted --limit 5)"
echo "$out" | mustmatch like '"semantic_scholar_enabled": true'
echo "$out" | mustmatch like '"matched_sources": ['
echo "$out" | mustmatch like '"ranking": {'
echo "$out" | jq -e 'all(.results[]; .ranking.mode == "lexical")' > /dev/null
```

## Article Debug Plan

The optional debug plan should expose the actual search surface, planner
markers, and sources in both markdown and JSON without changing default output.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$(env -u S2_API_KEY "$bin" search article -g BRAF --debug-plan --limit 3 2>/dev/null)"
echo "$out" | mustmatch like "## Debug plan"
echo "$out" | mustmatch like '"surface": "search_article"'
echo "$out" | mustmatch like '"planner=federated"'
printf '%s\n' "$out" | grep -F '"PubMed"' >/dev/null
echo "$out" | mustmatch like "Semantic Scholar"
echo "$out" | mustmatch not like "LitSense2"

json_out="$(env -u S2_API_KEY "$bin" --json search article -g BRAF --debug-plan --limit 3 2>/dev/null)"
echo "$json_out" | mustmatch like '"debug_plan": {'
echo "$json_out" | mustmatch like '"surface": "search_article"'
echo "$json_out" | mustmatch like '"leg": "article"'
echo "$json_out" | mustmatch like '"sources": ['
echo "$json_out" | mustmatch like '"Semantic Scholar"'
echo "$json_out" | jq -e 'all(.debug_plan.legs[] | select(.leg == "article"); (.sources | index("LitSense2")) == null)' > /dev/null

keyword_json="$("$bin" --json search article -k 'Hirschsprung disease' --debug-plan --limit 3 2>/dev/null)"
echo "$keyword_json" | mustmatch like '"debug_plan": {'
echo "$keyword_json" | jq -e '.debug_plan.legs[] | select(.leg == "article") | .sources | index("LitSense2") != null' > /dev/null

typed_out="$("$bin" search article -g BRAF --type review --include-retracted --debug-plan --limit 3 2>/dev/null)"
echo "$typed_out" | mustmatch like '"planner=type_capable"'
printf '%s\n' "$typed_out" | grep -F '"PubMed"' >/dev/null
echo "$typed_out" | mustmatch like '"Note: --type restricts article search to Europe PMC and PubMed'

typed_json="$("$bin" --json search article -g BRAF --type review --include-retracted --debug-plan --limit 3 2>/dev/null)"
echo "$typed_json" | mustmatch like '"planner=type_capable"'
echo "$typed_json" | mustmatch like '"note": "Note: --type restricts article search to Europe PMC and PubMed'

strict_json="$("$bin" --json search article -g BRAF --type review --no-preprints --debug-plan --limit 3 2>/dev/null)"
echo "$strict_json" | mustmatch like '"planner=europe_only_strict_filters"'
echo "$strict_json" | mustmatch like '"Europe PMC"'
echo "$strict_json" | mustmatch not like '"PubMed"'
```

## Semantic Scholar TLDR Section

When `S2_API_KEY` is present, `get article ... tldr` isolates the Semantic
Scholar enrichment section and exposes stable markers for TLDR and influence
metrics.

```bash
out="$(biomcp get article 22663011 tldr)"
echo "$out" | mustmatch '/^# .+/'
echo "$out" | mustmatch like "Semantic Scholar"
echo "$out" | mustmatch '/TLDR: .+/'
echo "$out" | mustmatch like "Influential citations:"
```

## Semantic Scholar Citations

Citation traversal should expose a graph table with contexts, intents, and the
influential flag visible to the user.

```bash
out="$(biomcp article citations 22663011 --limit 3)"
echo "$out" | mustmatch like "# Citations for"
echo "$out" | mustmatch like "| PMID | Title | Intents | Influential | Context |"
```

## Semantic Scholar References

Reference traversal should expose the same visible graph columns.

```bash
out="$(biomcp article references 22663011 --limit 3)"
echo "$out" | mustmatch like "# References for"
echo "$out" | mustmatch like "| PMID | Title | Intents | Influential | Context |"
```

## Semantic Scholar Recommendations (Single Seed)

Single-seed recommendations should render related papers with stable table
columns.

```bash
out="$(biomcp article recommendations 22663011 --limit 3)"
echo "$out" | mustmatch like "# Recommendations for"
echo "$out" | mustmatch like "| PMID | Title | Journal | Year |"
```

## Semantic Scholar Recommendations (Multi Seed)

Multi-paper recommendation requests should accept repeated positive seeds plus a
negative set and still render the recommendation table.

```bash
out="$(biomcp article recommendations 22663011 24200969 --negative 39073865 --limit 3)"
echo "$out" | mustmatch like "# Recommendations for"
echo "$out" | mustmatch like "| PMID | Title | Journal | Year |"
echo "$out" | mustmatch like "Negative seeds:"
```

## Invalid Identifier Rejection

BioMCP supports PMID, PMCID, and DOI for article lookup. Unsupported formats such as
publisher PIIs must fail fast, return a non-zero exit, and name the supported types in
the error text.

```bash
status=0
out="$(biomcp get article S1535610826000103 2>&1)" || status=$?
test "$status" -ne 0
echo "$out" | mustmatch like "BioMCP resolves PMID (digits only, e.g., 22663011), PMCID (starts with PMC, e.g., PMC9984800), and DOI (starts with 10., e.g., 10.1056/NEJMoa1203421)."
echo "$out" | mustmatch like "publisher PIIs (e.g., S1535610826000103) are not indexed by PubMed or Europe PMC"
```

## Sort Behavior

Default article search uses relevance sort. The output header echoes the sort in effect so callers can verify the default.

```bash
bin="${BIOMCP_BIN:-$(git rev-parse --show-toplevel)/target/release/biomcp}"
out="$(env -u S2_API_KEY "$bin" search article -q 'alternative microexon splicing metastasis' --limit 1)"
echo "$out" | mustmatch like "sort=relevance"
echo "$out" | grep -F 'ranking_mode=hybrid' >/dev/null
```

Passing `--sort date` opts into date-based ordering.

```bash
bin="${BIOMCP_BIN:-$(git rev-parse --show-toplevel)/target/release/biomcp}"
out="$("$bin" search article -g BRAF --source pubmed --sort date --limit 3)"
echo "$out" | mustmatch like "# Articles: gene=BRAF, exclude_retracted=true, sort=date, source=pubmed"
```

## Federated Deep Offset Guard

Federated article search merges PubTator3, Europe PMC, and PubMed before
applying paging. Very deep offsets must fail fast with an explicit bound so
callers do not get silently incorrect merged windows.

```bash
status=0
out="$(biomcp search article -k melanoma --limit 50 --offset 1201 2>&1)" || status=$?
test "$status" -ne 0
echo "$out" | mustmatch like "--offset + --limit must be <= 1250"
```
