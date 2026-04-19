from __future__ import annotations

from typing import Any

import requests

from biomcp_news_spike import (
    DEFAULT_REGISTRY,
    access_label,
    age_hours,
    candidate_items_from_discovery as _candidate_items_from_discovery,
    clean_ws,
    extraction_quality_score,
    fetch_url as _fetch_url,
    iso_or_none,
    json_dump,
    load_json,
    normalize_link,
    parse_dt,
    paywall_signals,
    same_site,
    simple_html_text,
    utc_now,
    utc_now_iso,
)

_REGISTRY = load_json(DEFAULT_REGISTRY)
USER_AGENT = _REGISTRY["user_agent"]
TIMEOUT_SECONDS = int(_REGISTRY.get("default_timeout_seconds", 20))
REQUEST_PAUSE_SECONDS = float(_REGISTRY.get("default_request_pause_seconds", 0.0))
SOURCES: list[dict[str, Any]] = _REGISTRY.get("sources", [])


def fetch_url(session: requests.Session, url: str) -> tuple[dict[str, Any], str]:
    return _fetch_url(session, url, USER_AGENT, TIMEOUT_SECONDS, REQUEST_PAUSE_SECONDS)


def source_by_id(source_id: str) -> dict[str, Any]:
    for source in SOURCES:
        if source["id"] == source_id:
            return source
    raise KeyError(source_id)


def candidate_items_from_discovery(discovery: dict[str, Any], per_source: int = 4) -> list[dict[str, Any]]:
    return _candidate_items_from_discovery(discovery, per_source=per_source)


__all__ = [
    "REQUEST_PAUSE_SECONDS",
    "SOURCES",
    "TIMEOUT_SECONDS",
    "USER_AGENT",
    "access_label",
    "age_hours",
    "candidate_items_from_discovery",
    "clean_ws",
    "extraction_quality_score",
    "fetch_url",
    "iso_or_none",
    "json_dump",
    "load_json",
    "normalize_link",
    "parse_dt",
    "paywall_signals",
    "same_site",
    "simple_html_text",
    "source_by_id",
    "utc_now",
    "utc_now_iso",
]
