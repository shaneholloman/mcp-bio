"""Regression comparison helpers for ticket 381 source-coverage rows."""

from __future__ import annotations

from .constants import COUNT_KEYS
from .model import JsonObject, Row
from .rows import source_case_rows


def compare(baseline: JsonObject, current: JsonObject) -> list[Row]:
    return compare_rows(source_case_rows(baseline), source_case_rows(current))


def compare_rows(baseline_rows: list[Row], current_rows: list[Row]) -> list[Row]:
    base_rows = {(r["case"], r["source"]): r for r in baseline_rows}
    cur_rows = {(r["case"], r["source"]): r for r in current_rows}
    count_keys = COUNT_KEYS
    out: list[Row] = []
    out_append = out.append
    for key in sorted(set(base_rows) | set(cur_rows)):
        b = base_rows.get(key)
        c = cur_rows.get(key)
        b_avail = bool(b and b["available"])
        c_avail = bool(c and c["available"])
        b_elapsed = int(b.get("elapsed_ms") or 0) if b else 0
        c_elapsed = int(c.get("elapsed_ms") or 0) if c else 0
        latency_ratio = (c_elapsed / b_elapsed) if b_elapsed else None
        count_regressions = []
        count_regressions_append = count_regressions.append
        if b and c:
            for count_key in count_keys:
                if int(c.get(count_key) or 0) < int(b.get(count_key) or 0):
                    count_regressions_append(count_key)
        out_append(
            {
                "case": key[0],
                "source": key[1],
                "baseline_available": b_avail,
                "current_available": c_avail,
                "coverage_delta": int(c_avail) - int(b_avail),
                "baseline_status": b.get("status", "missing") if b else "missing",
                "current_status": c.get("status", "missing") if c else "missing",
                "baseline_elapsed_ms": b_elapsed,
                "current_elapsed_ms": c_elapsed,
                "latency_ratio": round(latency_ratio, 3) if latency_ratio is not None else "",
                "quality_count_regressions": ";".join(count_regressions),
                "baseline_quality_bits": b.get("quality_bits", "") if b else "",
                "current_quality_bits": c.get("quality_bits", "") if c else "",
                "note": comparison_note(b_avail, c_avail, count_regressions, latency_ratio),
            }
        )
    return out


def comparison_note(b_avail: bool, c_avail: bool, count_regressions: list[str], latency_ratio: float | None) -> str:
    notes = []
    if c_avail and not b_avail:
        notes.append("coverage_improved")
    elif b_avail and not c_avail:
        notes.append("coverage_regressed")
    if count_regressions:
        notes.append("count_regressed")
    if latency_ratio is not None and latency_ratio > 1.03:
        notes.append("live_latency_slower_than_3pct")
    return ";".join(notes) or "pass"
