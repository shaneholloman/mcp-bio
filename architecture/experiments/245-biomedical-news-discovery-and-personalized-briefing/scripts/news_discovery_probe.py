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

from biomcp_news_spike import Bench, DEFAULT_REGISTRY, discover_sources, json_dump, load_json, utc_now_iso

DEFAULT_OUTPUT = (
    "architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/"
    "results/discovery_results.json"
)


def run(output: str) -> None:
    bench = Bench()
    payload = discover_sources(load_json(DEFAULT_REGISTRY), bench)
    payload["generated_at"] = utc_now_iso()
    payload["metrics"] = bench.snapshot()
    json_dump(output, payload)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", default=DEFAULT_OUTPUT)
    args = parser.parse_args()
    run(args.output)


if __name__ == "__main__":
    main()
