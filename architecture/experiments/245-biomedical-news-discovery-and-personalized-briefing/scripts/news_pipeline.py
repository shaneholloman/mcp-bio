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
import hashlib
import json
import math
import re
import resource
import subprocess
import time
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path
from typing import Any
from urllib.parse import urljoin, urlparse

import feedparser
import requests
import trafilatura
from bs4 import BeautifulSoup

DEFAULT_EXPERIMENT_DIR = Path(
    "architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing"
)
DEFAULT_REGISTRY = DEFAULT_EXPERIMENT_DIR / "source_registry.json"
DEFAULT_PROFILE = DEFAULT_EXPERIMENT_DIR / "profile_oncology_kras_melanoma.json"
DEFAULT_OUTPUT = DEFAULT_EXPERIMENT_DIR / "results/news_pipeline_results.json"

NAV_RE = re.compile(
    r"^(home|news|events|about|advertise|subscribe|login|sign in|search|privacy|terms|newsletter)$",
    re.I,
)
DATE_RE = re.compile(
    r"\b(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Sept|Oct|Nov|Dec)[a-z]*\.?\s+\d{1,2},\s+\d{4}\b"
)
DOI_RE = re.compile(r"\b10\.\d{4,9}/[-._;()/:A-Z0-9]+\b", re.I)
PMID_RE = re.compile(r"\bPMID[:\s]*(\d{6,9})\b", re.I)
NCT_RE = re.compile(r"\bNCT\d{8}\b", re.I)
PHASE_RE = re.compile(r"\bphase\s+(?:1/2|2/3|I/II|II/III|I|II|III|IV|[1234])\b", re.I)

PAYWALL_PATTERNS = [
    "subscribe to continue",
    "subscribe now",
    "subscriber-only",
    "already a subscriber",
    "sign in to continue",
    "log in to continue",
    "login to continue",
    "register to continue",
    "create a free account",
    "free account",
    "metered access",
    "premium content",
    "unlimited access",
    "remaining free articles",
    "institutional access",
]

GENES = [
    "KRAS",
    "BRAF",
    "EGFR",
    "TP53",
    "BRCA1",
    "BRCA2",
    "ERBB2",
    "ALK",
    "ROS1",
    "NTRK",
    "RET",
    "MET",
    "PIK3CA",
    "IDH1",
    "IDH2",
    "JAK2",
    "CDK4",
    "CDK6",
    "PDCD1",
    "PDL1",
    "CD274",
]

DRUGS = [
    "pembrolizumab",
    "Keytruda",
    "nivolumab",
    "Opdivo",
    "ipilimumab",
    "Yervoy",
    "atezolizumab",
    "Tecentriq",
    "durvalumab",
    "Imfinzi",
    "avelumab",
    "Bavencio",
    "cemiplimab",
    "Libtayo",
    "dostarlimab",
    "sotorasib",
    "Lumakras",
    "adagrasib",
    "Krazati",
    "daraxonrasib",
    "osimertinib",
    "Tagrisso",
    "trastuzumab",
    "Herceptin",
    "olaparib",
    "Lynparza",
]

DISEASES = [
    "melanoma",
    "lung cancer",
    "non-small cell lung cancer",
    "NSCLC",
    "breast cancer",
    "colorectal cancer",
    "pancreatic cancer",
    "leukemia",
    "lymphoma",
    "myeloma",
    "glioblastoma",
    "solid tumor",
    "cancer",
]

COMPANIES = [
    "Merck",
    "Bristol Myers Squibb",
    "BMS",
    "Roche",
    "Genentech",
    "Novartis",
    "Pfizer",
    "AstraZeneca",
    "GSK",
    "Eli Lilly",
    "Lilly",
    "Amgen",
    "Regeneron",
    "Moderna",
    "BioNTech",
    "AbbVie",
    "Sanofi",
    "Johnson & Johnson",
    "J&J",
    "Gilead",
    "Vertex",
    "Bayer",
    "Takeda",
    "BeiGene",
    "UCB",
    "Revolution Medicines",
]


class Bench:
    def __init__(self) -> None:
        self.started = time.perf_counter()
        self.stage_started: dict[str, float] = {}
        self.stages: dict[str, dict[str, float]] = {}
        self.request_latencies_ms: list[int] = []
        self.stage_counts: dict[str, int] = {}

    def start(self, name: str) -> None:
        self.stage_started[name] = time.perf_counter()

    def stop(self, name: str, count: int | None = None) -> None:
        started = self.stage_started.pop(name, time.perf_counter())
        elapsed = time.perf_counter() - started
        entry: dict[str, float] = {"seconds": round(elapsed, 4)}
        if count is not None:
            self.stage_counts[name] = count
            entry["count"] = count
            entry["throughput_per_second"] = round(count / elapsed, 4) if elapsed > 0 else 0.0
        self.stages[name] = entry

    def observe_fetch(self, fetch_meta: dict[str, Any]) -> None:
        elapsed = fetch_meta.get("elapsed_ms")
        if isinstance(elapsed, int):
            self.request_latencies_ms.append(elapsed)

    def snapshot(self) -> dict[str, Any]:
        total = time.perf_counter() - self.started
        rss_mb = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024.0
        return {
            "total_seconds": round(total, 4),
            "stages": self.stages,
            "http_latency_ms": latency_summary(self.request_latencies_ms),
            "peak_rss_mb": round(rss_mb, 2),
        }


def utc_now() -> datetime:
    return datetime.now(timezone.utc)


def utc_now_iso() -> str:
    return utc_now().replace(microsecond=0).isoformat()


def clean_ws(value: str | None) -> str:
    if not value:
        return ""
    return re.sub(r"\s+", " ", value).strip()


