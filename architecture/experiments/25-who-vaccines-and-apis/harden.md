# Harden

## Decomposition

- Extracted the reusable spike code into the importable Python package
  `architecture/experiments/25-who-vaccines-and-apis/scripts/who_vaccines_apis/`.
- Kept the old flat modules as compatibility shims only:
  - `who_vaccines_apis_lib.py`
  - `who_vaccines_apis_exploit.py`
- Moved the monolith into focused library modules:
  - `who_vaccines_apis/types.py`
    - `WhoTable`
    - `WhoFinishedPharmaEntry`
    - `WhoFinishedPharmaTable`
    - `WhoDeviceCatalog`
    - `WhoFullScaleArtifacts`
  - `who_vaccines_apis/io.py`
    - experiment paths
    - cache paths
    - JSON/result IO
    - cached WHO fetch helpers
    - checksum and regression diff helpers
  - `who_vaccines_apis/loaders.py`
    - WHO finished-pharma, vaccine, API, and device loaders
    - schema overlap and schema summary helpers
  - `who_vaccines_apis/identity.py`
    - INN normalization
    - vaccine component splitting
    - MyChem query/caching
    - hit classification
  - `who_vaccines_apis/probes.py`
    - schema probe builder
    - vaccine identity probe builder
    - API linkage probe builder
    - metadata/device probe builder
    - probe-suite runner and summary writer
  - `who_vaccines_apis/reports.py`
    - validation payload builder
    - sample-record payload builder
    - loader-design payload builder
    - regression-control builder
    - full-scale artifact builder
- Thinned every script under `scripts/` to import-and-run wrappers only.
  All wrappers are now under `20` lines. The old `623` line exploit script and
  `692` line helper are gone from the execution path.

Why this shape:

- Drug entity extension work needs direct imports for normalized WHO loaders,
  design payloads, and contract numbers.
- WHO issue `#235` response work needs a library call that can rebuild the
  full-scale payload and recommendation without shelling out.
- The package split keeps the dependency direction correct: wrappers import the
  library, and the library does not import wrappers.

## Public API

Stable import root:

- `who_vaccines_apis`

Shared types:

- `WhoTable`
- `WhoFinishedPharmaEntry`
- `WhoFinishedPharmaTable`
- `WhoDeviceCatalog`
- `WhoFullScaleArtifacts`

Primary functions:

- `load_finished_pharma() -> WhoFinishedPharmaTable`
- `load_vaccines() -> WhoTable`
- `load_apis() -> WhoTable`
- `load_devices() -> WhoDeviceCatalog`
- `build_schema_probe_payload() -> dict[str, Any]`
- `build_vaccine_identity_probe_payload() -> dict[str, Any]`
- `build_api_linkage_probe_payload() -> dict[str, Any]`
- `build_metadata_probe_payload() -> dict[str, Any]`
- `run_probe_suite() -> tuple[dict[str, dict[str, Any]], dict[str, float]]`
- `build_validation_payload() -> dict[str, Any]`
- `build_loader_design_payload(probe_payloads, validation_payload) -> dict[str, Any]`
- `build_regression_control_payload() -> dict[str, Any]`
- `build_full_scale_results() -> WhoFullScaleArtifacts`

Supporting reusable helpers:

- `normalize_match_key()`
- `split_vaccine_components()`
- `classify_hits()`
- `write_json()`
- `source_snapshot()`

Usage examples:

```python
from who_vaccines_apis import build_full_scale_results

artifacts = build_full_scale_results()
print(artifacts.contract_numbers["api_identity"]["normalized_inn_phrase_or_exact_rate"])
print(artifacts.loader_design_payload["recommendation"])
```

```python
from who_vaccines_apis import build_loader_design_payload, build_validation_payload, run_probe_suite

probe_payloads, stage_timings = run_probe_suite()
validation = build_validation_payload()
design = build_loader_design_payload(probe_payloads, validation)
print(stage_timings["total_seconds"])
print(design["product_type_filter_design"])
```

```python
from who_vaccines_apis import load_finished_pharma, load_apis

finished = load_finished_pharma()
apis = load_apis()
print(finished.entries[0].who_reference_number)
print(apis.rows[0]["WHO Product ID"])
```

