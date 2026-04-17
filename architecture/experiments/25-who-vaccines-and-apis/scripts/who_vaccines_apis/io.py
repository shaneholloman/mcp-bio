#!/usr/bin/env python3
from __future__ import annotations

import csv
import hashlib
import io
import json
import os
import time
from dataclasses import asdict, is_dataclass
from pathlib import Path
from typing import Any
from urllib.request import Request, urlopen

WHO_FINISHED_URL = (
    "https://extranet.who.int/prequal/medicines/prequalified/"
    "finished-pharmaceutical-products/export?page&_format=csv"
)
WHO_VACCINES_URL = "https://extranet.who.int/prequal/vaccines/prequalified/export"
WHO_API_URL = (
    "https://extranet.who.int/prequal/medicines/prequalified/"
    "active-pharmaceutical-ingredients/export?page&_format=csv"
)
WHO_DEVICES_URL = (
    "https://extranet.who.int/prequal/sites/default/files/immunization_devices/"
    "json/catalogs/immunization_devices_catalogue.json"
)
MYCHEM_BASE = os.environ.get("BIOMCP_MYCHEM_BASE", "https://mychem.info/v1").rstrip("/")
MYCHEM_FIELDS = ",".join(
    [
        "_id",
        "_score",
        "drugbank.id",
        "drugbank.name",
        "drugbank.synonyms",
        "chembl.pref_name",
        "chebi.name",
        "unii.unii",
        "unii.display_name",
        "unii.substance_type",
        "openfda.generic_name",
        "openfda.brand_name",
    ]
)
USER_AGENT = "biomcp-who-vaccines-apis-spike/0.1"

SCRIPT_DIR = Path(__file__).resolve().parent.parent
EXPERIMENT_ROOT = SCRIPT_DIR.parent
RESULTS_DIR = EXPERIMENT_ROOT / "results"
WORK_DIR = EXPERIMENT_ROOT / "work"
UPSTREAM_CACHE_DIR = WORK_DIR / "upstream"
MYCHEM_CACHE_PATH = WORK_DIR / "mychem_query_cache.json"
DEFAULT_PROJECTION_DROP_KEYS = frozenset({"generated_at"})

SOURCE_CACHE_FILES = {
    WHO_FINISHED_URL: UPSTREAM_CACHE_DIR / "who_finished_pharma.csv",
    WHO_VACCINES_URL: UPSTREAM_CACHE_DIR / "who_vaccines.csv",
    WHO_API_URL: UPSTREAM_CACHE_DIR / "who_apis.csv",
    WHO_DEVICES_URL: UPSTREAM_CACHE_DIR / "who_immunization_devices.json",
}

_TEXT_CACHE: dict[str, str] = {}
_JSON_CACHE: dict[str, Any] = {}


def iso_now() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


def ensure_work_dirs() -> None:
    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    WORK_DIR.mkdir(parents=True, exist_ok=True)
    UPSTREAM_CACHE_DIR.mkdir(parents=True, exist_ok=True)


def round_seconds(value: float) -> float:
    return round(value, 4)


def clean_text(value: str | None) -> str | None:
    if value is None:
        return None
    normalized = " ".join(str(value).split())
    return normalized or None


def normalize_header(value: str) -> str:
    return " ".join(value.strip("\ufeff").strip().upper().split())


def header_key(value: str) -> str:
    return normalize_header(value).lower().replace(" ", "_")


def cache_path_for_url(url: str) -> Path | None:
    return SOURCE_CACHE_FILES.get(url)


def sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def to_plain_data(payload: Any) -> Any:
    if is_dataclass(payload):
        return to_plain_data(asdict(payload))
    if isinstance(payload, dict):
        return {key: to_plain_data(value) for key, value in payload.items()}
    if isinstance(payload, list):
        return [to_plain_data(item) for item in payload]
    return payload


def normalize_for_projection(payload: Any, drop_keys: frozenset[str] | None = None) -> Any:
    effective_drop_keys = DEFAULT_PROJECTION_DROP_KEYS if drop_keys is None else drop_keys
    payload = to_plain_data(payload)
    if isinstance(payload, dict):
        return {
            key: normalize_for_projection(value, drop_keys=effective_drop_keys)
            for key, value in sorted(payload.items())
            if key not in effective_drop_keys
        }
    if isinstance(payload, list):
        return [normalize_for_projection(item, drop_keys=effective_drop_keys) for item in payload]
    return payload