def parse_dt(value: Any) -> datetime | None:
    if not value:
        return None
    if isinstance(value, datetime):
        dt = value
    else:
        try:
            from dateutil import parser

            dt = parser.parse(str(value), fuzzy=True)
        except Exception:
            return None
    if dt.tzinfo is None:
        dt = dt.replace(tzinfo=timezone.utc)
    return dt.astimezone(timezone.utc)


def iso_or_none(value: datetime | None) -> str | None:
    if value is None:
        return None
    return value.replace(microsecond=0).isoformat()


def age_hours(value: datetime | None, now: datetime | None = None) -> float | None:
    if value is None:
        return None
    ref = now or utc_now()
    return round((ref - value).total_seconds() / 3600.0, 2)


def same_site(url: str, base_url: str) -> bool:
    host = urlparse(url).netloc.lower()
    base_host = urlparse(base_url).netloc.lower()
    if not host or not base_host:
        return False
    if host == base_host:
        return True
    host_parts = host.split(".")
    base_parts = base_host.split(".")
    return len(host_parts) >= 2 and len(base_parts) >= 2 and host_parts[-2:] == base_parts[-2:]


def normalize_link(href: str, base_url: str) -> str:
    return urljoin(base_url, href.split("#", 1)[0]).strip()


def article_id(source_id: str, url: str) -> str:
    digest = hashlib.sha1(url.encode("utf-8")).hexdigest()[:12]
    return f"{source_id}:{digest}"


def json_dump(path: str | Path, payload: dict[str, Any]) -> None:
    out = Path(path)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def load_json(path: str | Path) -> dict[str, Any]:
    return json.loads(Path(path).read_text(encoding="utf-8"))


def latency_summary(values: list[int]) -> dict[str, Any]:
    if not values:
        return {"count": 0}
    ordered = sorted(values)

    def percentile(p: float) -> int:
        if len(ordered) == 1:
            return ordered[0]
        index = math.ceil((p / 100.0) * len(ordered)) - 1
        return ordered[max(0, min(index, len(ordered) - 1))]

    return {
        "count": len(ordered),
        "min": ordered[0],
        "p50": percentile(50),
        "p95": percentile(95),
        "max": ordered[-1],
        "mean": round(sum(ordered) / len(ordered), 2),
    }


def fetch_url(
    session: requests.Session,
    url: str,
    user_agent: str,
    timeout_seconds: int,
    pause_seconds: float,
) -> tuple[dict[str, Any], str]:
    if pause_seconds > 0:
        time.sleep(pause_seconds)
    started = time.perf_counter()
    try:
        response = session.get(
            url,
            headers={"User-Agent": user_agent, "Accept": "text/html,application/rss+xml,*/*"},
            timeout=timeout_seconds,
            allow_redirects=True,
        )
        elapsed_ms = int((time.perf_counter() - started) * 1000)
        return (
            {
                "ok": response.ok,
                "status": response.status_code,
                "url": url,
                "final_url": response.url,
                "content_type": response.headers.get("content-type", ""),
                "bytes": len(response.content),
                "elapsed_ms": elapsed_ms,
                "error": None,
            },
            response.text,
        )
    except requests.RequestException as exc:
        elapsed_ms = int((time.perf_counter() - started) * 1000)
        return (
            {
                "ok": False,
                "status": None,
                "url": url,
                "final_url": None,
                "content_type": "",
                "bytes": 0,
                "elapsed_ms": elapsed_ms,
                "error": f"{exc.__class__.__name__}: {exc}",
            },
            "",
        )


def paywall_signals(*texts: str) -> list[str]:
    haystack = "\n".join(t for t in texts if t).lower()
    return sorted({pattern for pattern in PAYWALL_PATTERNS if pattern in haystack})


def simple_html_text(html: str) -> str:
    soup = BeautifulSoup(html, "html.parser")
    for tag in soup(["script", "style", "noscript", "svg", "form", "nav", "footer", "header"]):
        tag.decompose()
    node = soup.find("article") or soup.find("main") or soup.body or soup
    return clean_ws(node.get_text(" ", strip=True))


def extraction_quality_score(
    status: int | None, text_len: int, fallback_len: int, signals: list[str]
) -> int:
    score = 0
    if status and 200 <= status < 300:
        score += 1
    if text_len >= 800:
        score += 1
    if text_len >= 1800:
        score += 1
    if text_len >= 0.35 * max(fallback_len, 1):
        score += 1
    if not signals or text_len >= 1400:
        score += 1
    return min(score, 5)


def access_label(status: int | None, text_len: int, signals: list[str]) -> str:
    if status in {401, 402, 403}:
        return "blocked_http"
    if signals and text_len < 700:
        return "paywall_or_registration_stub"
    if signals:
        return "open_with_auth_or_subscribe_chrome"
    if text_len >= 1200:
        return "open_extracted"
    if text_len >= 400:
        return "partial_extracted"
    return "unreadable_or_too_short"


def extraction_status(status: int | None, text_len: int, quality: int, signals: list[str]) -> str:
    if status in {401, 402, 403}:
        return "http_blocked"
    if signals and text_len < 700:
        return "auth_required"
    if quality >= 3 and text_len >= 800:
        return "extracted"
    if text_len >= 400:
        return "partial"
    if status and 200 <= status < 300:
        return "unreadable"
    return "fetch_failed"


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
    for entry in entries[:50]:
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


