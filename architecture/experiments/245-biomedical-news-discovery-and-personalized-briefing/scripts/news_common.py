from __future__ import annotations

import json
import re
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any
from urllib.parse import urljoin, urlparse

import requests

USER_AGENT = (
    "BioMCP-News-Spike/0.1 "
    "(https://github.com/genomoncology/biomcp; live-source feasibility probe)"
)
TIMEOUT_SECONDS = 20

SOURCES: list[dict[str, Any]] = [
    {
        "id": "fierce-biotech",
        "name": "Fierce Biotech",
        "page_url": "https://www.fiercebiotech.com/",
        "rss_candidates": [
            "https://www.fiercebiotech.com/rss/xml",
            "https://www.fiercebiotech.com/rss",
            "https://www.fiercebiotech.com/biotech/rss/xml",
        ],
    },
    {
        "id": "fierce-pharma",
        "name": "Fierce Pharma",
        "page_url": "https://www.fiercepharma.com/",
        "rss_candidates": [
            "https://www.fiercepharma.com/rss/xml",
            "https://www.fiercepharma.com/rss",
            "https://www.fiercepharma.com/pharma/rss/xml",
        ],
    },
    {
        "id": "biopharma-dive",
        "name": "BioPharma Dive",
        "page_url": "https://www.biopharmadive.com/news/",
        "rss_candidates": [
            "https://www.biopharmadive.com/feeds/news/",
            "https://www.biopharmadive.com/feeds/",
            "https://www.biopharmadive.com/rss/",
        ],
    },
    {
        "id": "stat",
        "name": "STAT",
        "page_url": "https://www.statnews.com/",
        "rss_candidates": [
            "https://www.statnews.com/feed/",
            "https://www.statnews.com/category/biotech/feed/",
        ],
    },
    {
        "id": "endpoints",
        "name": "Endpoints News",
        "page_url": "https://endpoints.news/news/",
        "rss_candidates": [
            "https://endpoints.news/feed/",
            "https://endpoints.news/news/feed/",
        ],
    },
    {
        "id": "genomeweb",
        "name": "GenomeWeb",
        "page_url": "https://www.genomeweb.com/",
        "rss_candidates": [
            "https://www.genomeweb.com/rss.xml",
            "https://www.genomeweb.com/news/feed",
            "https://www.genomeweb.com/feed",
        ],
    },
]

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


def utc_now() -> datetime:
    return datetime.now(timezone.utc)


def utc_now_iso() -> str:
    return utc_now().replace(microsecond=0).isoformat()


def clean_ws(value: str | None) -> str:
    if not value:
        return ""
    return re.sub(r"\s+", " ", value).strip()


def json_dump(path: str | Path, payload: dict[str, Any]) -> None:
    out = Path(path)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


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


def fetch_url(session: requests.Session, url: str) -> tuple[dict[str, Any], str]:
    started = time.perf_counter()
    try:
        response = session.get(
            url,
            headers={"User-Agent": USER_AGENT, "Accept": "text/html,application/rss+xml,*/*"},
            timeout=TIMEOUT_SECONDS,
            allow_redirects=True,
        )
        elapsed_ms = int((time.perf_counter() - started) * 1000)
        text = response.text
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
            text,
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


def normalize_link(href: str, base_url: str) -> str:
    return urljoin(base_url, href.split("#", 1)[0]).strip()


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


def paywall_signals(*texts: str) -> list[str]:
    haystack = "\n".join(t for t in texts if t).lower()
    return sorted({pattern for pattern in PAYWALL_PATTERNS if pattern in haystack})


def simple_html_text(html: str) -> str:
    from bs4 import BeautifulSoup

    soup = BeautifulSoup(html, "html.parser")
    for tag in soup(["script", "style", "noscript", "svg", "form", "nav", "footer", "header"]):
        tag.decompose()
    node = soup.find("article") or soup.find("main") or soup.body or soup
    return clean_ws(node.get_text(" ", strip=True))


def extraction_quality_score(status: int | None, text_len: int, fallback_len: int, signals: list[str]) -> int:
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


def source_by_id(source_id: str) -> dict[str, Any]:
    for source in SOURCES:
        if source["id"] == source_id:
            return source
    raise KeyError(source_id)


def candidate_items_from_discovery(discovery: dict[str, Any], per_source: int = 4) -> list[dict[str, Any]]:
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
