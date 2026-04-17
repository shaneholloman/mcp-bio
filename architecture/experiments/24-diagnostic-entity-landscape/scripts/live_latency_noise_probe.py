#!/usr/bin/env python3
from __future__ import annotations

import time

from common import RateLimiter, mean, request_json
from diagnostic_landscape_lib import write_result
from fda_device_probe import OPENFDA_510K_URL

ESEARCH_URL = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esearch.fcgi"
ESUMMARY_URL = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esummary.fcgi"
GENE_TERMS = ["BRCA1[SYMB]", "EGFR[SYMB]", "BRAF[SYMB]", "KRAS[SYMB]", "TP53[SYMB]"]
DISEASE_TERMS = ["breast cancer[DISNAME]", "melanoma[DISNAME]", "lung cancer[DISNAME]"]


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
            timeout=120,
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


def main() -> None:
    gtr_gene_rounds = [run_gtr_round(index, GENE_TERMS, "gene") for index in range(2)]
    gtr_disease_rounds = [run_gtr_round(index, DISEASE_TERMS, "disease") for index in range(2)]
    payload = {
        "purpose": "Document live-service latency variance for regression-control waivers.",
        "gtr_gene_rounds": gtr_gene_rounds,
        "gtr_disease_rounds": gtr_disease_rounds,
        "openfda_510k_sample_runs": run_openfda_sample_probes(),
    }
    output_path = write_result("diagnostic_live_latency_noise_probe.json", payload)
    print(output_path)


if __name__ == "__main__":
    main()
