#!/usr/bin/env -S uv run
# /// script
# dependencies = [
#   "beautifulsoup4>=4.12",
#   "python-dateutil>=2.9",
#   "requests>=2.32",
#   "trafilatura>=2.0",
# ]
# ///

from __future__ import annotations

import argparse
import json
from pathlib import Path

import requests
import trafilatura

from news_common import (
    access_label,
    candidate_items_from_discovery,
    extraction_quality_score,
    fetch_url,
    json_dump,
    paywall_signals,
    simple_html_text,
    utc_now_iso,
)


def extract_one(session: requests.Session, item: dict, index: int) -> dict:
    fetch, html = fetch_url(session, item["url"])
    extracted = ""
    if html:
        extracted = trafilatura.extract(
            html,
            url=fetch.get("final_url") or item["url"],
            include_comments=False,
            include_tables=False,
            favor_precision=True,
        ) or ""
    fallback = simple_html_text(html) if html else ""
    signals = paywall_signals(html[:20000], extracted[:4000], fallback[:4000])
    text_len = len(extracted)
    fallback_len = len(fallback)
    quality = extraction_quality_score(fetch.get("status"), text_len, fallback_len, signals)
    return {
        "index": index,
        "source_id": item["source_id"],
        "source_name": item.get("source_name"),
        "title": item.get("title"),
        "url": item["url"],
        "published": item.get("published"),
        "discovery_mode": item.get("discovery_mode"),
        "fetch": fetch,
        "trafilatura_text_chars": text_len,
        "fallback_text_chars": fallback_len,
        "paywall_signals": signals,
        "access_label": access_label(fetch.get("status"), text_len, signals),
        "quality_score_0_5": quality,
        "has_useful_text": quality >= 3 and text_len >= 800,
    }


def run(discovery_path: str, output: str, max_articles: int) -> None:
    discovery = json.loads(Path(discovery_path).read_text(encoding="utf-8"))
    candidates = candidate_items_from_discovery(discovery, per_source=4)
    session = requests.Session()
    results = []
    for index, item in enumerate(candidates[:max_articles], start=1):
        results.append(extract_one(session, item, index))

    useful = [r for r in results if r["has_useful_text"]]
    useful_sources = sorted({r["source_id"] for r in useful})
    json_dump(
        output,
        {
            "generated_at": utc_now_iso(),
            "approach": "http_fetch_plus_trafilatura_extraction",
            "input_discovery_results": discovery_path,
            "max_articles": max_articles,
            "summary": {
                "attempted_articles": len(results),
                "useful_extractions": len(useful),
                "useful_sources": useful_sources,
                "useful_source_count": len(useful_sources),
                "mean_quality_score": round(
                    sum(r["quality_score_0_5"] for r in results) / max(len(results), 1), 2
                ),
            },
            "articles": results,
        },
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--discovery",
        default=(
            "architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/"
            "results/discovery_results.json"
        ),
    )
    parser.add_argument(
        "--output",
        default=(
            "architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/"
            "results/article_extraction_results.json"
        ),
    )
    parser.add_argument("--max-articles", type=int, default=18)
    args = parser.parse_args()
    run(args.discovery, args.output, args.max_articles)


if __name__ == "__main__":
    main()
