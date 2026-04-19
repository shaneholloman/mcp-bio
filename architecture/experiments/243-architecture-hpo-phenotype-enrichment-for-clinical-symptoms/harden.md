# Harden: HPO Phenotype Enrichment for Clinical Symptoms

## Decomposition

The optimized implementation is now split into a reusable library surface and a
thin CLI/report wrapper.

Extracted library code:
- `scripts/clinical_features_spike/api.py` contains the downstream import
  surface for one-disease extraction, batch extraction, HPO baseline loading,
  and aggregate summary metrics.
- `scripts/clinical_features_spike/types.py` contains shared `TypedDict` row
  shapes for topics, selected-topic payloads, clinical features, and per-disease
  output.
- `scripts/clinical_features_spike/extraction.py` remains the reusable
  algorithm module for topic scoring, direct-page selection, feature extraction,
  coverage metrics, and evidence checks.
- `scripts/clinical_features_spike/medlineplus.py` remains the reusable source
  reader/cache module for MedlinePlus topic search and fixture fallback.
- `scripts/clinical_features_spike/hpo_mapping.py` remains the reviewed HPO
  mapping fixture for spike concepts.

Thin wrapper code:
- `scripts/run_exploit.py` is a 10-line executable wrapper.
- `scripts/clinical_features_spike/cli.py` is a 34-line argument parser that
  calls `reports.write_all_results`.
- `scripts/clinical_features_spike/reports.py` owns artifact generation,
  validation/report payloads, and the HPO live-control subprocess. Downstream
  consumers should import `clinical_features_spike.api`, not `reports.py`, when
  they need library behavior without CLI/report side effects.

The package default import now exposes library functions and shared row types
instead of making report writing the default package surface.

## Public API

Primary import surface:

```python
from clinical_features_spike import (
    all_diseases,
    extract_clinical_feature_dataset,
    extract_disease_clinical_features,
    load_hpo_rows_by_disease,
    summarize_clinical_feature_dataset,
)
```

Available modules and calls:
- `clinical_features_spike.api.load_hpo_rows_by_disease()` loads the current
  curated HPO baseline rows keyed by disease.
- `clinical_features_spike.api.extract_disease_clinical_features(disease,
  hpo_rows=None, *, allow_live=True, refresh_cache=False)` returns one
  `DiseaseClinicalFeatures` row with selected MedlinePlus topics,
  source-native `clinical_features`, retained HPO phenotype rows, coverage
  metrics, and a feature checksum.
- `clinical_features_spike.api.extract_clinical_feature_dataset(diseases=None,
  hpo_rows_by_disease=None, *, allow_live=True, refresh_cache=False)` returns a
  list of per-disease rows for the full fixture or caller-supplied disease
  rows.
- `clinical_features_spike.api.summarize_clinical_feature_dataset(rows)`
  computes aggregate counts, recall, mismatch count, selected-topic counts, and
  the stable output checksum without writing artifacts or shelling out.
- `clinical_features_spike.extraction.select_topics(disease, topics)` exposes
  direct-page selection and related-topic fallback.
- `clinical_features_spike.extraction.extract_features(disease,
  selected_topics)` exposes source-native feature extraction.
- `clinical_features_spike.hpo_mapping.map_feature(label)` exposes the reviewed
  fixture HPO mapping.
- `clinical_features_spike.medlineplus.load_topics_for_disease(...)` exposes
  MedlinePlus source acquisition/cache metadata.

Shared row types:
- `ClinicalFeature`
- `DiseaseClinicalFeatures`
- `TopicRow`
- `TopicSelection`

One-disease example:

```python
from clinical_features_spike import (
    all_diseases,
    extract_disease_clinical_features,
    load_hpo_rows_by_disease,
)

hpo_rows = load_hpo_rows_by_disease()
disease = next(row for row in all_diseases() if row["key"] == "uterine_fibroid")

result = extract_disease_clinical_features(
    disease,
    hpo_rows=hpo_rows[disease["key"]],
    allow_live=False,
)

clinical_features = result["clinical_features"]
```