def sha256_json(payload: Any, drop_keys: frozenset[str] | None = None) -> str:
    normalized = normalize_for_projection(payload, drop_keys=drop_keys)
    encoded = json.dumps(normalized, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return sha256_bytes(encoded)


def source_snapshot(url: str) -> dict[str, Any]:
    cache_path = cache_path_for_url(url)
    if cache_path is None or not cache_path.is_file():
        return {"url": url, "cached": False}
    payload = cache_path.read_bytes()
    return {
        "url": url,
        "cached": True,
        "cache_path": str(cache_path.resolve()),
        "bytes": len(payload),
        "sha256": sha256_bytes(payload),
        "modified_at": time.strftime(
            "%Y-%m-%dT%H:%M:%SZ", time.gmtime(cache_path.stat().st_mtime)
        ),
    }


def fetch_bytes(url: str) -> bytes:
    ensure_work_dirs()
    cache_path = cache_path_for_url(url)
    if cache_path is not None and cache_path.is_file():
        return cache_path.read_bytes()
    request = Request(url, headers={"User-Agent": USER_AGENT})
    with urlopen(request, timeout=120) as response:
        payload = response.read()
    if cache_path is not None:
        cache_path.parent.mkdir(parents=True, exist_ok=True)
        cache_path.write_bytes(payload)
    return payload


def fetch_text(url: str) -> str:
    cached = _TEXT_CACHE.get(url)
    if cached is not None:
        return cached
    text = fetch_bytes(url).decode("utf-8-sig", errors="replace")
    _TEXT_CACHE[url] = text
    return text


def fetch_json_url(url: str) -> Any:
    cached = _JSON_CACHE.get(url)
    if cached is not None:
        return cached
    payload = json.loads(fetch_bytes(url).decode("utf-8", errors="replace"))
    _JSON_CACHE[url] = payload
    return payload


def parse_csv_rows(text: str) -> tuple[list[str], list[dict[str, str]]]:
    reader = csv.reader(io.StringIO(text))
    try:
        raw_headers = next(reader)
    except StopIteration:
        return [], []
    headers = [header.strip() for header in raw_headers]
    rows: list[dict[str, str]] = []
    width = len(headers)
    for raw_row in reader:
        if not any(cell.strip() for cell in raw_row):
            continue
        padded = list(raw_row[:width]) + [""] * max(0, width - len(raw_row))
        rows.append({headers[idx]: padded[idx].strip() for idx in range(width)})
    return headers, rows


def write_json(path: Path, payload: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(to_plain_data(payload), indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def sample_rows(rows: list[Any], count: int = 3) -> list[Any]:
    return [to_plain_data(row) for row in rows[:count]]


def diff_payloads(
    baseline: Any,
    current: Any,
    *,
    drop_keys: frozenset[str] | None = None,
    path: str = "",
) -> list[dict[str, Any]]:
    baseline = normalize_for_projection(baseline, drop_keys=drop_keys)
    current = normalize_for_projection(current, drop_keys=drop_keys)
    if type(baseline) is not type(current):
        return [{"path": path or "$", "baseline": baseline, "current": current}]
    if isinstance(baseline, dict):
        out: list[dict[str, Any]] = []
        keys = sorted(set(baseline) | set(current))
        for key in keys:
            child_path = f"{path}.{key}" if path else key
            if key not in baseline or key not in current:
                out.append(
                    {
                        "path": child_path,
                        "baseline": baseline.get(key),
                        "current": current.get(key),
                    }
                )
                continue
            out.extend(diff_payloads(baseline[key], current[key], drop_keys=drop_keys, path=child_path))
        return out
    if isinstance(baseline, list):
        out: list[dict[str, Any]] = []
        if len(baseline) != len(current):
            out.append(
                {
                    "path": path or "$",
                    "baseline_length": len(baseline),
                    "current_length": len(current),
                }
            )
        for idx, (baseline_item, current_item) in enumerate(zip(baseline, current, strict=False)):
            out.extend(
                diff_payloads(
                    baseline_item,
                    current_item,
                    drop_keys=drop_keys,
                    path=f"{path}[{idx}]" if path else f"[{idx}]",
                )
            )
        return out
    if baseline != current:
        return [{"path": path or "$", "baseline": baseline, "current": current}]
    return []


def time_call(fn, *args, **kwargs) -> tuple[Any, float]:
    started = time.perf_counter()
    result = fn(*args, **kwargs)
    elapsed = time.perf_counter() - started
    return result, round_seconds(elapsed)
