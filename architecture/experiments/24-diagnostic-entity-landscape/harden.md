# Harden

## Decomposition

- Extracted reusable code into the importable Python package
  `architecture/experiments/24-diagnostic-entity-landscape/scripts/diagnostic_landscape/`.
- Kept the old flat modules (`common.py`, `diagnostic_landscape_lib.py`) only as
  compatibility shims that re-export the package.
- Moved source and report logic out of the wrappers:
  - `diagnostic_landscape/io.py`
    - experiment paths, JSON/result IO, downloads, HTTP helpers, matching helpers
  - `diagnostic_landscape/types.py`
    - shared `SourceBundle` and `FullScaleLandscape` dataclasses
  - `diagnostic_landscape/probes.py`
    - GTR bulk/API, WHO IVD, FDA device, cross-source, and live-latency probe builders
  - `diagnostic_landscape/landscape.py`
    - ClinVar loaders, normalized GTR/WHO/FDA source builders, gene-source matrix,
      validation payload, full-scale landscape builder, and regression-control builder
  - `diagnostic_landscape/design.py`
    - unified data model, CLI proposal, source priority, and downstream Rust boundary notes
- Thinned all CLI wrappers under `scripts/` to import-and-run entrypoints only.
  Every wrapper is now under 40 lines; the largest is `run_all_probes.py` at 37 lines.

Why this shape:

- Future `diagnostic` entity implementation work needs importable normalized
  source bundles, not shell calls into one-off scripts.
- WHO issue `#235` response work needs a direct Python API for the final
  landscape and regression artifacts.
- The package split keeps the dependency direction correct: wrappers import the
  library, and the library does not import the wrappers.

## Public API

Stable import root:

- `diagnostic_landscape`

Shared types:

- `SourceBundle`
  - normalized records
  - `gene_to_records`
  - optional `disease_to_records`
  - source metrics
  - file provenance
- `FullScaleLandscape`
  - ClinVar universe
  - normalized GTR/WHO/FDA bundles
  - gene-source matrix
  - validation payload
  - final landscape payload
  - artifact paths

Primary functions:

- `load_clinvar_gene_summary(refresh: bool = False) -> dict[str, Any]`
- `load_clinvar_variant_sanity(refresh: bool = False) -> dict[str, Any]`
- `load_gtr_backbone(gene_universe: set[str] | None = None, refresh: bool = False) -> SourceBundle`
- `load_who_overlay(gene_universe: set[str], refresh: bool = False) -> SourceBundle`
- `load_fda_molecular_slice(gene_universe: set[str]) -> SourceBundle`
- `build_gene_source_matrix(...) -> dict[str, Any]`
- `build_full_scale_landscape(refresh: bool = False) -> FullScaleLandscape`
- `write_full_scale_results(landscape: FullScaleLandscape) -> FullScaleLandscape`
- `build_validation_payload(gtr_records, fda_records) -> dict[str, Any]`
- `build_gtr_bulk_probe_payload() -> dict[str, Any]`
- `build_gtr_api_probe_payload() -> dict[str, Any]`
- `build_who_ivd_probe_payload() -> dict[str, Any]`
- `build_fda_device_probe_payload() -> dict[str, Any]`
- `build_cross_source_matrix_payload(...) -> dict[str, Any]`
- `build_regression_control_payload() -> dict[str, Any]`
- `build_live_latency_noise_probe_payload() -> dict[str, Any]`
- `write_result(filename: str, payload: dict[str, Any]) -> Path`

Usage examples:

```python
from diagnostic_landscape import build_full_scale_landscape

landscape = build_full_scale_landscape()
print(landscape.gtr.gene_to_records["BRCA1"])
print(landscape.gene_source_matrix["coverage_summary"])
```

```python
from diagnostic_landscape import load_gtr_backbone

bundle = load_gtr_backbone(gene_universe={"BRCA1", "EGFR", "BRAF"})
print(bundle.metrics["schema_completeness"]["gene_links_pct"])
print(sorted(bundle.gene_to_records["BRCA1"])[:5])
```

```python
from diagnostic_landscape import build_regression_control_payload

report = build_regression_control_payload()
assert report["comparisons"]["gtr_bulk"]["mismatch_count"] == 0
```

## Build System

There is no `build.zig` in this spike because the ticket explicitly constrains
the implementation to Python. The equivalent build/dependency work is:

- added `architecture/experiments/24-diagnostic-entity-landscape/scripts/pyproject.toml`
- package name: `diagnostic-landscape-spike`
- import root: `diagnostic_landscape`

Downstream options:

1. Direct path import during a spike:

```bash
PYTHONPATH=architecture/experiments/24-diagnostic-entity-landscape/scripts python3 your_script.py
```

2. Local path dependency install:

```bash
python3 -m pip install --target /tmp/diagnostic_landscape_pkg_test \
  architecture/experiments/24-diagnostic-entity-landscape/scripts
```

Verified build/dependency result:

- `python3 -m pip install --no-deps --target /tmp/diagnostic_landscape_pkg_test architecture/experiments/24-diagnostic-entity-landscape/scripts`
- result: succeeded, built and installed `diagnostic-landscape-spike-0.1.0`

## Regression Check

Runtime pass performed after refactor:

- `python3 architecture/experiments/24-diagnostic-entity-landscape/scripts/build_full_scale_landscape.py`
- `python3 architecture/experiments/24-diagnostic-entity-landscape/scripts/regression_control.py`
- `python3 architecture/experiments/24-diagnostic-entity-landscape/scripts/live_latency_noise_probe.py`

Measured outputs:

- full-scale coverage held exactly at the optimized contract:
  - combined: `8389 / 11085` (`75.68%`)
  - GTR: `8389`
  - FDA: `22`
  - WHO: `4`
- deterministic regression-control checks stayed exact for:
  - `gtr_bulk`
  - `who_ivd`
  - `fda_device`
  - `cross_source_matrix`
- `gtr_api` still matched on exact counts and projection checksum, and still
  failed only the known live-latency waiver paths
- validation rerun preserved the anchor records:
  - GTR: `myChoice`, `FoundationOne`, `Tempus xT`
  - FDA: `MyChoice`, `FoundationOne`, `Tempus xT`

Artifacts regenerated:

- `architecture/experiments/24-diagnostic-entity-landscape/results/diagnostic_full_scale_landscape.json`
- `architecture/experiments/24-diagnostic-entity-landscape/results/diagnostic_regression_control.json`
- `architecture/experiments/24-diagnostic-entity-landscape/results/diagnostic_validation.json`
- `architecture/experiments/24-diagnostic-entity-landscape/results/diagnostic_live_latency_noise_probe.json`

## Reusable Assets

- `SourceBundle` and `FullScaleLandscape` shared types
- ClinVar universe loaders
- normalized GTR backbone loader with gene and disease maps
- normalized WHO IVD overlay loader
- normalized FDA molecular slice loader
- gene-source coverage matrix builder
- validation payload builder for anchor diagnostic records
- deterministic regression-control builder
- live-latency waiver probe builder
- path/package dependency pattern for downstream Python spikes
