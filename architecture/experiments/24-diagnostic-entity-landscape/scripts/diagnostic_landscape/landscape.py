#!/usr/bin/env python3
from __future__ import annotations

import csv
import gzip
import hashlib
import json
import re
import time
from collections import Counter, defaultdict
from copy import deepcopy
from pathlib import Path
from typing import Any

from .design import (
    build_cli_surface,
    build_rust_module_boundaries,
    build_source_priority,
    build_unified_data_model,
)
from .io import (
    EXPERIMENT_ROOT,
    REQUEST_TIMEOUT,
    RESULTS_DIR,
    WORK_DIR,
    dedupe_keep_order,
    download_file,
    load_json_result,
    matched_diseases,
    mean,
    pct,
    request_json,
    top_counts,
    write_result,
)
from .probes import (
    CDX_DRUG_PROBES,
    EINFO_URL,
    GTR_CONDITION_GENE_URL,
    GTR_TEST_VERSION_URL,
    NCBI_RATE_LIMITER,
    OPENFDA_510K_URL,
    OPENFDA_PMA_URL,
    WHO_IVD_URL,
    build_cross_source_matrix_payload,
    build_disease_probe,
    build_fda_device_probe_payload,
    build_gene_probe,
    build_gtr_api_probe_payload,
    build_gtr_bulk_probe_payload,
    build_who_ivd_probe_payload,
    build_type_query,
    exact_phrase_matches,
    fetch_510k_sample,
    load_current_tests,
    load_rows,
    overlap_count,
    parse_relation_file,
    relevant_disease_matches,
    relevant_gene_matches,
    search_openfda,
    summarize_examples,
)
from .types import FullScaleLandscape, SourceBundle

CLINVAR_GENE_SUMMARY_URL = "https://ftp.ncbi.nlm.nih.gov/pub/clinvar/tab_delimited/gene_specific_summary.txt"
CLINVAR_VARIANT_SUMMARY_URL = "https://ftp.ncbi.nlm.nih.gov/pub/clinvar/tab_delimited/variant_summary.txt.gz"

FDA_PMA_QUERY_TERMS = [
    ("trade_name:cdx", "trade_name_cdx"),
    ("generic_name:gene", "generic_name_gene"),
    ("generic_name:mutation", "generic_name_mutation"),
    ("generic_name:sequencing", "generic_name_sequencing"),
    ("generic_name:pcr", "generic_name_pcr"),
]

FDA_510K_QUERY_TERMS = [
    ("device_name:genetic", "device_name_genetic"),
    ("device_name:gene", "device_name_gene"),
    ("device_name:sequencing", "device_name_sequencing"),
    ("device_name:pcr", "device_name_pcr"),
]

GENE_TOKEN_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9'/-]*")
FDA_APPROVAL_ORDER_SHORT_GENE_WHITELIST = {"ATM", "MET", "RET"}

