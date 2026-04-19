from __future__ import annotations

import math
import resource
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .io import DEFAULT_OUTPUT, DEFAULT_PROFILE, DEFAULT_REGISTRY


@dataclass(slots=True)
class PipelineConfig:
    registry: Path = DEFAULT_REGISTRY
    profile: Path = DEFAULT_PROFILE
    output: Path = DEFAULT_OUTPUT
    label: str = "full-scale"
    replay_discovery: Path | None = None
    baseline_discovery: Path | None = None
    baseline_extraction: Path | None = None
    baseline_entities: Path | None = None
    per_source: int = 12
    max_articles: int = 60
    max_entity_articles: int = 30
    pivot_limit: int = 6
    conservative_ranking: bool = False


def latency_summary(values: list[int]) -> dict[str, Any]:
    if not values:
        return {"count": 0}
    ordered = sorted(values)

    def percentile(p: float) -> int:
        if len(ordered) == 1:
            return ordered[0]
        index = math.ceil((p / 100.0) * len(ordered)) - 1
        return ordered[max(0, min(index, len(ordered) - 1))]

    return {
        "count": len(ordered),
        "min": ordered[0],
        "p50": percentile(50),
        "p95": percentile(95),
        "max": ordered[-1],
        "mean": round(sum(ordered) / len(ordered), 2),
    }


class Bench:
    def __init__(self) -> None:
        self.started = time.perf_counter()
        self.stage_started: dict[str, float] = {}
        self.stages: dict[str, dict[str, float]] = {}
        self.request_latencies_ms: list[int] = []
        self.stage_counts: dict[str, int] = {}

    def start(self, name: str) -> None:
        self.stage_started[name] = time.perf_counter()

    def stop(self, name: str, count: int | None = None) -> None:
        started = self.stage_started.pop(name, time.perf_counter())
        elapsed = time.perf_counter() - started
        entry: dict[str, float] = {"seconds": round(elapsed, 4)}
        if count is not None:
            self.stage_counts[name] = count
            entry["count"] = count
            entry["throughput_per_second"] = round(count / elapsed, 4) if elapsed > 0 else 0.0
        self.stages[name] = entry

    def observe_fetch(self, fetch_meta: dict[str, Any]) -> None:
        elapsed = fetch_meta.get("elapsed_ms")
        if isinstance(elapsed, int):
            self.request_latencies_ms.append(elapsed)

    def snapshot(self) -> dict[str, Any]:
        total = time.perf_counter() - self.started
        rss_mb = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024.0
        return {
            "total_seconds": round(total, 4),
            "stages": self.stages,
            "http_latency_ms": latency_summary(self.request_latencies_ms),
            "peak_rss_mb": round(rss_mb, 2),
        }
