from __future__ import annotations

import html
import json
import os
import re
import time
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET
from functools import lru_cache
from typing import Any

from phenotype_spike_common import DISEASES, HTTP_TIMEOUT_SECONDS, USER_AGENT

from .common import RESULTS_DIR, WORK_DIR, load_json, normalize_text, slugify, source_native_id


EXPLORE_MEDLINEPLUS_PATH = RESULTS_DIR / "clinical_summary_medlineplus_probe.json"
CACHE_DIR = WORK_DIR / "medlineplus"


def clean_text(value: str) -> str:
    value = html.unescape(value)
    value = re.sub(r"(?is)<[^>]+>", " ", value)
    return re.sub(r"\s+", " ", value).strip()


def _cache_path(query: str) -> str:
    return str(CACHE_DIR / f"{slugify(query) or 'query'}.json")


def _parse_search_xml(body: str) -> list[dict[str, str]]:
    root = ET.fromstring(body)
    topics: list[dict[str, str]] = []
    for doc in root.findall(".//document"):
        row: dict[str, str] = {"url": doc.attrib.get("url", "")}
        for content in doc.findall("content"):
            name = content.attrib.get("name", "")
            text = clean_text(content.text or "")
            if name == "title" and text:
                row["title"] = text
            elif name in {"FullSummary", "snippet"} and text and "summary" not in row:
                row["summary"] = text
        if row.get("title"):
            row["source_native_id"] = source_native_id(row.get("url", ""))
            topics.append(row)
    return topics


def medlineplus_search(query: str, *, allow_live: bool, refresh_cache: bool) -> dict[str, Any]:
    if not allow_live:
        return {
            "ok": False,
            "status": None,
            "elapsed_ms": 0.0,
            "query": query,
            "topics": [],
            "cache_hit": False,
            "cache_path": _cache_path(query),
            "error": "live fetch disabled",
        }

    path = CACHE_DIR / f"{slugify(query) or 'query'}.json"
    if path.exists() and not refresh_cache:
        cached = json.loads(path.read_text(encoding="utf-8"))
        cached["cache_hit"] = True
        return cached

    params = urllib.parse.urlencode({"db": "healthTopics", "term": query, "retmax": "5"})
    url = f"https://wsearch.nlm.nih.gov/ws/query?{params}"
    req = urllib.request.Request(
        url,
        headers={"Accept": "application/xml", "User-Agent": USER_AGENT},
        method="GET",
    )
    started = time.perf_counter()
    try:
        with urllib.request.urlopen(req, timeout=HTTP_TIMEOUT_SECONDS) as resp:
            body = resp.read().decode("utf-8", errors="replace")
            payload = {
                "ok": True,
                "status": resp.status,
                "elapsed_ms": round((time.perf_counter() - started) * 1000, 1),
                "query": query,
                "url": url,
                "topics": _parse_search_xml(body),
                "cache_hit": False,
                "cache_path": str(path.resolve()),
            }
            CACHE_DIR.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
            return payload
    except Exception as exc:  # noqa: BLE001 - exploit report records failures.
        return {
            "ok": False,
            "status": None,
            "elapsed_ms": round((time.perf_counter() - started) * 1000, 1),
            "query": query,
            "topics": [],
            "cache_hit": False,
            "cache_path": str(path.resolve()),
            "error": f"{type(exc).__name__}: {exc}",
        }


@lru_cache(maxsize=1)
def explore_topics_by_disease() -> dict[str, list[dict[str, str]]]:
    payload = load_json(EXPLORE_MEDLINEPLUS_PATH)
    out: dict[str, list[dict[str, str]]] = {}
    for disease in payload.get("diseases", []):
        topics: list[dict[str, str]] = []
        for topic in disease.get("topics", []):
            row = dict(topic)
            row["source_native_id"] = source_native_id(row.get("url", ""))
            topics.append(row)
        out[disease["disease_key"]] = topics
    return out


def load_topics_for_disease(
    disease: dict[str, Any],
    *,
    allow_live: bool,
    refresh_cache: bool = False,
) -> dict[str, Any]:
    attempts: list[dict[str, Any]] = []
    seen_urls: set[str] = set()
    topics: list[dict[str, str]] = []
    for query in disease["source_queries"]:
        response = medlineplus_search(query, allow_live=allow_live, refresh_cache=refresh_cache)
        attempts.append(
            {
                "query": query,
                "ok": response["ok"],
                "status": response["status"],
                "elapsed_ms": response["elapsed_ms"],
                "topic_count": len(response.get("topics", [])),
                "cache_hit": response.get("cache_hit", False),
                "cache_path": response.get("cache_path"),
                "error": response.get("error"),
            }
        )
        for topic in response.get("topics", []):
            url = topic.get("url", "")
            if not url or url in seen_urls:
                continue
            seen_urls.add(url)
            topics.append(topic)

    source_mode = "live_or_cache"
    fallback_used = False
    if not topics:
        fallback_used = True
        source_mode = "explore_result_fixture"
        topics = explore_topics_by_disease().get(disease["key"], [])

    return {
        "attempts": attempts,
        "topics": topics,
        "topic_count": len(topics),
        "source_mode": source_mode,
        "fallback_used": fallback_used,
        "work_dir": str(CACHE_DIR),
    }


def direct_title_queries(disease: dict[str, Any]) -> set[str]:
    queries = set(disease["source_queries"])
    queries.add(disease["label"])
    normalized = {normalize_text(query) for query in queries}
    variants = set(normalized)
    for query in normalized:
        if query.endswith("s"):
            variants.add(query[:-1])
        else:
            variants.add(f"{query}s")
    return variants


def all_diseases() -> list[dict[str, Any]]:
    # Return a copy shallow enough for the spike scripts to annotate safely.
    return [dict(disease) for disease in DISEASES]
