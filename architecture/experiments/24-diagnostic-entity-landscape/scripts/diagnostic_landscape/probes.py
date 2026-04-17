#!/usr/bin/env python3
from __future__ import annotations

import csv
import gzip
import time
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

from .io import (
    DISEASES,
    GENES,
    REQUEST_TIMEOUT,
    RateLimiter,
    contains_gene_symbol,
    contains_phrase,
    download_file,
    matched_diseases,
    mean,
    pct,
    request_json,
    split_pipe,
    top_counts,
)

GTR_TEST_VERSION_URL = "https://ftp.ncbi.nlm.nih.gov/pub/GTR/data/test_version.gz"
GTR_CONDITION_GENE_URL = "https://ftp.ncbi.nlm.nih.gov/pub/GTR/data/test_condition_gene.txt"

WHO_IVD_URL = "https://extranet.who.int/prequal/vitro-diagnostics/prequalified/in-vitro-diagnostics/export?page&_format=csv"

OPENFDA_510K_URL = "https://api.fda.gov/device/510k.json"
OPENFDA_PMA_URL = "https://api.fda.gov/device/pma.json"
CDX_DRUG_PROBES = ["pembrolizumab", "osimertinib", "vemurafenib", "trastuzumab"]

EINFO_URL = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/einfo.fcgi"
ESEARCH_URL = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esearch.fcgi"
ESUMMARY_URL = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esummary.fcgi"
NCBI_RATE_LIMITER = RateLimiter(0.4)

LIVE_GTR_GENE_TERMS = [
    "BRCA1[SYMB]",
    "EGFR[SYMB]",
    "BRAF[SYMB]",
    "KRAS[SYMB]",
    "TP53[SYMB]",
]
LIVE_GTR_DISEASE_TERMS = [
    "breast cancer[DISNAME]",
    "melanoma[DISNAME]",
    "lung cancer[DISNAME]",
]


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