BASELINE_FILES = {
    "gtr_bulk": RESULTS_DIR / "gtr_bulk.json",
    "gtr_api": RESULTS_DIR / "gtr_api.json",
    "who_ivd": RESULTS_DIR / "who_ivd.json",
    "fda_device": RESULTS_DIR / "fda_device.json",
    "cross_source_matrix": RESULTS_DIR / "cross_source_matrix.json",
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


def parse_int(value: str | None) -> int:
    if value is None:
        return 0
    value = value.strip()
    if not value or value == "-":
        return 0
    return int(value)


def split_multi_value(value: str | None) -> list[str]:
    if not value:
        return []
    parts = re.split(r"[|/;,]", value)
    return [part.strip() for part in parts if part.strip() and part.strip() != "-"]


def extract_gene_hits(text: str, gene_universe: set[str]) -> list[str]:
    if not text:
        return []
    tokens = {token.upper() for token in GENE_TOKEN_RE.findall(text.upper())}
    return sorted(token for token in tokens if token in gene_universe)


def extract_case_sensitive_gene_hits(text: str, gene_universe: set[str]) -> list[str]:
    if not text:
        return []
    tokens = {token for token in GENE_TOKEN_RE.findall(text)}
    return sorted(token for token in tokens if token in gene_universe)


def extract_case_sensitive_fda_gene_hits(text: str, gene_universe: set[str]) -> list[str]:
    if not text:
        return []

    hits: set[str] = set()
    for token in GENE_TOKEN_RE.findall(text):
        if token not in gene_universe:
            continue
        if (
            len(token) >= 4
            or any(character.isdigit() for character in token)
            or token in FDA_APPROVAL_ORDER_SHORT_GENE_WHITELIST
        ):
            hits.add(token)
    return sorted(hits)


def listify_text(value: Any) -> list[str]:
    if isinstance(value, list):
        return [str(item).strip() for item in value if str(item).strip()]
    if isinstance(value, str) and value.strip():
        return [value.strip()]
    return []


def select_sample(records: list[dict[str, Any]], limit: int = 100) -> list[dict[str, Any]]:
    return records[:limit]


def top_gene_counts(gene_to_records: dict[str, set[str]], limit: int = 25) -> list[dict[str, Any]]:
    ranked = sorted(
        ((gene, len(source_ids)) for gene, source_ids in gene_to_records.items()),
        key=lambda item: (-item[1], item[0]),
    )
    return [{"gene": gene, "count": count} for gene, count in ranked[:limit]]


def top_uncovered_genes(
    gene_rows: list[dict[str, Any]],
    gene_to_records: dict[str, set[str]],
    limit: int = 25,
) -> list[dict[str, Any]]:
    uncovered = [
        row
        for row in gene_rows
        if len(gene_to_records.get(row["symbol"], set())) == 0
    ]
    ranked = sorted(
        uncovered,
        key=lambda row: (-row["pathogenic_allele_count"], row["symbol"]),
    )
    return ranked[:limit]


def load_clinvar_gene_summary(refresh: bool = False) -> dict[str, Any]:
    started = time.perf_counter()
    path = download_file(
        CLINVAR_GENE_SUMMARY_URL,
        "clinvar_gene_specific_summary.txt",
        refresh=refresh,
    )
    lines = path.read_text(encoding="utf-8").splitlines()
    overview = lines[0].lstrip("#").strip()
    header = lines[1].lstrip("#")
    reader = csv.DictReader([header, *lines[2:]], delimiter="\t")

    gene_map: dict[str, dict[str, Any]] = {}
    for row in reader:
        symbol = (row.get("Symbol") or "").strip()
        pathogenic_allele_count = parse_int(
            row.get("Alleles_reported_Pathogenic_Likely_pathogenic")
        )
        if not symbol or symbol == "-" or pathogenic_allele_count <= 0:
            continue
        if symbol not in gene_map:
            gene_map[symbol] = {
                "symbol": symbol,
                "gene_id": parse_int(row.get("GeneID")),
                "pathogenic_allele_count": pathogenic_allele_count,
                "total_alleles": parse_int(row.get("Total_alleles")),
                "total_submissions": parse_int(row.get("Total_submissions")),
                "number_uncertain": parse_int(row.get("Number_uncertain")),
                "number_with_conflicts": parse_int(row.get("Number_with_conflicts")),
                "omim_gene_mim_number": parse_int(row.get("Gene_MIM_number")),
            }
            continue

        current = gene_map[symbol]
        current["pathogenic_allele_count"] += pathogenic_allele_count
        current["total_alleles"] += parse_int(row.get("Total_alleles"))
        current["total_submissions"] += parse_int(row.get("Total_submissions"))
        current["number_uncertain"] += parse_int(row.get("Number_uncertain"))
        current["number_with_conflicts"] += parse_int(row.get("Number_with_conflicts"))
        if current["gene_id"] == 0:
            current["gene_id"] = parse_int(row.get("GeneID"))
        if current["omim_gene_mim_number"] == 0:
            current["omim_gene_mim_number"] = parse_int(row.get("Gene_MIM_number"))

    gene_rows = sorted(gene_map.values(), key=lambda row: row["symbol"])
    gene_counts = {
        row["symbol"]: row["pathogenic_allele_count"]
        for row in gene_rows
    }
    return {
        "source": "clinvar_gene_specific_summary",
        "file": str(path),
        "overview": overview,
        "pathogenic_gene_count": len(gene_rows),
        "genes": gene_rows,
        "gene_counts": gene_counts,
        "timing": {
            "elapsed_seconds": round(time.perf_counter() - started, 2),
        },
    }


def load_clinvar_variant_sanity(refresh: bool = False) -> dict[str, Any]:
    started = time.perf_counter()
    path = download_file(
        CLINVAR_VARIANT_SUMMARY_URL,
        "clinvar_variant_summary.txt.gz",
        refresh=refresh,
    )

    all_pathogenic: set[str] = set()
    germline_pathogenic: set[str] = set()
    with gzip.open(path, "rt", encoding="utf-8", newline="") as handle:
        reader = csv.DictReader(handle, delimiter="\t")
        for row in reader:
            significance = (row.get("ClinicalSignificance") or "").lower()
            if "pathogenic" not in significance:
                continue
            origin = (row.get("OriginSimple") or "").lower()
            genes = split_multi_value(row.get("GeneSymbol"))
            for gene in genes:
                all_pathogenic.add(gene)
                if "germline" in origin:
                    germline_pathogenic.add(gene)

    return {
        "source": "clinvar_variant_summary",
        "file": str(path),
        "all_pathogenic_gene_count": len(all_pathogenic),
        "germline_pathogenic_gene_count": len(germline_pathogenic),
        "timing": {
            "elapsed_seconds": round(time.perf_counter() - started, 2),
        },
    }


def load_gtr_backbone(
    gene_universe: set[str] | None = None,
    refresh: bool = False,
) -> SourceBundle:
    started = time.perf_counter()
    test_version_path = download_file(
        GTR_TEST_VERSION_URL,
        "gtr_test_version.gz",
        refresh=refresh,
    )
    relation_path = download_file(
        GTR_CONDITION_GENE_URL,
        "gtr_test_condition_gene.txt",
        refresh=refresh,
    )

    current_tests, current_summary = load_current_tests(test_version_path)
    genes_by_test, diseases_by_test, _ = parse_relation_file(relation_path, current_tests)

    gene_to_tests: dict[str, set[str]] = defaultdict(set)
    disease_to_tests: dict[str, set[str]] = defaultdict(set)
    gene_counts: list[int] = []
    disease_counts: list[int] = []
    tests_with_gene_links = 0
    tests_with_disease_links = 0
    records: list[dict[str, Any]] = []

    for accession, test_info in current_tests.items():
        title_gene_hits: set[str] = set()
        if gene_universe is not None:
            title_gene_hits = set(
                extract_case_sensitive_gene_hits(
                    " ".join(
                        part
                        for part in [
                            test_info["name"],
                            test_info["manufacturer_test_name"],
                        ]
                        if part
                    ),
                    gene_universe,
                )
            )
        genes = sorted(
            set(genes_by_test.get(accession, set()))
            | set(test_info["gene_field"])
            | title_gene_hits
        )
        conditions = sorted(
            set(diseases_by_test.get(accession, set())) | set(test_info["condition_field"])
        )
        gene_counts.append(len(genes))
        disease_counts.append(len(conditions))
        tests_with_gene_links += int(bool(genes))
        tests_with_disease_links += int(bool(conditions))

        for gene in genes:
            gene_to_tests[gene].add(accession)
        for condition in conditions:
            disease_to_tests[condition].add(accession)

        records.append(
            {
                "source": "gtr",
                "source_id": accession,
                "name": test_info["name"],
                "test_category": test_info["test_type"],
                "manufacturer_or_lab": test_info["laboratory"],
                "institution": test_info["institution"],
                "genes": genes,
                "conditions": conditions,
                "methods": test_info["methods"],
                "method_categories": test_info["method_categories"],
                "country": test_info["country"],
                "regulatory_status": test_info["test_current_status"],
                "public_status": test_info["test_public_status"],
                "regulatory_identifier": test_info["clia_number"] or test_info["state_licenses"],
                "clia_number": test_info["clia_number"],
                "state_licenses": test_info["state_licenses"],
                "region": "us" if test_info["country"] == "United States" else "international",
            }
        )

    records.sort(key=lambda row: row["source_id"])
    return SourceBundle(
        records=records,
        gene_to_records=gene_to_tests,
        disease_to_records=disease_to_tests,
        files={
            "test_version": str(test_version_path),
            "test_condition_gene": str(relation_path),
        },
        metrics={
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
            },
            "test_type_counts": current_summary["test_type_counts"],
            "top_countries": current_summary["top_countries"],
            "top_method_categories": current_summary["top_method_categories"],
            "top_methods": current_summary["top_methods"],
            "timing": {
                "elapsed_seconds": round(time.perf_counter() - started, 2),
            },
        },
    )


