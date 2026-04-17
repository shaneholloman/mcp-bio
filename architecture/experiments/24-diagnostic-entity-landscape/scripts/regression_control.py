#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import time
from copy import deepcopy
from pathlib import Path
from typing import Any

from common import DISEASES, GENES, load_json, mean, pct, request_json
from cross_source_matrix import overlap_count
from fda_device_probe import (
    CDX_DRUG_PROBES,
    OPENFDA_510K_URL,
    OPENFDA_PMA_URL,
    exact_phrase_matches,
    fetch_510k_sample,
    relevant_disease_matches,
    relevant_gene_matches,
    search_openfda,
    summarize_examples,
)
from gtr_api_probe import (
    EINFO_URL,
    NCBI_RATE_LIMITER,
    build_disease_probe,
    build_gene_probe,
    build_type_query,
)
from gtr_bulk_probe import load_current_tests, parse_relation_file
from who_ivd_probe import load_rows

from diagnostic_landscape_lib import write_result

EXPERIMENT_RESULTS = Path(__file__).resolve().parent.parent / "results"

BASELINE_FILES = {
    "gtr_bulk": EXPERIMENT_RESULTS / "gtr_bulk.json",
    "gtr_api": EXPERIMENT_RESULTS / "gtr_api.json",
    "who_ivd": EXPERIMENT_RESULTS / "who_ivd.json",
    "fda_device": EXPERIMENT_RESULTS / "fda_device.json",
    "cross_source_matrix": EXPERIMENT_RESULTS / "cross_source_matrix.json",
}

