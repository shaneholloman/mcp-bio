# Optimize

## Starting Baseline

- Primary optimization target: vaccine phrase-or-exact identity linkage to
  existing MyChem-backed drug records.
- Baseline warm-cache benchmark on `2026-04-17`:
  - finished pharma rows: `656`
  - vaccine rows: `284`
  - API rows: `191`
  - immunization-device rows: `459`
  - device categories: `11`
  - vaccine winning strategy: `component_vaccine_coverage`
  - vaccine phrase or exact identity rate: `30.28%`
  - vaccine exact identity rate: `17.25%`
  - API normalized-INN phrase or exact rate: `91.10%`
  - API normalized-INN exact rate: `79.06%`
  - API finished-pharma exact overlap rate: `71.73%`
  - API finished-pharma component overlap rate: `93.72%`
  - dose-count completeness: `99.65%`
  - immunization schedule completeness: `0.0%`
  - cold-chain completeness: `0.0%`
  - warm-cache full-build wall time: `0.1808 s`
- Baseline regression control:
  - exact mismatch count: `0` for `schema_comparison`, `vaccine_identity`,
    `api_linkage`, and `metadata_and_devices`

## Optimization Passes

### Pass 1

- Hotspot:
  - `classify_hits()` in
    `architecture/experiments/25-who-vaccines-and-apis/scripts/who_vaccines_apis_lib.py`
    (`569-597`)
  - measured cost: `0.101 s` cumulative in the baseline warm-cache
    `vaccine_identity_probe.py` profile
- Approach:
  - broadened `split_vaccine_components()` so WHO vaccine labels are split on
    commas, `and`, and alphabetic hyphens instead of only `/`
- Before / after:
  - vaccine phrase or exact rate: `30.28%` -> `44.72%`
  - vaccine exact rate: `17.25%` -> `17.25%`
  - API normalized-INN phrase or exact rate: `91.10%` -> `91.10%`
  - warm-cache full-build wall time: `0.1808 s` -> `0.1304 s`
- Result:
  - committed as `c2e7e18a`

### Pass 2

- Hotspot:
  - `classify_hits()` in the same critical path, now at `0.171 s` cumulative
- Approach:
  - stripped formulation-only fragments such as `seasonal`, `trivalent`,
    `oral`, and `types 1 and 3` inside `split_vaccine_components()`
- Before / after:
  - vaccine phrase or exact rate: `44.72%` -> `44.72%`
  - vaccine exact rate: `17.25%` -> `17.25%`
  - API normalized-INN phrase or exact rate: `91.10%` -> `91.10%`
  - warm-cache full-build wall time: `0.1304 s` -> `0.1478 s`
- Result:
  - reverted
  - the real component strategy still appended ` vaccine` to already
    vaccine-bearing fragments, so the dry-run gain did not survive the actual
    pipeline

### Pass 3

- Hotspot:
  - `classify_hits()` in
    `architecture/experiments/25-who-vaccines-and-apis/scripts/who_vaccines_apis_lib.py`
    (`573-601`)
  - measured cost: `0.171 s` cumulative before the pass
- Approach:
  - added component-query normalization in
    `architecture/experiments/25-who-vaccines-and-apis/scripts/vaccine_identity_probe.py`
    to remove formulation-only tokens, drop numeric fragments, and avoid
    doubled `vaccine` suffixes
- Before / after:
  - vaccine phrase or exact rate: `44.72%` -> `57.04%`
  - vaccine exact rate: `17.25%` -> `17.25%`
  - mean component phrase or exact coverage: `56.68%` -> `62.84%`
  - API normalized-INN phrase or exact rate: `91.10%` -> `91.10%`
  - warm-cache full-build wall time: `0.1304 s` -> `0.1445 s`
- Result:
  - committed as `da02983a`

### Pass 4

- Hotspot:
  - `classify_hits()` in the same critical path at `0.169 s` cumulative after
    pass 3
- Approach:
  - added a narrow `component_with_commercial_fallback` strategy that preserves
    the improved component path but lets a row resolve on `Commercial Name`
    when component matching still misses
