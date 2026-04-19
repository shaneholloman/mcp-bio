# Harden

## Decomposition

Extracted the optimized implementation into the importable Python package:

- `architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/scripts/biomcp_news_spike/`

Package modules:

- `biomcp_news_spike/types.py`
  - `PipelineConfig`
  - `Bench`
  - `latency_summary`
- `biomcp_news_spike/io.py`
  - default experiment paths
  - JSON read/write helpers
- `biomcp_news_spike/pipeline.py`
  - RSS/headline discovery
  - article candidate construction
  - HTTP fetch and Trafilatura extraction
  - access/extraction classification
  - heuristic entity and identifier extraction
  - profile scoring and briefing card generation
  - BioMCP pivot selection and validation
  - result validation, stable projection, checksums, and regression comparison
- `biomcp_news_spike/cli.py`
  - argument parsing and stdout summary only
- `biomcp_news_spike/__init__.py`
  - stable public re-export surface for downstream imports

The old flat scripts now delegate to the package:

- `news_pipeline.py`: 18 lines, calls `biomcp_news_spike.cli.main`
- `news_discovery_probe.py`: 40 lines, wraps `discover_sources`
- `news_extract_articles.py`: 69 lines, wraps `extract_articles`
- `news_entity_briefing.py`: 77 lines, wraps `analyze_entities_and_briefing`
- `news_common.py`: 71-line compatibility shim over package helpers

Why this shape:

- Future `biomcp search news` and `biomcp get news` work needs direct access to discovery, extraction, entity, pivot, and validation logic without shelling out to a spike binary.
- Source-registry work needs the source matrix, access labels, extraction statuses, and JSON payload builders as direct imports.
- Personalized briefing work needs `extract_entities`, `profile_score`, and `analyze_entities_and_briefing` without copying the ranking heuristics.
- The CLI remains a convenience wrapper; the package owns the implementation.

## Public API

Stable import root:

- `biomcp_news_spike`

Shared types:

- `PipelineConfig`
  - registry/profile/output paths
  - replay baseline paths
  - workload limits
  - pivot limit
  - conservative ranking flag
- `Bench`
  - stage timing
  - HTTP latency samples
  - peak RSS snapshot

Primary functions:

- `run_pipeline(config: PipelineConfig) -> dict[str, Any]`
- `discover_sources(registry, bench, replay_discovery=None) -> dict[str, Any]`
- `source_matrix(source_results) -> list[dict[str, Any]]`
- `candidate_items_from_discovery(discovery, per_source) -> list[dict[str, Any]]`
- `extract_articles(discovery, registry, per_source, max_articles, bench) -> tuple[list[dict[str, Any]], dict[str, Any]]`
- `extract_entities(text) -> dict[str, Any]`
- `profile_score(title, text, entities, profile, conservative) -> tuple[int, list[str]]`
- `analyze_entities_and_briefing(...) -> tuple[list[dict[str, Any]], list[dict[str, Any]], list[dict[str, Any]], dict[str, Any]]`
- `choose_pivots(article_results, pivot_limit) -> list[dict[str, Any]]`
- `validate_results(payload) -> dict[str, Any]`
- `checksum_projection(payload) -> str`
- `compare_regression(current, baseline) -> dict[str, Any]`
- `build_recommendations(payload) -> dict[str, Any]`
- `load_json(path) -> dict[str, Any]`
- `json_dump(path, payload) -> None`

Usage examples:

```python
from pathlib import Path
from biomcp_news_spike import PipelineConfig, run_pipeline

payload = run_pipeline(
    PipelineConfig(
        label="daily-news-smoke",
        output=Path("results/news_smoke.json"),
        per_source=4,
        max_articles=20,
        max_entity_articles=10,
        conservative_ranking=True,
    )
)
print(payload["entity_summary"]["successful_pivots"])
print(payload["projection_checksum"])
```

```python
from biomcp_news_spike import Bench, DEFAULT_REGISTRY, discover_sources, load_json, source_matrix

registry = load_json(DEFAULT_REGISTRY)
bench = Bench()
discovery = discover_sources(registry, bench)
for row in source_matrix(discovery["sources"]):
    print(row["source_id"], row["mode"], row["feed_entries"])
```

```python
from biomcp_news_spike import extract_entities, profile_score

text = "KRAS G12C trial readout in pancreatic cancer"
entities = extract_entities(text)
score, reasons = profile_score(
    "KRAS readout",
    text,
    entities,
    {"keywords": {"KRAS": 5, "clinical trial": 2}, "interests": ["KRAS"]},
    conservative=True,
)
print(entities["genes"], score, reasons)
```

## Build System

This spike is Python, not Zig. There is no `build.zig` in the worktree. The harden equivalent is:

- `architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/scripts/pyproject.toml`

Package metadata:

- package name: `biomcp-news-spike`
- import root: `biomcp_news_spike`
- console script: `biomcp-news-pipeline = biomcp_news_spike.cli:main`
- dependencies:
  - `beautifulsoup4>=4.12`
  - `feedparser>=6.0`
  - `python-dateutil>=2.9`
  - `requests>=2.32`
  - `trafilatura>=2.0`

Downstream import options:

```bash
PYTHONPATH=architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/scripts \
  python3 your_downstream_spike.py
```

```bash
python3 -m pip install --no-deps --target /tmp/biomcp_news_spike_pkg_test \
  architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/scripts
```