EXACT_PATHS = {
    "gtr_bulk": [
        "record_counts.current_tests",
        "schema_completeness.gene_links_pct",
        "schema_completeness.disease_links_pct",
        "schema_completeness.manufacturer_or_lab_name_pct",
        "schema_completeness.clia_number_pct",
        "schema_completeness.state_licenses_pct",
        "schema_completeness.any_regulatory_metadata_pct",
        "link_density.mean_genes_per_test",
        "link_density.mean_diseases_per_test",
        "link_density.tests_per_sample_gene.BRCA1",
        "link_density.tests_per_sample_gene.EGFR",
        "link_density.tests_per_sample_gene.BRAF",
        "link_density.tests_per_sample_gene.KRAS",
        "link_density.tests_per_sample_gene.TP53",
        "link_density.tests_per_sample_disease.breast cancer",
        "link_density.tests_per_sample_disease.melanoma",
        "link_density.tests_per_sample_disease.lung cancer",
    ],
    "gtr_api": [
        "sample_gene_matches.BRCA1.count",
        "sample_gene_matches.EGFR.count",
        "sample_gene_matches.BRAF.count",
        "sample_gene_matches.KRAS.count",
        "sample_gene_matches.TP53.count",
        "sample_disease_matches.breast cancer.count",
        "sample_disease_matches.melanoma.count",
        "sample_disease_matches.lung cancer.count",
        "type_queries.brca1_targeted_variant_analysis.query.count",
    ],
    "who_ivd": [
        "record_counts.rows",
        "schema_completeness.manufacturer_pct",
        "schema_completeness.pathogen_disease_marker_pct",
        "schema_completeness.regulatory_version_pct",
        "schema_completeness.prequalification_year_pct",
        "schema_completeness.regulatory_metadata_pct",
        "sample_gene_matches.BRCA1.count",
        "sample_gene_matches.EGFR.count",
        "sample_gene_matches.BRAF.count",
        "sample_gene_matches.KRAS.count",
        "sample_gene_matches.TP53.count",
        "sample_disease_matches.breast cancer.count",
        "sample_disease_matches.melanoma.count",
        "sample_disease_matches.lung cancer.count",
    ],
    "fda_device": [
        "record_counts.openfda_510k_total",
        "record_counts.sampled_records_for_schema_check",
        "schema_completeness.device_name_pct",
        "schema_completeness.applicant_pct",
        "schema_completeness.decision_date_pct",
        "schema_completeness.k_number_pct",
        "schema_completeness.advisory_committee_pct",
        "sample_gene_matches.BRCA1.count",
        "sample_gene_matches.EGFR.count",
        "sample_gene_matches.BRAF.count",
        "sample_gene_matches.KRAS.count",
        "sample_gene_matches.TP53.count",
        "sample_disease_matches.breast cancer.count",
        "sample_disease_matches.melanoma.count",
        "sample_disease_matches.lung cancer.count",
        "companion_diagnostic_probe.drug_name_side_probe.pembrolizumab.pma_count",
        "companion_diagnostic_probe.drug_name_side_probe.osimertinib.pma_count",
        "companion_diagnostic_probe.drug_name_side_probe.vemurafenib.pma_count",
        "companion_diagnostic_probe.drug_name_side_probe.trastuzumab.pma_count",
        "companion_diagnostic_probe.drug_name_side_probe.pembrolizumab.510k_count",
        "companion_diagnostic_probe.drug_name_side_probe.osimertinib.510k_count",
        "companion_diagnostic_probe.drug_name_side_probe.vemurafenib.510k_count",
        "companion_diagnostic_probe.drug_name_side_probe.trastuzumab.510k_count",
    ],
    "cross_source_matrix": [
        "gene_source_matrix.BRCA1.gtr_bulk",
        "gene_source_matrix.BRCA1.fda_510k",
        "gene_source_matrix.BRCA1.who_ivd",
        "gene_source_matrix.EGFR.gtr_bulk",
        "gene_source_matrix.EGFR.fda_510k",
        "gene_source_matrix.EGFR.who_ivd",
        "gene_source_matrix.BRAF.gtr_bulk",
        "gene_source_matrix.BRAF.fda_510k",
        "gene_source_matrix.BRAF.who_ivd",
        "gene_source_matrix.KRAS.gtr_bulk",
        "gene_source_matrix.KRAS.fda_510k",
        "gene_source_matrix.KRAS.who_ivd",
        "gene_source_matrix.TP53.gtr_bulk",
        "gene_source_matrix.TP53.fda_510k",
        "gene_source_matrix.TP53.who_ivd",
        "disease_source_matrix.breast cancer.gtr_bulk",
        "disease_source_matrix.breast cancer.fda_510k",
        "disease_source_matrix.breast cancer.who_ivd",
        "disease_source_matrix.melanoma.gtr_bulk",
        "disease_source_matrix.melanoma.fda_510k",
        "disease_source_matrix.melanoma.who_ivd",
        "disease_source_matrix.lung cancer.gtr_bulk",
        "disease_source_matrix.lung cancer.fda_510k",
        "disease_source_matrix.lung cancer.who_ivd",
        "normalized_name_overlap.gtr_bulk_vs_fda_510k",
        "normalized_name_overlap.gtr_bulk_vs_who_ivd",
        "normalized_name_overlap.who_ivd_vs_fda_510k",
    ],
}

PERF_PATHS = {
    "gtr_bulk": [
        ("timing.elapsed_seconds", 0.03),
    ],
    "gtr_api": [
        ("latency_summary_ms.mean_gene_search_latency_ms", 0.03),
        ("latency_summary_ms.mean_gene_summary_latency_ms", 0.03),
        ("latency_summary_ms.mean_disease_search_latency_ms", 0.03),
        ("latency_summary_ms.mean_disease_summary_latency_ms", 0.03),
    ],
    "fda_device": [
        ("api_availability.sample_latency_ms", 0.03),
    ],
}


def get_path(payload: dict[str, Any], path: str) -> Any:
    current: Any = payload
    for part in path.split("."):
        current = current[part]
    return current


