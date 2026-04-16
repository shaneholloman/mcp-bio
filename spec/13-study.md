# Study-Native Queries

This spec validates the local cBioPortal study command family. Assertions stay anchored on stable headings, column names, and group labels rather than volatile dataset counts.

| Section | Command focus | Why it matters |
|---|---|---|
| Study help | `study --help` and subcommand help | Confirms study commands are self-describing in CLI help |
| Environment setup | `BIOMCP_STUDY_DIR` | Makes the local dataset dependency explicit |
| Study listing | `study list` | Confirms study discovery and list table schema |
| Top mutated genes | `study top-mutated` | Confirms ranked gene-frequency table for a study |
| Mutation frequency | `study query --type mutations` | Confirms mutation query headings and metrics |
| CNA distribution | `study query --type cna` | Confirms CNA bucket rows |
| Expression distribution | `study query --type expression` | Confirms expression summary metrics |
| Cohort split | `study cohort` | Confirms mutation-based cohort partitioning |
| Survival aggregates | `study survival` | Confirms per-group event/follow-up summary output |
| Expression comparison | `study compare --type expression` | Confirms per-group expression summary tables |
| Mutation rate comparison | `study compare --type mutations` | Confirms per-group mutation-rate summary tables |
| Co-occurrence | `study co-occurrence` | Confirms pairwise table schema |
| Chart help | `chart --help` | Confirms chart topics are self-describing in CLI help |
| Multi-omics filter | `study filter` | Confirms cross-table intersection output and validation |
| Missing data handling | missing expression or survival inputs | Confirms actionable source-unavailable messages |
| Unknown study handling | invalid `--study` | Confirms actionable not-found message |

## Study Help

Study help should describe every study subcommand directly in CLI help, plus the key flags, canonical values, and accepted aliases for the study workflows operators use most often.

```bash
out="$(biomcp study --help)"
echo "$out" | mustmatch like "List locally available cBioPortal studies"
echo "$out" | mustmatch like "List downloadable study IDs or install a study locally"
echo "$out" | mustmatch like "Run per-study gene query"
echo "$out" | mustmatch like "Rank the most frequently mutated genes in a study"
echo "$out" | mustmatch like "Intersect sample filters across mutation, CNA, expression, and clinical data"
echo "$out" | mustmatch like "Split a cohort into mutant vs wildtype groups"
echo "$out" | mustmatch like "Summarize KM survival and log-rank statistics by mutation group"
echo "$out" | mustmatch like "Compare expression or mutation rate across mutation groups"
echo "$out" | mustmatch like "Compute pairwise mutation co-occurrence across genes"
```

```bash
out="$(biomcp study query --help)"
echo "$out" | mustmatch like "--study <STUDY>"
echo "$out" | mustmatch like "--gene <GENE>"
echo "$out" | mustmatch like "--type <QUERY_TYPE>"
echo "$out" | mustmatch like "cBioPortal study ID"
echo "$out" | mustmatch like "HGNC gene symbol to summarize"
echo "$out" | mustmatch '/Canonical values:\s+mutations,\s+cna,\s+expression\./'
echo "$out" | mustmatch '/Accepted aliases:\s+mutation,\s+copy_number,\s+copy-number,\s+expr/'
```

```bash
out="$(biomcp study filter --help)"
echo "$out" | mustmatch like "--mutated <MUTATED>"
echo "$out" | mustmatch like "--amplified <AMPLIFIED>"
echo "$out" | mustmatch like "--deleted <DELETED>"
echo "$out" | mustmatch like "--expression-above <EXPRESSION_ABOVE>"
echo "$out" | mustmatch like "--expression-below <EXPRESSION_BELOW>"
echo "$out" | mustmatch like "--cancer-type <CANCER_TYPE>"
echo "$out" | mustmatch like "Keep samples with a mutation in GENE"
echo "$out" | mustmatch like "Keep samples with high-level copy-number amplification in GENE"
echo "$out" | mustmatch like "Keep samples with a deep copy-number deletion in GENE"
echo "$out" | mustmatch '/format:\s+GENE:THRESHOLD;\s+repeatable/'
echo "$out" | mustmatch like "Keep samples matching this cancer type label"
```

