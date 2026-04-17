#!/usr/bin/env python3
from __future__ import annotations

from common import DISEASES, GENES, load_json, write_json


def counts_by_key(payload: dict, section: str, keys: list[str]) -> dict[str, int]:
    return {key: int(payload[section][key]["count"]) for key in keys}


def overlap_count(left: list[str], right: list[str]) -> int:
    return len(set(left) & set(right))


def main() -> None:
    gtr_bulk = load_json("gtr_bulk.json")
    who_ivd = load_json("who_ivd.json")
    fda_device = load_json("fda_device.json")
    gtr_api = load_json("gtr_api.json")

    source_payloads = {
        "gtr_bulk": gtr_bulk,
        "who_ivd": who_ivd,
        "fda_510k": fda_device,
    }

    gene_matrix = {
        gene: {
            source_name: int(source_payloads[source_name]["sample_gene_matches"][gene]["count"])
            for source_name in source_payloads
        }
        for gene in GENES
    }
    disease_matrix = {
        disease: {
            source_name: int(source_payloads[source_name]["sample_disease_matches"][disease]["count"])
            for source_name in source_payloads
        }
        for disease in DISEASES
    }

    overlap = {
        "gtr_bulk_vs_who_ivd": overlap_count(gtr_bulk["sample_name_index"], who_ivd["sample_name_index"]),
        "gtr_bulk_vs_fda_510k": overlap_count(gtr_bulk["sample_name_index"], fda_device["sample_name_index"]),
        "who_ivd_vs_fda_510k": overlap_count(who_ivd["sample_name_index"], fda_device["sample_name_index"]),
    }

    payload = {
        "sources": {
            "gtr_bulk": {
                "all_sample_genes_have_hits": gtr_bulk["success_signals"]["all_sample_genes_have_hits"],
                "regulatory_metadata_pct": gtr_bulk["schema_completeness"]["any_regulatory_metadata_pct"],
                "gene_links_pct": gtr_bulk["schema_completeness"]["gene_links_pct"],
            },
            "gtr_api": {
                "all_sample_genes_have_hits": gtr_api["success_signals"]["all_sample_genes_have_hits"],
                "mean_gene_search_latency_ms": gtr_api["latency_summary_ms"]["mean_gene_search_latency_ms"],
                "mean_gene_summary_latency_ms": gtr_api["latency_summary_ms"]["mean_gene_summary_latency_ms"],
            },
            "who_ivd": {
                "all_sample_genes_have_hits": who_ivd["success_signals"]["all_sample_genes_have_hits"],
                "regulatory_metadata_pct": who_ivd["schema_completeness"]["regulatory_metadata_pct"],
            },
            "fda_510k": {
                "all_sample_genes_have_hits": fda_device["success_signals"]["all_sample_genes_have_hits"],
                "decision_date_pct": fda_device["schema_completeness"]["decision_date_pct"],
                "companion_diagnostic_pma_counts": fda_device["companion_diagnostic_probe"]["drug_name_side_probe"],
            },
        },
        "gene_source_matrix": gene_matrix,
        "disease_source_matrix": disease_matrix,
        "normalized_name_overlap": overlap,
        "candidate_unified_data_model": {
            "required_fields": [
                "source",
                "source_id",
                "name",
                "test_category",
                "manufacturer_or_lab",
                "genes",
                "conditions",
                "methods",
                "specimen_types",
                "regulatory_status",
                "regulatory_identifier",
                "region",
            ],
            "source_specific_extensions": {
                "gtr": ["offerer", "certifications", "clinical_validity", "clinical_utility"],
                "who_ivd": ["assay_format", "prequalification_year", "regulatory_version"],
                "fda_device": ["k_number_or_pma_number", "decision_date", "product_code", "advisory_committee"],
            },
        },
        "decision_hint": {
            "backbone_source": "gtr",
            "regulatory_overlays": ["fda_device", "who_ivd"],
            "note": "GTR is the only source that satisfies the ticket's gene-linkage success bar on the oncology sample. FDA is useful for regulation but not as the primary linkage spine; WHO IVD is orthogonal and more infectious-disease oriented.",
        },
    }

    output_path = write_json("cross_source_matrix.json", payload)
    print(output_path)


if __name__ == "__main__":
    main()
