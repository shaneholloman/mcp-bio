#!/usr/bin/env -S uv run
# /// script
# dependencies = [
#   "beautifulsoup4>=4.12",
#   "feedparser>=6.0",
#   "python-dateutil>=2.9",
#   "requests>=2.32",
# ]
# ///

from __future__ import annotations

import argparse
import re
from datetime import datetime
from typing import Any

import feedparser
import requests
from bs4 import BeautifulSoup

from news_common import (
    SOURCES,
    age_hours,
    clean_ws,
    fetch_url,
    iso_or_none,
    json_dump,
    normalize_link,
    parse_dt,
    same_site,
    utc_now,
    utc_now_iso,
)

NAV_RE = re.compile(
    r"^(home|news|events|about|advertise|subscribe|login|sign in|search|privacy|terms|newsletter)$",
    re.I,
)
DATE_RE = re.compile(
    r"\b(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Sept|Oct|Nov|Dec)[a-z]*\.?\s+\d{1,2},\s+\d{4}\b"
)


def discover_feed_links(page_url: str, html: str) -> list[str]:
    soup = BeautifulSoup(html, "html.parser")
    links: list[str] = []
    for tag in soup.find_all("link", href=True):
        rel = " ".join(tag.get("rel", [])).lower()
        typ = str(tag.get("type", "")).lower()
        if "alternate" not in rel:
            continue
        if not any(token in typ for token in ("rss", "atom", "xml")):
            continue
        link = normalize_link(tag["href"], page_url)
        if "comments/feed" in link or "oembed" in link:
            continue
        links.append(link)
    return sorted(set(links))


def feed_item_date(entry: Any) -> datetime | None:
    for key in ("published", "updated", "created"):
        value = entry.get(key)
        if value:
            return parse_dt(value)
    return None


def parse_feed(url: str, fetch_meta: dict[str, Any], text: str, now: datetime) -> dict[str, Any]:
    parsed = feedparser.parse(text)
    entries = parsed.entries or []
    item_dates = [feed_item_date(entry) for entry in entries]
    item_dates = [dt for dt in item_dates if dt is not None]
    latest = max(item_dates) if item_dates else None

    field_counts = {
        "title": sum(1 for entry in entries if clean_ws(entry.get("title"))),
        "link": sum(1 for entry in entries if clean_ws(entry.get("link"))),
        "published_or_updated": sum(1 for entry in entries if feed_item_date(entry)),
        "summary": sum(1 for entry in entries if clean_ws(entry.get("summary"))),
    }

    items = []
    for entry in entries[:20]:
        dt = feed_item_date(entry)
        items.append(
            {
                "title": clean_ws(entry.get("title")),
                "link": clean_ws(entry.get("link")),
                "published": iso_or_none(dt),
                "summary_chars": len(clean_ws(entry.get("summary"))),
            }
        )

    return {
        "url": url,
        "fetch": fetch_meta,
        "feed_title": clean_ws(parsed.feed.get("title")),
        "bozo": bool(parsed.bozo),
        "bozo_exception": str(parsed.bozo_exception)[:240] if getattr(parsed, "bozo", False) else None,
        "entries": len(entries),
        "latest_published": iso_or_none(latest),
        "latest_age_hours": age_hours(latest, now),
        "field_counts": field_counts,
        "items": items,
    }


def nearest_date_text(node: Any) -> str | None:
    current = node
    for _ in range(4):
        if current is None:
            return None
        time_tag = current.find("time") if hasattr(current, "find") else None
        if time_tag:
            return clean_ws(time_tag.get("datetime") or time_tag.get_text(" ", strip=True))
        text = clean_ws(current.get_text(" ", strip=True)) if hasattr(current, "get_text") else ""
        match = DATE_RE.search(text)
        if match:
            return match.group(0)
        current = getattr(current, "parent", None)
    return None


def link_score(a: Any, url: str, base_url: str) -> int:
    text = clean_ws(a.get_text(" ", strip=True))
    path = url.lower()
    score = 0
    if same_site(url, base_url):
        score += 2
    if a.find_parent("article"):
        score += 2
    if any(token in path for token in ("/news/", "/article/", "/story/", "/20")):
        score += 2
    if len(text) >= 45:
        score += 1
    if NAV_RE.match(text):
        score -= 5
    if any(
        token in path
        for token in ("newsletter", "podcast", "events", "jobs", "sponsor", "topic-hub", "/tag/")
    ):
        score -= 2
    return score