def build_gtr_bulk_probe_payload(refresh: bool = False) -> dict[str, Any]:
    started = time.perf_counter()
    test_version_path = download_file(GTR_TEST_VERSION_URL, "gtr_test_version.gz", refresh=refresh)
    relation_path = download_file(GTR_CONDITION_GENE_URL, "gtr_test_condition_gene.txt", refresh=refresh)

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

    return {
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


def esearch(term: str, retmax: int = 10) -> dict[str, Any]:
    payload, latency_ms, status_code = request_json(
        ESEARCH_URL,
        params={"db": "gtr", "term": term, "retmode": "json", "retmax": retmax},
        rate_limiter=NCBI_RATE_LIMITER,
        timeout=20,
    )
    assert payload is not None
    result = payload["esearchresult"]
    return {
        "term": term,
        "count": int(result.get("count", 0)),
        "ids": list(result.get("idlist", [])),
        "query_translation": result.get("querytranslation"),
        "warnings": result.get("warninglist", {}),
        "errors": result.get("errorlist", {}),
        "latency_ms": latency_ms,
        "status_code": status_code,
    }


def esummary(ids: list[str]) -> tuple[list[dict[str, Any]], float]:
    if not ids:
        return [], 0.0
    payload, latency_ms, _ = request_json(
        ESUMMARY_URL,
        params={"db": "gtr", "id": ",".join(ids), "retmode": "json"},
        rate_limiter=NCBI_RATE_LIMITER,
        timeout=20,
    )
    assert payload is not None
    result = payload["result"]
    docs = [result[uid] for uid in result.get("uids", [])]
    return docs, latency_ms


def summarize_docs(docs: list[dict[str, Any]]) -> dict[str, Any]:
    total = len(docs)
    test_type_counts: Counter[str] = Counter()
    analytes_present = 0
    conditions_present = 0
    offerer_present = 0
    certifications_present = 0
    methods_present = 0
    specimens_present = 0
    example_records: list[dict[str, Any]] = []

    for doc in docs:
        test_type = (doc.get("testtype") or "").strip() or "Unknown"
        test_type_counts[test_type] += 1
        analytes_present += int(bool(doc.get("analytes") or doc.get("genelist")))
        conditions_present += int(bool(doc.get("conditionlist") or doc.get("conditionlist2")))
        offerer_present += int(bool(doc.get("offerer")))
        certifications_present += int(bool(doc.get("certifications")))
        methods_present += int(bool(doc.get("method")))
        specimens_present += int(bool(doc.get("specimens")))

        if len(example_records) < 5:
            example_records.append(
                {
                    "uid": doc.get("uid"),
                    "accession": doc.get("accession"),
                    "test_name": doc.get("testname"),
                    "test_type": test_type,
                    "offerer": doc.get("offerer"),
                    "certifications": doc.get("certifications"),
                    "gene_count": doc.get("genecount"),
                    "condition_count": doc.get("conditioncount"),
                }
            )

    return {
        "fetched_docs": total,
        "schema_completeness": {
            "analytes_pct": pct(analytes_present, total),
            "conditions_pct": pct(conditions_present, total),
            "offerer_pct": pct(offerer_present, total),
            "certifications_pct": pct(certifications_present, total),
            "methods_pct": pct(methods_present, total),
            "specimens_pct": pct(specimens_present, total),
        },
        "test_type_counts": dict(sorted(test_type_counts.items())),
        "examples": example_records,
    }


def build_gene_probe(gene: str) -> dict[str, Any]:
    try:
        primary = esearch(f"{gene}[SYMB]", retmax=10)
        docs, summary_latency_ms = esummary(primary["ids"])
        doc_summary = summarize_docs(docs)
        return {
            "primary_query": primary,
            "summary_latency_ms": summary_latency_ms,
            **doc_summary,
        }
    except Exception as exc:
        return {
            "primary_query": {
                "term": f"{gene}[SYMB]",
                "count": 0,
                "ids": [],
                "query_translation": None,
                "warnings": {},
                "errors": {},
                "latency_ms": None,
                "status_code": None,
            },
            "summary_latency_ms": None,
            "fetched_docs": 0,
            "schema_completeness": {},
            "test_type_counts": {},
            "examples": [],
            "error": str(exc),
        }


def build_disease_probe(disease: str) -> dict[str, Any]:
    try:
        disname = esearch(f"{disease}[DISNAME]", retmax=10)
        docs, summary_latency_ms = esummary(disname["ids"])
        doc_summary = summarize_docs(docs)
        return {
            "query": {
                "term": disname["term"],
                "count": disname["count"],
                "query_translation": disname["query_translation"],
                "latency_ms": disname["latency_ms"],
            },
            "summary_latency_ms": summary_latency_ms,
            **doc_summary,
        }
    except Exception as exc:
        return {
            "query": {
                "term": f"{disease}[DISNAME]",
                "count": 0,
                "query_translation": None,
                "latency_ms": None,
            },
            "summary_latency_ms": None,
            "fetched_docs": 0,
            "schema_completeness": {},
            "test_type_counts": {},
            "examples": [],
            "error": str(exc),
        }


def build_type_query(term: str) -> dict[str, Any]:
    try:
        probe = esearch(term, retmax=5)
        docs, summary_latency_ms = esummary(probe["ids"])
        doc_summary = summarize_docs(docs)
        return {
            "query": probe,
            "summary_latency_ms": summary_latency_ms,
            **doc_summary,
        }
    except Exception as exc:
        return {
            "query": {
                "term": term,
                "count": 0,
                "ids": [],
                "query_translation": None,
                "warnings": {},
                "errors": {},
                "latency_ms": None,
                "status_code": None,
            },
            "summary_latency_ms": None,
            "fetched_docs": 0,
            "schema_completeness": {},
            "test_type_counts": {},
            "examples": [],
            "error": str(exc),
        }


def build_gtr_api_probe_payload() -> dict[str, Any]:
    try:
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
    except Exception as exc:
        tracked_fields = {"error": str(exc)}

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
                [probe["summary_latency_ms"] for probe in gene_queries.values() if probe["summary_latency_ms"] is not None]
            ),
            "mean_disease_search_latency_ms": mean(
                [probe["query"]["latency_ms"] for probe in disease_queries.values() if probe["query"]["latency_ms"] is not None]
            ),
            "mean_disease_summary_latency_ms": mean(
                [probe["summary_latency_ms"] for probe in disease_queries.values() if probe["summary_latency_ms"] is not None]
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


def load_rows(path: Path) -> list[dict[str, str]]:
    with path.open("rt", encoding="utf-8", newline="") as handle:
        return list(csv.DictReader(handle))


def build_who_ivd_probe_payload(refresh: bool = False) -> dict[str, Any]:
    csv_path = download_file(WHO_IVD_URL, "who_ivd.csv", refresh=refresh)
    rows = load_rows(csv_path)

    assay_format_counts: Counter[str] = Counter()
    manufacturer_present = 0
    marker_present = 0
    regulatory_version_present = 0
    year_present = 0
    regulatory_present = 0

    sample_gene_matches: dict[str, list[dict[str, Any]]] = {gene: [] for gene in GENES}
    sample_disease_matches: dict[str, list[dict[str, Any]]] = {disease: [] for disease in DISEASES}
    gene_match_seen: dict[str, set[str]] = {gene: set() for gene in GENES}
    disease_match_seen: dict[str, set[str]] = {disease: set() for disease in DISEASES}

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
            assay_format_counts[assay_format] += 1

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
        "file": str(csv_path),
        "record_counts": {
            "rows": total,
        },
        "schema_completeness": {
            "manufacturer_pct": pct(manufacturer_present, total),
            "pathogen_disease_marker_pct": pct(marker_present, total),
            "regulatory_version_pct": pct(regulatory_version_present, total),
            "prequalification_year_pct": pct(year_present, total),
            "regulatory_metadata_pct": pct(regulatory_present, total),
        },
        "assay_formats": top_counts(dict(assay_format_counts)),
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
            "all_sample_diseases_have_hits": all(sample_disease_matches[disease] for disease in DISEASES),
        },
    }