```bash
out="$(biomcp study survival --help)"
echo "$out" | mustmatch like "--study <STUDY>"
echo "$out" | mustmatch like "--endpoint <ENDPOINT>"
echo "$out" | mustmatch like "cBioPortal study ID"
echo "$out" | mustmatch like "HGNC gene symbol used to define mutant vs wildtype groups"
echo "$out" | mustmatch '/Canonical values:\s+os,\s+dfs,\s+pfs,\s+dss\./'
echo "$out" | mustmatch '/Accepted aliases:\s+overall,\s+overall_survival,\s+disease_free,\s+progression_free,\s+disease_specific/'
```

```bash
out="$(biomcp study compare --help)"
echo "$out" | mustmatch like "--study <STUDY>"
echo "$out" | mustmatch like "--gene <GENE>"
echo "$out" | mustmatch like "--target <TARGET>"
echo "$out" | mustmatch like "cBioPortal study ID"
echo "$out" | mustmatch like "HGNC gene symbol used to define mutant vs wildtype groups"
echo "$out" | mustmatch like "Target gene symbol to compare across mutation groups"
echo "$out" | mustmatch '/Canonical values:\s+expression,\s+mutations\./'
echo "$out" | mustmatch '/Accepted aliases:\s+expr,\s+mutation/'
```

```bash
out="$(biomcp study co-occurrence --help)"
echo "$out" | mustmatch like "--study <STUDY>"
echo "$out" | mustmatch like "--genes <GENES>"
echo "$out" | mustmatch like "cBioPortal study ID"
echo "$out" | mustmatch like "Comma-separated HGNC gene symbols"
echo "$out" | mustmatch like "2-10 genes"
```

```bash
out="$(biomcp study download --help)"
echo "$out" | mustmatch '/--list\s+List available remote/'
echo "$out" | mustmatch '/\[STUDY_ID\]/'
echo "$out" | mustmatch like "List available remote study IDs instead of downloading a study"
echo "$out" | mustmatch like "required unless --list"
```

```bash
out="$(biomcp study top-mutated --help)"
echo "$out" | mustmatch like "--study <STUDY>"
echo "$out" | mustmatch like "--limit <LIMIT>"
echo "$out" | mustmatch like "Maximum number of genes to display"
```

```bash
out="$(biomcp study cohort --help)"
echo "$out" | mustmatch like "--study <STUDY>"
echo "$out" | mustmatch like "--gene <GENE>"
echo "$out" | mustmatch like "mutant vs wildtype groups"
```

## Environment Setup

These commands require local study files. This spec provisions a minimal local fixture dataset, then exports `BIOMCP_STUDY_DIR` from a cached env file that later sections can source.

For real datasets, `biomcp study download --list` and `biomcp study download <study_id>` install studies into the same directory, but the spec stays fixture-backed and offline.

```bash
bash fixtures/setup-study-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-study-env"
test -n "${BIOMCP_STUDY_DIR:-}"
test -d "$BIOMCP_STUDY_DIR"
echo "$BIOMCP_STUDY_DIR" | mustmatch like "spec-study-datasets"
```

## Study Listing

Listing should return a stable heading and table columns with at least one known starter study ID.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study list)"
echo "$out" | mustmatch like "# Study Datasets"
echo "$out" | mustmatch like "| Study ID | Name | Cancer Type | Samples | Available Data |"
echo "$out" | mustmatch like "msk_impact_2017"
```

## Top Mutated Genes

Top-mutated should provide a study-level ranked view using distinct mutated
samples as the primary ordering, while keeping event counts and rates visible.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study top-mutated --study msk_impact_2017 --limit 10)"
echo "$out" | mustmatch like "# Study Top Mutated Genes: msk_impact_2017"
echo "$out" | mustmatch like "| Gene | Mutated Samples | Mutation Events | Total Samples | Mutation Rate |"
echo "$out" | mustmatch like "| TP53 | 2 | 2 | 3 |"
echo "$out" | mustmatch like "| KRAS | 2 | 2 | 3 |"
```

## Mutation Frequency Query

Mutation query should render the expected heading, metric table, and detail sections.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study query --study msk_impact_2017 --gene TP53 --type mutations)"
echo "$out" | mustmatch like "# Study Mutation Frequency: TP53 (msk_impact_2017)"
echo "$out" | mustmatch like "| Metric | Value |"
echo "$out" | mustmatch like "## Top Variant Classes"
echo "$out" | mustmatch like "## Top Protein Changes"
```

## CNA Distribution Query

CNA query should render canonical CNA buckets and total sample line.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study query --study brca_tcga_pan_can_atlas_2018 --gene ERBB2 --type cna)"
echo "$out" | mustmatch like "# Study CNA Distribution: ERBB2 (brca_tcga_pan_can_atlas_2018)"
echo "$out" | mustmatch like "| Bucket | Count |"
echo "$out" | mustmatch like "| Deep deletion (-2) |"
echo "$out" | mustmatch like "| Total samples |"
```