## Build System

There is no `build.zig` in this spike because the ticket explicitly constrains
the implementation to Python. The equivalent build/dependency work is the
experiment-local package at
`architecture/experiments/25-who-vaccines-and-apis/scripts/pyproject.toml`.

Build/dependency changes:

- package discovery now exposes `who_vaccines_apis`
- console-script entrypoints now expose the thin CLI wrappers
- downstream spikes can depend on the package directly instead of importing
  script files

Verified build/import paths:

1. Direct path import during another spike:

```bash
PYTHONPATH=architecture/experiments/25-who-vaccines-and-apis/scripts python3 your_script.py
```

2. Local path dependency install:

```bash
python3 -m pip install --no-deps --target /tmp/who_vaccines_apis_pkg_test \
  architecture/experiments/25-who-vaccines-and-apis/scripts
```

Verified result:

- `python3 -m pip install --no-deps --target /tmp/who_vaccines_apis_pkg_test architecture/experiments/25-who-vaccines-and-apis/scripts`
- outcome: succeeded, installed `who-vaccines-and-apis-spike-0.1.0`
- installed artifacts included:
  - `/tmp/who_vaccines_apis_pkg_test/who_vaccines_apis`
  - `/tmp/who_vaccines_apis_pkg_test/bin`

## Regression Check

Runtime pass performed after the refactor:

- `python3 architecture/experiments/25-who-vaccines-and-apis/scripts/build_full_scale_results.py`
- `python3 architecture/experiments/25-who-vaccines-and-apis/scripts/regression_control.py`
- `python3 architecture/experiments/25-who-vaccines-and-apis/scripts/validate_vaccines.py`

Warm-cache build timings:

- first post-refactor full-build run: `0.2241 s`
- five immediate reruns: `0.2066 s`, `0.1999 s`, `0.1930 s`, `0.1936 s`,
  `0.1980 s`
- final recorded full-build wall time: `0.1980 s`
- optimized baseline from `optimize.md`: `0.2082 s`

Contract metrics held exactly:

- vaccine winning strategy: `component_with_commercial_fallback`
- vaccine phrase-or-exact rate: `57.39%`
- vaccine exact rate: `18.31%`
- API normalized-INN phrase-or-exact rate: `91.10%`
- API normalized-INN exact rate: `79.06%`
- API finished-pharma exact overlap rate: `71.73%`
- API finished-pharma component overlap rate: `93.72%`
- dose-count completeness: `99.65%`
- schedule completeness: `0.0%`
- cold-chain completeness: `0.0%`

Regression control:

- overall passed: `true`
- `schema_comparison`: exact match, `0` mismatches
- `api_linkage`: exact match, `0` mismatches
- `metadata_and_devices`: exact match, `0` mismatches
- `vaccine_identity`: match-or-beat, `0` mismatches

Validation:

- overall passed: `true`
- anchor counts unchanged:
  - `BCG`: `7`
  - `measles`: `22`
  - `HPV`: `6`
  - `COVID-19`: `4`
  - `yellow fever`: `10`

Artifacts regenerated by the refactor pass:

- `architecture/experiments/25-who-vaccines-and-apis/results/who_vaccines_apis_summary.json`
- `architecture/experiments/25-who-vaccines-and-apis/results/who_full_scale_results.json`
- `architecture/experiments/25-who-vaccines-and-apis/results/who_regression_control.json`
- `architecture/experiments/25-who-vaccines-and-apis/results/who_validation.json`
- `architecture/experiments/25-who-vaccines-and-apis/results/who_sample_records.json`
- `architecture/experiments/25-who-vaccines-and-apis/results/who_loader_design.json`

## Reusable Assets

- importable WHO source loaders for finished pharma, vaccines, APIs, and devices
- normalized finished-pharma typed rows via `WhoFinishedPharmaEntry`
- shared WHO table and device-catalog dataclasses
- MyChem cache-aware identity resolver and hit-classification helpers
- reusable schema, vaccine-identity, API-linkage, and metadata probe builders
- reusable validation, loader-design, regression-control, and full-scale-build
  payload builders
- compatibility shims preserving the old flat import names
- a package-install pattern and console-script pattern that downstream Python
  spikes can copy without inventing their own packaging layout