def load_who_overlay(gene_universe: set[str], refresh: bool = False) -> SourceBundle:
    started = time.perf_counter()
    csv_path = download_file(WHO_IVD_URL, "who_ivd.csv", refresh=refresh)
    rows = load_rows(csv_path)

    assay_format_counts: Counter[str] = Counter()
    manufacturer_present = 0
    marker_present = 0
    regulatory_version_present = 0
    year_present = 0
    regulatory_present = 0
    gene_to_records: dict[str, set[str]] = defaultdict(set)
    records: list[dict[str, Any]] = []

    for row in rows:
        product_name = (row.get("Product name") or "").strip()
        product_code = (row.get("Product Code") or "").strip()
        marker = (row.get("Pathogen/Disease/Marker") or "").strip()
        manufacturer = (row.get("Manufacturer name") or "").strip()
        assay_format = (row.get("Assay Format") or "").strip()
        regulatory_version = (row.get("Regulatory Version") or "").strip()
        prequalification_year = (row.get("Year prequalification") or "").strip()
        search_text = " ".join(part for part in [product_name, marker] if part)
        matched_genes = extract_gene_hits(search_text, gene_universe)

        manufacturer_present += int(bool(manufacturer))
        marker_present += int(bool(marker))
        regulatory_version_present += int(bool(regulatory_version))
        year_present += int(bool(prequalification_year))
        regulatory_present += int(bool(regulatory_version and prequalification_year))
        if assay_format:
            assay_format_counts[assay_format] += 1

        for gene in matched_genes:
            gene_to_records[gene].add(product_code)

        records.append(
            {
                "source": "who_ivd",
                "source_id": product_code,
                "name": product_name,
                "test_category": assay_format or "WHO IVD",
                "manufacturer_or_lab": manufacturer,
                "genes": matched_genes,
                "conditions": matched_diseases(search_text),
                "marker": marker,
                "regulatory_version": regulatory_version,
                "prequalification_year": prequalification_year,
                "region": "who",
            }
        )

    records.sort(key=lambda row: row["source_id"])
    total = len(rows)
    records_with_gene_hits = sum(1 for record in records if record["genes"])
    return SourceBundle(
        records=records,
        gene_to_records=gene_to_records,
        files={"who_ivd_csv": str(csv_path)},
        metrics={
            "record_counts": {
                "rows": total,
            },
            "schema_completeness": {
                "manufacturer_pct": pct(manufacturer_present, total),
                "pathogen_disease_marker_pct": pct(marker_present, total),
                "regulatory_version_pct": pct(regulatory_version_present, total),
                "prequalification_year_pct": pct(year_present, total),
                "regulatory_metadata_pct": pct(regulatory_present, total),
                "gene_linked_records_pct": pct(records_with_gene_hits, total),
            },
            "assay_formats": top_counts(dict(assay_format_counts)),
            "timing": {
                "elapsed_seconds": round(time.perf_counter() - started, 2),
            },
        },
    )


