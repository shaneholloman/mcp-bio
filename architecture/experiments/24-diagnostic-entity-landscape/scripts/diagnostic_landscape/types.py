#!/usr/bin/env python3
from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


@dataclass
class SourceBundle:
    records: list[dict[str, Any]]
    gene_to_records: dict[str, set[str]]
    disease_to_records: dict[str, set[str]] = field(default_factory=dict)
    metrics: dict[str, Any] = field(default_factory=dict)
    files: dict[str, str] = field(default_factory=dict)


@dataclass
class FullScaleLandscape:
    clinvar: dict[str, Any]
    clinvar_variant_sanity: dict[str, Any]
    gtr: SourceBundle
    who: SourceBundle
    fda: SourceBundle
    gene_source_matrix: dict[str, Any]
    validation_payload: dict[str, Any]
    payload: dict[str, Any]
    artifact_paths: dict[str, Path | str]
    started_at: float = 0.0
    elapsed_seconds: float = 0.0
