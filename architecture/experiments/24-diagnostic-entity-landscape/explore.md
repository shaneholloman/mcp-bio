# Explore

## Spike Question

Can BioMCP build a useful `diagnostic` entity from GTR, WHO IVD, and FDA
device data, and what cross-entity links are actually achievable from genes,
diseases, and variants to test records?

Measured against the ticket's small-scale panel, the answer is yes for a
GTR-backed diagnostic entity, but not for an equal-weight three-source merge.
GTR is strong enough to justify a first-class entity. WHO IVD and FDA device
data are better treated as overlays than as the core linkage spine.

## Prior Art Summary

The relevant BioMCP pattern already exists in `src/sources/who_pq.rs`,
`src/sources/ema.rs`, and `src/entities/drug/`: keep each source responsible
for sync, validation, and typed parsing, then compose source-specific views at
the entity layer through alias-aware identity bridges instead of forcing a
single global schema too early.

That pattern held up in this spike. WHO IVD behaves like the existing WHO PQ
CSV integration. GTR bulk data fits the same local-cache pattern, just with a
two-file join instead of a single CSV. FDA data looks like another regional
regulatory slice. What changed is the merge strategy: diagnostics should not
start as a symmetric multi-source union. The measured winner is a GTR backbone
with FDA and WHO overlays.

## Approaches Tried

### 1. GTR Bulk Download Parse

What: parsed `test_version.gz` and `test_condition_gene.txt` with Python,
restricted to `now_current=1` tests.

How: joined on accession version, then measured current-test counts, gene and
disease linkage, lab/regulatory metadata, and sample-gene/sample-disease
coverage.

Measurements:
- Current tests: `64374`
- Gene links: `96.17%`
- Disease links: `100.0%`
- Lab/manufacturer field: `100.0%`
- CLIA/state-license metadata: `55.76%`
- Sample gene counts:
  - `BRCA1`: `438`
  - `EGFR`: `181`
  - `BRAF`: `437`
  - `KRAS`: `455`
  - `TP53`: `598`
- Sample disease counts:
  - `breast cancer`: `92`
  - `melanoma`: `283`
  - `lung cancer`: `58`

Comparison to prior art: this cleanly validates the cached local-file source
pattern already used for WHO PQ. The main difference is that diagnostics need
an internal join between a versioned metadata file and a relation file.

Takeaway: this is the only approach that clearly satisfies the ticket success
criteria on the oncology sample.

### 2. GTR Live Query Path

What: queried NCBI GTR E-utilities at ticket scale using gene, disease, and a
method-category test-type proxy.

How: rate-limited public requests, used `SYMB` and `DISNAME` search fields, and
fetched `esummary` payloads for returned IDs to measure schema richness and
latency.

Measurements:
- Mean gene search latency: `121.1 ms`
- Mean gene summary latency: `382.02 ms`
- Mean disease search latency: `126.47 ms`
- Mean disease summary latency: `604.93 ms`
- Sample gene counts:
  - `BRCA1`: `441`
  - `EGFR`: `182`
  - `BRAF`: `444`
  - `KRAS`: `457`
  - `TP53`: `599`
- Sample disease counts:
  - `breast cancer`: `435`
  - `melanoma`: `283`
  - `lung cancer`: `132`
- Type-style probe:
  - `BRCA1[SYMB] AND Targeted variant analysis[MCAT]`: `45`

Comparison to prior art: the entity-layer pattern is reusable, but this source
is less operationally smooth than EMA/WHO because the public API was
intermittently fragile enough to require defensive retries and narrower probes.

Takeaway: the API is good enough for discovery and on-demand fetches, but bulk
data is the safer implementation backbone.

### 3. WHO IVD CSV Parse

What: downloaded the WHO prequalified IVD CSV and measured schema completeness
plus overlap with the ticket sample.

How: parsed the live CSV export, counted rows, assessed regulatory fields, and
looked for exact sample-gene matches plus disease phrase matches.

Measurements:
- Rows: `435`
- Manufacturer: `100.0%`
- Pathogen/disease/marker field: `100.0%`
- Regulatory version: `100.0%`
- Prequalification year: `100.0%`
- Sample genes: all `0`
- Sample diseases: all `0`

Comparison to prior art: this is almost a drop-in extension of the existing WHO
PQ CSV pattern.

Takeaway: useful regulatory data, but not useful for the oncology-focused gene
and disease pivots in this spike.

### 4. FDA Device Search Probe

What: queried openFDA 510(k) at ticket scale, then ran a small PMA side probe
for companion-diagnostic coverage.

How: measured a general schema sample from 510(k), then searched gene and
disease terms in device names with local relevance filtering to avoid obvious
false positives such as `eGFR`.