def fetch_openfda_query(
    url: str,
    query: str,
    *,
    limit: int = 100,
) -> dict[str, Any]:
    records: list[dict[str, Any]] = []
    page_latencies_ms: list[float] = []
    total = 0
    skip = 0

    while True:
        payload, latency_ms, _ = request_json(
            url,
            params={"search": query, "limit": limit, "skip": skip},
            allow_404=True,
            timeout=REQUEST_TIMEOUT,
        )
        page_latencies_ms.append(latency_ms)
        if payload is None:
            break

        batch = payload["results"]
        total = int(payload["meta"]["results"]["total"])
        records.extend(batch)
        skip += limit
        if len(records) >= total or len(batch) < limit:
            break

    return {
        "query": query,
        "reported_total": total,
        "page_latencies_ms": page_latencies_ms,
        "records": records,
    }


def normalize_openfda_device_name(openfda_payload: dict[str, Any] | None) -> str:
    if not openfda_payload:
        return ""
    value = openfda_payload.get("device_name")
    if isinstance(value, list):
        return " | ".join(str(item).strip() for item in value if str(item).strip())
    if value is None:
        return ""
    return str(value).strip()


def combined_fda_text(row: dict[str, Any]) -> str:
    return " ".join(
        part
        for part in [
            str(row.get("trade_name") or "").strip(),
            str(row.get("device_name") or "").strip(),
            str(row.get("generic_name") or "").strip(),
            normalize_openfda_device_name(row.get("openfda")),
        ]
        if part
    )


