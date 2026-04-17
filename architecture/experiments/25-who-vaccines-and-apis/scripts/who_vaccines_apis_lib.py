#!/usr/bin/env python3
from __future__ import annotations

import csv
import hashlib
import io
import json
import os
import re
import time
from pathlib import Path
from typing import Any
from urllib.parse import urlencode
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

SCRIPT_DIR = Path(__file__).resolve().parent
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

REQUIRED_FINISHED_HEADERS = [
    "WHO REFERENCE NUMBER",
    "INN, DOSAGE FORM AND STRENGTH",
    "PRODUCT TYPE",
    "THERAPEUTIC AREA",
    "APPLICANT",
    "DOSAGE FORM",
    "BASIS OF LISTING",
    "BASIS OF ALTERNATIVE LISTING",
    "DATE OF PREQUALIFICATION",
]
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


def normalize_who_date(value: str | None) -> str | None:
    cleaned = clean_text(value)
    if cleaned is None:
        return None
    parts = cleaned.replace(",", " ").split()
    if len(parts) != 3:
        return None
    try:
        day = int(parts[0])
        year = int(parts[2])
    except ValueError:
        return None
    month_lookup = {
        "jan": 1,
        "feb": 2,
        "mar": 3,
        "apr": 4,
        "may": 5,
        "jun": 6,
        "jul": 7,
        "aug": 8,
        "sep": 9,
        "oct": 10,
        "nov": 11,
        "dec": 12,
    }
    month = month_lookup.get(parts[1].lower())
    if month is None:
        return None
    return f"{year:04d}-{month:02d}-{day:02d}"


def derive_inn(presentation: str, dosage_form: str) -> str:
    cleaned_presentation = clean_text(presentation) or ""
    cleaned_dosage = clean_text(dosage_form) or ""
    if not cleaned_presentation or not cleaned_dosage:
        return cleaned_presentation
    presentation_lower = cleaned_presentation.lower()
    dosage_lower = cleaned_dosage.lower()
    idx = presentation_lower.find(dosage_lower)
    if idx == -1:
        return cleaned_presentation
    prefix = cleaned_presentation[:idx].strip().rstrip(",+/;-").strip()
    return prefix or cleaned_presentation


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
    parts = [clean_text(part) for part in re.split(r"\s*/\s*", core) if clean_text(part)]
    return list(dict.fromkeys(part for part in parts if part))


