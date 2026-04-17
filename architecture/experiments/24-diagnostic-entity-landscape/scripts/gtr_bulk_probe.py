#!/usr/bin/env python3
from __future__ import annotations

import csv
import gzip
import time
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

from common import GENES, DISEASES, download_file, mean, pct, split_pipe, top_counts, write_json

GTR_TEST_VERSION_URL = "https://ftp.ncbi.nlm.nih.gov/pub/GTR/data/test_version.gz"
GTR_CONDITION_GENE_URL = "https://ftp.ncbi.nlm.nih.gov/pub/GTR/data/test_condition_gene.txt"


def load_current_tests(path: Path) -> tuple[dict[str, dict[str, Any]], dict[str, Any]]:
    current_tests: dict[str, dict[str, Any]] = {}
    test_type_counts: Counter[str] = Counter()
    country_counts: Counter[str] = Counter()
    method_category_counts: Counter[str] = Counter()
    method_counts: Counter[str] = Counter()
    manufacturer_present = 0
    clia_present = 0
    state_license_present = 0
    regulatory_present = 0

    with gzip.open(path, "rt", encoding="utf-8", newline="") as handle:
        reader = csv.DictReader(handle, delimiter="\t")
        for row in reader:
            if row.get("now_current") != "1":
                continue
            accession = (row.get("test_accession_ver") or "").strip()
            if not accession:
                continue

            methods = split_pipe(row.get("methods"))
            method_categories = split_pipe(row.get("method_categories"))
            laboratory = (row.get("name_of_laboratory") or "").strip()
            clia_number = (row.get("CLIA_number") or "").strip()
            state_licenses = (row.get("state_licenses") or "").strip()
            country = (row.get("facility_country") or "").strip()
            test_type = (row.get("test_type") or "").strip() or "Unknown"

            manufacturer_present += int(bool(laboratory))
            clia_present += int(bool(clia_number))
            state_license_present += int(bool(state_licenses))
            regulatory_present += int(bool(clia_number or state_licenses))

            test_type_counts[test_type] += 1
            if country:
                country_counts[country] += 1
            for value in method_categories:
                method_category_counts[value] += 1
            for value in methods:
                method_counts[value] += 1

            current_tests[accession] = {
                "accession": accession,
                "name": (row.get("lab_test_name") or "").strip(),
                "manufacturer_test_name": (row.get("manufacturer_test_name") or "").strip(),
                "test_type": test_type,
                "laboratory": laboratory,
                "institution": (row.get("name_of_institution") or "").strip(),
                "country": country,
                "clia_number": clia_number,
                "state_licenses": state_licenses,
                "test_current_status": (row.get("test_currStat") or "").strip(),
                "test_public_status": (row.get("test_pubStat") or "").strip(),
                "method_categories": method_categories,
                "methods": methods,
                "gene_field": split_pipe(row.get("genes")),
                "condition_field": split_pipe(row.get("condition_identifiers")),
            }

    total = len(current_tests)
    summary = {
        "current_test_count": total,
        "test_type_counts": dict(sorted(test_type_counts.items())),
        "top_countries": top_counts(dict(country_counts)),
        "top_method_categories": top_counts(dict(method_category_counts)),
        "top_methods": top_counts(dict(method_counts)),
        "schema_completeness": {
            "manufacturer_or_lab_name_pct": pct(manufacturer_present, total),
            "clia_number_pct": pct(clia_present, total),
            "state_licenses_pct": pct(state_license_present, total),
            "any_regulatory_metadata_pct": pct(regulatory_present, total),
        },
    }
    return current_tests, summary