def link_score(a: Any, url: str, source: dict[str, Any]) -> int:
    text = clean_ws(a.get_text(" ", strip=True))
    path = urlparse(url).path.lower()
    discovery = source.get("discovery", {})
    include_tokens = [token.lower() for token in discovery.get("article_path_tokens", [])]
    exclude_tokens = [token.lower() for token in discovery.get("exclude_path_tokens", [])]
    score = 0
    if same_site(url, source["page_url"]):
        score += 2
    if a.find_parent("article"):
        score += 2
    if any(token and token in path for token in include_tokens):
        score += 2
    if any(token in path for token in ("/news/", "/article/", "/story/", "/20")):
        score += 1
    if len(text) >= 45:
        score += 1
    if NAV_RE.match(text):
        score -= 5
    if any(token and token in path for token in exclude_tokens):
        score -= 3
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
        score = link_score(a, url, source)
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
    items = sorted(by_url.values(), key=lambda item: item["score"], reverse=True)[:50]
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


def feed_rank(feed: dict[str, Any]) -> tuple[int, int, float]:
    url = feed.get("url") or ""
    penalty = -10 if "comments/feed" in url or "oembed" in url else 0
    link_count = feed.get("field_counts", {}).get("link", 0)
    age = feed.get("latest_age_hours")
    freshness = -float(age) if age is not None else -99999.0
    return (feed.get("entries", 0) + penalty, link_count, freshness)


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


def discover_sources(
    registry: dict[str, Any],
    bench: Bench,
    replay_discovery: dict[str, Any] | None = None,
) -> dict[str, Any]:
    if replay_discovery is not None:
        return normalize_replay_discovery(replay_discovery)

    now = utc_now()
    session = requests.Session()
    user_agent = registry["user_agent"]
    timeout = int(registry.get("default_timeout_seconds", 20))
    pause = float(registry.get("default_request_pause_seconds", 0.0))
    sources = [s for s in registry.get("sources", []) if s.get("enabled_by_default", True)]
    results: list[dict[str, Any]] = []

    for source in sources:
        page_fetch, page_html = fetch_url(session, source["page_url"], user_agent, timeout, pause)
        bench.observe_fetch(page_fetch)
        autodiscovered = discover_feed_links(source["page_url"], page_html) if page_html else []
        candidates = sorted(set(source.get("rss_candidates", []) + autodiscovered))

        feeds = []
        for url in candidates:
            feed_fetch, feed_text = fetch_url(session, url, user_agent, timeout, pause)
            bench.observe_fetch(feed_fetch)
            feeds.append(parse_feed(url, feed_fetch, feed_text, now))

        best_feed = max(feeds, key=feed_rank, default={})
        if page_html:
            page = parse_headline_page(source, page_fetch, page_html)
        else:
            page = {
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
                "registry": {
                    "access": source.get("access", {}),
                    "fetch": source.get("fetch", {}),
                    "extract": source.get("extract", {}),
                    "rights": source.get("rights", {}),
                    "topics": source.get("topics", []),
                },
                "autodiscovered_feed_urls": autodiscovered,
                "rss_candidates_tested": candidates,
                "rss_results": feeds,
                "rss_best": best_feed,
                "headline_page": page,
                "recommended_discovery_mode": choose_best_mode(feeds, page),
            }
        )

    return {
        "approach": "registry_backed_rss_first_discovery",
        "source_matrix": source_matrix(results),
        "sources": results,
    }


def normalize_replay_discovery(discovery: dict[str, Any]) -> dict[str, Any]:
    sources = []
    for source_result in discovery.get("sources", []):
        sources.append(
            {
                "source_id": source_result["source_id"],
                "source_name": source_result.get("source_name"),
                "page_url": source_result.get("page_url"),
                "registry": source_result.get("registry", {}),
                "autodiscovered_feed_urls": source_result.get("autodiscovered_feed_urls", []),
                "rss_candidates_tested": source_result.get("rss_candidates_tested", []),
                "rss_results": source_result.get("rss_results", []),
                "rss_best": source_result.get("rss_best") or {},
                "headline_page": source_result.get("headline_page") or {},
                "recommended_discovery_mode": source_result.get("recommended_discovery_mode"),
            }
        )
    return {
        "approach": "replayed_explore_discovery_dataset",
        "source_matrix": discovery.get("source_matrix") or source_matrix(sources),
        "sources": sources,
    }


def source_matrix(source_results: list[dict[str, Any]]) -> list[dict[str, Any]]:
    matrix = []
    for result in source_results:
        best_feed = result.get("rss_best") or {}
        page = result.get("headline_page") or {}
        matrix.append(
            {
                "source_id": result["source_id"],
                "mode": result.get("recommended_discovery_mode"),
                "best_feed_url": best_feed.get("url"),
                "feed_entries": best_feed.get("entries", 0),
                "feed_latest_age_hours": best_feed.get("latest_age_hours"),
                "headline_items": len(page.get("items", [])),
                "headline_date_fields": page.get("date_fields", 0),
                "page_status": page.get("fetch", {}).get("status"),
            }
        )
    return matrix


def candidate_items_from_discovery(discovery: dict[str, Any], per_source: int) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    for source_result in discovery.get("sources", []):
        source_id = source_result["source_id"]
        seen: set[str] = set()
        items: list[dict[str, Any]] = []
        best_feed = source_result.get("rss_best") or {}
        for item in best_feed.get("items", []):
            link = item.get("link")
            if link and link not in seen:
                seen.add(link)
                items.append(
                    {
                        "id": article_id(source_id, link),
                        "source_id": source_id,
                        "source_name": source_result.get("source_name"),
                        "title": item.get("title"),
                        "url": link,
                        "published": item.get("published"),
                        "discovery_mode": "rss",
                    }
                )
        for item in source_result.get("headline_page", {}).get("items", []):
            link = item.get("url")
            if link and link not in seen:
                seen.add(link)
                items.append(
                    {
                        "id": article_id(source_id, link),
                        "source_id": source_id,
                        "source_name": source_result.get("source_name"),
                        "title": item.get("title"),
                        "url": link,
                        "published": item.get("date"),
                        "discovery_mode": "headline_page",
                    }
                )
        out.extend(items[:per_source])
    return out