Measurements:
- openFDA 510(k) total records: `174612`
- Schema sample completeness:
  - device name: `100.0%`
  - applicant: `100.0%`
  - decision date: `100.0%`
  - advisory committee: `100.0%`
  - k-number: `100.0%`
- Sample gene counts:
  - `BRCA1`: `2`
  - `EGFR`: `0`
  - `BRAF`: `1`
  - `KRAS`: `0`
  - `TP53`: `2`
- Sample disease counts:
  - `breast cancer`: `2`
  - `melanoma`: `2`
  - `lung cancer`: `5`
- Companion-diagnostic side probe:
  - 510(k) drug-name searches for `pembrolizumab`, `osimertinib`,
    `vemurafenib`, and `trastuzumab` all returned `0`
  - PMA side searches returned `15`, `13`, `4`, and `21`

Comparison to prior art: the regional overlay model still fits, but FDA device
data is much weaker as a direct identity bridge than EMA/WHO are for drugs.

Takeaway: FDA is useful as a regulatory overlay, not as the core link graph.
Companion diagnostics likely require PMA coverage, not only 510(k).

### 5. Cross-Source Link Matrix

What: combined the sample counts from GTR bulk, WHO IVD, and FDA 510(k).

How: built gene-by-source and disease-by-source matrices and compared normalized
sample record names across sources.

Measurements:
- Gene matrix:
  - GTR bulk: `181` to `598` hits across the five genes
  - FDA 510(k): `0` to `2`
  - WHO IVD: `0`
- Disease matrix:
  - GTR bulk: `58` to `283`
  - FDA 510(k): `2` to `5`
  - WHO IVD: `0`
- Exact normalized-name overlap between sources: `0` for every pairwise
  comparison

Comparison to prior art: this reinforces the existing source-specific modeling
approach instead of arguing for an aggressively unified record layer at ingest
time.

Takeaway: cross-source joins are realistic at the entity level, but not as
clean record-level deduplication for the first exploit.

## Decision

Winner: `GTR bulk download parse`, with `GTR live query` as the interactive
query surface and `FDA device` / `WHO IVD` as later overlays.

Why:
- It is the only source that clears the ticket's success bar with more than
  `100` gene-linked tests and meaningful hits for all five sample genes.
- It has dense gene and disease links, and enough lab/certification metadata to
  justify a diagnostic entity before FDA/WHO integration is complete.
- The live GTR API reproduces the same basic counts at acceptable latency, so a
  CLI can still support targeted online lookups even if the implementation
  relies on bulk data for completeness.
- WHO IVD adds regulatory value but no oncology gene coverage.
- FDA 510(k) adds regulatory value but not enough direct gene linkage to anchor
  the entity, and companion diagnostics appear to live primarily in PMA.

Recommended first unified data model:
- Required core fields:
  - `source`
  - `source_id`
  - `name`
  - `test_category`
  - `manufacturer_or_lab`
  - `genes`
  - `conditions`
  - `methods`
  - `specimen_types`
  - `regulatory_status`
  - `regulatory_identifier`
  - `region`
- Source-specific extensions:
  - GTR: `offerer`, `certifications`, `clinical_validity`, `clinical_utility`
  - WHO IVD: `assay_format`, `prequalification_year`, `regulatory_version`
  - FDA device: `k_number_or_pma_number`, `decision_date`, `product_code`,
    `advisory_committee`

Cross-entity links actually achievable from this spike:
- `gene -> test`: strong in GTR
- `disease -> test`: strong in GTR, weak in FDA, absent in WHO for this sample
- `variant -> test`: not proven from the current bulk experiment; GTR exposes
  variant-capable fields in summaries, but exploit needs dedicated variant
  normalization work
- `drug/trial -> companion diagnostic`: weak from 510(k), more plausible from
  PMA, and not yet connected to trials in this spike

## Outcome

`promote`

## Risks for Exploit

- GTR should be the backbone, but the live public API showed enough transport
  fragility that exploit should not depend on it for completeness-critical
  workflows.
- FDA companion diagnostics should include PMA, not just 510(k), or the entity
  will miss important drug-linked diagnostics.
- Variant linkage is still an open question. The exploit phase needs a concrete
  plan for variant normalization and for deciding whether the first release is
  gene/disease-first.
- Cross-source exact-name overlap was `0` on the sample set, so exploit should
  avoid promising cross-source deduplication early. A better first step is
  source-specific slices under one entity surface.
- WHO IVD appears valuable mostly for infectious-disease and global-health
  diagnostics. If exploit is oncology-first, WHO IVD can be deferred or kept as
  a clearly scoped regional overlay.