Batch example:

```python
from clinical_features_spike import (
    extract_clinical_feature_dataset,
    summarize_clinical_feature_dataset,
)

rows = extract_clinical_feature_dataset(allow_live=False)
summary = summarize_clinical_feature_dataset(rows)
```

## Build System

This BioMCP worktree has `Cargo.toml` and `pyproject.toml`; it has no
`build.zig`. The generic March harden instruction to update `build.zig` is not
applicable to this ticket.

The spike implementation is a Python package under:

```text
architecture/experiments/243-architecture-hpo-phenotype-enrichment-for-clinical-symptoms/scripts/clinical_features_spike
```

Downstream spike scripts can depend on it by adding the experiment `scripts`
directory to `PYTHONPATH`:

```bash
export PYTHONPATH="$REPO/architecture/experiments/243-architecture-hpo-phenotype-enrichment-for-clinical-symptoms/scripts:$PYTHONPATH"
```

Or from an in-repo Python script:

```python
import sys

sys.path.insert(
    0,
    "architecture/experiments/243-architecture-hpo-phenotype-enrichment-for-clinical-symptoms/scripts",
)

from clinical_features_spike import extract_clinical_feature_dataset
```

Production Rust integration should port the library contract into the BioMCP
disease module rather than shelling out to `run_exploit.py`. The importable
Python API is the spike handoff surface for downstream architecture work.

## Regression Check

Full offline benchmark/regression command:

```bash
python3 architecture/experiments/243-architecture-hpo-phenotype-enrichment-for-clinical-symptoms/scripts/run_exploit.py --offline
```

Regenerated artifact summary:

| Metric | Result |
|---|---:|
| Diseases | 3 |
| Clinical features | 15 |
| HPO-mapped clinical features | 15 |
| Expected symptom recall | 0.652 |
| Mismatch count | 8 |
| Selected MedlinePlus topics | 5 |
| Topic noise reduction | 7 |
| Fixture extraction elapsed | 3.6 ms |
| Features per second | 4166.667 |
| Peak RSS | 25736 KB |
| Output checksum | `f08c35ff31306ff4696bd953eaba4b00aeed9e6746a1228469e1479238e3d34f` |
| Regression passed | true |
| Validation passed | true |

The refactor preserved the optimized cold benchmark contract exactly for
elapsed time, feature throughput, recall, mismatch count, feature count, and
checksum. Peak RSS remained within the existing exploit tolerance.

Focused validation command:

```bash
pytest -q architecture/experiments/243-architecture-hpo-phenotype-enrichment-for-clinical-symptoms/scripts/test_clinical_features_spike.py
```

Result:

```text
4 passed, 1 existing pytest config warning in 2.79s
```

Compile/import smoke checks:

```bash
python3 -m py_compile architecture/experiments/243-architecture-hpo-phenotype-enrichment-for-clinical-symptoms/scripts/clinical_features_spike/*.py architecture/experiments/243-architecture-hpo-phenotype-enrichment-for-clinical-symptoms/scripts/run_exploit.py
```

```text
passed
```

Public API smoke:

```text
15 0.652 8 f08c35ff31306ff4696bd953eaba4b00aeed9e6746a1228469e1479238e3d34f
```

## Reusable Assets

Downstream spikes inherit:
- Source-native `clinical_features` row schema with rank, label, feature type,
  source URL, source-native ID, evidence tier/text, body system, topic
  selection metadata, and optional HPO mapping metadata.
- Shared `TypedDict` row definitions in `types.py`.
- MedlinePlus topic reader/cache/fallback logic with `allow_live` and
  `refresh_cache` controls.
- Direct disease-page selection and related-topic fallback scoring.
- Feature extraction with evidence snippets and reviewed extra extraction
  synonyms.
- Confidence-scored HPO mapping fixture for the spike symptom concepts.
- HPO baseline reader for preserving current curated phenotype rows.
- Coverage and checksum helpers for regression controls.
- Report/validation artifact builder for architecture evidence, kept separate
  from the importable library API.