def extract_one(
    session: requests.Session,
    item: dict[str, Any],
    index: int,
    registry: dict[str, Any],
    bench: Bench,
) -> dict[str, Any]:
    user_agent = registry["user_agent"]
    timeout = int(registry.get("default_timeout_seconds", 20))
    pause = float(registry.get("default_request_pause_seconds", 0.0))
    fetch, html = fetch_url(session, item["url"], user_agent, timeout, pause)
    bench.observe_fetch(fetch)
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
    analysis_text = extracted if len(extracted) >= 500 else fallback
    quality = extraction_quality_score(fetch.get("status"), text_len, fallback_len, signals)
    status = extraction_status(fetch.get("status"), text_len, quality, signals)
    return {
        "index": index,
        "id": item["id"],
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
        "extraction_status": status,
        "quality_score_0_5": quality,
        "has_useful_text": quality >= 3 and text_len >= 800,
        "text_sample": clean_ws(extracted[:500]),
        "_analysis_text": analysis_text,
        "_analysis_fetch_meta": {
            "fetch": fetch,
            "text_chars": len(analysis_text),
            "paywall_signals": signals,
        },
    }


def extract_articles(
    discovery: dict[str, Any],
    registry: dict[str, Any],
    per_source: int,
    max_articles: int,
    bench: Bench,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    candidates = candidate_items_from_discovery(discovery, per_source=per_source)
    session = requests.Session()
    articles = []
    for index, item in enumerate(candidates[:max_articles], start=1):
        articles.append(extract_one(session, item, index, registry, bench))
    useful = [a for a in articles if a["has_useful_text"]]
    useful_sources = sorted({a["source_id"] for a in useful})
    by_source = []
    for source_id in sorted({a["source_id"] for a in articles}):
        group = [a for a in articles if a["source_id"] == source_id]
        statuses = {a.get("fetch", {}).get("status") for a in group}
        by_source.append(
            {
                "source_id": source_id,
                "attempted": len(group),
                "useful": sum(1 for a in group if a["has_useful_text"]),
                "http_statuses": sorted(status for status in statuses if status is not None)
                + ([None] if None in statuses else []),
                "extraction_statuses": sorted({a["extraction_status"] for a in group}),
                "min_trafilatura_chars": min((a["trafilatura_text_chars"] for a in group), default=0),
                "max_trafilatura_chars": max((a["trafilatura_text_chars"] for a in group), default=0),
                "max_fallback_chars": max((a["fallback_text_chars"] for a in group), default=0),
                "quality_scores": sorted({a["quality_score_0_5"] for a in group}),
                "access_labels": sorted({a["access_label"] for a in group}),
            }
        )
    summary = {
        "candidate_articles": len(candidates),
        "attempted_articles": len(articles),
        "useful_extractions": len(useful),
        "useful_sources": useful_sources,
        "useful_source_count": len(useful_sources),
        "mean_quality_score": round(
            sum(a["quality_score_0_5"] for a in articles) / max(len(articles), 1), 2
        ),
        "by_source": by_source,
    }
    return articles, summary


def find_terms(text: str, terms: list[str]) -> list[str]:
    found = []
    for term in terms:
        if re.search(rf"(?<![A-Za-z0-9]){re.escape(term)}(?![A-Za-z0-9])", text, re.I):
            found.append(term)
    return sorted(set(found), key=str.lower)


def extract_entities(text: str) -> dict[str, Any]:
    suffix_drugs = sorted(
        {
            match.group(0)
            for match in re.finditer(
                r"\b[A-Za-z][A-Za-z0-9-]{3,}(?:mab|nib|rasib|parib|ciclib|limab|zumab|tinib)\b",
                text,
            )
        },
        key=str.lower,
    )
    drugs = sorted(set(find_terms(text, DRUGS) + suffix_drugs), key=str.lower)
    return {
        "dois": sorted(set(m.group(0).rstrip(".,);") for m in DOI_RE.finditer(text))),
        "pmids": sorted(set(m.group(1) for m in PMID_RE.finditer(text))),
        "nct_ids": sorted(set(m.group(0).upper() for m in NCT_RE.finditer(text))),
        "genes": find_terms(text, GENES),
        "drugs": drugs[:20],
        "diseases": find_terms(text, DISEASES),
        "companies": find_terms(text, COMPANIES),
        "phase_mentions": sorted(set(m.group(0) for m in PHASE_RE.finditer(text))),
        "approval_cues": find_terms(text, ["FDA", "approval", "approved", "label", "CRL"]),
        "trial_cues": find_terms(text, ["trial", "study", "readout", "endpoint", "overall survival"]),
    }


def fetch_extract_text(
    session: requests.Session,
    url: str,
    registry: dict[str, Any],
    bench: Bench,
) -> tuple[str, dict[str, Any]]:
    fetch, html = fetch_url(
        session,
        url,
        registry["user_agent"],
        int(registry.get("default_timeout_seconds", 20)),
        float(registry.get("default_request_pause_seconds", 0.0)),
    )
    bench.observe_fetch(fetch)
    text = ""
    if html:
        text = trafilatura.extract(
            html,
            url=fetch.get("final_url") or url,
            include_comments=False,
            include_tables=False,
            favor_precision=True,
        ) or ""
    if len(text) < 500 and html:
        text = simple_html_text(html)
    signals = paywall_signals(html[:20000], text[:4000])
    return text, {"fetch": fetch, "text_chars": len(text), "paywall_signals": signals}


def profile_score(
    title: str,
    text: str,
    entities: dict[str, Any],
    profile: dict[str, Any],
    conservative: bool,
) -> tuple[int, list[str]]:
    combined = f"{title}\n{text[:4000]}"
    score = 0
    reasons = []
    for term, weight in profile.get("keywords", {}).items():
        if re.search(rf"(?<![A-Za-z0-9]){re.escape(term)}(?![A-Za-z0-9])", combined, re.I):
            score += int(weight)
            reasons.append(term)
    if entities["genes"]:
        score += 2
    if entities["drugs"]:
        score += 1
    if entities["nct_ids"] or entities["trial_cues"]:
        score += 1
    if conservative:
        specific_terms = set(profile.get("interests", []))
        has_specific_interest = any(
            re.search(rf"(?<![A-Za-z0-9]){re.escape(term)}(?![A-Za-z0-9])", combined, re.I)
            for term in specific_terms
        )
        generic_only = set(reasons).issubset({"oncology", "cancer", "phase", "clinical trial"})
        if generic_only and not has_specific_interest:
            score = max(0, score - 3)
    return score, reasons


def run_biomcp(args: list[str], timeout: int = 25) -> dict[str, Any]:
    command = ["biomcp", "--json", *args]
    started = time.perf_counter()
    try:
        proc = subprocess.run(command, capture_output=True, text=True, timeout=timeout, check=False)
    except Exception as exc:
        return {
            "command": command,
            "ok": False,
            "elapsed_ms": int((time.perf_counter() - started) * 1000),
            "error": f"{exc.__class__.__name__}: {exc}",
        }
    elapsed_ms = int((time.perf_counter() - started) * 1000)
    parsed: Any = None
    if proc.stdout.strip():
        try:
            parsed = json.loads(proc.stdout)
        except json.JSONDecodeError:
            parsed = None
    summary = {
        "command": command,
        "ok": proc.returncode == 0,
        "returncode": proc.returncode,
        "elapsed_ms": elapsed_ms,
        "stderr": proc.stderr.strip()[-500:],
    }
    if isinstance(parsed, dict):
        if "count" in parsed:
            summary["count"] = parsed.get("count")
        if "name" in parsed:
            summary["name"] = parsed.get("name")
        if "symbol" in parsed:
            summary["symbol"] = parsed.get("symbol")
        if "id" in parsed:
            summary["id"] = parsed.get("id")
        if "results" in parsed and isinstance(parsed["results"], list) and parsed["results"]:
            first = parsed["results"][0]
            summary["first_result"] = {
                key: first.get(key)
                for key in ("id", "nct_id", "title", "name", "source")
                if isinstance(first, dict) and first.get(key) is not None
            }
    return summary


def choose_pivots(article_results: list[dict[str, Any]], pivot_limit: int) -> list[dict[str, Any]]:
    pivots = []
    seen: set[tuple[str, str]] = set()
    for article in article_results:
        entities = article["entities"]
        for drug in entities.get("drugs", []):
            key = ("drug", drug.lower())
            if key not in seen:
                seen.add(key)
                pivots.append(
                    {
                        "article_id": article["id"],
                        "article_url": article["url"],
                        "article_title": article["title"],
                        "type": "drug",
                        "value": drug,
                        "result": run_biomcp(["get", "drug", drug]),
                    }
                )
                break
        for gene in entities.get("genes", []):
            key = ("gene", gene.upper())
            if key not in seen:
                seen.add(key)
                pivots.append(
                    {
                        "article_id": article["id"],
                        "article_url": article["url"],
                        "article_title": article["title"],
                        "type": "gene",
                        "value": gene,
                        "result": run_biomcp(["get", "gene", gene]),
                    }
                )
                break
        for disease in sorted(entities.get("diseases", []), key=len, reverse=True):
            if disease.lower() == "cancer":
                continue
            key = ("disease", disease.lower())
            if key not in seen:
                seen.add(key)
                pivots.append(
                    {
                        "article_id": article["id"],
                        "article_url": article["url"],
                        "article_title": article["title"],
                        "type": "disease",
                        "value": disease,
                        "result": run_biomcp(["get", "disease", disease]),
                    }
                )
                break
        for nct_id in entities.get("nct_ids", []):
            key = ("trial", nct_id)
            if key not in seen:
                seen.add(key)
                pivots.append(
                    {
                        "article_id": article["id"],
                        "article_url": article["url"],
                        "article_title": article["title"],
                        "type": "trial",
                        "value": nct_id,
                        "result": run_biomcp(["get", "trial", nct_id]),
                    }
                )
                break
        if len(pivots) >= pivot_limit:
            break

    if not any(pivot["type"] == "trial_search" for pivot in pivots):
        drug_counts = Counter(
            drug.lower()
            for article in article_results
            for drug in article["entities"].get("drugs", [])
            if len(drug) > 3
        )
        if drug_counts and len(pivots) < pivot_limit:
            drug = drug_counts.most_common(1)[0][0]
            pivots.append(
                {
                    "type": "trial_search",
                    "value": drug,
                    "result": run_biomcp(["search", "trial", "-i", drug, "--limit", "1"]),
                }
            )
    return pivots[:pivot_limit]


def analyze_entities_and_briefing(
    articles: list[dict[str, Any]],
    registry: dict[str, Any],
    profile: dict[str, Any],
    max_entity_articles: int,
    pivot_limit: int,
    conservative_ranking: bool,
    bench: Bench,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], list[dict[str, Any]], dict[str, Any]]:
    candidates = [article for article in articles if article.get("fetch", {}).get("status") and article.get("url")]
    candidates.sort(
        key=lambda article: (
            article.get("has_useful_text", False),
            article.get("quality_score_0_5", 0),
            article.get("trafilatura_text_chars", 0),
        ),
        reverse=True,
    )
    article_results = []
    for article in candidates[:max_entity_articles]:
        cached_text = article.get("_analysis_text")
        if isinstance(cached_text, str):
            text = cached_text
            fetch_meta = dict(article.get("_analysis_fetch_meta") or {})
            fetch_meta["reused_from_article_extraction"] = True
        else:
            session = requests.Session()
            text, fetch_meta = fetch_extract_text(session, article["url"], registry, bench)
        combined = f"{article.get('title') or ''}\n{text}"
        entities = extract_entities(combined)
        score, reasons = profile_score(
            article.get("title") or "",
            text,
            entities,
            profile,
            conservative=conservative_ranking,
        )
        article_results.append(
            {
                "id": article["id"],
                "source_id": article.get("source_id"),
                "source_name": article.get("source_name"),
                "title": article.get("title"),
                "url": article.get("url"),
                "fetch_extract": fetch_meta,
                "entities": entities,
                "profile_score": score,
                "profile_reasons": reasons,
                "precision_assessment": {
                    "ids": "high precision regex"
                    if entities["dois"] or entities["pmids"] or entities["nct_ids"]
                    else "no ids found",
                    "genes": "dictionary exact-match; possible false positives for short symbols",
                    "drugs": "dictionary plus suffix heuristic; suffix heuristic needs curation",
                    "companies": "small dictionary; high precision but low recall",
                },
            }
        )
    briefing = sorted(article_results, key=lambda item: item["profile_score"], reverse=True)[:5]

    bench.start("pivot_validation")
    pivots = choose_pivots(article_results, pivot_limit=pivot_limit)
    bench.stop("pivot_validation", count=len(pivots))

    successful_pivots = [pivot for pivot in pivots if pivot.get("result", {}).get("ok")]
    summary = {
        "articles_analyzed": len(article_results),
        "articles_with_any_entity": sum(1 for a in article_results if any(a["entities"].values())),
        "pivot_attempts": len(pivots),
        "successful_pivots": len(successful_pivots),
        "top_briefing_title": briefing[0]["title"] if briefing else None,
        "top_briefing_score": briefing[0]["profile_score"] if briefing else None,
    }
    briefing_cards = [
        {
            "rank": idx,
            "article_id": article["id"],
            "source_id": article["source_id"],
            "title": article["title"],
            "url": article["url"],
            "profile_score": article["profile_score"],
            "reasons": article["profile_reasons"],
            "entities": {
                "genes": article["entities"]["genes"],
                "drugs": article["entities"]["drugs"][:8],
                "diseases": article["entities"]["diseases"],
                "companies": article["entities"]["companies"],
                "nct_ids": article["entities"]["nct_ids"],
            },
        }
        for idx, article in enumerate(briefing, start=1)
    ]
    return article_results, pivots, briefing_cards, summary