def collapse_fda_records(
    raw_results: list[tuple[dict[str, Any], str]],
    *,
    source_db: str,
    gene_universe: set[str],
) -> list[dict[str, Any]]:
    grouped: dict[str, dict[str, Any]] = {}

    for row, query_slug in raw_results:
        source_id = (
            (row.get("pma_number") or "").strip()
            if source_db == "pma"
            else (row.get("k_number") or "").strip()
        )
        if not source_id:
            continue

        decision_date = (row.get("decision_date") or "").strip()
        current = grouped.get(source_id)
        if current is None or decision_date > current["decision_date"]:
            grouped[source_id] = {
                "row": row,
                "decision_date": decision_date,
                "matched_queries": {query_slug},
                "supplement_numbers": {
                    (row.get("supplement_number") or "").strip()
                }
                if source_db == "pma"
                else set(),
            }
            continue

        current["matched_queries"].add(query_slug)
        if source_db == "pma":
            supplement_number = (row.get("supplement_number") or "").strip()
            if supplement_number:
                current["supplement_numbers"].add(supplement_number)

    normalized: list[dict[str, Any]] = []
    for source_id, bundle in grouped.items():
        row = bundle["row"]
        text = combined_fda_text(row)
        approval_order_text = " ".join(listify_text(row.get("ao_statement")))
        genes = sorted(
            set(extract_gene_hits(text, gene_universe))
            | set(extract_case_sensitive_fda_gene_hits(approval_order_text, gene_universe))
        )
        normalized.append(
            {
                "source": "fda_device",
                "source_db": source_db,
                "source_id": source_id,
                "name": (
                    str(row.get("trade_name") or "").strip()
                    or str(row.get("device_name") or "").strip()
                    or str(row.get("generic_name") or "").strip()
                    or normalize_openfda_device_name(row.get("openfda"))
                ),
                "trade_name": str(row.get("trade_name") or "").strip(),
                "device_name": str(row.get("device_name") or "").strip(),
                "generic_name": str(row.get("generic_name") or "").strip(),
                "openfda_device_name": normalize_openfda_device_name(row.get("openfda")),
                "manufacturer_or_lab": str(row.get("applicant") or "").strip(),
                "genes": genes,
                "conditions": [],
                "decision_date": str(row.get("decision_date") or "").strip(),
                "product_code": str(row.get("product_code") or "").strip(),
                "advisory_committee": str(
                    row.get("advisory_committee_description") or ""
                ).strip(),
                "regulatory_identifier": source_id,
                "region": "us",
                "matched_queries": sorted(bundle["matched_queries"]),
                "supplement_count": len(
                    {
                        value
                        for value in bundle["supplement_numbers"]
                        if value
                    }
                ),
            }
        )

    normalized.sort(key=lambda record: (record["source_db"], record["source_id"]))
    return normalized