def projection_checksum(payload: dict[str, Any], artifact_name: str) -> str:
    stripped = deepcopy(payload)
    if artifact_name == "gtr_bulk":
        stripped.pop("timing", None)
    elif artifact_name == "gtr_api":
        stripped.pop("latency_summary_ms", None)
        for probe in stripped["gene_queries"].values():
            probe["primary_query"].pop("latency_ms", None)
            probe["summary_latency_ms"] = None
        for probe in stripped["disease_queries"].values():
            probe["query"].pop("latency_ms", None)
            probe["summary_latency_ms"] = None
        for probe in stripped["type_queries"].values():
            probe["query"].pop("latency_ms", None)
            probe["summary_latency_ms"] = None
    elif artifact_name == "fda_device":
        stripped.pop("api_availability", None)
        for section in ["sample_gene_matches", "sample_disease_matches"]:
            for probe in stripped[section].values():
                probe.pop("latency_ms", None)
    elif artifact_name == "cross_source_matrix":
        stripped["sources"]["gtr_api"].pop("mean_gene_search_latency_ms", None)
        stripped["sources"]["gtr_api"].pop("mean_gene_summary_latency_ms", None)
    encoded = json.dumps(stripped, indent=2, sort_keys=True).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def compare_artifact(
    artifact_name: str,
    baseline: dict[str, Any],
    current: dict[str, Any],
) -> dict[str, Any]:
    metric_mismatches: list[dict[str, Any]] = []
    perf_findings: list[dict[str, Any]] = []

    for path in EXACT_PATHS[artifact_name]:
        baseline_value = get_path(baseline, path)
        current_value = get_path(current, path)
        if baseline_value != current_value:
            metric_mismatches.append(
                {
                    "path": path,
                    "baseline": baseline_value,
                    "current": current_value,
                }
            )

    for path, tolerance in PERF_PATHS.get(artifact_name, []):
        baseline_value = float(get_path(baseline, path))
        current_value = float(get_path(current, path))
        allowed = baseline_value * (1.0 + tolerance)
        perf_findings.append(
            {
                "path": path,
                "baseline": baseline_value,
                "current": current_value,
                "allowed_max": round(allowed, 4),
                "pass": current_value <= allowed,
            }
        )

    baseline_checksum = projection_checksum(baseline, artifact_name)
    current_checksum = projection_checksum(current, artifact_name)
    checksum_ok = baseline_checksum == current_checksum or bool(metric_mismatches)
    perf_ok = all(item["pass"] for item in perf_findings)
    passes = not metric_mismatches and perf_ok and checksum_ok

    return {
        "artifact": artifact_name,
        "baseline_projection_checksum": baseline_checksum,
        "current_projection_checksum": current_checksum,
        "projection_checksum_match": baseline_checksum == current_checksum,
        "mismatch_count": len(metric_mismatches),
        "metric_mismatches": metric_mismatches,
        "performance_findings": perf_findings,
        "pass": passes,
    }


def build_current_gtr_bulk(baseline: dict[str, Any]) -> dict[str, Any]:
    started = time.perf_counter()
    current_tests, current_summary = load_current_tests(
        Path(baseline["files"]["test_version"])
    )
    genes_by_test, diseases_by_test, relation_summary = parse_relation_file(
        Path(baseline["files"]["test_condition_gene"]),
        current_tests,
    )

    gene_counts: list[int] = []
    disease_counts: list[int] = []
    tests_with_gene_links = 0
    tests_with_disease_links = 0
    sample_name_index: set[str] = set()

    for payload in relation_summary["sample_gene_matches"].values():
        for example in payload["examples"]:
            sample_name_index.add(example["name"].strip().lower())
    for payload in relation_summary["sample_disease_matches"].values():
        for example in payload["examples"]:
            sample_name_index.add(example["name"].strip().lower())

    for accession, test_info in current_tests.items():
        gene_values = set(genes_by_test.get(accession, set())) | set(test_info["gene_field"])
        disease_values = set(diseases_by_test.get(accession, set())) | set(
            test_info["condition_field"]
        )
        gene_counts.append(len(gene_values))
        disease_counts.append(len(disease_values))
        tests_with_gene_links += int(bool(gene_values))
        tests_with_disease_links += int(bool(disease_values))

    return {
        "approach": baseline["approach"],
        "source": baseline["source"],
        "files": baseline["files"],
        "record_counts": {
            "current_tests": current_summary["current_test_count"],
        },
        "schema_completeness": {
            **current_summary["schema_completeness"],
            "gene_links_pct": pct(tests_with_gene_links, len(current_tests)),
            "disease_links_pct": pct(tests_with_disease_links, len(current_tests)),
        },
        "link_density": {
            "mean_genes_per_test": mean(gene_counts),
            "mean_diseases_per_test": mean(disease_counts),
            "tests_per_sample_gene": {
                gene: relation_summary["sample_gene_matches"][gene]["count"]
                for gene in GENES
            },
            "tests_per_sample_disease": {
                disease: relation_summary["sample_disease_matches"][disease]["count"]
                for disease in DISEASES
            },
        },
        "test_type_counts": current_summary["test_type_counts"],
        "top_countries": current_summary["top_countries"],
        "top_method_categories": current_summary["top_method_categories"],
        "top_methods": current_summary["top_methods"],
        "sample_gene_matches": relation_summary["sample_gene_matches"],
        "sample_disease_matches": relation_summary["sample_disease_matches"],
        "sample_name_index": sorted(sample_name_index),
        "success_signals": {
            "over_100_tests_with_gene_links": tests_with_gene_links > 100,
            "all_sample_genes_have_hits": all(
                relation_summary["sample_gene_matches"][gene]["count"] > 0
                for gene in GENES
            ),
        },
        "timing": {
            "elapsed_seconds": round(time.perf_counter() - started, 2),
        },
    }


