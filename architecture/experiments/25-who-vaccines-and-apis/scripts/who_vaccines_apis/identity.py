#!/usr/bin/env python3
from __future__ import annotations

import json
import re
import time
from typing import Any
from urllib.parse import urlencode
from urllib.request import Request, urlopen

from .io import (
    MYCHEM_BASE,
    MYCHEM_CACHE_PATH,
    MYCHEM_FIELDS,
    USER_AGENT,
    clean_text,
    ensure_work_dirs,
)

SALT_SUFFIXES = (
    "acetate",
    "besylate",
    "diphosphate",
    "hydrochloride",
    "maleate",
    "mesylate",
    "phosphate",
    "sodium",
    "sulfate",
)
PARENTHETICAL_SALT_RE = re.compile(
    r"\((acetate|besylate|diphosphate|hydrochloride|maleate|mesylate|phosphate|sodium|sulfate)\)"
)
PUNCTUATION_RE = re.compile(r"[,;]")
SLASH_PLUS_RE = re.compile(r"\s*(/|\+| and )\s*")
PARENTHETICAL_RE = re.compile(r"\([^)]*\)")
LUCENE_SPECIAL_RE = re.compile(r"[+\-=&|><!(){}\[\]^\"~*?:\\/]")


def normalize_match_segment(value: str | None) -> str | None:
    cleaned = clean_text(value)
    if cleaned is None:
        return None
    lowered = cleaned.lower()
    for suffix in SALT_SUFFIXES:
        if lowered == suffix:
            return None
        trailer = f" {suffix}"
        if lowered.endswith(trailer):
            lowered = lowered[: -len(trailer)].strip()
    normalized = clean_text(lowered)
    return normalized


def normalize_match_key(value: str | None) -> str | None:
    cleaned = clean_text(value)
    if cleaned is None:
        return None
    lowered = cleaned.lower()
    lowered = PARENTHETICAL_SALT_RE.sub(" ", lowered)
    lowered = PUNCTUATION_RE.sub(" ", lowered)
    lowered = SLASH_PLUS_RE.sub(" + ", lowered)
    parts = [normalize_match_segment(part) for part in lowered.split(" + ")]
    normalized = [part for part in parts if part]
    return " + ".join(normalized) if normalized else None


def contains_boundary_phrase(field: str, term: str) -> bool:
    if not field or not term:
        return False
    search_from = 0
    while True:
        idx = field.find(term, search_from)
        if idx == -1:
            return False
        end = idx + len(term)
        before_ok = idx == 0 or not field[idx - 1].isalnum()
        after_ok = end == len(field) or not field[end].isalnum()
        if before_ok and after_ok:
            return True
        search_from = idx + 1


def split_normalized_segments(value: str | None) -> list[str]:
    normalized = normalize_match_key(value)
    if normalized is None:
        return []
    return [segment for segment in normalized.split(" + ") if segment]


def strip_parentheticals(value: str | None) -> str:
    cleaned = clean_text(value) or ""
    return clean_text(PARENTHETICAL_RE.sub(" ", cleaned)) or cleaned


def sanitize_mychem_query(value: str | None) -> str | None:
    cleaned = clean_text(value)
    if cleaned is None:
        return None
    sanitized = clean_text(LUCENE_SPECIAL_RE.sub(" ", cleaned))
    return sanitized or cleaned


def split_vaccine_components(value: str | None) -> list[str]:
    core = strip_parentheticals(value)
    if not core:
        return []
    parts = [
        clean_text(part)
        for part in re.split(r"\s*(?:/|,| and )\s*|(?<=[A-Za-z])-(?=[A-Za-z])", core)
        if clean_text(part)
    ]
    return list(dict.fromkeys(part for part in parts if part))


def extract_hit_names(hit: dict[str, Any]) -> list[str]:
    seen: set[str] = set()
    out: list[str] = []

    def push(value: Any) -> None:
        if isinstance(value, str):
            cleaned = clean_text(value)
            if cleaned and cleaned not in seen:
                seen.add(cleaned)
                out.append(cleaned)
        elif isinstance(value, list):
            for item in value:
                push(item)
        elif isinstance(value, dict):
            for field in (
                "name",
                "pref_name",
                "display_name",
                "generic_name",
                "brand_name",
                "synonyms",
            ):
                if field in value:
                    push(value[field])

    push(hit.get("drugbank"))
    push(hit.get("chembl"))
    push(hit.get("chebi"))
    push(hit.get("unii"))
    push(hit.get("openfda"))
    return out


