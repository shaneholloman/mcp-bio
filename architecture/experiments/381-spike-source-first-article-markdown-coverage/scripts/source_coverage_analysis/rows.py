"""Normalize persisted probe JSON into stable source/case rows."""

from __future__ import annotations

from typing import Any

from .constants import COUNT_KEYS, QUALITY_KEYS, SOURCE_FAMILIES
from .model import JsonObject, Row


def is_available(source_name: str, source: dict[str, Any]) -> bool:
    if source_name == "europe_pmc_core_search":
        return bool(source.get("ok") and source.get("found"))
    if source_name == "pmc_oa_manifest":
        return bool(source.get("ok") and source.get("parse_ok") and source.get("record_found"))
    return bool(source.get("ok") and source.get("parse_ok", True) and source.get("bytes", 0) > 0)


def quality_bits(source: dict[str, Any]) -> list[str]:
    bits: list[str] = []
    for key in QUALITY_KEYS:
        value = source.get(key)
        if value is True or (isinstance(value, int) and value > 0):
            bits.append(key)
    return bits


def count_value(source: dict[str, Any], key: str) -> int:
    value = source.get(key)
    return value if isinstance(value, int) else 0


def license_value(source: dict[str, Any]) -> str:
    licenses = source.get("licenses")
    if isinstance(licenses, list) and licenses:
        return str(licenses[0])
    record_attrs = source.get("record_attrs")
    if isinstance(record_attrs, dict):
        return str(record_attrs.get("license") or record_attrs.get("license-type") or "")
    result = source.get("result")
    if isinstance(result, dict):
        return str(result.get("license") or "")
    return ""


def source_case_rows(data: JsonObject) -> list[Row]:
    rows: list[Row] = []
    rows_append = rows.append
    quality_keys = QUALITY_KEYS
    count_keys = COUNT_KEYS
    source_families = SOURCE_FAMILIES
    for article in data.get("articles", []):
        case = article["input"]["slug"]
        ids = article.get("resolved_ids", {})
        pmid = ids.get("pmid") or ""
        pmcid = ids.get("pmcid") or ""
        doi = ids.get("doi") or ""
        for source_name, source in sorted(article.get("sources", {}).items()):
            source_get = source.get
            quality = []
            quality_append = quality.append
            for key in quality_keys:
                value = source_get(key)
                if value is True or (isinstance(value, int) and value > 0):
                    quality_append(key)
            licenses = source_get("licenses")
            if isinstance(licenses, list) and licenses:
                license_text = str(licenses[0])
            else:
                record_attrs = source_get("record_attrs")
                if isinstance(record_attrs, dict):
                    license_text = str(record_attrs.get("license") or record_attrs.get("license-type") or "")
                else:
                    result = source_get("result")
                    license_text = str(result.get("license") or "") if isinstance(result, dict) else ""
            ok = bool(source_get("ok"))
            status = source_get("status")
            if source_name == "europe_pmc_core_search":
                available = bool(ok and source_get("found"))
            elif source_name == "pmc_oa_manifest":
                available = bool(ok and source_get("parse_ok") and source_get("record_found"))
            else:
                available = bool(ok and source_get("parse_ok", True) and source_get("bytes", 0) > 0)
            row = {
                "case": case,
                "pmid": pmid,
                "pmcid": pmcid,
                "doi": doi,
                "source": source_name,
                "family": source_families.get(source_name, source_name),
                "available": available,
                "ok": ok,
                "status": status if status is not None else "",
                "parse_ok": bool(source_get("parse_ok", source_get("found", False))),
                "elapsed_ms": source_get("elapsed_ms") if source_get("elapsed_ms") is not None else "",
                "bytes": source_get("bytes") if source_get("bytes") is not None else 0,
                "quality_bits": ";".join(quality),
                "license": license_text,
                "failure": source_get("error") or ("" if ok else str(status or "")),
            }
            for key in count_keys:
                value = source_get(key)
                row[key] = value if isinstance(value, int) else 0
            rows_append(row)
    s2 = data.get("s2orc_dataset_fit") or {}
    if s2:
        s2_get = s2.get
        row = {
            "case": "s2orc_dataset_fit",
            "pmid": "",
            "pmcid": "",
            "doi": "",
            "source": "semantic_scholar_s2orc_dataset",
            "family": "semantic_scholar_s2orc_dataset",
            "available": bool(s2_get("ok") and s2_get("parse_ok") and s2_get("matching_datasets")),
            "ok": bool(s2_get("ok")),
            "status": s2_get("status") if s2_get("status") is not None else "",
            "parse_ok": bool(s2_get("parse_ok")),
            "elapsed_ms": s2_get("elapsed_ms") if s2_get("elapsed_ms") is not None else "",
            "bytes": s2_get("bytes") if s2_get("bytes") is not None else 0,
            "quality_bits": "dataset_metadata;bulk_json;license_context",
            "license": "ODC-BY dataset; article-level rights still apply",
            "failure": s2_get("error") or "",
        }
        for key in count_keys:
            row[key] = 0
        rows_append(row)
    return rows