def build_current_gtr_api() -> dict[str, Any]:
    payload, _, _ = request_json(
        EINFO_URL,
        params={"db": "gtr", "retmode": "json"},
        rate_limiter=NCBI_RATE_LIMITER,
        timeout=20,
    )
    assert payload is not None
    fieldlist = payload["einforesult"]["dbinfo"][0]["fieldlist"]
    tracked_fields = {
        field["name"]: field["fullname"]
        for field in fieldlist
        if field["name"] in {"SYMB", "DISNAME", "MCAT", "MTOD", "clinical_category"}
    }

    gene_queries = {gene: build_gene_probe(gene) for gene in GENES}
    disease_queries = {disease: build_disease_probe(disease) for disease in DISEASES}
    type_queries = {
        "brca1_targeted_variant_analysis": build_type_query(
            "BRCA1[SYMB] AND Targeted variant analysis[MCAT]"
        ),
    }
    sample_name_index: set[str] = set()
    for payload_map in [gene_queries, disease_queries]:
        for probe in payload_map.values():
            for example in probe["examples"]:
                name = (example.get("test_name") or "").strip().lower()
                if name:
                    sample_name_index.add(name)

    return {
        "approach": "GTR live query path",
        "source": "gtr_api",
        "tracked_search_fields": tracked_fields,
        "gene_queries": gene_queries,
        "disease_queries": disease_queries,
        "type_queries": type_queries,
        "latency_summary_ms": {
            "mean_gene_search_latency_ms": mean(
                [
                    probe["primary_query"]["latency_ms"]
                    for probe in gene_queries.values()
                    if probe["primary_query"]["latency_ms"] is not None
                ]
            ),
            "mean_gene_summary_latency_ms": mean(
                [
                    probe["summary_latency_ms"]
                    for probe in gene_queries.values()
                    if probe["summary_latency_ms"] is not None
                ]
            ),
            "mean_disease_search_latency_ms": mean(
                [
                    probe["query"]["latency_ms"]
                    for probe in disease_queries.values()
                    if probe["query"]["latency_ms"] is not None
                ]
            ),
            "mean_disease_summary_latency_ms": mean(
                [
                    probe["summary_latency_ms"]
                    for probe in disease_queries.values()
                    if probe["summary_latency_ms"] is not None
                ]
            ),
        },
        "sample_gene_matches": {
            gene: {
                "count": gene_queries[gene]["primary_query"]["count"],
                "examples": gene_queries[gene]["examples"],
            }
            for gene in GENES
        },
        "sample_disease_matches": {
            disease: {
                "count": disease_queries[disease]["query"]["count"],
                "examples": disease_queries[disease]["examples"],
            }
            for disease in DISEASES
        },
        "sample_name_index": sorted(sample_name_index),
        "success_signals": {
            "all_sample_genes_have_hits": all(
                gene_queries[gene]["primary_query"]["count"] > 0 for gene in GENES
            ),
            "all_sample_genes_return_summaries": all(
                gene_queries[gene]["fetched_docs"] > 0 for gene in GENES
            ),
        },
    }


