"""Import surface for ticket 381 source-first article Markdown analysis."""

from __future__ import annotations

from .compare import compare, compare_rows, comparison_note
from .constants import COUNT_KEYS, QUALITY_KEYS, SOURCE_FAMILIES
from .io import load_json, write_csv, write_json
from .model import JsonObject, Row, Summary
from .rows import count_value, is_available, license_value, quality_bits, source_case_rows
from .summary import contract_numbers, summarize

__all__ = [
    "COUNT_KEYS",
    "QUALITY_KEYS",
    "SOURCE_FAMILIES",
    "JsonObject",
    "Row",
    "Summary",
    "compare",
    "compare_rows",
    "comparison_note",
    "contract_numbers",
    "count_value",
    "is_available",
    "license_value",
    "load_json",
    "quality_bits",
    "source_case_rows",
    "summarize",
    "write_csv",
    "write_json",
]
