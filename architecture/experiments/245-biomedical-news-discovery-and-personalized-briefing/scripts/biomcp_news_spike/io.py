from __future__ import annotations

import json
from pathlib import Path
from typing import Any

DEFAULT_EXPERIMENT_DIR = Path(
    "architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing"
)
DEFAULT_REGISTRY = DEFAULT_EXPERIMENT_DIR / "source_registry.json"
DEFAULT_PROFILE = DEFAULT_EXPERIMENT_DIR / "profile_oncology_kras_melanoma.json"
DEFAULT_OUTPUT = DEFAULT_EXPERIMENT_DIR / "results/news_pipeline_results.json"


def json_dump(path: str | Path, payload: dict[str, Any]) -> None:
    out = Path(path)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def load_json(path: str | Path) -> dict[str, Any]:
    return json.loads(Path(path).read_text(encoding="utf-8"))