def fetch_510k_sample(limit: int = 100) -> dict[str, Any]:
    payload, latency_ms, _ = request_json(OPENFDA_510K_URL, params={"limit": limit})
    assert payload is not None
    return {
        "total": int(payload["meta"]["results"]["total"]),
        "latency_ms": latency_ms,
        "results": payload["results"],
    }


def search_openfda(
    url: str,
    query: str,
    *,
    limit: int = 25,
    allow_404: bool = True,
) -> dict[str, Any]:
    payload, latency_ms, status_code = request_json(
        url,
        params={"search": query, "limit": limit},
        allow_404=allow_404,
    )
    if payload is None:
        return {
            "query": query,
            "count": 0,
            "latency_ms": latency_ms,
            "status_code": status_code,
            "results": [],
        }
    return {
        "query": query,
        "count": int(payload["meta"]["results"]["total"]),
        "latency_ms": latency_ms,
        "status_code": status_code,
        "results": payload["results"],
    }


def relevant_gene_matches(results: list[dict[str, Any]], gene: str) -> list[dict[str, Any]]:
    matches: list[dict[str, Any]] = []
    for row in results:
        candidate_text = " ".join(
            part
            for part in [
                row.get("device_name"),
                " ".join((row.get("openfda") or {}).get("device_name", [])),
            ]
            if part
        )
        if contains_gene_symbol(candidate_text, gene):
            matches.append(row)
    return matches


def relevant_disease_matches(results: list[dict[str, Any]], disease: str) -> list[dict[str, Any]]:
    matches: list[dict[str, Any]] = []
    for row in results:
        candidate_text = " ".join(
            part
            for part in [
                row.get("device_name"),
                " ".join((row.get("openfda") or {}).get("device_name", [])),
            ]
            if part
        )
        if contains_phrase(candidate_text, disease):
            matches.append(row)
    return matches


