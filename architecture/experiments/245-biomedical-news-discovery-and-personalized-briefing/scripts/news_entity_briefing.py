#!/usr/bin/env -S uv run
# /// script
# dependencies = [
#   "beautifulsoup4>=4.12",
#   "feedparser>=6.0",
#   "python-dateutil>=2.9",
#   "requests>=2.32",
#   "trafilatura>=2.0",
# ]
# ///

from __future__ import annotations

import argparse

from biomcp_news_spike import (
    Bench,
    DEFAULT_PROFILE,
    DEFAULT_REGISTRY,
    analyze_entities_and_briefing,
    json_dump,
    load_json,
    utc_now_iso,
)

DEFAULT_EXTRACTION = (
    "architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/"
    "results/article_extraction_results.json"
)
DEFAULT_OUTPUT = (
    "architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/"
    "results/entity_briefing_results.json"
)


def run(extraction_path: str, output: str, max_articles: int) -> None:
    bench = Bench()
    extraction = load_json(extraction_path)
    articles, pivots, briefing, summary = analyze_entities_and_briefing(
        extraction.get("articles", []),
        load_json(DEFAULT_REGISTRY),
        load_json(DEFAULT_PROFILE),
        max_entity_articles=max_articles,
        pivot_limit=6,
        conservative_ranking=False,
        bench=bench,
    )
    json_dump(
        output,
        {
            "generated_at": utc_now_iso(),
            "approach": "heuristic_entities_biomcp_pivots_keyword_profile_briefing",
            "input_extraction_results": extraction_path,
            "profile": {
                "role": "oncologist",
                "interests": ["immunotherapy", "KRAS", "melanoma"],
            },
            "summary": summary,
            "articles": articles,
            "cross_reference_pivots": pivots,
            "personalized_briefing": briefing,
            "metrics": bench.snapshot(),
        },
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--extraction", default=DEFAULT_EXTRACTION)
    parser.add_argument("--output", default=DEFAULT_OUTPUT)
    parser.add_argument("--max-articles", type=int, default=10)
    args = parser.parse_args()
    run(args.extraction, args.output, args.max_articles)


if __name__ == "__main__":
    main()