## Expression Distribution Query

Expression query should show summary statistics fields and source file label.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study query --study paad_qcmg_uq_2016 --gene KRAS --type expression)"
echo "$out" | mustmatch like "# Study Expression Distribution: KRAS (paad_qcmg_uq_2016)"
echo "$out" | mustmatch like "| Metric | Value |"
echo "$out" | mustmatch like "| Sample count |"
echo "$out" | mustmatch '/\| Mean \| [0-9.-]+/'
```

## Cohort Split

The cohort command should partition the study into mutation-defined groups. The output should keep both group labels and the total row stable even when counts drift upstream.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study cohort --study brca_tcga_pan_can_atlas_2018 --gene TP53)"
echo "$out" | mustmatch like "# Study Cohort: TP53"
echo "$out" | mustmatch like "| Group | Samples | Patients |"
echo "$out" | mustmatch like "TP53-mutant"
echo "$out" | mustmatch like "TP53-wildtype"
echo "$out" | mustmatch like "| Total | "
```

## Survival Aggregates

The survival command should return KM-derived group summaries rather than raw follow-up medians. The stable contract is the endpoint label, per-group KM/landmark columns, and the log-rank line.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study survival --study brca_tcga_pan_can_atlas_2018 --gene TP53)"
echo "$out" | mustmatch like "# Study Survival: TP53"
echo "$out" | mustmatch like "Endpoint: Overall Survival"
echo "$out" | mustmatch like "| Group | N | Events | Censored | Event Rate | KM Median | 1yr | 3yr | 5yr |"
echo "$out" | mustmatch like "Log-rank p-value:"
echo "$out" | mustmatch like "TP53-mutant"
echo "$out" | mustmatch like "TP53-wildtype"
```

## Expression Comparison

Expression comparison should summarize the target gene distribution across mutation-defined groups and report the Mann-Whitney test lines. The spec asserts the structural columns, not numeric values.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study compare --study brca_tcga_pan_can_atlas_2018 --gene TP53 --type expression --target ERBB2)"
echo "$out" | mustmatch like "# Study Group Comparison: Expression"
echo "$out" | mustmatch like "| Group | N | Mean | Median |"
echo "$out" | mustmatch like "Mann-Whitney U:"
echo "$out" | mustmatch like "Mann-Whitney p-value:"
echo "$out" | mustmatch like "TP53-mutant"
echo "$out" | mustmatch like "TP53-wildtype"
```

## Mutation Rate Comparison

Mutation-rate comparison should summarize the target gene mutation rate in each cohort. This keeps the group names and table schema stable while actual study counts vary.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study compare --study brca_tcga_pan_can_atlas_2018 --gene TP53 --type mutations --target PIK3CA)"
echo "$out" | mustmatch like "# Study Group Comparison: Mutation Rate"
echo "$out" | mustmatch like "| Group | N | Mutated | Mutation Rate |"
echo "$out" | mustmatch like "TP53-mutant"
echo "$out" | mustmatch like "TP53-wildtype"
```

## Co-occurrence Query

Co-occurrence query should return the expected pairwise table headings.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study co-occurrence --study msk_impact_2017 --genes TP53,KRAS)"
echo "$out" | mustmatch like "# Study Co-occurrence: msk_impact_2017"
echo "$out" | mustmatch like "Genes: TP53, KRAS"
echo "$out" | mustmatch like "Sample universe: clinical sample file"
echo "$out" | mustmatch like "| Gene A | Gene B | Both | A only | B only | Neither | Log Odds Ratio | p-value |"
```

## Chart Help

The chart documentation entrypoint should summarize each chart topic directly in
CLI help so operators can discover the right surface before opening the full
markdown page.

```bash
out="$(biomcp chart --help)"
echo "$out" | mustmatch like "bar          Categorical counts as vertical bars"
echo "$out" | mustmatch like "stacked-bar  Mutation-grouped sample counts split by outcome"
echo "$out" | mustmatch like "survival     Kaplan-Meier survival curves"
```

## Multi-Omics Filter

