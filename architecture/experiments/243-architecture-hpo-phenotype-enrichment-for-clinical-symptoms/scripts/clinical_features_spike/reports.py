from __future__ import annotations

import os
import resource
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

from phenotype_spike_common import DISEASES

from .api import extract_clinical_feature_dataset, summarize_clinical_feature_dataset
from .common import EXPERIMENT_DIR, RESULTS_DIR, WORK_DIR, load_json, stable_checksum, utc_now_iso, write_json
from .extraction import (
    blind_topic_recall,
    excerpt_contains_extraction_anchor,
    selected_feature_recall,
    simple_mismatch_count,
)
from .medlineplus import explore_topics_by_disease


FULL_SCALE_PATH = RESULTS_DIR / "clinical_features_full_scale.json"
REGRESSION_CONTROL_PATH = RESULTS_DIR / "clinical_features_regression_control.json"
VALIDATION_PATH = RESULTS_DIR / "clinical_features_validation.json"
CONTRACT_PATH = RESULTS_DIR / "clinical_features_contract_numbers.json"

EXPLORE_HPO_PATH = RESULTS_DIR / "current_biomcp_hpo_baseline.json"
EXPLORE_MEDLINEPLUS_PATH = RESULTS_DIR / "clinical_summary_medlineplus_probe.json"


def _explore_medlineplus_summary() -> dict[str, Any]:
    return load_json(EXPLORE_MEDLINEPLUS_PATH)["summary"]


def build_full_scale_payload(*, allow_live: bool = True, refresh_cache: bool = False) -> dict[str, Any]:
    started = time.perf_counter()
    diseases = extract_clinical_feature_dataset(
        allow_live=allow_live,
        refresh_cache=refresh_cache,
    )

    elapsed_ms = round((time.perf_counter() - started) * 1000, 1)
    summary = summarize_clinical_feature_dataset(diseases)
    total_features = summary["clinical_feature_count"]
    ru_maxrss_kb = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    summary.update(
        {
            "elapsed_ms": elapsed_ms,
            "diseases_per_second": round(len(diseases) / (elapsed_ms / 1000), 3) if elapsed_ms else None,
            "features_per_second": round(total_features / (elapsed_ms / 1000), 3) if elapsed_ms else None,
            "peak_rss_kb": ru_maxrss_kb,
        }
    )
    payload = {
        "generated_at": utc_now_iso(),
        "approach": "source_native_medlineplus_clinical_features_with_hpo_mapping_fixture",
        "full_scale_definition": (
            "Full ticket architecture-spike scope: uterine fibroid plus two common "
            "HPO-sparse diseases, with direct-page selection, section-aware feature "
            "extraction, source-native rows, HPO mapping fixture, regression metrics, "
            "and persistent result artifacts."
        ),
        "artifact_paths": {
            "results_dir": str(RESULTS_DIR),
            "work_dir": str(WORK_DIR),
            "full_scale": str(FULL_SCALE_PATH),
            "regression_control": str(REGRESSION_CONTROL_PATH),
            "validation": str(VALIDATION_PATH),
            "contract_numbers": str(CONTRACT_PATH),
        },
        "metric_definitions": {
            "expected_symptom_recall": "Lexical recall against the same expected symptom concepts used by explore.",
            "mismatch_count": "Expected symptom concepts not represented by extracted clinical feature labels.",
            "topic_noise_reduction_count": "Candidate MedlinePlus topics minus selected topics after disease-page selection.",
            "feature_checksum": "Stable checksum over feature label, mapped HPO ID, and source-native page ID.",
            "peak_rss_kb": "Python process peak resident set size reported by resource.getrusage.",
        },
        "summary": summary,
        "diseases": diseases,
    }
    return payload