def summarize_examples(results: list[dict[str, Any]]) -> list[dict[str, Any]]:
    examples: list[dict[str, Any]] = []
    for row in results[:10]:
        examples.append(
            {
                "k_number": row.get("k_number"),
                "device_name": row.get("device_name"),
                "applicant": row.get("applicant"),
                "decision_date": row.get("decision_date"),
                "decision_description": row.get("decision_description"),
                "advisory_committee_description": row.get("advisory_committee_description"),
                "product_code": row.get("product_code"),
            }
        )
    return examples


def exact_phrase_matches(results: list[dict[str, Any]], phrase: str) -> list[dict[str, Any]]:
    phrase_lower = phrase.lower()
    return [
        row
        for row in results
        if phrase_lower in (row.get("device_name") or "").lower()
    ]


def build_fda_device_probe_payload() -> dict[str, Any]:
    sample = fetch_510k_sample(limit=100)
    records = sample["results"]
    total = len(records)
    manufacturer_present = sum(1 for row in records if row.get("applicant"))
    decision_present = sum(1 for row in records if row.get("decision_date"))
    device_name_present = sum(1 for row in records if row.get("device_name"))
    committee_present = sum(1 for row in records if row.get("advisory_committee_description"))
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
        query_result = search_openfda(OPENFDA_510K_URL, f'device_name:"{disease}"', limit=25)
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

    companion_510k = search_openfda(OPENFDA_510K_URL, "device_name:companion diagnostic", limit=100)
    companion_pma = search_openfda(OPENFDA_PMA_URL, "device_name:companion diagnostic", limit=100)
    exact_companion_510k = exact_phrase_matches(companion_510k["results"], "companion diagnostic")
    exact_companion_pma = exact_phrase_matches(companion_pma["results"], "companion diagnostic")
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


def overlap_count(left: list[str], right: list[str]) -> int:
    return len(set(left) & set(right))


def build_cross_source_matrix_payload(
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

    return {
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


def run_gtr_round(round_index: int, terms: list[str], label: str) -> dict[str, object]:
    limiter = RateLimiter(0.8)
    probes = []
    search_latencies = []
    summary_latencies = []

    for term in terms:
        payload, search_latency_ms, _ = request_json(
            ESEARCH_URL,
            params={"db": "gtr", "term": term, "retmode": "json", "retmax": 10},
            rate_limiter=limiter,
            timeout=20,
        )
        assert payload is not None
        ids = payload["esearchresult"].get("idlist", [])
        _, summary_latency_ms, _ = request_json(
            ESUMMARY_URL,
            params={"db": "gtr", "id": ",".join(ids), "retmode": "json"},
            rate_limiter=limiter,
            timeout=20,
        )
        probes.append(
            {
                "term": term,
                "search_latency_ms": search_latency_ms,
                "summary_latency_ms": summary_latency_ms,
                "id_count": len(ids),
            }
        )
        search_latencies.append(search_latency_ms)
        summary_latencies.append(summary_latency_ms)

    return {
        "label": label,
        "round": round_index,
        "mean_search_latency_ms": mean(search_latencies),
        "mean_summary_latency_ms": mean(summary_latencies),
        "probes": probes,
    }


def run_openfda_sample_probes() -> dict[str, object]:
    runs = []
    for index in range(3):
        payload, latency_ms, _ = request_json(
            OPENFDA_510K_URL,
            params={"limit": 100},
            timeout=REQUEST_TIMEOUT,
        )
        assert payload is not None
        runs.append(
            {
                "run": index,
                "latency_ms": latency_ms,
                "reported_total": int(payload["meta"]["results"]["total"]),
            }
        )
        time.sleep(1.0)

    return {
        "runs": runs,
        "mean_latency_ms": mean([run["latency_ms"] for run in runs]),
    }


def build_live_latency_noise_probe_payload() -> dict[str, Any]:
    return {
        "purpose": "Document live-service latency variance for regression-control waivers.",
        "gtr_gene_rounds": [run_gtr_round(index, LIVE_GTR_GENE_TERMS, "gene") for index in range(2)],
        "gtr_disease_rounds": [run_gtr_round(index, LIVE_GTR_DISEASE_TERMS, "disease") for index in range(2)],
        "openfda_510k_sample_runs": run_openfda_sample_probes(),
    }