def load_fda_molecular_slice(gene_universe: set[str]) -> SourceBundle:
    started = time.perf_counter()

    pma_query_results: list[dict[str, Any]] = []
    raw_pma_records: list[tuple[dict[str, Any], str]] = []
    for query, slug in FDA_PMA_QUERY_TERMS:
        query_result = fetch_openfda_query(OPENFDA_PMA_URL, query)
        pma_query_results.append(
            {
                "query": query,
                "slug": slug,
                "reported_total": query_result["reported_total"],
                "page_count": len(query_result["page_latencies_ms"]),
                "mean_page_latency_ms": mean(query_result["page_latencies_ms"]),
            }
        )
        raw_pma_records.extend((row, slug) for row in query_result["records"])

    k510_query_results: list[dict[str, Any]] = []
    raw_k510_records: list[tuple[dict[str, Any], str]] = []
    for query, slug in FDA_510K_QUERY_TERMS:
        query_result = fetch_openfda_query(OPENFDA_510K_URL, query)
        k510_query_results.append(
            {
                "query": query,
                "slug": slug,
                "reported_total": query_result["reported_total"],
                "page_count": len(query_result["page_latencies_ms"]),
                "mean_page_latency_ms": mean(query_result["page_latencies_ms"]),
            }
        )
        raw_k510_records.extend((row, slug) for row in query_result["records"])

    pma_records = collapse_fda_records(
        raw_pma_records,
        source_db="pma",
        gene_universe=gene_universe,
    )
    k510_records = collapse_fda_records(
        raw_k510_records,
        source_db="510k",
        gene_universe=gene_universe,
    )
    combined_records = sorted(
        [*pma_records, *k510_records],
        key=lambda record: (record["source_db"], record["source_id"]),
    )

    gene_to_records: dict[str, set[str]] = defaultdict(set)
    product_code_counts: Counter[str] = Counter()
    advisory_committee_counts: Counter[str] = Counter()
    name_present = 0
    applicant_present = 0
    decision_present = 0
    product_code_present = 0
    committee_present = 0
    records_with_gene_hits = 0
    for record in combined_records:
        identifier = f'{record["source_db"]}:{record["source_id"]}'
        name_present += int(bool(record["name"]))
        applicant_present += int(bool(record["manufacturer_or_lab"]))
        decision_present += int(bool(record["decision_date"]))
        product_code_present += int(bool(record["product_code"]))
        committee_present += int(bool(record["advisory_committee"]))
        records_with_gene_hits += int(bool(record["genes"]))
        if record["product_code"]:
            product_code_counts[record["product_code"]] += 1
        if record["advisory_committee"]:
            advisory_committee_counts[record["advisory_committee"]] += 1
        for gene in record["genes"]:
            gene_to_records[gene].add(identifier)

    total = len(combined_records)
    return SourceBundle(
        records=combined_records,
        gene_to_records=gene_to_records,
        metrics={
            "record_counts": {
                "pma_unique_records": len(pma_records),
                "k510_unique_records": len(k510_records),
                "combined_unique_records": total,
            },
            "schema_completeness": {
                "name_pct": pct(name_present, total),
                "applicant_pct": pct(applicant_present, total),
                "decision_date_pct": pct(decision_present, total),
                "product_code_pct": pct(product_code_present, total),
                "advisory_committee_pct": pct(committee_present, total),
                "gene_linked_records_pct": pct(records_with_gene_hits, total),
            },
            "query_summary": {
                "pma": pma_query_results,
                "k510": k510_query_results,
            },
            "top_product_codes": top_counts(dict(product_code_counts)),
            "top_advisory_committees": top_counts(dict(advisory_committee_counts)),
            "timing": {
                "elapsed_seconds": round(time.perf_counter() - started, 2),
            },
        },
    )


def build_gene_source_matrix(
    clinvar_gene_rows: list[dict[str, Any]],
    gtr_gene_to_tests: dict[str, set[str]],
    who_gene_to_records: dict[str, set[str]],
    fda_gene_to_records: dict[str, set[str]],
) -> dict[str, Any]:
    matrix_rows: list[dict[str, Any]] = []
    covered_by_any = 0
    covered_by_gtr = 0
    covered_by_who = 0
    covered_by_fda = 0

    for row in clinvar_gene_rows:
        gene = row["symbol"]
        gtr_count = len(gtr_gene_to_tests.get(gene, set()))
        who_count = len(who_gene_to_records.get(gene, set()))
        fda_count = len(fda_gene_to_records.get(gene, set()))
        covered = int(bool(gtr_count or who_count or fda_count))
        covered_by_any += covered
        covered_by_gtr += int(bool(gtr_count))
        covered_by_who += int(bool(who_count))
        covered_by_fda += int(bool(fda_count))
        matrix_rows.append(
            {
                **row,
                "gtr_test_count": gtr_count,
                "who_ivd_record_count": who_count,
                "fda_record_count": fda_count,
                "any_source_record_count": gtr_count + who_count + fda_count,
                "covered_by_any_source": bool(covered),
            }
        )

    total = len(clinvar_gene_rows)
    matrix_rows.sort(key=lambda row: row["symbol"])
    return {
        "rows": matrix_rows,
        "coverage_summary": {
            "clinvar_pathogenic_gene_count": total,
            "genes_with_any_source_hit": covered_by_any,
            "genes_with_any_source_hit_pct": pct(covered_by_any, total),
            "genes_with_gtr_hit": covered_by_gtr,
            "genes_with_gtr_hit_pct": pct(covered_by_gtr, total),
            "genes_with_who_hit": covered_by_who,
            "genes_with_who_hit_pct": pct(covered_by_who, total),
            "genes_with_fda_hit": covered_by_fda,
            "genes_with_fda_hit_pct": pct(covered_by_fda, total),
        },
    }