The filter command should show the criteria table, intersection summary, and matched sample section for study-level joins across mutation, CNA, expression, and clinical inputs.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study filter --study brca_tcga_pan_can_atlas_2018 --mutated TP53)"
echo "$out" | mustmatch like "# Study Filter: brca_tcga_pan_can_atlas_2018"
echo "$out" | mustmatch like "## Criteria"
echo "$out" | mustmatch like "| Filter | Matching Samples |"
echo "$out" | mustmatch like $'## Criteria\n\n| Filter | Matching Samples |\n|---|---|\n| mutated TP53 |'
echo "$out" | mustmatch like "| Study Total Samples |"
```

Multiple criteria should be combined with AND semantics and keep the user-supplied filter labels visible in the criteria table.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study filter --study brca_tcga_pan_can_atlas_2018 --mutated TP53 --amplified ERBB2 --expression-above ERBB2:1.5)"
echo "$out" | mustmatch like "mutated TP53"
echo "$out" | mustmatch like "amplified ERBB2"
echo "$out" | mustmatch like "expression > 1.5 for ERBB2"
echo "$out" | mustmatch like "## Matched Samples"
```

Calling the command without any criteria should fail before looking up study data.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study filter --study brca_tcga_pan_can_atlas_2018 2>&1 || true)"
echo "$out" | mustmatch like "At least one filter criterion is required"
```

## Missing Expression Data

Requesting expression for a study without expression matrices should fail with a clear source-unavailable message.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study query --study msk_impact_2017 --gene TP53 --type expression 2>&1 || true)"
echo "$out" | mustmatch like "Source unavailable: cbioportal-study"
echo "$out" | mustmatch like "No supported expression matrix found"
```

## Missing Survival Data

Studies without canonical survival inputs should fail clearly instead of inferring unsupported behavior. The error should point at the required patient clinical file and missing columns.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study survival --study paad_qcmg_uq_2016 --gene KRAS 2>&1 || true)"
echo "$out" | mustmatch like "Source unavailable: cbioportal-study"
echo "$out" | mustmatch like "Missing required column"
echo "$out" | mustmatch like "data_clinical_patient.txt"
```

## Unknown Study Handling

Unknown study IDs should produce a not-found message with a direct next step.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study query --study missing_study --gene TP53 --type mutations 2>&1 || true)"
echo "$out" | mustmatch like "study 'missing_study' not found"
echo "$out" | mustmatch like "biomcp study list"
```

## Missing Remote Study Download

Remote study downloads should translate cBioPortal datahub access-denied responses into a study-focused not-found message. The fixture keeps the proof offline by serving a localhost 403 XML denial while still asserting the CLI guidance points operators to `--list`.

```bash
bash fixtures/setup-study-download-error-fixture.sh "$PWD"
. "$PWD/.cache/spec-study-download-error-env"
trap 'bash fixtures/cleanup-study-download-error-fixture.sh "$PWD"' EXIT
out="$(biomcp study download missing_study 2>&1 || true)"
echo "$out" | mustmatch like "missing_study"
echo "$out" | mustmatch like "not found"
echo "$out" | mustmatch like "biomcp study download --list"
echo "$out" | mustmatch not like "AccessDenied"
echo "$out" | mustmatch not like "API error from cbioportal-datahub"
```

## Chart Flag: Mutation Bar Chart

`--chart bar --terminal` on a mutation query should produce terminal chart output instead of the standard markdown heading. The output should be non-empty and not contain the standard markdown heading.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study query --study msk_impact_2017 --gene TP53 --type mutations --chart bar --terminal)"
test -n "$out"
echo "$out" | mustmatch not like "# Study Mutation Frequency"
```

## Chart Flag: Expression Histogram

`--chart histogram --terminal` on an expression query should produce terminal chart output.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study query --study brca_tcga_pan_can_atlas_2018 --gene ERBB2 --type expression --chart histogram --terminal)"
test -n "$out"
echo "$out" | mustmatch not like "# Study Expression Distribution"
```

## Chart Flag: Co-occurrence Pie Chart

`--chart pie --terminal` on co-occurrence should produce terminal chart output.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study co-occurrence --study msk_impact_2017 --genes TP53,KRAS --chart pie --terminal)"
test -n "$out"
echo "$out" | mustmatch not like "# Study Co-occurrence"
```

## Chart Flag: Custom Terminal Dimensions

`--cols` and `--rows` should constrain terminal chart output when the resolved target is the terminal.

```bash
[ -f "$PWD/.cache/spec-study-env" ] || bash fixtures/setup-study-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-study-env"
out="$(biomcp study query --study msk_impact_2017 --gene TP53 --type mutations --chart bar --cols 40 --rows 12)"
clean="$(printf '%s\n' "$out" | sed 's/\x1b\[[0-9;?]*[A-Za-z]//g')"
line_count="$(printf '%s\n' "$clean" | wc -l | tr -d ' ')"
test "$line_count" -le 12
printf '%s\n' "$clean" | mustmatch not like "# Study"
```

## Chart Flag: Co-occurrence Heatmap

`--chart heatmap --terminal` on co-occurrence should render chart output instead of the standard markdown table.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study co-occurrence --study msk_impact_2017 --genes TP53,KRAS,PIK3CA --chart heatmap --terminal)"
test -n "$out"
echo "$out" | mustmatch not like "# Study Co-occurrence"
```