def parse_headline_page(source: dict[str, Any], fetch_meta: dict[str, Any], html: str) -> dict[str, Any]:
    soup = BeautifulSoup(html, "html.parser")
    raw_count = 0
    by_url: dict[str, dict[str, Any]] = {}
    for a in soup.find_all("a", href=True):
        title = clean_ws(a.get_text(" ", strip=True))
        if len(title) < 18 or len(title) > 220 or NAV_RE.match(title):
            continue
        url = normalize_link(a["href"], fetch_meta.get("final_url") or source["page_url"])
        if not url.startswith("http") or not same_site(url, source["page_url"]):
            continue
        raw_count += 1
        score = link_score(a, url, source["page_url"])
        if score < 1:
            continue
        date_text = nearest_date_text(a)
        candidate = {
            "title": title,
            "url": url,
            "date": iso_or_none(parse_dt(date_text)) if date_text else None,
            "date_text": date_text,
            "score": score,
        }
        existing = by_url.get(url)
        if existing is None or candidate["score"] > existing["score"]:
            by_url[url] = candidate

    items = sorted(by_url.values(), key=lambda item: item["score"], reverse=True)[:25]
    dates = [parse_dt(item.get("date")) for item in items]
    dates = [dt for dt in dates if dt is not None]
    latest = max(dates) if dates else None
    return {
        "page_url": source["page_url"],
        "fetch": fetch_meta,
        "raw_candidate_links": raw_count,
        "deduped_candidate_links": len(by_url),
        "items": items,
        "latest_date": iso_or_none(latest),
        "latest_age_hours": age_hours(latest),
        "date_fields": sum(1 for item in items if item.get("date")),
    }


def choose_best_mode(feeds: list[dict[str, Any]], page: dict[str, Any]) -> str:
    best_entries = max((feed.get("entries", 0) for feed in feeds), default=0)
    best_age = min(
        (feed["latest_age_hours"] for feed in feeds if feed.get("latest_age_hours") is not None),
        default=None,
    )
    page_count = len(page.get("items", []))
    if best_entries >= 10 and (best_age is None or best_age <= 168):
        return "rss"
    if page_count >= 8:
        return "headline_page"
    if best_entries > 0:
        return "rss_low_freshness_or_sparse"
    if page_count > 0:
        return "headline_page_sparse"
    return "blocked_or_no_discovery"


def feed_rank(feed: dict[str, Any]) -> tuple[int, int, float]:
    url = feed.get("url") or ""
    penalty = 0
    if "comments/feed" in url or "oembed" in url:
        penalty -= 10
    link_count = feed.get("field_counts", {}).get("link", 0)
    age = feed.get("latest_age_hours")
    freshness = -float(age) if age is not None else -99999.0
    return (feed.get("entries", 0) + penalty, link_count, freshness)


def run(output: str) -> None:
    now = utc_now()
    session = requests.Session()
    results: list[dict[str, Any]] = []

    for source in SOURCES:
        page_fetch, page_html = fetch_url(session, source["page_url"])
        autodiscovered = discover_feed_links(source["page_url"], page_html) if page_html else []
        candidates = sorted(set(source["rss_candidates"] + autodiscovered))

        feeds = []
        for url in candidates:
            feed_fetch, feed_text = fetch_url(session, url)
            feeds.append(parse_feed(url, feed_fetch, feed_text, now))

        best_feed = max(feeds, key=feed_rank, default={})
        page = parse_headline_page(source, page_fetch, page_html) if page_html else {
            "page_url": source["page_url"],
            "fetch": page_fetch,
            "raw_candidate_links": 0,
            "deduped_candidate_links": 0,
            "items": [],
            "latest_date": None,
            "latest_age_hours": None,
            "date_fields": 0,
        }

        results.append(
            {
                "source_id": source["id"],
                "source_name": source["name"],
                "page_url": source["page_url"],
                "autodiscovered_feed_urls": autodiscovered,
                "rss_candidates_tested": candidates,
                "rss_results": feeds,
                "rss_best": best_feed,
                "headline_page": page,
                "recommended_discovery_mode": choose_best_mode(feeds, page),
            }
        )

    matrix = []
    for result in results:
        best_feed = result.get("rss_best") or {}
        page = result.get("headline_page") or {}
        matrix.append(
            {
                "source_id": result["source_id"],
                "mode": result["recommended_discovery_mode"],
                "best_feed_url": best_feed.get("url"),
                "feed_entries": best_feed.get("entries", 0),
                "feed_latest_age_hours": best_feed.get("latest_age_hours"),
                "headline_items": len(page.get("items", [])),
                "headline_date_fields": page.get("date_fields", 0),
                "page_status": page.get("fetch", {}).get("status"),
            }
        )

    json_dump(
        output,
        {
            "generated_at": utc_now_iso(),
            "approach": "rss_feed_first_vs_headline_page_discovery",
            "source_matrix": matrix,
            "sources": results,
        },
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--output",
        default=(
            "architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/"
            "results/discovery_results.json"
        ),
    )
    args = parser.parse_args()
    run(args.output)


if __name__ == "__main__":
    main()