def primary_hit_name(hit: dict[str, Any]) -> str | None:
    for value in (
        hit.get("drugbank", {}).get("name") if isinstance(hit.get("drugbank"), dict) else None,
        hit.get("chembl", {}).get("pref_name") if isinstance(hit.get("chembl"), dict) else None,
        hit.get("unii", {}).get("display_name") if isinstance(hit.get("unii"), dict) else None,
        hit.get("chebi", {}).get("name") if isinstance(hit.get("chebi"), dict) else None,
    ):
        cleaned = clean_text(value)
        if cleaned:
            return cleaned
    names = extract_hit_names(hit)
    return names[0] if names else None


def summarize_hit(hit: dict[str, Any]) -> dict[str, Any]:
    drugbank = hit.get("drugbank") if isinstance(hit.get("drugbank"), dict) else {}
    unii = hit.get("unii") if isinstance(hit.get("unii"), dict) else {}
    return {
        "score": hit.get("_score"),
        "primary_name": primary_hit_name(hit),
        "alias_sample": extract_hit_names(hit)[:5],
        "drugbank_id": drugbank.get("id"),
        "unii": unii.get("unii"),
        "substance_type": unii.get("substance_type"),
    }


def classify_hits(term: str, hits: list[dict[str, Any]]) -> dict[str, Any]:
    normalized_term = normalize_match_key(term)
    exact_match = False
    phrase_match = False
    matched_hit: dict[str, Any] | None = None
    for hit in hits:
        names = extract_hit_names(hit)
        normalized_names = [normalize_match_key(name) for name in names]
        normalized_names = [name for name in normalized_names if name]
        if normalized_term and any(name == normalized_term for name in normalized_names):
            exact_match = True
            matched_hit = hit
            break
        if normalized_term and any(
            contains_boundary_phrase(name, normalized_term)
            or contains_boundary_phrase(normalized_term, name)
            for name in normalized_names
        ):
            phrase_match = True
            if matched_hit is None:
                matched_hit = hit
    return {
        "query": term,
        "normalized_query": normalized_term,
        "total_hits": len(hits),
        "exact_match": exact_match,
        "phrase_match": exact_match or phrase_match,
        "best_hit": summarize_hit(matched_hit) if matched_hit else None,
        "top_hits": [summarize_hit(hit) for hit in hits[:3]],
    }


class MyChemResolver:
    def __init__(self, min_interval_seconds: float = 0.03) -> None:
        ensure_work_dirs()
        self.cache: dict[str, dict[str, Any]] = {}
        self.min_interval_seconds = min_interval_seconds
        self._last_request = 0.0
        self._dirty = False
        if MYCHEM_CACHE_PATH.is_file():
            cached = json.loads(MYCHEM_CACHE_PATH.read_text(encoding="utf-8"))
            if isinstance(cached, dict):
                self.cache = {str(key): value for key, value in cached.items() if isinstance(value, dict)}

    @staticmethod
    def _cache_key(query: str, size: int) -> str:
        return f"{size}\t{query}"

    def flush(self) -> None:
        if not self._dirty:
            return
        MYCHEM_CACHE_PATH.parent.mkdir(parents=True, exist_ok=True)
        MYCHEM_CACHE_PATH.write_text(
            json.dumps(self.cache, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        self._dirty = False

    def search(self, term: str, size: int = 10) -> dict[str, Any]:
        cleaned = clean_text(term)
        if not cleaned:
            return {"hits": [], "total": 0, "query": term}
        effective_query = sanitize_mychem_query(cleaned) or cleaned
        cache_key = self._cache_key(effective_query, size)
        cached = self.cache.get(cache_key)
        if cached is not None:
            return cached
        wait_for = self.min_interval_seconds - (time.monotonic() - self._last_request)
        if wait_for > 0:
            time.sleep(wait_for)
        params = urlencode({"q": effective_query, "size": size, "fields": MYCHEM_FIELDS})
        request = Request(
            f"{MYCHEM_BASE}/query?{params}",
            headers={"User-Agent": USER_AGENT, "Accept": "application/json"},
        )
        last_error: Exception | None = None
        for attempt in range(3):
            try:
                self._last_request = time.monotonic()
                with urlopen(request, timeout=120) as response:
                    payload = json.loads(response.read().decode("utf-8"))
                result = {
                    "query": cleaned,
                    "effective_query": effective_query,
                    "total": payload.get("total", 0),
                    "hits": payload.get("hits", []),
                }
                self.cache[cache_key] = result
                self._dirty = True
                return result
            except Exception as exc:  # pragma: no cover - spike retry path
                last_error = exc
                time.sleep(0.4 * (attempt + 1))
        raise RuntimeError(f"MyChem request failed for {effective_query!r}: {last_error}") from last_error