def build_current_who_ivd(baseline: dict[str, Any]) -> dict[str, Any]:
    rows = load_rows(Path(baseline["file"]))
    assay_format_counts: dict[str, int] = {}
    manufacturer_present = 0
    marker_present = 0
    regulatory_version_present = 0
    year_present = 0
    regulatory_present = 0
    sample_gene_matches = {gene: [] for gene in GENES}
    sample_disease_matches = {disease: [] for disease in DISEASES}
    gene_match_seen = {gene: set() for gene in GENES}
    disease_match_seen = {disease: set() for disease in DISEASES}

    from common import contains_gene_symbol, matched_diseases, top_counts

    for row in rows:
        product_name = (row.get("Product name") or "").strip()
        product_code = (row.get("Product Code") or "").strip()
        marker = (row.get("Pathogen/Disease/Marker") or "").strip()
        manufacturer = (row.get("Manufacturer name") or "").strip()
        assay_format = (row.get("Assay Format") or "").strip()
        regulatory_version = (row.get("Regulatory Version") or "").strip()
        prequalification_year = (row.get("Year prequalification") or "").strip()
        search_text = " ".join(part for part in [product_name, marker] if part)

        manufacturer_present += int(bool(manufacturer))
        marker_present += int(bool(marker))
        regulatory_version_present += int(bool(regulatory_version))
        year_present += int(bool(prequalification_year))
        regulatory_present += int(bool(regulatory_version and prequalification_year))
        if assay_format:
            assay_format_counts[assay_format] = assay_format_counts.get(assay_format, 0) + 1

        for gene in GENES:
            if contains_gene_symbol(search_text, gene) and product_code not in gene_match_seen[gene]:
                gene_match_seen[gene].add(product_code)
                sample_gene_matches[gene].append(
                    {
                        "product_code": product_code,
                        "name": product_name,
                        "manufacturer": manufacturer,
                        "marker": marker,
                        "regulatory_version": regulatory_version,
                        "prequalification_year": prequalification_year,
                    }
                )

        for disease in matched_diseases(search_text):
            if product_code not in disease_match_seen[disease]:
                disease_match_seen[disease].add(product_code)
                sample_disease_matches[disease].append(
                    {
                        "product_code": product_code,
                        "name": product_name,
                        "manufacturer": manufacturer,
                        "marker": marker,
                        "regulatory_version": regulatory_version,
                        "prequalification_year": prequalification_year,
                    }
                )

    total = len(rows)
    sample_name_index: set[str] = set()
    for payload in list(sample_gene_matches.values()) + list(sample_disease_matches.values()):
        for example in payload:
            name = example["name"].strip().lower()
            if name:
                sample_name_index.add(name)

    return {
        "approach": "WHO IVD CSV parse",
        "source": "who_ivd",
        "file": baseline["file"],
        "record_counts": {"rows": total},
        "schema_completeness": {
            "manufacturer_pct": pct(manufacturer_present, total),
            "pathogen_disease_marker_pct": pct(marker_present, total),
            "regulatory_version_pct": pct(regulatory_version_present, total),
            "prequalification_year_pct": pct(year_present, total),
            "regulatory_metadata_pct": pct(regulatory_present, total),
        },
        "assay_formats": top_counts(assay_format_counts),
        "sample_gene_matches": {
            gene: {
                "count": len(sample_gene_matches[gene]),
                "examples": sample_gene_matches[gene][:10],
            }
            for gene in GENES
        },
        "sample_disease_matches": {
            disease: {
                "count": len(sample_disease_matches[disease]),
                "examples": sample_disease_matches[disease][:10],
            }
            for disease in DISEASES
        },
        "sample_name_index": sorted(sample_name_index),
        "success_signals": {
            "all_sample_genes_have_hits": all(sample_gene_matches[gene] for gene in GENES),
            "all_sample_diseases_have_hits": all(
                sample_disease_matches[disease] for disease in DISEASES
            ),
        },
    }