## Chart Flag: Co-occurrence Heatmap SVG Output

`--chart heatmap -o file.svg` should write a valid SVG file through the CLI output path.

```bash
. "$PWD/.cache/spec-study-env"
tmpdir="$(mktemp -d)"
out="$(biomcp study co-occurrence --study msk_impact_2017 --genes TP53,KRAS,PIK3CA --chart heatmap -o "$tmpdir/cooccurrence-heatmap.svg")"
echo "$out" | mustmatch like "Wrote SVG chart to"
test -s "$tmpdir/cooccurrence-heatmap.svg"
grep -q "<svg" "$tmpdir/cooccurrence-heatmap.svg"
rm -rf "$tmpdir"
```

## Chart Flag: Custom SVG Dimensions

`--width` and `--height` should set explicit SVG canvas dimensions.

```bash
. "$PWD/.cache/spec-study-env"
tmpdir="$(mktemp -d)"
out="$(biomcp study query --study brca_tcga_pan_can_atlas_2018 --gene ERBB2 --type expression --chart histogram --width 1200 --height 600 -o "$tmpdir/erbb2-histogram.svg")"
echo "$out" | mustmatch like "Wrote SVG chart to"
grep -q 'width="1200' "$tmpdir/erbb2-histogram.svg"
grep -q 'height="600' "$tmpdir/erbb2-histogram.svg"
rm -rf "$tmpdir"
```

## Chart Flag: Heatmap Palette Rejection

Heatmaps use a fixed continuous colormap in this release, so `--palette` should fail with the documented guidance instead of being ignored.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study co-occurrence --study msk_impact_2017 --genes TP53,KRAS,PIK3CA --chart heatmap --palette wong --terminal 2>&1 || true)"
echo "$out" | mustmatch like "study co-occurrence --chart heatmap"
echo "$out" | mustmatch like "fixed continuous colormap"
```

## Chart Flag: Compare Violin Plot

`--chart violin --terminal` on compare expression should produce terminal chart output.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study compare --study brca_tcga_pan_can_atlas_2018 --gene TP53 --type expression --target ERBB2 --chart violin --terminal)"
test -n "$out"
echo "$out" | mustmatch not like "# Study Group Comparison"
```

## Chart Flag: Compare Scatter Plot

`--chart scatter --terminal` on compare expression should render paired two-gene expression values instead of the standard markdown summary.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study compare --study brca_tcga_pan_can_atlas_2018 --gene TP53 --type expression --target ERBB2 --chart scatter --terminal)"
test -n "$out"
echo "$out" | mustmatch not like "# Study Group Comparison"
```

## Chart Flag: Mutation Waterfall

`--chart waterfall --terminal` on mutation queries should render ranked sample mutation counts instead of the standard markdown heading.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study query --study msk_impact_2017 --gene TP53 --type mutations --chart waterfall --terminal)"
test -n "$out"
echo "$out" | mustmatch not like "# Study Mutation Frequency"
```

## Chart Flag: Mutation Compare Stacked Bar

`--chart stacked-bar --terminal` on mutation comparison should render chart output instead of the standard markdown summary.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study compare --study brca_tcga_pan_can_atlas_2018 --gene TP53 --type mutations --target PIK3CA --chart stacked-bar --terminal)"
test -n "$out"
echo "$out" | mustmatch not like "# Study Group Comparison"
```

## Chart Flag: Mutation Compare Stacked Bar SVG Output

`--chart stacked-bar -o file.svg` should write a valid SVG file through the CLI output path.

```bash
. "$PWD/.cache/spec-study-env"
tmpdir="$(mktemp -d)"
out="$(biomcp study compare --study brca_tcga_pan_can_atlas_2018 --gene TP53 --type mutations --target PIK3CA --chart stacked-bar -o "$tmpdir/pik3ca-by-tp53-stacked.svg")"
echo "$out" | mustmatch like "Wrote SVG chart to"
test -s "$tmpdir/pik3ca-by-tp53-stacked.svg"
grep -q "<svg" "$tmpdir/pik3ca-by-tp53-stacked.svg"
rm -rf "$tmpdir"
```

## Chart Flag: Survival Bar Chart

`--chart bar --terminal` on survival should produce terminal chart output instead of the standard survival markdown heading. The fixture-backed proof only checks that chart mode is active; the rendered bar heights are covered by Rust tests and later real-data verification.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study survival --study brca_tcga_pan_can_atlas_2018 --gene TP53 --chart bar --terminal)"
test -n "$out"
echo "$out" | mustmatch not like "# Study Survival"
```

