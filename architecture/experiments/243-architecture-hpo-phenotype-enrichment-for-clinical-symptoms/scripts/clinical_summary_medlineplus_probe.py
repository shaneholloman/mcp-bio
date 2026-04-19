#!/usr/bin/env python3
from __future__ import annotations

import html
import re
import time
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET
from typing import Any

from phenotype_spike_common import (
    DISEASES,
    HTTP_TIMEOUT_SECONDS,
    RESULTS_DIR,
    USER_AGENT,
    ensure_results_dir,
    expected_overlap,
    main_guard,
    utc_now_iso,
    write_json,
)


def clean_text(value: str) -> str:
    value = html.unescape(value)
    value = re.sub(r"(?is)<[^>]+>", " ", value)
    return re.sub(r"\s+", " ", value).strip()


def medlineplus_search(query: str) -> dict[str, Any]:
    params = urllib.parse.urlencode(
        {
            "db": "healthTopics",
            "term": query,
            "retmax": "3",
        }
    )
    url = f"https://wsearch.nlm.nih.gov/ws/query?{params}"
    req = urllib.request.Request(
        url,
        headers={
            "Accept": "application/xml",
            "User-Agent": USER_AGENT,
        },
        method="GET",
    )
    started = time.perf_counter()
    try:
        with urllib.request.urlopen(req, timeout=HTTP_TIMEOUT_SECONDS) as resp:
            body = resp.read().decode("utf-8", errors="replace")
            root = ET.fromstring(body)
            topics: list[dict[str, str]] = []
            for doc in root.findall(".//document"):
                row: dict[str, str] = {"url": doc.attrib.get("url", "")}
                for content in doc.findall("content"):
                    name = content.attrib.get("name", "")
                    text = clean_text(content.text or "")
                    if name == "title" and text:
                        row["title"] = text
                    elif name in {"FullSummary", "snippet"} and text and "summary" not in row:
                        row["summary"] = text
                if row.get("title"):
                    topics.append(row)
            return {
                "ok": True,
                "status": resp.status,
                "elapsed_ms": round((time.perf_counter() - started) * 1000, 1),
                "topics": topics,
            }
    except Exception as exc:  # noqa: BLE001 - probe records failures.
        return {
            "ok": False,
            "status": None,
            "elapsed_ms": round((time.perf_counter() - started) * 1000, 1),
            "error": f"{type(exc).__name__}: {exc}",
            "topics": [],
        }


def summarize_disease(disease: dict[str, Any]) -> dict[str, Any]:
    attempts: list[dict[str, Any]] = []
    seen_urls: set[str] = set()
    topics: list[dict[str, str]] = []
    for query in disease["source_queries"]:
        response = medlineplus_search(query)
        attempts.append(
            {
                "query": query,
                "ok": response["ok"],
                "status": response["status"],
                "elapsed_ms": response["elapsed_ms"],
                "topic_count": len(response.get("topics", [])),
                "error": response.get("error"),
            }
        )
        for topic in response.get("topics", []):
            url = topic.get("url", "")
            if url in seen_urls:
                continue
            seen_urls.add(url)
            topics.append(topic)

    text_terms: list[str] = []
    for topic in topics:
        text_terms.append(topic.get("title", ""))
        text_terms.append(topic.get("summary", ""))
    return {
        "disease_key": disease["key"],
        "label": disease["label"],
        "attempts": attempts,
        "topic_count": len(topics),
        "topics": topics,
        "expected_symptom_overlap": expected_overlap(text_terms, disease["expected_symptoms"]),
    }


def main() -> None:
    main_guard()
    ensure_results_dir()
    diseases = [summarize_disease(disease) for disease in DISEASES]
    total_expected = sum(
        row["expected_symptom_overlap"]["expected_total"] for row in diseases
    )
    total_matched = sum(row["expected_symptom_overlap"]["matched_total"] for row in diseases)
    payload = {
        "generated_at": utc_now_iso(),
        "approach": "source_native_medlineplus_clinical_summary",
        "metric_definitions": {
            "topic_count": "Number of MedlinePlus health topic search results retained for the disease query.",
            "expected_symptom_recall": "Manual small-set lexical recall against expected recognizable clinical symptoms using topic titles and summaries.",
        },
        "source_metadata": {
            "quality_tier": "plain-language NLM clinical summary",
            "api_availability": "public MedlinePlus health topic search XML API",
            "refresh_cadence": "MedlinePlus XML files are updated Tuesday-Saturday",
            "source_url": "https://wsearch.nlm.nih.gov/ws/query",
            "integration_note": "Useful as a source-native clinical summary fallback, but requires extraction and HPO mapping before becoming structured phenotypes.",
        },
        "summary": {
            "disease_count": len(diseases),
            "diseases_with_topic": sum(1 for row in diseases if row["topic_count"] > 0),
            "total_topics": sum(row["topic_count"] for row in diseases),
            "total_expected_symptoms": total_expected,
            "total_matched_expected_symptoms": total_matched,
            "expected_symptom_recall": round(total_matched / total_expected, 3)
            if total_expected
            else None,
        },
        "diseases": diseases,
    }
    write_json(RESULTS_DIR / "clinical_summary_medlineplus_probe.json", payload)


if __name__ == "__main__":
    main()