def build_current_fda_device() -> dict[str, Any]:
    sample = fetch_510k_sample(limit=100)
    records = sample["results"]
    total = len(records)
    manufacturer_present = sum(1 for row in records if row.get("applicant"))
    decision_present = sum(1 for row in records if row.get("decision_date"))
    device_name_present = sum(1 for row in records if row.get("device_name"))
    committee_present = sum(
        1 for row in records if row.get("advisory_committee_description")
    )
    k_number_present = sum(1 for row in records if row.get("k_number"))

    sample_gene_matches: dict[str, dict[str, Any]] = {}
    sample_disease_matches: dict[str, dict[str, Any]] = {}
    sample_name_index: set[str] = set()

    for gene in GENES:
        query_result = search_openfda(OPENFDA_510K_URL, f'device_name:"{gene}"', limit=25)
        relevant = relevant_gene_matches(query_result["results"], gene)
        examples = summarize_examples(relevant)
        sample_gene_matches[gene] = {
            "query": query_result["query"],
            "count": len(relevant),
            "reported_total": query_result["count"],
            "latency_ms": query_result["latency_ms"],
            "examples": examples,
        }
        for example in examples:
            if example["device_name"]:
                sample_name_index.add(example["device_name"].strip().lower())

    for disease in DISEASES:
        query_result = search_openfda(
            OPENFDA_510K_URL,
            f'device_name:"{disease}"',
            limit=25,
        )
        relevant = relevant_disease_matches(query_result["results"], disease)
        examples = summarize_examples(relevant)
        sample_disease_matches[disease] = {
            "query": query_result["query"],
            "count": len(relevant),
            "reported_total": query_result["count"],
            "latency_ms": query_result["latency_ms"],
            "examples": examples,
        }
        for example in examples:
            if example["device_name"]:
                sample_name_index.add(example["device_name"].strip().lower())

    companion_510k = search_openfda(
        OPENFDA_510K_URL,
        "device_name:companion diagnostic",
        limit=100,
    )
    companion_pma = search_openfda(
        OPENFDA_PMA_URL,
        "device_name:companion diagnostic",
        limit=100,
    )
    exact_companion_510k = exact_phrase_matches(
        companion_510k["results"],
        "companion diagnostic",
    )
    exact_companion_pma = exact_phrase_matches(
        companion_pma["results"],
        "companion diagnostic",
    )
    cdx_drug_queries = {
        drug: {
            "pma_count": search_openfda(OPENFDA_PMA_URL, drug, limit=1)["count"],
            "510k_count": search_openfda(OPENFDA_510K_URL, drug, limit=1)["count"],
        }
        for drug in CDX_DRUG_PROBES
    }

    return {
        "approach": "FDA device search probe",
        "source": "fda_device",
        "record_counts": {
            "openfda_510k_total": sample["total"],
            "sampled_records_for_schema_check": total,
        },
        "schema_completeness": {
            "device_name_pct": pct(device_name_present, total),
            "applicant_pct": pct(manufacturer_present, total),
            "decision_date_pct": pct(decision_present, total),
            "k_number_pct": pct(k_number_present, total),
            "advisory_committee_pct": pct(committee_present, total),
        },
        "api_availability": {
            "sample_latency_ms": sample["latency_ms"],
        },
        "sample_gene_matches": sample_gene_matches,
        "sample_disease_matches": sample_disease_matches,
        "sample_name_index": sorted(sample_name_index),
        "companion_diagnostic_probe": {
            "openfda_510k_token_query_count": companion_510k["count"],
            "openfda_510k_exact_phrase_hits_in_first_page": len(exact_companion_510k),
            "openfda_510k_exact_phrase_examples": summarize_examples(exact_companion_510k),
            "openfda_pma_token_query_count": companion_pma["count"],
            "openfda_pma_exact_phrase_hits_in_first_page": len(exact_companion_pma),
            "openfda_pma_exact_phrase_examples": summarize_examples(exact_companion_pma),
            "drug_name_side_probe": cdx_drug_queries,
        },
        "success_signals": {
            "all_sample_genes_have_hits": all(
                sample_gene_matches[gene]["count"] > 0 for gene in GENES
            ),
            "all_sample_diseases_have_hits": all(
                sample_disease_matches[disease]["count"] > 0 for disease in DISEASES
            ),
        },
    }


