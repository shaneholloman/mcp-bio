#!/usr/bin/env python3
from __future__ import annotations

from typing import Any

from .identity import normalize_match_key
from .io import (
    WHO_API_URL,
    WHO_DEVICES_URL,
    WHO_FINISHED_URL,
    WHO_VACCINES_URL,
    clean_text,
    fetch_json_url,
    fetch_text,
    normalize_header,
    parse_csv_rows,
)
from .types import WhoDeviceCatalog, WhoFinishedPharmaEntry, WhoFinishedPharmaTable, WhoTable

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


def load_finished_pharma() -> WhoFinishedPharmaTable:
    headers, rows = parse_csv_rows(fetch_text(WHO_FINISHED_URL))
    entries: list[WhoFinishedPharmaEntry] = []
    for row in rows:
        normalized = {normalize_header(key): value for key, value in row.items()}
        if any(not clean_text(normalized.get(header)) for header in REQUIRED_FINISHED_HEADERS[:-2]):
            continue
        presentation = normalized["INN, DOSAGE FORM AND STRENGTH"]
        dosage_form = normalized["DOSAGE FORM"]
        entry = WhoFinishedPharmaEntry(
            who_reference_number=normalized["WHO REFERENCE NUMBER"],
            inn=derive_inn(presentation, dosage_form),
            presentation=presentation,
            dosage_form=dosage_form,
            product_type=normalized["PRODUCT TYPE"],
            therapeutic_area=normalized["THERAPEUTIC AREA"],
            applicant=normalized["APPLICANT"],
            listing_basis=normalized["BASIS OF LISTING"],
            alternative_listing_basis=clean_text(normalized.get("BASIS OF ALTERNATIVE LISTING", "")),
            prequalification_date=normalize_who_date(
                normalized.get("DATE OF PREQUALIFICATION", "")
            ),
            normalized_inn=None,
            normalized_presentation=None,
        )
        entry.normalized_inn = normalize_match_key(entry.inn)
        entry.normalized_presentation = normalize_match_key(entry.presentation)
        entries.append(entry)
    return WhoFinishedPharmaTable(headers=headers, rows=rows, entries=entries)


def load_vaccines() -> WhoTable:
    headers, rows = parse_csv_rows(fetch_text(WHO_VACCINES_URL))
    return WhoTable(headers=headers, rows=rows)


def load_apis() -> WhoTable:
    headers, rows = parse_csv_rows(fetch_text(WHO_API_URL))
    for row in rows:
        row["normalized_inn"] = normalize_match_key(row.get("INN"))
        row["prequalification_date_iso"] = normalize_who_date(row.get("Date of prequalification"))
        row["confirmation_document_date_iso"] = normalize_who_date(
            row.get("Confirmation of Prequalification Document Date")
        )
    return WhoTable(headers=headers, rows=rows)


def load_devices() -> WhoDeviceCatalog:
    payload = fetch_json_url(WHO_DEVICES_URL)
    categories: dict[str, list[dict[str, Any]]] = {}
    all_items: list[dict[str, Any]] = []
    for category, items in payload.items():
        if not isinstance(items, list):
            continue
        normalized_items = [item for item in items if isinstance(item, dict)]
        categories[category] = normalized_items
        all_items.extend(normalized_items)
    return WhoDeviceCatalog(categories=categories, items=all_items)


def schema_summary(headers: list[str], rows: list[dict[str, Any]]) -> dict[str, Any]:
    normalized_headers = [normalize_header(header) for header in headers]
    return {
        "row_count": len(rows),
        "headers": headers,
        "normalized_headers": normalized_headers,
    }


def device_schema_summary(devices: WhoDeviceCatalog) -> dict[str, Any]:
    key_counts: dict[str, int] = {}
    for item in devices.items:
        for key in item:
            key_counts[key] = key_counts.get(key, 0) + 1
    headers = sorted(key_counts)
    return {
        "category_count": len(devices.categories),
        "categories": {key: len(value) for key, value in devices.categories.items()},
        "item_count": len(devices.items),
        "headers": headers,
        "field_completeness": {
            key: round(count / len(devices.items) * 100, 2) if devices.items else 0.0
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