## Chart Flag: Survival KM Chart

`--chart survival --terminal` on survival should switch the command into chart mode and render a Kaplan-Meier curve rather than the default markdown summary.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study survival --study brca_tcga_pan_can_atlas_2018 --gene TP53 --chart survival --terminal)"
test -n "$out"
echo "$out" | mustmatch not like "# Study Survival"
```

## Chart Flag: Invalid Chart Type Error

Incompatible chart type and query type combinations should fail with a clear error listing valid options.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study query --study msk_impact_2017 --gene TP53 --type mutations --chart violin --terminal 2>&1 || true)"
echo "$out" | mustmatch like "chart type 'violin' is not valid for 'study query --type mutations'. Valid types: bar, pie, waterfall"
```

## Chart Flag: Co-occurrence Invalid Chart Type Error

Incompatible co-occurrence chart types should list the new heatmap option alongside the existing valid types.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study co-occurrence --study msk_impact_2017 --genes TP53,KRAS --chart violin --terminal 2>&1 || true)"
echo "$out" | mustmatch like "chart type 'violin' is not valid for 'study co-occurrence'. Valid types: bar, pie, heatmap"
```

## Chart Flag: Mutation Compare Invalid Chart Type Error

Incompatible mutation-comparison chart types should list both `bar` and `stacked-bar`.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study compare --study msk_impact_2017 --gene TP53 --type mutations --target KRAS --chart violin --terminal 2>&1 || true)"
echo "$out" | mustmatch like "chart type 'violin' is not valid for 'study compare --type mutations'. Valid types: bar, stacked-bar"
```

## Chart Flag: Expression Compare Invalid Chart Type Error

Incompatible expression-comparison chart types should list `scatter` alongside the grouped distribution plots.

```bash
. "$PWD/.cache/spec-study-env"
out="$(biomcp study compare --study msk_impact_2017 --gene TP53 --type expression --target ERBB2 --chart pie --terminal 2>&1 || true)"
echo "$out" | mustmatch like "study compare --type expression"
echo "$out" | mustmatch like "Valid types: box, violin, ridgeline, scatter"
```

## Chart Subcommand: Documentation

`biomcp chart` should show chart overview documentation. Specific chart pages should include the new heatmap and stacked-bar topics alongside survival.

```bash
out="$(biomcp chart)"
test -n "$out"
echo "$out" | mustmatch like "## Chart Types by Command"
echo "$out" | mustmatch like "study query --type mutations"
echo "$out" | mustmatch like "study co-occurrence"
echo "$out" | mustmatch like "study compare --type expression"
echo "$out" | mustmatch like "study survival"
echo "$out" | mustmatch like "Heatmaps use a fixed continuous colormap."
```

```bash
out="$(biomcp chart bar)"
test -n "$out"
echo "$out" | mustmatch like "# Bar Chart"
```

```bash
out="$(biomcp chart survival)"
test -n "$out"
echo "$out" | mustmatch like "# Survival Chart"
```

```bash
out="$(biomcp chart heatmap)"
test -n "$out"
echo "$out" | mustmatch like "Heatmaps render pairwise co-mutation counts as an NxN matrix"
echo "$out" | mustmatch like "`--palette` is not supported for heatmaps in this release."
```

```bash
out="$(biomcp chart stacked-bar)"
test -n "$out"
echo "$out" | mustmatch like "# Stacked Bar Chart"
echo "$out" | mustmatch like "mutation rate"
```

```bash
out="$(biomcp chart waterfall)"
test -n "$out"
echo "$out" | mustmatch like "# Waterfall"
echo "$out" | mustmatch like "mutation burden"
```

```bash
out="$(biomcp chart scatter)"
test -n "$out"
echo "$out" | mustmatch like "Scatter plots render paired expression values for two genes"
echo "$out" | mustmatch like "Only samples with numeric expression values for both genes are plotted."
```
