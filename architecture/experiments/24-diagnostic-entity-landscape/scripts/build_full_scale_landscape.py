#!/usr/bin/env python3
from __future__ import annotations

import time
from pathlib import Path
from typing import Any

from common import RESULTS_DIR
from diagnostic_landscape_lib import (
    build_gene_source_matrix,
    find_named_records,
    load_clinvar_gene_summary,
    load_clinvar_variant_sanity,
    load_fda_molecular_slice,
    load_gtr_backbone,
    load_who_overlay,
    select_sample,
    top_gene_counts,
    top_uncovered_genes,
    write_result,
)

EXPERIMENT_ROOT = Path(__file__).resolve().parent.parent


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


def main() -> None:
    started = time.perf_counter()

    clinvar = load_clinvar_gene_summary()
    clinvar_variant_sanity = load_clinvar_variant_sanity()
    gene_universe = {row["symbol"] for row in clinvar["genes"]}

    gtr = load_gtr_backbone()
    who = load_who_overlay(gene_universe)
    fda = load_fda_molecular_slice(gene_universe)

    gene_matrix = build_gene_source_matrix(
        clinvar["genes"],
        gtr["gene_to_tests"],
        who["gene_to_records"],
        fda["gene_to_records"],
    )
    omim_rows = [row for row in gene_matrix["rows"] if row["omim_gene_mim_number"] > 0]
    omim_covered = sum(1 for row in omim_rows if row["covered_by_any_source"])
    loc_like_rows = [
        row
        for row in gene_matrix["rows"]
        if row["symbol"].startswith("LOC")
        or "-AS" in row["symbol"]
        or "-" in row["symbol"]
    ]
    loc_like_covered = sum(1 for row in loc_like_rows if row["covered_by_any_source"])
    gtr_clinvar_covered = sum(1 for gene in gene_universe if gene in gtr["gene_to_tests"])
    who_clinvar_covered = sum(1 for gene in gene_universe if gene in who["gene_to_records"])
    fda_clinvar_covered = sum(1 for gene in gene_universe if gene in fda["gene_to_records"])

    gtr_sample_path = write_result(
        "diagnostic_gtr_sample_100.json",
        {
            "source": "gtr",
            "record_count": len(gtr["records"]),
            "sample_size": 100,
            "records": select_sample(gtr["records"]),
        },
    )
    who_sample_path = write_result(
        "diagnostic_who_ivd_sample_100.json",
        {
            "source": "who_ivd",
            "record_count": len(who["records"]),
            "sample_size": 100,
            "records": select_sample(who["records"]),
        },
    )
    fda_sample_path = write_result(
        "diagnostic_fda_molecular_sample_100.json",
        {
            "source": "fda_device",
            "record_count": len(fda["records"]),
            "sample_size": 100,
            "records": select_sample(fda["records"]),
        },
    )

    matrix_rows_path = write_result(
        "diagnostic_gene_source_matrix.json",
        {
            "gene_universe_source": clinvar["source"],
            "row_count": len(gene_matrix["rows"]),
            "rows": gene_matrix["rows"],
        },
    )

    validation_payload = {
        "gtr": {
            "mychoice": find_named_records(
                gtr["records"],
                "mychoice",
                fields=["name", "manufacturer_or_lab"],
            ),
            "foundationone": find_named_records(
                gtr["records"],
                "foundationone",
                fields=["name", "manufacturer_or_lab"],
            ),
            "tempus_xt": find_named_records(
                gtr["records"],
                "tempus xt",
                fields=["name", "manufacturer_or_lab"],
            ),
        },
        "fda_device": {
            "mychoice": find_named_records(
                fda["records"],
                "mychoice",
                fields=["name", "trade_name", "generic_name", "manufacturer_or_lab"],
            ),
            "foundationone": find_named_records(
                fda["records"],
                "foundationone",
                fields=["name", "trade_name", "generic_name", "manufacturer_or_lab"],
            ),
            "tempus_xt": find_named_records(
                fda["records"],
                "tempus",
                fields=["name", "trade_name", "generic_name", "manufacturer_or_lab"],
            ),
        },
    }
    validation_path = write_result("diagnostic_validation.json", validation_payload)

    full_scale_payload = {
        "spike_slug": "24-diagnostic-entity-landscape",
        "artifact_root": str(EXPERIMENT_ROOT),
        "full_scale_definition": {
            "clinvar_gene_universe": "All genes with nonzero `Alleles_reported_Pathogenic_Likely_pathogenic` in ClinVar `gene_specific_summary.txt`.",
            "gtr_backbone": "All current GTR tests from `test_version.gz` joined to `test_condition_gene.txt`.",
            "who_overlay": "Full WHO IVD CSV export scanned against the ClinVar pathogenic-gene universe.",
            "fda_overlay": "Combined PMA and 510(k) molecular-diagnostics slice from openFDA, deduped by regulatory identifier.",
        },
        "clinvar_gene_universe": {
            "primary": {
                "source": clinvar["source"],
                "file": clinvar["file"],
                "overview": clinvar["overview"],
                "pathogenic_gene_count": clinvar["pathogenic_gene_count"],
                "timing": clinvar["timing"],
            },
            "variant_summary_sanity_check": clinvar_variant_sanity,
        },
        "sources": {
            "gtr": {
                **gtr["metrics"],
                "source_gene_count": len(gtr["gene_to_tests"]),
                "clinvar_genes_with_any_test": gtr_clinvar_covered,
                "clinvar_genes_with_any_test_pct": round(
                    gtr_clinvar_covered / clinvar["pathogenic_gene_count"] * 100.0,
                    2,
                ),
                "top_genes_by_test_count": top_gene_counts(gtr["gene_to_tests"]),
                "sample_extract_path": str(gtr_sample_path),
            },
            "who_ivd": {
                **who["metrics"],
                "source_gene_count": len(who["gene_to_records"]),
                "clinvar_genes_with_any_record": who_clinvar_covered,
                "clinvar_genes_with_any_record_pct": round(
                    who_clinvar_covered / clinvar["pathogenic_gene_count"] * 100.0,
                    2,
                ),
                "top_genes_by_record_count": top_gene_counts(who["gene_to_records"]),
                "sample_extract_path": str(who_sample_path),
            },
            "fda_device": {
                **fda["metrics"],
                "source_gene_count": len(fda["gene_to_records"]),
                "clinvar_genes_with_any_record": fda_clinvar_covered,
                "clinvar_genes_with_any_record_pct": round(
                    fda_clinvar_covered / clinvar["pathogenic_gene_count"] * 100.0,
                    2,
                ),
                "top_genes_by_record_count": top_gene_counts(fda["gene_to_records"]),
                "sample_extract_path": str(fda_sample_path),
            },
        },
        "gene_source_coverage": {
            **gene_matrix["coverage_summary"],
            "omim_gene_subset": {
                "gene_count": len(omim_rows),
                "genes_with_any_source_hit": omim_covered,
                "genes_with_any_source_hit_pct": round(
                    omim_covered / len(omim_rows) * 100.0,
                    2,
                )
                if omim_rows
                else 0.0,
            },
            "loc_or_antisense_like_subset": {
                "gene_count": len(loc_like_rows),
                "genes_with_any_source_hit": loc_like_covered,
                "genes_with_any_source_hit_pct": round(
                    loc_like_covered / len(loc_like_rows) * 100.0,
                    2,
                )
                if loc_like_rows
                else 0.0,
            },
            "top_uncovered_genes": top_uncovered_genes(
                clinvar["genes"],
                {
                    gene: (
                        set(gtr["gene_to_tests"].get(gene, set()))
                        | set(who["gene_to_records"].get(gene, set()))
                        | set(fda["gene_to_records"].get(gene, set()))
                    )
                    for gene in gene_universe
                },
            ),
            "matrix_path": str(matrix_rows_path),
        },
        "proposed_unified_data_model": build_unified_data_model(),
        "cli_surface_proposal": build_cli_surface(),
        "recommended_source_priority": build_source_priority(),
        "proposed_rust_module_boundaries": build_rust_module_boundaries(),
        "validation_artifact": str(validation_path),
        "timing": {
            "elapsed_seconds": round(time.perf_counter() - started, 2),
        },
    }
    full_scale_path = write_result("diagnostic_full_scale_landscape.json", full_scale_payload)

    print(full_scale_path)
    print(matrix_rows_path)
    print(gtr_sample_path)
    print(who_sample_path)
    print(fda_sample_path)
    print(validation_path)


if __name__ == "__main__":
    main()