- Before / after:
  - vaccine phrase or exact rate: `57.04%` -> `57.39%`
  - vaccine exact rate: `17.25%` -> `18.31%`
  - API normalized-INN phrase or exact rate: `91.10%` -> `91.10%`
  - warm-cache full-build wall time: `0.1445 s` -> `0.2081 s`
- Result:
  - committed as `a28c6042`

## Final Numbers

- Final warm-cache benchmark:
  - finished pharma rows: `656`
  - vaccine rows: `284`
  - API rows: `191`
  - immunization-device rows: `459`
  - device categories: `11`
  - vaccine winning strategy: `component_with_commercial_fallback`
  - vaccine phrase or exact identity rate: `57.39%`
  - vaccine exact identity rate: `18.31%`
  - API normalized-INN phrase or exact rate: `91.10%`
  - API normalized-INN exact rate: `79.06%`
  - API finished-pharma exact overlap rate: `71.73%`
  - API finished-pharma component overlap rate: `93.72%`
  - dose-count completeness: `99.65%`
  - immunization schedule completeness: `0.0%`
  - cold-chain completeness: `0.0%`
  - warm-cache full-build wall time: `0.2082 s`
- Final validation:
  - all five ticket vaccines still passed
  - counts unchanged: `BCG 7`, `measles 22`, `HPV 6`, `COVID-19 4`,
    `yellow fever 10`
- Final regression control:
  - overall passed: `true`
  - comparison mode:
    - `schema_comparison`: exact match
    - `api_linkage`: exact match
    - `metadata_and_devices`: exact match
    - `vaccine_identity`: match or beat
  - mismatch count: `0` for all four probes

## Total Improvement

| Metric | Baseline | Final | Delta |
| --- | --- | --- | --- |
| Finished pharma rows | `656` | `656` | `0` |
| Vaccine rows | `284` | `284` | `0` |
| API rows | `191` | `191` | `0` |
| Immunization-device rows | `459` | `459` | `0` |
| Device categories | `11` | `11` | `0` |
| Vaccine phrase-or-exact identity rate | `30.28%` | `57.39%` | `+27.11` percentage points |
| Vaccine exact identity rate | `17.25%` | `18.31%` | `+1.06` percentage points |
| API raw phrase-or-exact rate | `90.05%` | `90.05%` | `0.00` percentage points |
| API normalized phrase-or-exact rate | `91.10%` | `91.10%` | `0.00` percentage points |
| API normalized exact rate | `79.06%` | `79.06%` | `0.00` percentage points |
| API finished-pharma exact overlap rate | `71.73%` | `71.73%` | `0.00` percentage points |
| API finished-pharma component overlap rate | `93.72%` | `93.72%` | `0.00` percentage points |
| Dose-count completeness | `99.65%` | `99.65%` | `0.00` percentage points |
| Immunization schedule completeness | `0.0%` | `0.0%` | `0.00` percentage points |
| Cold-chain completeness | `0.0%` | `0.0%` | `0.00` percentage points |
| Warm-cache full-build wall time | `0.1808 s` | `0.2082 s` | `+0.0274 s` |

## Convergence

- The optimize step stopped after pass 4 because the primary metric improved by
  only `0.35` percentage points (`0.61%` relative), which is below the
  `5%` convergence threshold.
- The ticket target was not reached. Final vaccine phrase-or-exact identity
  linkage is `57.39%`, still well below the `>80%` target.
- The remaining misses are no longer mostly parsing mistakes. They are class or
  brand labels such as pneumococcal, polio, HPV, RSV, and COVID-19 terms that
  need curated vaccine-domain synonym knowledge rather than more delimiter or
  token cleanup.
- API metrics were already above target at baseline and stayed unchanged.

## Remaining Opportunities

- Add a curated vaccine synonym layer for unresolved class labels such as
  pneumococcal, poliomyelitis/polio, HPV, RSV, and COVID-19 vaccine variants.
- Bridge WHO commercial names to antigen identities with an external vaccine
  ontology or a maintained brand-to-antigen map.
- Memoize normalized MyChem alias summaries inside `classify_hits()` if future
  work cares about throughput as much as identity coverage.
- Stop forcing vaccines through a drug-INN-shaped identity surface and model
  them as a vaccine-specific entity with explicit pathogen and formulation
  semantics.