Verified build/import checks:

- `uv run --with beautifulsoup4 --with feedparser --with python-dateutil --with requests --with trafilatura python -c "from biomcp_news_spike import PipelineConfig, extract_entities; ..."`
  - result: succeeded
  - observed `PipelineConfig().per_source == 12`
  - observed `extract_entities("KRAS trial")["genes"] == ["KRAS"]`
- `python3 -m pip install --no-deps --target /tmp/biomcp_news_spike_pkg_test architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/scripts`
  - result: succeeded
  - built and installed `biomcp-news-spike-0.1.0`

Generated build artifacts (`build/`, `*.egg-info`, `__pycache__`, uv virtualenv/lock artifacts) were removed after verification.

## Regression Check

Commands run after refactor:

```bash
uv run architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/scripts/news_pipeline.py \
  --label harden-full-scale-rerun \
  --output architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/results/harden_full_scale_rerun.json \
  --per-source 12 \
  --max-articles 60 \
  --max-entity-articles 30 \
  --pivot-limit 6 \
  --conservative-ranking
```

```bash
uv run architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/scripts/news_pipeline.py \
  --label harden-regression-control \
  --output architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/results/harden_regression_control.json \
  --replay-discovery architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/results/discovery_results.json \
  --baseline-discovery architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/results/discovery_results.json \
  --baseline-extraction architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/results/article_extraction_results.json \
  --baseline-entities architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/results/entity_briefing_results.json \
  --per-source 4 \
  --max-articles 20 \
  --max-entity-articles 10 \
  --pivot-limit 6 \
  --conservative-ranking
```

Regression control result:

- output: `architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/results/harden_regression_control.json`
- status: `pass`
- projection checksum: `c8692d6a1a1a68b143c84da1584d60c70e1e13452e2ad00e9efca1bd72affcf2`
- optimized final checksum: `c8692d6a1a1a68b143c84da1584d60c70e1e13452e2ad00e9efca1bd72affcf2`
- useful extractions: `8/20`
- useful sources: `2` (`biopharma-dive`, `stat`)
- entity articles analyzed: `10`
- entity-bearing articles: `9`
- BioMCP pivots: `5/5`
- validation: `0` mismatches, `1` known source-access warning
- total wall time: `4.0367 s`
- article extraction: `1.0147 s`
- entity/briefing: `3.0201 s`
- pivot validation: `2.9561 s`
- peak RSS: `115.01 MB`

Full-scale live result:

- output: `architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/results/harden_full_scale_rerun.json`
- useful extractions: `21/60`
- useful sources: `2` (`biopharma-dive`, `stat`)
- entity articles analyzed: `30`
- entity-bearing articles: `24`
- BioMCP pivots: `6/6`
- validation: `0` mismatches, `1` known source-access warning
- total wall time: `13.8826 s`
- discovery: `4.1502 s`
- article extraction: `5.3395 s`
- entity/briefing: `4.3924 s`
- pivot validation: `4.2149 s`
- HTTP latency: p50 `90 ms`, p95 `390 ms`
- peak RSS: `153.31 MB`

The replayed regression-control checksum and all correctness counters match the optimized final run exactly. The live full-scale correctness counters also match exactly: `21/60` useful extractions, `24/30` entity-bearing articles, `6/6` pivots, `0` mismatches, and `1` warning. Full-scale wall time varied with live HTTP latency; the rerun remained below the optimized pass-3 live envelope (`15.7292 s`) and preserved the optimized correctness contract.

Validation assertions run:

```bash
jq -e '.validation.mismatch_count == 0 and .validation.warning_count == 1 and .entity_summary.successful_pivots >= 5 and .extraction_summary.useful_extractions >= 8 and .regression_control.status == "pass"' \
  architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/results/harden_regression_control.json
```

```bash
jq -e '.validation.mismatch_count == 0 and .validation.warning_count == 1 and .entity_summary.successful_pivots == 6 and .extraction_summary.useful_extractions == 21 and .entity_summary.articles_analyzed == 30 and .entity_summary.articles_with_any_entity == 24' \
  architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/results/harden_full_scale_rerun.json
```

Both returned `true`.

## Reusable Assets

- `PipelineConfig` for downstream experiment and product command wiring.
- `Bench` for stage timing, HTTP latency, and peak RSS collection.
- Source registry loader and default path constants.
- RSS/headline discovery helpers:
  - `discover_sources`
  - `discover_feed_links`
  - `parse_feed`
  - `parse_headline_page`
  - `source_matrix`
- Article candidate and extraction helpers:
  - `candidate_items_from_discovery`
  - `article_id`
  - `extract_articles`
  - `extract_one`
  - `fetch_url`
  - `simple_html_text`
  - `paywall_signals`
  - `access_label`
  - `extraction_status`
  - `extraction_quality_score`
- Entity, pivot, and briefing helpers:
  - `extract_entities`
  - `profile_score`
  - `choose_pivots`
  - `analyze_entities_and_briefing`
  - `run_biomcp`
- Validation/regression helpers:
  - `validate_results`
  - `stable_projection`
  - `checksum_projection`
  - `baseline_metrics`
  - `compare_regression`
  - `build_recommendations`
- Thin wrapper scripts preserving the old experiment command names while removing duplicate logic.
- A package/dependency pattern that downstream Python spikes can import directly or install as a local path dependency.