def write_json(path: Path, payload: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def sample_rows(rows: list[dict[str, Any]], count: int = 3) -> list[dict[str, Any]]:
    return rows[:count]


def normalize_for_projection(payload: Any, drop_keys: frozenset[str] | None = None) -> Any:
    effective_drop_keys = DEFAULT_PROJECTION_DROP_KEYS if drop_keys is None else drop_keys
    if isinstance(payload, dict):
        return {
            key: normalize_for_projection(value, drop_keys=effective_drop_keys)
            for key, value in sorted(payload.items())
            if key not in effective_drop_keys
        }
    if isinstance(payload, list):
        return [normalize_for_projection(item, drop_keys=effective_drop_keys) for item in payload]
    return payload


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


def load_finished_pharma() -> dict[str, Any]:
    headers, rows = parse_csv_rows(fetch_text(WHO_FINISHED_URL))
    entries = []
    for row in rows:
        normalized = {normalize_header(key): value for key, value in row.items()}
        if any(not clean_text(normalized.get(header)) for header in REQUIRED_FINISHED_HEADERS[:-2]):
            continue
        presentation = normalized["INN, DOSAGE FORM AND STRENGTH"]
        dosage_form = normalized["DOSAGE FORM"]
        entry = {
            "who_reference_number": normalized["WHO REFERENCE NUMBER"],
            "inn": derive_inn(presentation, dosage_form),
            "presentation": presentation,
            "dosage_form": dosage_form,
            "product_type": normalized["PRODUCT TYPE"],
            "therapeutic_area": normalized["THERAPEUTIC AREA"],
            "applicant": normalized["APPLICANT"],
            "listing_basis": normalized["BASIS OF LISTING"],
            "alternative_listing_basis": clean_text(
                normalized.get("BASIS OF ALTERNATIVE LISTING", "")
            ),
            "prequalification_date": normalize_who_date(
                normalized.get("DATE OF PREQUALIFICATION", "")
            ),
        }
        entry["normalized_inn"] = normalize_match_key(entry["inn"])
        entry["normalized_presentation"] = normalize_match_key(entry["presentation"])
        entries.append(entry)
    return {"headers": headers, "rows": rows, "entries": entries}


def load_vaccines() -> dict[str, Any]:
    headers, rows = parse_csv_rows(fetch_text(WHO_VACCINES_URL))
    return {"headers": headers, "rows": rows}


def load_apis() -> dict[str, Any]:
    headers, rows = parse_csv_rows(fetch_text(WHO_API_URL))
    for row in rows:
        row["normalized_inn"] = normalize_match_key(row.get("INN"))
        row["prequalification_date_iso"] = normalize_who_date(row.get("Date of prequalification"))
        row["confirmation_document_date_iso"] = normalize_who_date(
            row.get("Confirmation of Prequalification Document Date")
        )
    return {"headers": headers, "rows": rows}


def load_devices() -> dict[str, Any]:
    payload = fetch_json_url(WHO_DEVICES_URL)
    categories: dict[str, list[dict[str, Any]]] = {}
    all_items: list[dict[str, Any]] = []
    for category, items in payload.items():
        if not isinstance(items, list):
            continue
        normalized_items = [item for item in items if isinstance(item, dict)]
        categories[category] = normalized_items
        all_items.extend(normalized_items)
    return {"categories": categories, "items": all_items}


def schema_summary(headers: list[str], rows: list[dict[str, Any]]) -> dict[str, Any]:
    normalized_headers = [normalize_header(header) for header in headers]
    return {
        "row_count": len(rows),
        "headers": headers,
        "normalized_headers": normalized_headers,
    }


def device_schema_summary(devices: dict[str, Any]) -> dict[str, Any]:
    key_counts: dict[str, int] = {}
    for item in devices["items"]:
        for key in item:
            key_counts[key] = key_counts.get(key, 0) + 1
    headers = sorted(key_counts)
    return {
        "category_count": len(devices["categories"]),
        "categories": {key: len(value) for key, value in devices["categories"].items()},
        "item_count": len(devices["items"]),
        "headers": headers,
        "field_completeness": {
            key: round(count / len(devices["items"]) * 100, 2) if devices["items"] else 0.0
            for key, count in sorted(key_counts.items())
        },
    }


def schema_overlap(base_headers: list[str], other_headers: list[str]) -> dict[str, Any]:
    base = {normalize_header(header) for header in base_headers}
    other = {normalize_header(header) for header in other_headers}
    shared = sorted(base & other)
    return {
        "shared": shared,
        "shared_count": len(shared),
        "base_only": sorted(base - other),
        "other_only": sorted(other - base),
    }


def unique_values(rows: list[dict[str, str]], key: str) -> list[str]:
    values = [clean_text(row.get(key)) for row in rows]
    return sorted({value for value in values if value})


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


def count_presence(rows: list[dict[str, Any]], key: str) -> dict[str, Any]:
    total = len(rows)
    present = sum(1 for row in rows if clean_text(row.get(key)))
    return {
        "present": present,
        "total": total,
        "percent": round((present / total) * 100, 2) if total else 0.0,
    }


def product_mentions(rows: list[dict[str, Any]], terms: list[str], fields: list[str]) -> dict[str, Any]:
    results: dict[str, Any] = {}
    for term in terms:
        lowered = term.lower()
        matches = []
        for row in rows:
            haystack = " | ".join(row.get(field, "") for field in fields).lower()
            if lowered in haystack:
                matches.append(row)
        results[term] = {
            "count": len(matches),
            "samples": sample_rows(matches, 2),
        }
    return results