def build_current_cross_source_matrix(
    gtr_bulk: dict[str, Any],
    who_ivd: dict[str, Any],
    fda_device: dict[str, Any],
    gtr_api: dict[str, Any],
) -> dict[str, Any]:
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
            source_name: int(
                source_payloads[source_name]["sample_disease_matches"][disease]["count"]
            )
            for source_name in source_payloads
        }
        for disease in DISEASES
    }
    overlap = {
        "gtr_bulk_vs_who_ivd": overlap_count(
            gtr_bulk["sample_name_index"],
            who_ivd["sample_name_index"],
        ),
        "gtr_bulk_vs_fda_510k": overlap_count(
            gtr_bulk["sample_name_index"],
            fda_device["sample_name_index"],
        ),
        "who_ivd_vs_fda_510k": overlap_count(
            who_ivd["sample_name_index"],
            fda_device["sample_name_index"],
        ),
    }

    return {
        "sources": {
            "gtr_bulk": {
                "all_sample_genes_have_hits": gtr_bulk["success_signals"][
                    "all_sample_genes_have_hits"
                ],
                "regulatory_metadata_pct": gtr_bulk["schema_completeness"][
                    "any_regulatory_metadata_pct"
                ],
                "gene_links_pct": gtr_bulk["schema_completeness"]["gene_links_pct"],
            },
            "gtr_api": {
                "all_sample_genes_have_hits": gtr_api["success_signals"][
                    "all_sample_genes_have_hits"
                ],
                "mean_gene_search_latency_ms": gtr_api["latency_summary_ms"][
                    "mean_gene_search_latency_ms"
                ],
                "mean_gene_summary_latency_ms": gtr_api["latency_summary_ms"][
                    "mean_gene_summary_latency_ms"
                ],
            },
            "who_ivd": {
                "all_sample_genes_have_hits": who_ivd["success_signals"][
                    "all_sample_genes_have_hits"
                ],
                "regulatory_metadata_pct": who_ivd["schema_completeness"][
                    "regulatory_metadata_pct"
                ],
            },
            "fda_510k": {
                "all_sample_genes_have_hits": fda_device["success_signals"][
                    "all_sample_genes_have_hits"
                ],
                "decision_date_pct": fda_device["schema_completeness"][
                    "decision_date_pct"
                ],
                "companion_diagnostic_pma_counts": fda_device[
                    "companion_diagnostic_probe"
                ]["drug_name_side_probe"],
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
                "fda_device": [
                    "k_number_or_pma_number",
                    "decision_date",
                    "product_code",
                    "advisory_committee",
                ],
            },
        },
        "decision_hint": {
            "backbone_source": "gtr",
            "regulatory_overlays": ["fda_device", "who_ivd"],
            "note": "GTR is the only source that satisfies the ticket's gene-linkage success bar on the oncology sample. FDA is useful for regulation but not as the primary linkage spine; WHO IVD is orthogonal and more infectious-disease oriented.",
        },
    }


def main() -> None:
    baselines = {
        name: json.loads(path.read_text(encoding="utf-8"))
        for name, path in BASELINE_FILES.items()
    }

    current_gtr_bulk = build_current_gtr_bulk(baselines["gtr_bulk"])
    current_gtr_api = build_current_gtr_api()
    current_who_ivd = build_current_who_ivd(baselines["who_ivd"])
    current_fda_device = build_current_fda_device()
    current_cross_source_matrix = build_current_cross_source_matrix(
        current_gtr_bulk,
        current_who_ivd,
        current_fda_device,
        current_gtr_api,
    )

    current_payloads = {
        "gtr_bulk": current_gtr_bulk,
        "gtr_api": current_gtr_api,
        "who_ivd": current_who_ivd,
        "fda_device": current_fda_device,
        "cross_source_matrix": current_cross_source_matrix,
    }

    comparisons = {
        name: compare_artifact(name, baselines[name], current_payloads[name])
        for name in current_payloads
    }

    output = {
        "baseline_files": {name: str(path) for name, path in BASELINE_FILES.items()},
        "comparisons": comparisons,
        "overall_pass": all(item["pass"] for item in comparisons.values()),
    }
    output_path = write_result("diagnostic_regression_control.json", output)
    print(output_path)


if __name__ == "__main__":
    main()
