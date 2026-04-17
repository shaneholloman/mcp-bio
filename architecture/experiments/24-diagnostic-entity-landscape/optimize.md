# Optimize

## Starting Baseline

- Primary target: push combined ClinVar-pathogenic gene coverage above `80%`.
- Baseline full-scale coverage on `2026-04-17`:
  - combined coverage: `8386 / 11085` (`75.65%`)
  - GTR coverage: `8386 / 11085` (`75.65%`)
  - FDA coverage: `14 / 11085` (`0.13%`)
  - WHO coverage: `4 / 11085` (`0.04%`)
- Baseline build timing:
  - internal full-build timer: `65.84 s`
  - wall clock: `66.09 s`
- Regression control baseline:
  - exact metric mismatch count: `0` for `gtr_bulk`, `gtr_api`, `who_ivd`,
    `fda_device`, and `cross_source_matrix`
  - projection checksums matched for all five artifacts
  - live `gtr_api` and openFDA latency waivers remained necessary

## Optimization Passes

### Pass 1

- Hotspot:
  - `load_gtr_backbone()` in
    `architecture/experiments/24-diagnostic-entity-landscape/scripts/diagnostic_landscape_lib.py`
    (`222-308`)
  - measured cost: `8.89 s` cumulative in the profiled full build
- Approach:
  - tested every larger GTR recall lever first: GeneID remapping, delimiter
    expansion, object-name parsing, and full `gtr_ftp.xml.gz` parsing
  - all of those produced `0` incremental covered ClinVar genes
  - kept only a conservative title fallback for exact case-sensitive symbols in
    `lab_test_name` / `manufacturer_test_name`
- Before / after:
  - combined coverage: `8386 / 11085` (`75.65%`) ->
    `8389 / 11085` (`75.68%`)
  - GTR coverage: `8386 / 11085` (`75.65%`) ->
    `8389 / 11085` (`75.68%`)
  - full-build timer: `65.84 s` -> `66.60 s`
- Result:
  - committed as `9c29cd1b`

### Pass 2

- Hotspot:
  - `fetch_openfda_query()` plus `load_fda_molecular_slice()` in
    `architecture/experiments/24-diagnostic-entity-landscape/scripts/diagnostic_landscape_lib.py`
    (`443-477`, `504-550`, `591-688`)
  - measured cost: about `6.71 s` cumulative in the profiled full build
- Approach:
  - current FDA linkage only used short device naming fields
  - FDA `ao_statement` approval-order text contains explicit companion
    diagnostic gene claims, so I added a conservative case-sensitive extractor
    there, with a short-symbol whitelist only for `ATM`, `MET`, and `RET`
- Before / after:
  - combined coverage: `8389 / 11085` (`75.68%`) ->
    `8389 / 11085` (`75.68%`)
  - FDA coverage: `14 / 11085` (`0.13%`) -> `22 / 11085` (`0.20%`)
  - FDA gene-linked records: `38.28%` -> `42.19%`
  - full-build timer: `66.60 s` -> `67.36 s`
- Result:
  - committed as `9e2a7724`

### Pass 3

- Hotspot:
  - same FDA retrieval path as pass 2
- Approach:
  - expanded the FDA PMA and 510(k) query sets with companion-diagnostic and
    NGS terms to test whether retrieval breadth, not linkage logic, was still
    the limiting factor
- Before / after:
  - combined coverage: `8389 / 11085` (`75.68%`) ->
    `8389 / 11085` (`75.68%`)
  - FDA coverage: `22 / 11085` (`0.20%`) -> `24 / 11085` (`0.22%`)
  - FDA combined records: `128` -> `210`
  - FDA gene-linked records: `42.19%` -> `28.10%`
  - full-build timer: `67.36 s` -> `68.08 s`
- Result:
  - reverted
  - added breadth mostly produced off-target records and diluted the FDA
    linkage density without moving the ticket metric

## Final Numbers

- Combined coverage: `8389 / 11085` (`75.68%`)
- GTR coverage: `8389 / 11085` (`75.68%`)
- FDA coverage: `22 / 11085` (`0.20%`)
- WHO coverage: `4 / 11085` (`0.04%`)
- Final build timing:
  - internal full-build timer: `63.16 s`
  - wall clock: `63.35 s`
- Final regression-control status:
  - deterministic checks still matched exactly for `gtr_bulk`, `who_ivd`,
    `fda_device`, and `cross_source_matrix`
  - `gtr_api` correctness still matched exactly, but live latency remained
    outside the `3%` band and required the same waiver pattern as exploit
- Final live-latency noise probe:
  - GTR gene rounds: `194.76 / 477.54 ms` and `129.76 / 402.16 ms`
  - GTR disease rounds: `131.1 / 614.97 ms` and `136.8 / 623.57 ms`
  - openFDA sample fetches: `626.4 ms`, `623.5 ms`, `624.0 ms`

## Total Improvement

| Metric | Baseline | Final | Delta |
| --- | --- | --- | --- |
| Combined covered ClinVar genes | `8386` | `8389` | `+3` |
| Combined coverage pct | `75.65%` | `75.68%` | `+0.03` percentage points |
| GTR covered ClinVar genes | `8386` | `8389` | `+3` |
| FDA covered ClinVar genes | `14` | `22` | `+8` |
| FDA gene-linked records pct | `38.28%` | `42.19%` | `+3.91` percentage points |
| WHO covered ClinVar genes | `4` | `4` | `0` |
| Full build timer | `65.84 s` | `63.16 s` | `-2.68 s` |

## Convergence

- The `>80%` target was not reached; the final combined metric stopped at
  `75.68%`.
- Pass 1 improved the primary combined metric by only `3` genes, which is far
  below the `5%` convergence threshold.
- Pass 2 improved FDA source-native linkage density, but combined coverage
  stayed flat because every newly recovered FDA-linked gene was already present
  in the GTR backbone.
- Pass 3 increased retrieval breadth, added runtime, and diluted FDA linkage
  density, so it was reverted.
- The highest-upside GTR completeness experiment, parsing the full
  `gtr_ftp.xml.gz` dump, reproduced exactly the same covered-gene count as the
  tabular join. That closed off the main remaining bulk-data recall path.
- At this point, further local optimization is in diminishing-returns territory.
  Meaningful progress toward `>80%` would require a different data model or a
  different source strategy, not more tuning of the existing exploit pipeline.

## Remaining Opportunities

- Curate a clinically justified mapping from noncanonical ClinVar loci,
  antisense transcripts, and fragile-site symbols to anchor diagnostic genes.
- Add field-level FDA parsing beyond `ao_statement`, but only with explicit
  ambiguity controls for short or common symbols.
- Introduce a new source or a separate clinically named denominator; the
  current GTR bulk plus FDA plus WHO stack appears saturated against the raw
  ClinVar-symbol universe.
- If downstream implementation cares more about the OMIM-tagged subset than the
  raw ClinVar tail, optimize for that clinically named denominator explicitly
  instead of the full `11085`-symbol set.
