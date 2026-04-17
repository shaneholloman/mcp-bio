# Explore

## Spike Question

Can WHO vaccines and active pharmaceutical ingredients be loaded into the
existing `--region who` drug pipeline, or do vaccines need schema extensions?
What is the live data quality, and how well does each source cross-reference
with the existing MyChem-backed drug identity surface?

Measured on the live WHO exports fetched on `2026-04-17`, the answer is split:
APIs fit the existing WHO drug identity model well enough to extend the current
pipeline, but vaccines do not. Vaccines are missing an INN field, only weakly
resolve through the current MyChem bridge, and omit schedule and cold-chain
metadata entirely.

## Prior Art Summary

The current WHO implementation in `src/sources/who_pq.rs` is a tight,
finished-pharma-specific CSV integration:

- one local cached CSV export with strict required-header validation
- one typed `WhoPrequalificationEntry`
- `inn` derived heuristically from a combined presentation field
- WHO identity matching driven by lightweight normalization and MyChem-backed
  aliases
- structured WHO search implemented as "MyChem first, then filter WHO rows"

That prior art is a good fit for APIs because APIs also expose a real `INN`
field and respond well to the existing salt/combo normalization rules. It is a
poor fit for vaccines because the vaccine export has no `INN`, no dosage form,
no listing basis, and only one direct header overlap with the current required
finished-pharma contract.

## Approaches Tried

### 1. Schema-Fit Baseline

What:
- Parsed the live WHO finished-pharma, vaccine, API, and immunization-device
  exports.
- Compared live headers against the current finished-pharma WHO contract.

How:
- `scripts/schema_probe.py`
- Results: `results/who_schema_comparison.json`

Measurements:
- Finished pharma rows: `656`
- Vaccine rows: `284`
- API rows: `191`
- Immunization devices: `459` products across `11` categories
- Direct shared required finished-pharma headers:
  - Vaccines: `1/9` (`DATE OF PREQUALIFICATION`)
  - APIs: `2/9` (`DATE OF PREQUALIFICATION`, `THERAPEUTIC AREA`)

Comparison to prior art:
- The current parser contract does not accept either new export unchanged.
- APIs are still semantically close enough to map into the existing row model.
- Vaccines are structurally much farther away from the current WHO PQ design.

Takeaway:
- APIs look like an extension of the existing source.
- Vaccines do not fit `WhoPrequalificationEntry` without lossy field
  substitution.

### 2. Vaccine Identity Resolution Probe

What:
- Tried four MyChem bridge strategies against the live vaccine export:
  - `Commercial Name`
  - `Vaccine Type`
  - `Vaccine Type + " vaccine"`
  - component-wise coverage for slash-delimited combination vaccine types

How:
- `scripts/vaccine_identity_probe.py`
- Results: `results/vaccine_identity_probe.json`

Measurements:
- `Commercial Name`: `4.58%` phrase/exact, `2.46%` exact
- `Vaccine Type`: `30.28%` phrase/exact, `7.04%` exact
- `Vaccine Type + vaccine`: `17.61%` phrase/exact, `12.68%` exact
- `component_vaccine_coverage` winner: `30.28%` phrase/exact, `17.25%` exact

Comparison to prior art:
- The existing WHO search bridge assumes WHO rows can be pulled back to a drug
  identity through MyChem aliases.
- That assumption holds for finished pharma but breaks for vaccines because the
  WHO vaccine file mostly names pathogens or vaccine classes, not a stable INN.

Takeaway:
- The best measured vaccine strategy is far below the ticket success bar
  (`>70%`).
- Vaccines should not be loaded into the current WHO drug path by pretending
  `Vaccine Type` is an INN.

### 3. API Linkage Probe

What:
- Reused the current WHO salt/combo normalization logic on API `INN` values.
- Cross-referenced APIs against MyChem and against existing WHO finished-pharma
  rows.

How:
- `scripts/api_linkage_probe.py`
- Results: `results/api_linkage_probe.json`

