#!/usr/bin/env python3
from __future__ import annotations

from typing import Any


def build_unified_data_model() -> dict[str, Any]:
    return {
        "required_fields": [
            "source",
            "source_id",
            "name",
            "test_category",
            "manufacturer_or_lab",
            "genes",
            "conditions",
            "methods",
            "regulatory_status",
            "regulatory_identifier",
            "region",
        ],
        "source_specific_extensions": {
            "gtr": [
                "institution",
                "method_categories",
                "clia_number",
                "state_licenses",
                "country",
                "public_status",
            ],
            "who_ivd": [
                "marker",
                "regulatory_version",
                "prequalification_year",
            ],
            "fda_device": [
                "source_db",
                "trade_name",
                "device_name",
                "generic_name",
                "decision_date",
                "product_code",
                "advisory_committee",
                "matched_queries",
                "supplement_count",
            ],
        },
    }


def build_cli_surface() -> list[dict[str, str]]:
    return [
        {
            "command": 'biomcp search diagnostic --gene BRCA1',
            "purpose": "Gene to test pivot backed by GTR, with FDA and WHO overlays attached when present.",
        },
        {
            "command": 'biomcp search diagnostic --disease "breast cancer"',
            "purpose": "Condition to test pivot using GTR conditions first, then overlay regulatory records.",
        },
        {
            "command": "biomcp get diagnostic GTR000603548.1",
            "purpose": "Fetch a single GTR-backed diagnostic card with source-native provenance and metadata.",
        },
        {
            "command": "biomcp get diagnostic P170019 --source fda",
            "purpose": "Fetch a specific FDA PMA or 510(k) regulatory overlay record by identifier.",
        },
        {
            "command": "biomcp search diagnostic --gene BRCA1 --region us --regulatory",
            "purpose": "Prefer U.S. records and surface FDA overlays alongside the GTR backbone.",
        },
    ]


def build_source_priority() -> list[dict[str, str]]:
    return [
        {
            "source": "gtr",
            "priority": "1",
            "why": "Only source with dense gene and disease links at both explore and full scale.",
        },
        {
            "source": "fda_device",
            "priority": "2",
            "why": "Best regulatory overlay for cleared and approved U.S. diagnostics, including BRCA1 validation targets.",
        },
        {
            "source": "who_ivd",
            "priority": "3",
            "why": "Useful regulatory overlay but negligible ClinVar-gene coverage in the measured landscape.",
        },
    ]


def build_rust_module_boundaries() -> list[dict[str, str]]:
    return [
        {
            "module": "src/entities/diagnostic/mod.rs",
            "responsibility": "Entity facade, search orchestration, and regional composition.",
        },
        {
            "module": "src/entities/diagnostic/model.rs",
            "responsibility": "Public `Diagnostic`, `DiagnosticRegion`, and source-specific typed sections.",
        },
        {
            "module": "src/entities/diagnostic/bridge.rs",
            "responsibility": "Alias-aware joins between GTR records and FDA/WHO overlays.",
        },
        {
            "module": "src/sources/gtr.rs",
            "responsibility": "Bulk sync, file validation, and GTR-specific parsing for the backbone dataset.",
        },
        {
            "module": "src/sources/fda_device.rs",
            "responsibility": "PMA plus 510(k) sync and normalization for U.S. regulatory overlays.",
        },
        {
            "module": "src/sources/who_ivd.rs",
            "responsibility": "CSV-backed WHO IVD sync and normalization for WHO overlays.",
        },
    ]
