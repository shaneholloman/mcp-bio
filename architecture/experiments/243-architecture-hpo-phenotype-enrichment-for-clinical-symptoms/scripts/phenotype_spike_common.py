#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


SLUG = "243-architecture-hpo-phenotype-enrichment-for-clinical-symptoms"
EXPERIMENT_DIR = Path(__file__).resolve().parents[1]
RESULTS_DIR = EXPERIMENT_DIR / "results"
USER_AGENT = os.environ.get(
    "PHENOTYPE_SPIKE_USER_AGENT",
    "BioMCP phenotype-enrichment architecture spike 243 (mailto:devnull@example.org)",
)
HTTP_TIMEOUT_SECONDS = float(os.environ.get("PHENOTYPE_SPIKE_TIMEOUT", "25"))


DISEASES: list[dict[str, Any]] = [
    {
        "key": "uterine_fibroid",
        "label": "uterine fibroid",
        "biomcp_query": "uterine leiomyoma",
        "ticket_id": "ICD10CM:D25",
        "identifiers": {
            "icd10": "D25",
            "mesh": "D007889",
            "omim": "150699",
            "snomed": "44598004",
        },
        "wikidata_labels": [
            "uterine fibroid",
            "uterine leiomyoma",
            "leiomyoma of uterus",
        ],
        "source_queries": [
            "uterine fibroid",
            "uterine leiomyoma",
            "leiomyoma of uterus",
        ],
        "expected_symptoms": {
            "heavy menstrual bleeding": [
                "heavy menstrual bleeding",
                "heavy bleeding",
                "heavy or painful periods",
                "menorrhagia",
                "abnormal uterine bleeding",
                "bleeding between periods",
                "prolonged menstrual bleeding",
            ],
            "pelvic pain": ["pelvic pain", "pelvic pressure", "feeling full in the lower abdomen"],
            "lower back pain": ["back pain", "lower back pain"],
            "fatigue": ["fatigue", "tiredness"],
            "urinary frequency": ["urinary frequency", "frequent urination", "urinating often"],
            "constipation": ["constipation"],
            "infertility": ["infertility", "subfertility"],
            "dyspareunia": ["dyspareunia", "painful intercourse", "pain during sex"],
        },
    },
    {
        "key": "endometriosis",
        "label": "endometriosis",
        "biomcp_query": "endometriosis",
        "identifiers": {
            "icd10": "N80",
            "mesh": "D004715",
            "snomed": "11871002",
        },
        "wikidata_labels": ["endometriosis"],
        "source_queries": ["endometriosis"],
        "expected_symptoms": {
            "pelvic pain": ["pelvic pain", "chronic pelvic pain"],
            "dysmenorrhea": [
                "dysmenorrhea",
                "painful periods",
                "menstrual pain",
                "painful menstrual cramps",
            ],
            "dyspareunia": ["dyspareunia", "painful intercourse", "pain during or after sex"],
            "infertility": ["infertility", "subfertility"],
            "dyschezia": ["dyschezia", "painful defecation", "pain with bowel movements"],
            "dysuria": ["dysuria", "painful urination", "pain with urination"],
            "abdominal pain": ["abdominal pain"],
        },
    },
    {
        "key": "chronic_venous_insufficiency",
        "label": "chronic venous insufficiency",
        "biomcp_query": "chronic venous insufficiency",
        "identifiers": {
            "doid": "DOID:0050853",
            "umls": "C1306557",
        },
        "wikidata_labels": [
            "chronic venous insufficiency",
            "venous insufficiency",
            "venous ulcer",
            "venous leg ulcer",
        ],
        "source_queries": [
            "chronic venous insufficiency",
            "venous insufficiency",
            "venous leg ulcer",
        ],
        "expected_symptoms": {
            "leg swelling": ["leg swelling", "swelling", "edema", "oedema"],
            "leg pain": ["leg pain", "pain"],
            "varicose veins": ["varicose veins", "varicosities"],
            "venous ulcer": ["venous ulcer", "leg ulcer", "skin ulcer"],
            "skin discoloration": ["skin discoloration", "hyperpigmentation"],
            "stasis dermatitis": ["stasis dermatitis", "venous eczema"],
            "itching": ["itching", "pruritus"],
            "heaviness": ["heaviness", "heavy legs"],
        },
    },
]