def parse_relation_file(
    path: Path,
    current_tests: dict[str, dict[str, Any]],
) -> tuple[dict[str, set[str]], dict[str, set[str]], dict[str, Any]]:
    genes_by_test: dict[str, set[str]] = defaultdict(set)
    diseases_by_test: dict[str, set[str]] = defaultdict(set)
    sample_gene_matches: dict[str, list[dict[str, Any]]] = {gene: [] for gene in GENES}
    sample_disease_matches: dict[str, list[dict[str, Any]]] = {disease: [] for disease in DISEASES}
    gene_match_seen: dict[str, set[str]] = {gene: set() for gene in GENES}
    disease_match_seen: dict[str, set[str]] = {disease: set() for disease in DISEASES}

    with path.open("rt", encoding="utf-8", newline="") as handle:
        reader = csv.DictReader(handle, delimiter="\t")
        for row in reader:
            accession = (row.get("#accession_version") or row.get("accession_version") or "").strip()
            if accession not in current_tests:
                continue

            object_type = (row.get("object") or "").strip().lower()
            object_name = (row.get("object_name") or "").strip()
            gene_symbol = (row.get("gene_symbol") or "").strip()
            test_info = current_tests[accession]

            if object_type == "gene":
                symbol = gene_symbol if gene_symbol and gene_symbol != "N/A" else ""
                if symbol:
                    genes_by_test[accession].add(symbol)
                for gene in GENES:
                    if symbol == gene and accession not in gene_match_seen[gene]:
                        gene_match_seen[gene].add(accession)
                        sample_gene_matches[gene].append(
                            {
                                "accession": accession,
                                "name": test_info["name"],
                                "laboratory": test_info["laboratory"],
                                "test_type": test_info["test_type"],
                                "country": test_info["country"],
                            }
                        )
            elif object_type == "condition":
                if object_name:
                    diseases_by_test[accession].add(object_name)
                for disease in DISEASES:
                    if disease in object_name.lower() and accession not in disease_match_seen[disease]:
                        disease_match_seen[disease].add(accession)
                        sample_disease_matches[disease].append(
                            {
                                "accession": accession,
                                "name": test_info["name"],
                                "laboratory": test_info["laboratory"],
                                "matched_condition": object_name,
                                "test_type": test_info["test_type"],
                            }
                        )

    summary = {
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
    }
    return genes_by_test, diseases_by_test, summary


def main() -> None:
    started = time.perf_counter()
    test_version_path = download_file(GTR_TEST_VERSION_URL, "gtr_test_version.gz")
    relation_path = download_file(GTR_CONDITION_GENE_URL, "gtr_test_condition_gene.txt")

    current_tests, current_summary = load_current_tests(test_version_path)
    genes_by_test, diseases_by_test, relation_summary = parse_relation_file(relation_path, current_tests)

    gene_counts: list[int] = []
    disease_counts: list[int] = []
    tests_with_gene_links = 0
    tests_with_disease_links = 0

    sample_name_index: set[str] = set()
    for gene_payload in relation_summary["sample_gene_matches"].values():
        for example in gene_payload["examples"]:
            sample_name_index.add(example["name"].strip().lower())
    for disease_payload in relation_summary["sample_disease_matches"].values():
        for example in disease_payload["examples"]:
            sample_name_index.add(example["name"].strip().lower())

    for accession, test_info in current_tests.items():
        gene_values = set(genes_by_test.get(accession, set())) | set(test_info["gene_field"])
        disease_values = set(diseases_by_test.get(accession, set())) | set(test_info["condition_field"])
        gene_counts.append(len(gene_values))
        disease_counts.append(len(disease_values))
        tests_with_gene_links += int(bool(gene_values))
        tests_with_disease_links += int(bool(disease_values))

    payload = {
        "approach": "GTR bulk download parse",
        "source": "gtr",
        "files": {
            "test_version": str(test_version_path),
            "test_condition_gene": str(relation_path),
        },
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
                gene: relation_summary["sample_gene_matches"][gene]["count"] for gene in GENES
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
                relation_summary["sample_gene_matches"][gene]["count"] > 0 for gene in GENES
            ),
        },
        "timing": {
            "elapsed_seconds": round(time.perf_counter() - started, 2),
        },
    }
    output_path = write_json("gtr_bulk.json", payload)
    print(output_path)


if __name__ == "__main__":
    main()
