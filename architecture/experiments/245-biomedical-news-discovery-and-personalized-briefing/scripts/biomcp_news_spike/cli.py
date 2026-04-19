from __future__ import annotations

import argparse
import json
from pathlib import Path

from .io import DEFAULT_OUTPUT, DEFAULT_PROFILE, DEFAULT_REGISTRY
from .pipeline import run_pipeline
from .types import PipelineConfig


def parse_args() -> PipelineConfig:
    parser = argparse.ArgumentParser(description="Run the BioMCP biomedical news spike pipeline.")
    parser.add_argument("--registry", type=Path, default=DEFAULT_REGISTRY)
    parser.add_argument("--profile", type=Path, default=DEFAULT_PROFILE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--label", default="full-scale")
    parser.add_argument("--replay-discovery", type=Path)
    parser.add_argument("--baseline-discovery", type=Path)
    parser.add_argument("--baseline-extraction", type=Path)
    parser.add_argument("--baseline-entities", type=Path)
    parser.add_argument("--per-source", type=int, default=12)
    parser.add_argument("--max-articles", type=int, default=60)
    parser.add_argument("--max-entity-articles", type=int, default=30)
    parser.add_argument("--pivot-limit", type=int, default=6)
    parser.add_argument("--conservative-ranking", action="store_true")
    args = parser.parse_args()
    return PipelineConfig(
        registry=args.registry,
        profile=args.profile,
        output=args.output,
        label=args.label,
        replay_discovery=args.replay_discovery,
        baseline_discovery=args.baseline_discovery,
        baseline_extraction=args.baseline_extraction,
        baseline_entities=args.baseline_entities,
        per_source=args.per_source,
        max_articles=args.max_articles,
        max_entity_articles=args.max_entity_articles,
        pivot_limit=args.pivot_limit,
        conservative_ranking=args.conservative_ranking,
    )


def main() -> None:
    config = parse_args()
    payload = run_pipeline(config)
    print(
        json.dumps(
            {
                "output": str(config.output),
                "label": config.label,
                "validation_mismatch_count": payload["validation"]["mismatch_count"],
                "validation_warning_count": payload["validation"]["warning_count"],
                "successful_pivots": payload["entity_summary"]["successful_pivots"],
                "useful_extractions": payload["extraction_summary"]["useful_extractions"],
                "projection_checksum": payload["projection_checksum"],
                "regression_status": payload["regression_control"]["status"],
            },
            sort_keys=True,
        )
    )
