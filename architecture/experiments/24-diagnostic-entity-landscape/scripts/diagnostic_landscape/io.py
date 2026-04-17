#!/usr/bin/env python3
from __future__ import annotations

import json
import re
import time
from pathlib import Path
from typing import Any

import requests

SCRIPT_DIR = Path(__file__).resolve().parent.parent
EXPERIMENT_ROOT = SCRIPT_DIR.parent
RESULTS_DIR = EXPERIMENT_ROOT / "results"
WORK_DIR = EXPERIMENT_ROOT / "work"

GENES = ["BRCA1", "EGFR", "BRAF", "KRAS", "TP53"]
DISEASES = ["breast cancer", "melanoma", "lung cancer"]

USER_AGENT = "biomcp-diagnostic-landscape-spike/0.1"
REQUEST_TIMEOUT = 120


def ensure_dirs() -> None:
    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    WORK_DIR.mkdir(parents=True, exist_ok=True)


def write_json_result(filename: str, payload: dict[str, Any]) -> Path:
    ensure_dirs()
    path = RESULTS_DIR / filename
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return path


def write_result(filename: str, payload: dict[str, Any]) -> Path:
    return write_json_result(filename, payload)


def load_json_result(filename: str) -> dict[str, Any]:
    path = RESULTS_DIR / filename
    return json.loads(path.read_text(encoding="utf-8"))


def load_json(filename: str) -> dict[str, Any]:
    return load_json_result(filename)


def download_file(url: str, filename: str, refresh: bool = False) -> Path:
    ensure_dirs()
    path = WORK_DIR / filename
    if path.exists() and not refresh:
        return path

    response = requests.get(
        url,
        headers={"User-Agent": USER_AGENT},
        stream=True,
        timeout=REQUEST_TIMEOUT,
    )
    response.raise_for_status()

    tmp_path = path.with_suffix(path.suffix + ".tmp")
    with tmp_path.open("wb") as handle:
        for chunk in response.iter_content(chunk_size=1024 * 1024):
            if chunk:
                handle.write(chunk)
    tmp_path.replace(path)
    return path


class RateLimiter:
    def __init__(self, min_interval_seconds: float) -> None:
        self.min_interval_seconds = min_interval_seconds
        self._last_request_started = 0.0

    def wait(self) -> None:
        now = time.perf_counter()
        elapsed = now - self._last_request_started
        if elapsed < self.min_interval_seconds:
            time.sleep(self.min_interval_seconds - elapsed)
        self._last_request_started = time.perf_counter()


def request_json(
    url: str,
    *,
    params: dict[str, Any] | None = None,
    allow_404: bool = False,
    rate_limiter: RateLimiter | None = None,
    retries: int = 4,
    timeout: int | float = REQUEST_TIMEOUT,
) -> tuple[dict[str, Any] | None, float, int]:
    last_error: Exception | None = None
    for attempt in range(retries):
        try:
            if rate_limiter is not None:
                rate_limiter.wait()
            started = time.perf_counter()
            response = requests.get(
                url,
                params=params,
                headers={"User-Agent": USER_AGENT},
                timeout=timeout,
            )
            latency_ms = round((time.perf_counter() - started) * 1000.0, 1)
        except requests.RequestException as exc:
            last_error = exc
            if attempt + 1 < retries:
                time.sleep(1.0 + attempt)
                continue
            raise

        if response.status_code == 404 and allow_404:
            return None, latency_ms, 404
        if response.status_code == 429 and attempt + 1 < retries:
            time.sleep(1.0 + attempt)
            continue

        response.raise_for_status()
        try:
            return response.json(), latency_ms, response.status_code
        except Exception as exc:  # pragma: no cover - defensive in spike script
            last_error = exc
            if attempt + 1 >= retries:
                raise
    if last_error is not None:  # pragma: no cover - defensive in spike script
        raise last_error
    raise RuntimeError("request_json exhausted retries without a response")


def normalize_text(value: str | None) -> str:
    if not value:
        return ""
    return re.sub(r"[^a-z0-9]+", " ", value.lower()).strip()


def contains_phrase(text: str | None, phrase: str) -> bool:
    normalized_text = normalize_text(text)
    normalized_phrase = normalize_text(phrase)
    return bool(normalized_text and normalized_phrase and normalized_phrase in normalized_text)


def matched_diseases(text: str | None) -> list[str]:
    return [disease for disease in DISEASES if contains_phrase(text, disease)]


def contains_gene_symbol(text: str | None, gene: str) -> bool:
    if not text:
        return False
    pattern = rf"(?<![A-Za-z0-9]){re.escape(gene)}(?![A-Za-z0-9])"
    return re.search(pattern, text) is not None


def split_pipe(value: str | None) -> list[str]:
    if not value:
        return []
    return [part.strip() for part in value.split("|") if part.strip()]


def dedupe_keep_order(values: list[str]) -> list[str]:
    seen: set[str] = set()
    out: list[str] = []
    for value in values:
        if value not in seen:
            seen.add(value)
            out.append(value)
    return out


def pct(count: int, total: int) -> float:
    if total == 0:
        return 0.0
    return round((count / total) * 100.0, 2)


def mean(values: list[int | float]) -> float:
    if not values:
        return 0.0
    return round(sum(values) / len(values), 2)


def top_counts(counter: dict[str, int], limit: int = 10) -> list[dict[str, Any]]:
    ranked = sorted(counter.items(), key=lambda item: (-item[1], item[0]))
    return [{"value": value, "count": count} for value, count in ranked[:limit]]