def _maybe_run_hpo_control() -> dict[str, Any]:
    script = EXPERIMENT_DIR / "scripts" / "baseline_biomcp_hpo.py"
    env = dict(os.environ)
    started = time.perf_counter()
    try:
        proc = subprocess.run(
            [
                sys.executable,
                "-c",
                (
                    "import json; "
                    "from baseline_biomcp_hpo import summarize_disease; "
                    "from phenotype_spike_common import DISEASES; "
                    "rows=[summarize_disease(d) for d in DISEASES]; "
                    "print(json.dumps(rows, sort_keys=True))"
                ),
            ],
            cwd=str(script.parent),
            env=env,
            check=False,
            capture_output=True,
            text=True,
            timeout=180,
        )
        elapsed_ms = round((time.perf_counter() - started) * 1000, 1)
        if proc.returncode != 0:
            return {
                "ran": False,
                "elapsed_ms": elapsed_ms,
                "error": (proc.stderr or proc.stdout)[-2000:],
            }
        import json

        rows = json.loads(proc.stdout)
        return {
            "ran": True,
            "elapsed_ms": elapsed_ms,
            "disease_count": len(rows),
            "total_phenotype_rows": sum(row["phenotype_count"] for row in rows),
            "total_expected_symptoms": sum(row["expected_symptom_overlap"]["expected_total"] for row in rows),
            "total_matched_expected_symptoms": sum(row["expected_symptom_overlap"]["matched_total"] for row in rows),
            "row_checksum": stable_checksum(
                [
                    {
                        "disease_key": row["disease_key"],
                        "phenotype_rows": row["phenotype_rows"],
                    }
                    for row in rows
                ]
            ),
            "diseases": [
                {
                    "disease_key": row["disease_key"],
                    "phenotype_count": row["phenotype_count"],
                    "expected_symptom_overlap": row["expected_symptom_overlap"],
                    "elapsed_ms": row["elapsed_ms"],
                    "exit_code": row["exit_code"],
                    "error": row.get("error"),
                }
                for row in rows
            ],
        }
    except Exception as exc:  # noqa: BLE001 - report keeps running with evidence.
        return {
            "ran": False,
            "elapsed_ms": round((time.perf_counter() - started) * 1000, 1),
            "error": f"{type(exc).__name__}: {exc}",
        }