def find_named_records(
    records: list[dict[str, Any]],
    needle: str,
    *,
    fields: list[str],
    limit: int = 10,
) -> list[dict[str, Any]]:
    needle_lower = needle.lower()
    hits: list[dict[str, Any]] = []
    for record in records:
        haystack = " ".join(str(record.get(field) or "") for field in fields).lower()
        if needle_lower in haystack:
            hits.append(record)
        if len(hits) >= limit:
            break
    return hits


def build_validation_payload(
    gtr_records: list[dict[str, Any]],
    fda_records: list[dict[str, Any]],
) -> dict[str, Any]:
    return {
        "gtr": {
            "mychoice": find_named_records(
                gtr_records,
                "mychoice",
                fields=["name", "manufacturer_or_lab"],
            ),
            "foundationone": find_named_records(
                gtr_records,
                "foundationone",
                fields=["name", "manufacturer_or_lab"],
            ),
            "tempus_xt": find_named_records(
                gtr_records,
                "tempus xt",
                fields=["name", "manufacturer_or_lab"],
            ),
        },
        "fda_device": {
            "mychoice": find_named_records(
                fda_records,
                "mychoice",
                fields=["name", "trade_name", "generic_name", "manufacturer_or_lab"],
            ),
            "foundationone": find_named_records(
                fda_records,
                "foundationone",
                fields=["name", "trade_name", "generic_name", "manufacturer_or_lab"],
            ),
            "tempus_xt": find_named_records(
                fda_records,
                "tempus",
                fields=["name", "trade_name", "generic_name", "manufacturer_or_lab"],
            ),
        },
    }


def _build_full_scale_payload(
    landscape: FullScaleLandscape,
    artifact_paths: dict[str, Path | str] | None = None,
) -> dict[str, Any]:
    artifact_paths = artifact_paths or {}
    clinvar = landscape.clinvar
    gtr = landscape.gtr
    who = landscape.who
    fda = landscape.fda
    gene_matrix = landscape.gene_source_matrix
    gene_universe = {row["symbol"] for row in clinvar["genes"]}
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
    gtr_clinvar_covered = sum(1 for gene in gene_universe if gene in gtr.gene_to_records)
    who_clinvar_covered = sum(1 for gene in gene_universe if gene in who.gene_to_records)
    fda_clinvar_covered = sum(1 for gene in gene_universe if gene in fda.gene_to_records)

    return {
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
            "variant_summary_sanity_check": landscape.clinvar_variant_sanity,
        },
        "sources": {
            "gtr": {
                **gtr.metrics,
                "source_gene_count": len(gtr.gene_to_records),
                "clinvar_genes_with_any_test": gtr_clinvar_covered,
                "clinvar_genes_with_any_test_pct": round(
                    gtr_clinvar_covered / clinvar["pathogenic_gene_count"] * 100.0,
                    2,
                ),
                "top_genes_by_test_count": top_gene_counts(gtr.gene_to_records),
                "sample_extract_path": str(artifact_paths.get("gtr_sample_path") or ""),
            },
            "who_ivd": {
                **who.metrics,
                "source_gene_count": len(who.gene_to_records),
                "clinvar_genes_with_any_record": who_clinvar_covered,
                "clinvar_genes_with_any_record_pct": round(
                    who_clinvar_covered / clinvar["pathogenic_gene_count"] * 100.0,
                    2,
                ),
                "top_genes_by_record_count": top_gene_counts(who.gene_to_records),
                "sample_extract_path": str(artifact_paths.get("who_sample_path") or ""),
            },
            "fda_device": {
                **fda.metrics,
                "source_gene_count": len(fda.gene_to_records),
                "clinvar_genes_with_any_record": fda_clinvar_covered,
                "clinvar_genes_with_any_record_pct": round(
                    fda_clinvar_covered / clinvar["pathogenic_gene_count"] * 100.0,
                    2,
                ),
                "top_genes_by_record_count": top_gene_counts(fda.gene_to_records),
                "sample_extract_path": str(artifact_paths.get("fda_sample_path") or ""),
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
                        set(gtr.gene_to_records.get(gene, set()))
                        | set(who.gene_to_records.get(gene, set()))
                        | set(fda.gene_to_records.get(gene, set()))
                    )
                    for gene in gene_universe
                },
            ),
            "matrix_path": str(artifact_paths.get("matrix_rows_path") or ""),
        },
        "proposed_unified_data_model": build_unified_data_model(),
        "cli_surface_proposal": build_cli_surface(),
        "recommended_source_priority": build_source_priority(),
        "proposed_rust_module_boundaries": build_rust_module_boundaries(),
        "validation_artifact": str(artifact_paths.get("validation_path") or ""),
        "timing": {
            "elapsed_seconds": landscape.elapsed_seconds,
        },
    }


