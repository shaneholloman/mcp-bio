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
    DEFAULT_REGISTRY,
    extract_articles,
    json_dump,
    load_json,
    public_article_records,
    utc_now_iso,
)

DEFAULT_DISCOVERY = (
    "architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/"
    "results/discovery_results.json"
)
DEFAULT_OUTPUT = (
    "architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/"
    "results/article_extraction_results.json"
)


def run(discovery_path: str, output: str, max_articles: int) -> None:
    bench = Bench()
    articles, summary = extract_articles(
        load_json(discovery_path),
        load_json(DEFAULT_REGISTRY),
        per_source=4,
        max_articles=max_articles,
        bench=bench,
    )
    json_dump(
        output,
        {
            "generated_at": utc_now_iso(),
            "approach": "http_fetch_plus_trafilatura_extraction",
            "input_discovery_results": discovery_path,
            "max_articles": max_articles,
            "summary": summary,
            "articles": public_article_records(articles),
            "metrics": bench.snapshot(),
        },
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--discovery", default=DEFAULT_DISCOVERY)
    parser.add_argument("--output", default=DEFAULT_OUTPUT)
    parser.add_argument("--max-articles", type=int, default=24)
    args = parser.parse_args()
    run(args.discovery, args.output, args.max_articles)


if __name__ == "__main__":
    main()