def validate_results(payload: dict[str, Any]) -> dict[str, Any]:
    discovery = payload.get("discovery", {})
    extraction_summary = payload.get("extraction_summary", {})
    entity_summary = payload.get("entity_summary", {})
    articles = payload.get("articles", [])
    entity_articles = payload.get("entity_articles", [])
    briefing = payload.get("personalized_briefing", [])
    pivots = payload.get("cross_reference_pivots", [])

    checks = []

    def add(name: str, passed: bool, observed: Any, expected: str, severity: str = "failure") -> None:
        checks.append(
            {
                "name": name,
                "passed": bool(passed),
                "observed": observed,
                "expected": expected,
                "severity": severity,
            }
        )

    source_ids = [item.get("source_id") for item in discovery.get("source_matrix", [])]
    add("six_core_sources_attempted", len(set(source_ids)) >= 6, sorted(set(source_ids)), ">= 6 source IDs")
    add(
        "source_discovery_matrix_present",
        bool(discovery.get("source_matrix")),
        len(discovery.get("source_matrix", [])),
        "non-empty source matrix",
    )
    add(
        "rss_or_blocked_status_recorded",
        all(item.get("mode") for item in discovery.get("source_matrix", [])),
        [item.get("mode") for item in discovery.get("source_matrix", [])],
        "all sources have an explicit discovery mode",
    )
    sampled_sources = sorted({article.get("source_id") for article in articles})
    add(
        "content_extraction_samples_three_sources",
        len(sampled_sources) >= 3,
        sampled_sources,
        "quality-scored extraction samples from >= 3 sources",
    )
    add(
        "useful_open_extraction_three_sources",
        extraction_summary.get("useful_source_count", 0) >= 3,
        extraction_summary.get("useful_sources", []),
        "useful direct extraction from >= 3 sources",
        severity="warning",
    )
    add(
        "entity_extraction_five_articles",
        entity_summary.get("articles_analyzed", 0) >= 5,
        entity_summary.get("articles_analyzed", 0),
        ">= 5 articles analyzed for entities",
    )
    precision_count = sum(1 for article in entity_articles if article.get("precision_assessment"))
    add(
        "precision_assessment_present",
        precision_count >= min(5, len(entity_articles)),
        precision_count,
        "precision assessment on analyzed entity articles",
    )
    add(
        "three_successful_biomcp_pivots",
        entity_summary.get("successful_pivots", 0) >= 3,
        entity_summary.get("successful_pivots", 0),
        ">= 3 successful BioMCP pivots",
    )
    add(
        "personalized_briefing_present",
        len(briefing) >= 1,
        len(briefing),
        "sample personalized briefing has at least one item",
    )
    add(
        "briefing_relevance_signal",
        any(
            "KRAS" in card.get("entities", {}).get("genes", [])
            or "melanoma" in [d.lower() for d in card.get("entities", {}).get("diseases", [])]
            or "immunotherapy" in [r.lower() for r in card.get("reasons", [])]
            for card in briefing[:3]
        ),
        briefing[:3],
        "top briefing items include KRAS, melanoma, or immunotherapy signal",
    )
    add(
        "pivot_results_are_structured",
        all("result" in pivot for pivot in pivots),
        len(pivots),
        "pivot attempts include structured command results",
    )

    failures = [check for check in checks if not check["passed"] and check["severity"] == "failure"]
    warnings = [check for check in checks if not check["passed"] and check["severity"] == "warning"]
    return {
        "mismatch_count": len(failures),
        "warning_count": len(warnings),
        "checks": checks,
        "failures": failures,
        "warnings": warnings,
    }