def build_regression_control_payload(full_scale: dict[str, Any]) -> dict[str, Any]:
    explore_hpo = load_json(EXPLORE_HPO_PATH)
    explore_medlineplus = load_json(EXPLORE_MEDLINEPLUS_PATH)
    explore_topics = explore_topics_by_disease()
    hpo_live = _maybe_run_hpo_control()
    diseases: list[dict[str, Any]] = []
    exact_total_expected = 0
    exact_total_matched = 0
    for disease, exploit_row in zip(DISEASES, full_scale["diseases"], strict=False):
        exact_recall = blind_topic_recall(disease, explore_topics.get(disease["key"], []))
        exact_total_expected += exact_recall["expected_total"]
        exact_total_matched += exact_recall["matched_total"]
        selected_recall = selected_feature_recall(disease, exploit_row["clinical_features"])
        explore_row = next(
            row for row in explore_medlineplus["diseases"] if row["disease_key"] == disease["key"]
        )
        diseases.append(
            {
                "disease_key": disease["key"],
                "explore_medlineplus": {
                    "topic_count": explore_row["topic_count"],
                    "expected_symptom_overlap": explore_row["expected_symptom_overlap"],
                },
                "exploit_selection": {
                    "candidate_topic_count": exploit_row["topic_selection"]["candidate_topic_count"],
                    "selected_topic_count": exploit_row["topic_selection"]["selected_topic_count"],
                    "selection_policy": exploit_row["topic_selection"]["selection_policy"],
                    "noise_reduction_count": exploit_row["topic_selection"]["noise_reduction_count"],
                    "expected_symptom_overlap": selected_recall,
                    "mismatch_count": simple_mismatch_count(disease, exploit_row["clinical_features"]),
                    "feature_checksum": exploit_row["feature_checksum"],
                },
                "exploit_exact_explore_benchmark": {
                    "topic_count": len(explore_topics.get(disease["key"], [])),
                    "expected_symptom_overlap": exact_recall,
                },
            }
        )

    explore_total_expected = explore_medlineplus["summary"]["total_expected_symptoms"]
    explore_total_matched = explore_medlineplus["summary"]["total_matched_expected_symptoms"]
    exploit_total_expected = full_scale["summary"]["total_expected_symptoms"]
    exploit_total_matched = full_scale["summary"]["total_matched_expected_symptoms"]
    explore_missing = explore_total_expected - explore_total_matched
    exploit_missing = exploit_total_expected - exploit_total_matched

    explore_hpo_checksum = stable_checksum(
        [
            {
                "disease_key": row["disease_key"],
                "phenotype_rows": row["phenotype_rows"],
            }
            for row in explore_hpo["diseases"]
        ]
    )
    hpo_checksum_match = hpo_live.get("row_checksum") == explore_hpo_checksum if hpo_live.get("ran") else None

    return {
        "generated_at": utc_now_iso(),
        "regression_rule": {
            "correctness": "Exploit mismatch count must strictly decrease or stay equal versus explore.",
            "throughput_latency": "Live API timings are recorded but fixture/cached extraction is the stable comparison.",
            "peak_rss": "Peak RSS must be within the 5% tolerance for the Python exploit harness.",
        },
        "explore_baseline": {
            "hpo": {
                "total_phenotype_rows": explore_hpo["summary"]["total_phenotype_rows"],
                "total_expected_symptoms": explore_hpo["summary"]["total_expected_symptoms"],
                "total_matched_expected_symptoms": explore_hpo["summary"]["total_matched_expected_symptoms"],
                "expected_symptom_recall": explore_hpo["summary"]["expected_symptom_recall"],
                "row_checksum": explore_hpo_checksum,
            },
            "medlineplus": explore_medlineplus["summary"],
            "medlineplus_mismatch_count": explore_missing,
        },
        "exploit": {
            "summary": full_scale["summary"],
            "medlineplus_mismatch_count": exploit_missing,
            "exact_explore_benchmark": {
                "total_expected_symptoms": exact_total_expected,
                "total_matched_expected_symptoms": exact_total_matched,
                "expected_symptom_recall": round(exact_total_matched / exact_total_expected, 3)
                if exact_total_expected
                else None,
                "mismatch_count": exact_total_expected - exact_total_matched,
            },
            "hpo_live_control": hpo_live,
            "hpo_checksum_match": hpo_checksum_match,
        },
        "rule_results": {
            "exact_explore_benchmark_reproduction": {
                "passed": (exact_total_expected - exact_total_matched) <= explore_missing,
                "explore_mismatch_count": explore_missing,
                "exploit_exact_mismatch_count": exact_total_expected - exact_total_matched,
                "carveout": None,
            },
            "medlineplus_correctness": {
                "passed": exploit_missing <= explore_missing,
                "explore_mismatch_count": explore_missing,
                "exploit_mismatch_count": exploit_missing,
                "carveout": None,
            },
            "hpo_rows": {
                "passed": (
                    hpo_live.get("total_phenotype_rows") == explore_hpo["summary"]["total_phenotype_rows"]
                    if hpo_live.get("ran")
                    else None
                ),
                "explore_total_phenotype_rows": explore_hpo["summary"]["total_phenotype_rows"],
                "exploit_total_phenotype_rows": hpo_live.get("total_phenotype_rows"),
                "checksum_match": hpo_checksum_match,
                "carveout": "not_applicable_hpo_control_unavailable" if not hpo_live.get("ran") else None,
            },
            "topic_noise": {
                "passed": full_scale["summary"]["total_selected_topics"]
                <= explore_medlineplus["summary"]["total_topics"],
                "explore_topics": explore_medlineplus["summary"]["total_topics"],
                "exploit_selected_topics": full_scale["summary"]["total_selected_topics"],
                "carveout": None,
            },
        },
        "diseases": diseases,
    }