Measurements:
- MyChem, raw API INN: `90.05%` phrase/exact, `78.01%` exact
- MyChem, normalized API INN: `91.10%` phrase/exact, `79.06%` exact
- API to finished-pharma overlap:
  - exact normalized INN overlap: `71.73%`
  - component/segment overlap: `93.72%`

Comparison to prior art:
- This is the cleanest extension of the current WHO PQ flow.
- The same salt stripping and combination normalization already used by
  `src/sources/who_pq.rs` is enough to link most APIs back to known drug
  identities and existing finished products.

Takeaway:
- APIs are a strong fit for the existing WHO drug model.
- The remaining misses are mostly edge-normalization cases such as sterile or
  dispersion variants, not a schema mismatch.

### 4. Vaccine Metadata and Device Divergence

What:
- Measured whether the vaccine export contains vaccine-specific fields that
  matter downstream.
- Assessed whether the immunization-device catalog looks close enough to share
  the WHO drug loader.

How:
- `scripts/vaccine_metadata_and_device_probe.py`
- Results: `results/vaccine_metadata_and_device_probe.json`

Measurements:
- Vaccine field completeness:
  - `Vaccine Type` present: `100.0%`
  - `No. of doses` present: `99.65%`
  - schedule field present: `0.0%`
  - cold-chain/storage field present: `0.0%`
- Ticket validation sample presence:
  - `BCG`: `7`
  - `measles`: `22`
  - `HPV`: `6` (listed as `Human Papillomavirus`)
  - `COVID-19`: `4`
  - `yellow fever`: `10`
- Immunization devices:
  - `459` products
  - category-mapped catalog with keys like `id`, `title`, `details`,
    `specifications`, `product_sites`, and `status`

Comparison to prior art:
- Vaccines need fields that are not present in the finished-pharma row and, for
  schedule/cold chain, not present in the WHO vaccine export at all.
- The device catalog is a separate product catalog, not a near-neighbor of the
  current WHO drug CSV.

Takeaway:
- Vaccines need either schema extensions or a separate entity path.
- Devices should be skipped for the current WHO drug extension.

## Decision

Winner: the API extension path, using the existing WHO normalization and local
cache pattern.

Decision:
- Extend the current WHO drug pipeline for `product_type=api`.
- Do not load vaccines into the existing `WhoPrequalificationEntry` as if they
  were finished pharma or INN-backed drug rows.
- If vaccines are pursued, give them either:
  - a schema extension with vaccine-specific fields, or
  - a separate WHO vaccine loader that can still reuse the same sync/cache
    infrastructure
- Keep immunization devices out of scope for the WHO drug pipeline.

Why:
- APIs satisfy the identity-linkage part of the spike with `91.10%`
  phrase/exact MyChem resolution and `93.72%` overlap to existing finished
  pharma by normalized segments.
- Vaccines fail the success bar: the best measured identity bridge reaches only
  `30.28%` phrase/exact, and the export has no INN field to anchor the current
  search/filter model.
- Vaccine-specific metadata is incomplete even before modeling:
  target/pathogen is only a proxy via `Vaccine Type`, while schedule and cold
  chain are absent.

Recommended exploit direction:
- Add API support to the existing WHO pipeline first.
- Treat vaccines as a separate design problem, not a small parser tweak.
- Add `product_type` filtering for `finished_pharma | api`, and only include
  `vaccine` later if a vaccine-specific row model is approved.

## Outcome

`pivot`

## Risks for Exploit

- API misses are concentrated in normalization edge cases such as sterile or
  dispersion wording. Exploit should harden INN normalization before promising
  near-total linkage.
- Vaccines still need a clear modeling decision:
  - minimal extension of the drug entity, or
  - a distinct vaccine surface under WHO data
- If downstream consumers need schedule or cold-chain data, WHO vaccine export
  alone is insufficient. Exploit would need an additional enrichment source.
- `Vaccine Type` is useful as a pathogen/antigen proxy but is not a safe
  substitute for a canonical active-substance identity.
- The immunization-device catalog should remain out of scope unless a later
  ticket explicitly targets device entities or device overlays.