def stable_projection(payload: dict[str, Any]) -> dict[str, Any]:
    discovery = payload.get("discovery", {})
    return {
        "source_matrix": discovery.get("source_matrix", []),
        "extraction_summary": {
            key: payload.get("extraction_summary", {}).get(key)
            for key in (
                "attempted_articles",
                "useful_extractions",
                "useful_source_count",
                "useful_sources",
                "mean_quality_score",
            )
        },
        "entity_summary": {
            key: payload.get("entity_summary", {}).get(key)
            for key in (
                "articles_analyzed",
                "articles_with_any_entity",
                "pivot_attempts",
                "successful_pivots",
                "top_briefing_title",
            )
        },
        "briefing": [
            {
                "rank": card.get("rank"),
                "source_id": card.get("source_id"),
                "title": card.get("title"),
                "profile_score": card.get("profile_score"),
                "genes": card.get("entities", {}).get("genes", []),
                "drugs": card.get("entities", {}).get("drugs", []),
                "diseases": card.get("entities", {}).get("diseases", []),
            }
            for card in payload.get("personalized_briefing", [])
        ],
        "validation_mismatch_count": payload.get("validation", {}).get("mismatch_count"),
    }


def checksum_projection(payload: dict[str, Any]) -> str:
    encoded = json.dumps(stable_projection(payload), sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def baseline_metrics(
    discovery_path: Path | None,
    extraction_path: Path | None,
    entity_path: Path | None,
) -> dict[str, Any] | None:
    if not (discovery_path and extraction_path and entity_path):
        return None
    if not (discovery_path.exists() and extraction_path.exists() and entity_path.exists()):
        return None

    discovery = load_json(discovery_path)
    extraction = load_json(extraction_path)
    entities = load_json(entity_path)
    pseudo_payload = {
        "discovery": {
            "source_matrix": discovery.get("source_matrix", []),
        },
        "articles": extraction.get("articles", []),
        "extraction_summary": extraction.get("summary", {}),
        "entity_articles": entities.get("articles", []),
        "entity_summary": {
            **entities.get("summary", {}),
            "top_briefing_title": (
                entities.get("personalized_briefing", [{}])[0].get("title")
                if entities.get("personalized_briefing")
                else None
            ),
        },
        "personalized_briefing": entities.get("personalized_briefing", []),
        "cross_reference_pivots": entities.get("cross_reference_pivots", []),
    }
    pseudo_payload["validation"] = validate_results(pseudo_payload)
    return {
        "discovery_source_matrix": discovery.get("source_matrix", []),
        "extraction_summary": extraction.get("summary", {}),
        "entity_summary": entities.get("summary", {}),
        "top_briefing_title": pseudo_payload["entity_summary"].get("top_briefing_title"),
        "validation_mismatch_count": pseudo_payload["validation"]["mismatch_count"],
        "validation_warning_count": pseudo_payload["validation"]["warning_count"],
        "projection_checksum": checksum_projection(pseudo_payload),
    }


def compare_regression(current: dict[str, Any], baseline: dict[str, Any] | None) -> dict[str, Any]:
    if baseline is None:
        return {
            "baseline_available": False,
            "status": "not_applicable",
            "notes": ["No explore baseline JSON paths were provided."],
            "checks": [],
        }

    checks = []

    def add(metric: str, passed: bool, current_value: Any, baseline_value: Any, rule: str) -> None:
        checks.append(
            {
                "metric": metric,
                "passed": bool(passed),
                "current": current_value,
                "baseline": baseline_value,
                "rule": rule,
            }
        )

    current_validation = current.get("validation", {})
    current_extraction = current.get("extraction_summary", {})
    current_entity = current.get("entity_summary", {})
    base_extraction = baseline.get("extraction_summary", {})
    base_entity = baseline.get("entity_summary", {})

    add(
        "mismatch_count",
        current_validation.get("mismatch_count", 9999) <= baseline.get("validation_mismatch_count", 9999),
        current_validation.get("mismatch_count"),
        baseline.get("validation_mismatch_count"),
        "must strictly decrease or stay equal",
    )
    add(
        "attempted_articles",
        current_extraction.get("attempted_articles") == base_extraction.get("attempted_articles"),
        current_extraction.get("attempted_articles"),
        base_extraction.get("attempted_articles"),
        "same explore-scale workload",
    )
    add(
        "useful_extractions",
        current_extraction.get("useful_extractions", 0) >= base_extraction.get("useful_extractions", 0),
        current_extraction.get("useful_extractions"),
        base_extraction.get("useful_extractions"),
        "correctness must stay equal or improve",
    )
    add(
        "useful_source_count",
        current_extraction.get("useful_source_count", 0) >= base_extraction.get("useful_source_count", 0),
        current_extraction.get("useful_source_count"),
        base_extraction.get("useful_source_count"),
        "correctness must stay equal or improve",
    )
    add(
        "articles_analyzed",
        current_entity.get("articles_analyzed", 0) == base_entity.get("articles_analyzed", 0),
        current_entity.get("articles_analyzed"),
        base_entity.get("articles_analyzed"),
        "same explore-scale workload",
    )
    add(
        "articles_with_any_entity",
        current_entity.get("articles_with_any_entity", 0) >= base_entity.get("articles_with_any_entity", 0),
        current_entity.get("articles_with_any_entity"),
        base_entity.get("articles_with_any_entity"),
        "correctness must stay equal or improve",
    )
    add(
        "successful_pivots",
        current_entity.get("successful_pivots", 0) >= base_entity.get("successful_pivots", 0),
        current_entity.get("successful_pivots"),
        base_entity.get("successful_pivots"),
        "correctness must stay equal or improve",
    )
    add(
        "top_briefing_title",
        current_entity.get("top_briefing_title") == baseline.get("top_briefing_title"),
        current_entity.get("top_briefing_title"),
        baseline.get("top_briefing_title"),
        "same top personalized briefing item for regression control",
    )

    status = "pass" if all(check["passed"] for check in checks) else "fail"
    return {
        "baseline_available": True,
        "status": status,
        "checks": checks,
        "baseline_projection_checksum": baseline.get("projection_checksum"),
        "current_projection_checksum": current.get("projection_checksum"),
        "checksum_rule_applied": False,
        "checksum_note": (
            "Explore did not record a projection checksum during the explore phase; "
            "the baseline checksum here is computed post hoc from saved explore JSON and "
            "is reported for traceability, not used as a hard regression gate."
        ),
        "timing_rule_applied": False,
        "timing_note": (
            "Explore did not record throughput, request latency, peak RSS, or build/load time. "
            "Exploit control metrics establish the first contract values for those dimensions."
        ),
    }


def build_recommendations(payload: dict[str, Any]) -> dict[str, Any]:
    extraction_summary = payload.get("extraction_summary", {})
    by_source = {row["source_id"]: row for row in extraction_summary.get("by_source", [])}
    return {
        "feasibility": "feasible_with_curated_source_aware_mvp",
        "mvp_source_scope": {
            "default_discovery_enabled": [
                "fierce-biotech",
                "fierce-pharma",
                "biopharma-dive",
                "stat",
                "endpoints",
            ],
            "defer_or_auth_required": ["genomeweb"],
            "default_content_extraction_high_confidence": ["biopharma-dive"],
            "content_extraction_conservative_metered": ["stat"],
            "discovery_or_partial_only": ["fierce-biotech", "fierce-pharma", "endpoints"],
        },
        "mvp_features": [
            "biomcp search news with keyword/source/date/access and typed entity filters",
            "biomcp get news <id> with metadata, access status, summary, entities, and BioMCP pivots",
            "local metadata and structured-extraction cache",
            "publisher registry with source-specific discovery/access/rights rules",
        ],
        "defer": [
            "Playwright auth/login flows",
            "premium full-text export or sync",
            "embeddings",
            "broad biomedical NER",
            "arbitrary user-added crawling",
        ],
        "source_extraction_status": by_source,
    }


def public_article_records(articles: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [{key: value for key, value in article.items() if not key.startswith("_")} for article in articles]


def run_pipeline(args: argparse.Namespace) -> dict[str, Any]:
    bench = Bench()
    bench.start("startup_load")
    registry = load_json(args.registry)
    profile = load_json(args.profile)
    replay_discovery = load_json(args.replay_discovery) if args.replay_discovery else None
    baseline = baseline_metrics(
        Path(args.baseline_discovery) if args.baseline_discovery else None,
        Path(args.baseline_extraction) if args.baseline_extraction else None,
        Path(args.baseline_entities) if args.baseline_entities else None,
    )
    bench.stop("startup_load")

    bench.start("discovery")
    discovery = discover_sources(registry, bench, replay_discovery=replay_discovery)
    bench.stop("discovery", count=len(discovery.get("source_matrix", [])))

    bench.start("article_extraction")
    articles, extraction_summary = extract_articles(
        discovery,
        registry,
        per_source=args.per_source,
        max_articles=args.max_articles,
        bench=bench,
    )
    bench.stop("article_extraction", count=len(articles))

    bench.start("entity_briefing")
    entity_articles, pivots, briefing, entity_summary = analyze_entities_and_briefing(
        articles,
        registry,
        profile,
        max_entity_articles=args.max_entity_articles,
        pivot_limit=args.pivot_limit,
        conservative_ranking=args.conservative_ranking,
        bench=bench,
    )
    bench.stop("entity_briefing", count=len(entity_articles))

    payload: dict[str, Any] = {
        "generated_at": utc_now_iso(),
        "label": args.label,
        "approach": "rss_first_trafilatura_heuristic_entities_profile_briefing",
        "registry_path": str(args.registry),
        "profile_path": str(args.profile),
        "parameters": {
            "per_source": args.per_source,
            "max_articles": args.max_articles,
            "max_entity_articles": args.max_entity_articles,
            "pivot_limit": args.pivot_limit,
            "replayed_discovery": bool(args.replay_discovery),
            "conservative_ranking": bool(args.conservative_ranking),
        },
        "discovery": discovery,
        "articles": public_article_records(articles),
        "extraction_summary": extraction_summary,
        "entity_articles": entity_articles,
        "entity_summary": entity_summary,
        "cross_reference_pivots": pivots,
        "personalized_briefing": briefing,
        "recommendations": {},
    }
    payload["validation"] = validate_results(payload)
    payload["recommendations"] = build_recommendations(payload)
    payload["projection_checksum"] = checksum_projection(payload)
    payload["metrics"] = bench.snapshot()
    payload["regression_control"] = compare_regression(payload, baseline)
    json_dump(args.output, payload)
    return payload


def parse_args() -> argparse.Namespace:
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
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    payload = run_pipeline(args)
    print(
        json.dumps(
            {
                "output": str(args.output),
                "label": args.label,
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


if __name__ == "__main__":
    main()