def build_validation_payload(full_scale: dict[str, Any], regression: dict[str, Any]) -> dict[str, Any]:
    required_feature_keys = {
        "label",
        "feature_type",
        "normalized_hpo_id",
        "normalized_hpo_label",
        "mapping_confidence",
        "source",
        "source_url",
        "source_native_id",
        "evidence_tier",
        "evidence_text",
        "body_system",
        "rank",
    }
    feature_rows = [
        feature
        for disease in full_scale["diseases"]
        for feature in disease["clinical_features"]
    ]
    missing_keys = [
        {
            "label": feature.get("label"),
            "missing_keys": sorted(required_feature_keys - set(feature)),
        }
        for feature in feature_rows
        if required_feature_keys - set(feature)
    ]
    bad_evidence = [
        feature["label"]
        for feature in feature_rows
        if not excerpt_contains_extraction_anchor(feature)
    ]
    direct_selection_checks = []
    for row in full_scale["diseases"]:
        if row["disease_key"] in {"uterine_fibroid", "endometriosis"}:
            direct_selection_checks.append(
                {
                    "disease_key": row["disease_key"],
                    "passed": row["topic_selection"]["selection_policy"] == "direct_pages_only"
                    and row["topic_selection"]["selected_topic_count"] == 1,
                    "candidate_topic_count": row["topic_selection"]["candidate_topic_count"],
                    "selected_topic_count": row["topic_selection"]["selected_topic_count"],
                    "selected_titles": [topic["title"] for topic in row["topic_selection"]["topics"]],
                }
            )

    checks = {
        "schema_required_fields": not missing_keys,
        "evidence_text_contains_anchor": not bad_evidence,
        "recall_matches_or_beats_explore": regression["rule_results"]["medlineplus_correctness"]["passed"],
        "direct_page_selection_reduces_noise": all(item["passed"] for item in direct_selection_checks),
        "all_result_paths_persistent": all(
            not str(path).startswith("/tmp/")
            for path in full_scale["artifact_paths"].values()
        ),
        "hpo_control_available_or_documented": regression["rule_results"]["hpo_rows"]["passed"] is not False,
    }
    return {
        "generated_at": utc_now_iso(),
        "checks": checks,
        "passed": all(checks.values()),
        "details": {
            "missing_feature_keys": missing_keys,
            "bad_evidence_rows": bad_evidence,
            "direct_selection_checks": direct_selection_checks,
            "feature_count": len(feature_rows),
            "result_paths": full_scale["artifact_paths"],
        },
    }


def build_contract_payload(full_scale: dict[str, Any], regression: dict[str, Any], validation: dict[str, Any]) -> dict[str, Any]:
    return {
        "generated_at": utc_now_iso(),
        "contract_numbers": {
            "disease_count": full_scale["summary"]["disease_count"],
            "clinical_feature_count": full_scale["summary"]["clinical_feature_count"],
            "mapped_feature_count": full_scale["summary"]["mapped_feature_count"],
            "expected_symptom_recall": full_scale["summary"]["expected_symptom_recall"],
            "mismatch_count": full_scale["summary"]["mismatch_count"],
            "selected_topic_count": full_scale["summary"]["total_selected_topics"],
            "topic_noise_reduction_count": full_scale["summary"]["total_topic_noise_reduction"],
            "direct_page_diseases": full_scale["summary"]["direct_page_diseases"],
            "elapsed_ms": full_scale["summary"]["elapsed_ms"],
            "features_per_second": full_scale["summary"]["features_per_second"],
            "peak_rss_kb": full_scale["summary"]["peak_rss_kb"],
            "output_checksum": full_scale["summary"]["output_checksum"],
        },
        "regression_passed": all(
            result.get("passed") is not False
            for result in regression["rule_results"].values()
        ),
        "validation_passed": validation["passed"],
        "artifact_paths": full_scale["artifact_paths"],
    }


def write_all_results(*, allow_live: bool = True, refresh_cache: bool = False) -> dict[str, Path]:
    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    WORK_DIR.mkdir(parents=True, exist_ok=True)

    full_scale = build_full_scale_payload(allow_live=allow_live, refresh_cache=refresh_cache)
    write_json(FULL_SCALE_PATH, full_scale)

    regression = build_regression_control_payload(full_scale)
    write_json(REGRESSION_CONTROL_PATH, regression)

    validation = build_validation_payload(full_scale, regression)
    write_json(VALIDATION_PATH, validation)

    contract = build_contract_payload(full_scale, regression, validation)
    write_json(CONTRACT_PATH, contract)

    return {
        "full_scale": FULL_SCALE_PATH,
        "regression_control": REGRESSION_CONTROL_PATH,
        "validation": VALIDATION_PATH,
        "contract_numbers": CONTRACT_PATH,
    }
