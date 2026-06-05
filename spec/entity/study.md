# Study Queries

The `study` surface is BioMCP's local cBioPortal analytics layer, separate from
the remote trial registry surface. These canaries keep the local catalog,
typed query grammar, validation messages, and chartable summaries visible
without pinning install-specific row totals.

## Local Study Discovery

Listing studies should still look like a local dataset catalog, with stable
identity and availability columns that tell operators what data is actually on
disk.

```bash
../../tools/biomcp-ci study list | mustmatch like '# Study Datasets
...
| Study ID | Name | Cancer Type | Samples | Available Data |'
```

## Gene-Frequency Summary

Per-study mutation queries should keep a human-readable summary heading and the
variant-class breakout that explains what was counted.

```bash
../../tools/biomcp-ci study query --help | mustmatch like 'Canonical values: mutations, cna, expression.
Accepted aliases: mutation, copy_number, copy-number, expr'
../../tools/biomcp-ci study query --study msk_impact_2017 --gene TP53 --type mutations | mustmatch like '# Study Mutation Frequency: TP53 (msk_impact_2017)
## Top Variant Classes
## Top Protein Changes'
```

## Remote Download Stall Policy

Remote DataHub archives can be large, but a server that starts an archive
response and then stops sending bytes should fail clearly instead of hanging
forever.

```bash
cargo test download_study_stalled_archive_body_times_out_with_clear_error --lib 2>&1 | mustmatch like 'download_study_stalled_archive_body_times_out_with_clear_error
test result: ok'
```

## Filter Validation

Filter workflows should reject missing criteria explicitly instead of silently
returning the full cohort.

```bash
../../tools/biomcp-ci study filter --study brca_tcga_pan_can_atlas_2018 2>&1 | mustmatch like 'At least one filter criterion is required.
--mutated, --amplified, --deleted'
```

## Survival Validation

Survival analysis should stay typed: unknown endpoint names must fail fast and
tell the operator which endpoint vocabulary is valid.

```bash
../../tools/biomcp-ci study cohort --study msk_impact_2017 --gene TP53 | mustmatch like '# Study Cohort: TP53 (msk_impact_2017)
| Group | Samples | Patients |'
../../tools/biomcp-ci study survival --study msk_impact_2017 --gene TP53 --endpoint foo 2>&1 | mustmatch like "Unknown survival endpoint 'foo'.
Expected: os, dfs, pfs, dss."
```

## Typed Comparison Validation

Comparison and co-occurrence analytics should reject malformed inputs before
running local cohort work.

```bash
../../tools/biomcp-ci study compare --study msk_impact_2017 --gene TP53 --type foo --target ERBB2 2>&1 | mustmatch like "Unknown comparison type 'foo'. Expected: expression, mutations."
../../tools/biomcp-ci study co-occurrence --study msk_impact_2017 --genes TP53 2>&1 | mustmatch like '--genes must contain 2 to 10 comma-separated symbols'
```

## Comparison & Chart Output

Study analytics should remain usable from the terminal: comparison summaries
stay tabular, and chart mode still exposes a visible title and axis label.

```bash
../../tools/biomcp-ci study compare --study msk_impact_2017 --gene TP53 --type mutations --target ERBB2 | mustmatch like '# Study Group Comparison: Mutation Rate
| Group | N | Mutated | Mutation Rate |'
../../tools/biomcp-ci study query --study msk_impact_2017 --gene TP53 --type mutations --chart bar | mustmatch like 'TP53 mutation classes
Variant class'
```
