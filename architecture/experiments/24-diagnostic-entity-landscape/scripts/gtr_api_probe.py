#!/usr/bin/env python3
from __future__ import annotations

from collections import Counter
from typing import Any

from common import GENES, DISEASES, RateLimiter, mean, pct, request_json, write_json

EINFO_URL = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/einfo.fcgi"
ESEARCH_URL = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esearch.fcgi"
ESUMMARY_URL = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esummary.fcgi"
NCBI_RATE_LIMITER = RateLimiter(0.4)


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


def main() -> None:
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

    payload = {
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
    output_path = write_json("gtr_api.json", payload)
    print(output_path)


if __name__ == "__main__":
    main()
