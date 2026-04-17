#!/usr/bin/env python3
from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


@dataclass(slots=True)
class WhoTable:
    headers: list[str]
    rows: list[dict[str, Any]]


@dataclass(slots=True)
class WhoFinishedPharmaEntry:
    who_reference_number: str
    inn: str
    presentation: str
    dosage_form: str
    product_type: str
    therapeutic_area: str
    applicant: str
    listing_basis: str
    alternative_listing_basis: str | None
    prequalification_date: str | None
    normalized_inn: str | None
    normalized_presentation: str | None


@dataclass(slots=True)
class WhoFinishedPharmaTable(WhoTable):
    entries: list[WhoFinishedPharmaEntry] = field(default_factory=list)


@dataclass(slots=True)
class WhoDeviceCatalog:
    categories: dict[str, list[dict[str, Any]]]
    items: list[dict[str, Any]]


@dataclass(slots=True)
class WhoFullScaleArtifacts:
    probe_payloads: dict[str, dict[str, Any]]
    validation_payload: dict[str, Any]
    sample_records_payload: dict[str, Any]
    loader_design_payload: dict[str, Any]
    full_scale_payload: dict[str, Any]
    stage_timings: dict[str, float]
    contract_numbers: dict[str, Any]
    artifact_paths: dict[str, Path | str]