def utc_now_iso() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat()


def ensure_results_dir() -> None:
    RESULTS_DIR.mkdir(parents=True, exist_ok=True)


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def normalize_text(value: str) -> str:
    return re.sub(r"\s+", " ", re.sub(r"[^a-z0-9]+", " ", value.lower())).strip()


def expected_overlap(terms: list[str], expected: dict[str, list[str]]) -> dict[str, Any]:
    normalized_terms = [normalize_text(term) for term in terms if term and term.strip()]
    matched: dict[str, list[str]] = {}
    missing: list[str] = []

    for concept, patterns in expected.items():
        concept_matches: list[str] = []
        normalized_patterns = [normalize_text(pattern) for pattern in patterns]
        for term, normalized_term in zip(terms, normalized_terms, strict=False):
            for pattern in normalized_patterns:
                if not pattern:
                    continue
                if pattern in normalized_term or normalized_term in pattern:
                    concept_matches.append(term)
                    break
        if concept_matches:
            matched[concept] = sorted(set(concept_matches))
        else:
            missing.append(concept)

    total = len(expected)
    return {
        "expected_total": total,
        "matched_total": len(matched),
        "recall": round(len(matched) / total, 3) if total else None,
        "matched": matched,
        "missing": missing,
    }


def run_json_command(args: list[str], timeout_seconds: float = 90) -> dict[str, Any]:
    started = time.perf_counter()
    proc = subprocess.run(
        args,
        check=False,
        capture_output=True,
        text=True,
        timeout=timeout_seconds,
    )
    elapsed_ms = round((time.perf_counter() - started) * 1000, 1)
    result: dict[str, Any] = {
        "command": args,
        "exit_code": proc.returncode,
        "elapsed_ms": elapsed_ms,
    }
    if proc.stdout.strip():
        try:
            result["json"] = json.loads(proc.stdout)
        except json.JSONDecodeError:
            result["stdout"] = proc.stdout[-4000:]
    if proc.stderr.strip():
        result["stderr"] = proc.stderr[-4000:]
    return result


def http_json(
    url: str,
    *,
    method: str = "GET",
    data: dict[str, str] | None = None,
    headers: dict[str, str] | None = None,
    timeout_seconds: float = HTTP_TIMEOUT_SECONDS,
) -> dict[str, Any]:
    encoded: bytes | None = None
    request_url = url
    request_headers = {
        "Accept": "application/json",
        "User-Agent": USER_AGENT,
    }
    if headers:
        request_headers.update(headers)
    if data is not None:
        encoded = urllib.parse.urlencode(data).encode("utf-8")
        request_headers["Content-Type"] = "application/x-www-form-urlencoded"

    req = urllib.request.Request(
        request_url,
        data=encoded,
        headers=request_headers,
        method=method,
    )
    started = time.perf_counter()
    try:
        with urllib.request.urlopen(req, timeout=timeout_seconds) as resp:
            body = resp.read()
            elapsed_ms = round((time.perf_counter() - started) * 1000, 1)
            return {
                "ok": True,
                "status": resp.status,
                "elapsed_ms": elapsed_ms,
                "json": json.loads(body.decode("utf-8")),
            }
    except urllib.error.HTTPError as exc:
        body = exc.read().decode("utf-8", errors="replace")
        elapsed_ms = round((time.perf_counter() - started) * 1000, 1)
        return {
            "ok": False,
            "status": exc.code,
            "elapsed_ms": elapsed_ms,
            "error": body[:1000],
        }
    except Exception as exc:  # noqa: BLE001 - probes must record failures.
        elapsed_ms = round((time.perf_counter() - started) * 1000, 1)
        return {
            "ok": False,
            "status": None,
            "elapsed_ms": elapsed_ms,
            "error": f"{type(exc).__name__}: {exc}",
        }


def sparql_json(query: str) -> dict[str, Any]:
    return http_json(
        "https://query.wikidata.org/sparql",
        method="POST",
        data={"query": query, "format": "json"},
        headers={"Accept": "application/sparql-results+json"},
        timeout_seconds=45,
    )


def main_guard() -> None:
    if sys.version_info < (3, 11):
        raise SystemExit("Python 3.11+ required")