def build_full_scale_landscape(refresh: bool = False) -> FullScaleLandscape:
    started = time.perf_counter()
    clinvar = load_clinvar_gene_summary(refresh=refresh)
    clinvar_variant_sanity = load_clinvar_variant_sanity(refresh=refresh)
    gene_universe = {row["symbol"] for row in clinvar["genes"]}

    gtr = load_gtr_backbone(gene_universe=gene_universe, refresh=refresh)
    who = load_who_overlay(gene_universe, refresh=refresh)
    fda = load_fda_molecular_slice(gene_universe)

    gene_matrix = build_gene_source_matrix(
        clinvar["genes"],
        gtr.gene_to_records,
        who.gene_to_records,
        fda.gene_to_records,
    )
    validation_payload = build_validation_payload(gtr.records, fda.records)

    landscape = FullScaleLandscape(
        clinvar=clinvar,
        clinvar_variant_sanity=clinvar_variant_sanity,
        gtr=gtr,
        who=who,
        fda=fda,
        gene_source_matrix=gene_matrix,
        validation_payload=validation_payload,
        payload={},
        artifact_paths={},
        started_at=started,
    )
    landscape.payload = _build_full_scale_payload(landscape)
    return landscape


def write_full_scale_results(landscape: FullScaleLandscape) -> FullScaleLandscape:
    gtr_sample_path = write_result(
        "diagnostic_gtr_sample_100.json",
        {
            "source": "gtr",
            "record_count": len(landscape.gtr.records),
            "sample_size": 100,
            "records": select_sample(landscape.gtr.records),
        },
    )
    who_sample_path = write_result(
        "diagnostic_who_ivd_sample_100.json",
        {
            "source": "who_ivd",
            "record_count": len(landscape.who.records),
            "sample_size": 100,
            "records": select_sample(landscape.who.records),
        },
    )
    fda_sample_path = write_result(
        "diagnostic_fda_molecular_sample_100.json",
        {
            "source": "fda_device",
            "record_count": len(landscape.fda.records),
            "sample_size": 100,
            "records": select_sample(landscape.fda.records),
        },
    )
    matrix_rows_path = write_result(
        "diagnostic_gene_source_matrix.json",
        {
            "gene_universe_source": landscape.clinvar["source"],
            "row_count": len(landscape.gene_source_matrix["rows"]),
            "rows": landscape.gene_source_matrix["rows"],
        },
    )
    validation_path = write_result("diagnostic_validation.json", landscape.validation_payload)

    artifact_paths = {
        "gtr_sample_path": gtr_sample_path,
        "who_sample_path": who_sample_path,
        "fda_sample_path": fda_sample_path,
        "matrix_rows_path": matrix_rows_path,
        "validation_path": validation_path,
    }
    landscape.elapsed_seconds = round(time.perf_counter() - landscape.started_at, 2)
    payload = _build_full_scale_payload(landscape, artifact_paths)
    full_scale_path = write_result("diagnostic_full_scale_landscape.json", payload)

    landscape.payload = payload
    landscape.artifact_paths = {
        **artifact_paths,
        "full_scale_path": full_scale_path,
    }
    return landscape


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


def build_regression_control_payload() -> dict[str, Any]:
    baselines = {
        name: load_json_result(path.name)
        for name, path in BASELINE_FILES.items()
    }

    current_gtr_bulk = build_gtr_bulk_probe_payload()
    current_gtr_api = build_gtr_api_probe_payload()
    current_who_ivd = build_who_ivd_probe_payload()
    current_fda_device = build_fda_device_probe_payload()
    current_cross_source_matrix = build_cross_source_matrix_payload(
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

    return {
        "baseline_files": {name: str(path) for name, path in BASELINE_FILES.items()},
        "comparisons": comparisons,
        "overall_pass": all(item["pass"] for item in comparisons.values()),
    }
