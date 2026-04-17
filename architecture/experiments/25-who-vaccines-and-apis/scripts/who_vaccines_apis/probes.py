#!/usr/bin/env python3
from __future__ import annotations

import re
from typing import Any, Callable

from .identity import (
    MyChemResolver,
    classify_hits,
    clean_text,
    normalize_match_key,
    split_normalized_segments,
    split_vaccine_components,
    strip_parentheticals,
)
from .io import RESULTS_DIR, iso_now, sample_rows, time_call, write_json
from .loaders import (
    REQUIRED_FINISHED_HEADERS,
    device_schema_summary,
    load_apis,
    load_devices,
    load_finished_pharma,
    load_vaccines,
    schema_overlap,
    schema_summary,
)
from .types import WhoFinishedPharmaEntry

SCHEMA_RESULT_PATH = RESULTS_DIR / "who_schema_comparison.json"
VACCINE_IDENTITY_RESULT_PATH = RESULTS_DIR / "vaccine_identity_probe.json"
API_LINKAGE_RESULT_PATH = RESULTS_DIR / "api_linkage_probe.json"
METADATA_RESULT_PATH = RESULTS_DIR / "vaccine_metadata_and_device_probe.json"
PROBE_SUMMARY_PATH = RESULTS_DIR / "who_vaccines_apis_summary.json"

PROBE_RESULT_PATHS = {
    "schema_comparison": SCHEMA_RESULT_PATH,
    "vaccine_identity": VACCINE_IDENTITY_RESULT_PATH,
    "api_linkage": API_LINKAGE_RESULT_PATH,
    "metadata_and_devices": METADATA_RESULT_PATH,
}

VACCINE_COMPONENT_TYPE_RE = re.compile(
    r"\btypes?\s+[0-9]+(?:\s+and\s+[0-9]+)*\b", re.IGNORECASE
)
VACCINE_COMPONENT_NOISE_RE = re.compile(
    r"\b(?:seasonal|pandemic|trivalent|quadrivalent|bivalent|monovalent|oral|inactivated|"
    r"live|attenuated|paediatric|conjugate|reduced antigen content|whole cell|acellular|"
    r"recombinant|novel|sabin)\b",
    re.IGNORECASE,
)


def count_presence(rows: list[dict[str, Any]], key: str) -> dict[str, Any]:
    total = len(rows)
    present = sum(1 for row in rows if clean_text(row.get(key)))
    return {
        "present": present,
        "total": total,
        "percent": round((present / total) * 100, 2) if total else 0.0,
    }


def ticket_validation_samples(rows: list[dict[str, Any]]) -> dict[str, Any]:
    probes = {
        "BCG": ["bcg"],
        "measles": ["measles"],
        "HPV": ["human papillomavirus", "hpv"],
        "COVID-19": ["covid-19"],
        "yellow fever": ["yellow fever"],
    }
    results = {}
    for label, terms in probes.items():
        matches = []
        for row in rows:
            haystack = " | ".join(
                [row.get("Vaccine Type", ""), row.get("Commercial Name", "")]
            ).lower()
            if any(term in haystack for term in terms):
                matches.append(row)
        results[label] = {"count": len(matches), "samples": sample_rows(matches, 2)}
    return results


def build_schema_probe_payload() -> dict[str, Any]:
    finished = load_finished_pharma()
    vaccines = load_vaccines()
    apis = load_apis()
    devices = load_devices()
    devices_schema = device_schema_summary(devices)

    return {
        "generated_at": iso_now(),
        "record_counts": {
            "finished_pharma": len(finished.entries),
            "vaccines": len(vaccines.rows),
            "apis": len(apis.rows),
            "immunization_devices": devices_schema["item_count"],
            "immunization_device_categories": devices_schema["category_count"],
        },
        "source_schemas": {
            "finished_pharma": schema_summary(finished.headers, finished.rows),
            "vaccines": schema_summary(vaccines.headers, vaccines.rows),
            "apis": schema_summary(apis.headers, apis.rows),
            "immunization_devices": devices_schema,
        },
        "header_overlap_against_finished_pharma": {
            "vaccines": schema_overlap(finished.headers, vaccines.headers),
            "apis": schema_overlap(finished.headers, apis.headers),
            "immunization_devices": schema_overlap(
                REQUIRED_FINISHED_HEADERS, devices_schema["headers"]
            ),
        },
        "current_contract_direct_header_coverage": {
            "vaccines_shared_required_headers": schema_overlap(
                REQUIRED_FINISHED_HEADERS, vaccines.headers
            ),
            "apis_shared_required_headers": schema_overlap(REQUIRED_FINISHED_HEADERS, apis.headers),
            "devices_shared_required_headers": schema_overlap(
                REQUIRED_FINISHED_HEADERS, devices_schema["headers"]
            ),
        },
        "sample_rows": {
            "finished_pharma": sample_rows(finished.entries, 2),
            "vaccines": sample_rows(vaccines.rows, 2),
            "apis": sample_rows(apis.rows, 2),
            "immunization_devices": sample_rows(devices.items, 2),
        },
    }


def write_schema_probe_result() -> dict[str, Any]:
    payload = build_schema_probe_payload()
    write_json(SCHEMA_RESULT_PATH, payload)
    return payload


def match_score(report: dict[str, Any]) -> tuple[int, int]:
    if report["exact_match"]:
        return (2, report["total_hits"])
    if report["phrase_match"]:
        return (1, report["total_hits"])
    return (0, report["total_hits"])


def resolve_query(resolver: MyChemResolver, query: str) -> dict[str, Any]:
    response = resolver.search(query, size=10)
    report = classify_hits(query, response["hits"])
    report["mychem_total"] = response["total"]
    return report


def component_query_options(component: str) -> list[str]:
    cleaned = VACCINE_COMPONENT_TYPE_RE.sub(" ", component)
    cleaned = clean_text(VACCINE_COMPONENT_NOISE_RE.sub(" ", cleaned).replace(" - ", " "))
    if not cleaned or cleaned.isdigit():
        return []
    candidates = [cleaned]
    if "vaccine" not in cleaned.lower():
        candidates.insert(0, f"{cleaned} vaccine")
    return list(dict.fromkeys(candidates))


def summarize_single_query_strategy(
    name: str,
    rows: list[dict[str, Any]],
    query_fn: Callable[[dict[str, Any]], str | None],
    resolver: MyChemResolver,
) -> dict[str, Any]:
    exact = 0
    phrase = 0
    any_hit = 0
    no_query = 0
    samples_resolved = []
    samples_unresolved = []

    for row in rows:
        query = clean_text(query_fn(row))
        if not query:
            no_query += 1
            continue
        report = resolve_query(resolver, query)
        if report["exact_match"]:
            exact += 1
        if report["phrase_match"]:
            phrase += 1
        if report["total_hits"] > 0:
            any_hit += 1

        sample = {
            "commercial_name": row["Commercial Name"],
            "vaccine_type": row["Vaccine Type"],
            "query": query,
            "best_hit": report["best_hit"],
            "top_hits": report["top_hits"][:2],
        }
        if report["phrase_match"] and len(samples_resolved) < 5:
            samples_resolved.append(sample)
        if not report["phrase_match"] and len(samples_unresolved) < 5:
            samples_unresolved.append(sample)

    total = len(rows)
    return {
        "strategy": name,
        "rows": total,
        "no_query_rows": no_query,
        "exact_match_rows": exact,
        "exact_match_rate": round(exact / total * 100, 2) if total else 0.0,
        "phrase_or_exact_rows": phrase,
        "phrase_or_exact_rate": round(phrase / total * 100, 2) if total else 0.0,
        "any_hit_rows": any_hit,
        "any_hit_rate": round(any_hit / total * 100, 2) if total else 0.0,
        "sample_resolved_rows": samples_resolved,
        "sample_unresolved_rows": samples_unresolved,
    }


def summarize_component_strategy(
    rows: list[dict[str, Any]], resolver: MyChemResolver
) -> dict[str, Any]:
    fully_exact = 0
    fully_phrase = 0
    fully_any = 0
    no_component_rows = 0
    coverage_sum = 0.0
    resolved_samples = []
    unresolved_samples = []

    for row in rows:
        components = split_vaccine_components(row.get("Vaccine Type"))
        if not components:
            no_component_rows += 1
            continue
        component_reports = []
        for component in components:
            query_options = component_query_options(component)
            if not query_options:
                continue
            reports = [resolve_query(resolver, query) for query in query_options]
            best = max(reports, key=match_score)
            best["component"] = component
            component_reports.append(best)

        if not component_reports:
            no_component_rows += 1
            continue
        matched_components = sum(1 for report in component_reports if report["phrase_match"])
        any_components = sum(1 for report in component_reports if report["total_hits"] > 0)
        coverage = matched_components / len(component_reports)
        coverage_sum += coverage

        if all(report["exact_match"] for report in component_reports):
            fully_exact += 1
        if all(report["phrase_match"] for report in component_reports):
            fully_phrase += 1
        if any_components == len(component_reports):
            fully_any += 1

        sample = {
            "commercial_name": row["Commercial Name"],
            "vaccine_type": row["Vaccine Type"],
            "components": component_reports,
        }
        if all(report["phrase_match"] for report in component_reports):
            if len(resolved_samples) < 5:
                resolved_samples.append(sample)
        elif len(unresolved_samples) < 5:
            unresolved_samples.append(sample)

    total = len(rows)
    return {
        "strategy": "component_vaccine_coverage",
        "rows": total,
        "no_component_rows": no_component_rows,
        "all_components_exact_rows": fully_exact,
        "all_components_exact_rate": round(fully_exact / total * 100, 2) if total else 0.0,
        "all_components_phrase_or_exact_rows": fully_phrase,
        "all_components_phrase_or_exact_rate": round(fully_phrase / total * 100, 2)
        if total
        else 0.0,
        "all_components_any_hit_rows": fully_any,
        "all_components_any_hit_rate": round(fully_any / total * 100, 2) if total else 0.0,
        "mean_component_phrase_or_exact_coverage_rate": round(coverage_sum / total * 100, 2)
        if total
        else 0.0,
        "sample_resolved_rows": resolved_samples,
        "sample_unresolved_rows": unresolved_samples,
    }


def summarize_component_brand_fallback_strategy(
    rows: list[dict[str, Any]], resolver: MyChemResolver
) -> dict[str, Any]:
    exact = 0
    phrase = 0
    any_hit = 0
    no_query = 0
    resolved_samples = []
    unresolved_samples = []

    for row in rows:
        component_reports = []
        for component in split_vaccine_components(row.get("Vaccine Type")):
            query_options = component_query_options(component)
            if not query_options:
                continue
            reports = [resolve_query(resolver, query) for query in query_options]
            best = max(reports, key=match_score)
            best["component"] = component
            component_reports.append(best)

        brand_query = clean_text(row.get("Commercial Name"))
        brand_report = resolve_query(resolver, brand_query) if brand_query else None

        if not component_reports and brand_report is None:
            no_query += 1
            continue

        component_exact = bool(component_reports) and all(
            report["exact_match"] for report in component_reports
        )
        component_phrase = bool(component_reports) and all(
            report["phrase_match"] for report in component_reports
        )
        phrase_match = component_phrase or bool(brand_report and brand_report["phrase_match"])
        exact_match = component_exact or bool(brand_report and brand_report["exact_match"])
        any_match = any(report["total_hits"] > 0 for report in component_reports) or bool(
            brand_report and brand_report["total_hits"] > 0
        )

        if exact_match:
            exact += 1
        if phrase_match:
            phrase += 1
        if any_match:
            any_hit += 1

        sample = {
            "commercial_name": row["Commercial Name"],
            "vaccine_type": row["Vaccine Type"],
            "components": component_reports,
            "commercial_name_report": brand_report,
        }
        if phrase_match and len(resolved_samples) < 5:
            resolved_samples.append(sample)
        if not phrase_match and len(unresolved_samples) < 5:
            unresolved_samples.append(sample)

    total = len(rows)
    return {
        "strategy": "component_with_commercial_fallback",
        "rows": total,
        "no_query_rows": no_query,
        "exact_match_rows": exact,
        "exact_match_rate": round(exact / total * 100, 2) if total else 0.0,
        "phrase_or_exact_rows": phrase,
        "phrase_or_exact_rate": round(phrase / total * 100, 2) if total else 0.0,
        "any_hit_rows": any_hit,
        "any_hit_rate": round(any_hit / total * 100, 2) if total else 0.0,
        "sample_resolved_rows": resolved_samples,
        "sample_unresolved_rows": unresolved_samples,
    }


def build_vaccine_identity_probe_payload() -> dict[str, Any]:
    vaccines = load_vaccines().rows
    resolver = MyChemResolver()
    try:
        strategies = {
            "commercial_name": lambda row: row.get("Commercial Name"),
            "vaccine_type": lambda row: strip_parentheticals(row.get("Vaccine Type")),
            "vaccine_type_plus_vaccine": lambda row: (
                f"{strip_parentheticals(row.get('Vaccine Type'))} vaccine"
                if clean_text(row.get("Vaccine Type"))
                else None
            ),
        }
        strategy_results = {
            name: summarize_single_query_strategy(name, vaccines, query_fn, resolver)
            for name, query_fn in strategies.items()
        }
        strategy_results["component_vaccine_coverage"] = summarize_component_strategy(
            vaccines, resolver
        )
        strategy_results["component_with_commercial_fallback"] = (
            summarize_component_brand_fallback_strategy(vaccines, resolver)
        )
        winner = max(
            strategy_results.values(),
            key=lambda item: (
                item.get("phrase_or_exact_rows", item.get("all_components_phrase_or_exact_rows", 0)),
                item.get("exact_match_rows", item.get("all_components_exact_rows", 0)),
            ),
        )
        return {
            "generated_at": iso_now(),
            "row_count": len(vaccines),
            "strategy_results": strategy_results,
            "winner": {
                "strategy": winner["strategy"],
                "phrase_or_exact_rate": winner.get(
                    "phrase_or_exact_rate", winner.get("all_components_phrase_or_exact_rate")
                ),
                "exact_rate": winner.get(
                    "exact_match_rate", winner.get("all_components_exact_rate")
                ),
            },
        }
    finally:
        resolver.flush()


def write_vaccine_identity_probe_result() -> dict[str, Any]:
    payload = build_vaccine_identity_probe_payload()
    write_json(VACCINE_IDENTITY_RESULT_PATH, payload)
    return payload


def build_finished_indexes(
    entries: list[WhoFinishedPharmaEntry],
) -> tuple[set[str], set[str], dict[str, list[dict[str, Any]]]]:
    exact_inns: set[str] = set()
    segments: set[str] = set()
    segment_samples: dict[str, list[dict[str, Any]]] = {}
    for entry in entries:
        if entry.normalized_inn:
            exact_inns.add(entry.normalized_inn)
        for normalized_value in (entry.normalized_inn, entry.normalized_presentation):
            for segment in split_normalized_segments(normalized_value):
                segments.add(segment)
                bucket = segment_samples.setdefault(segment, [])
                if len(bucket) < 3:
                    bucket.append(
                        {
                            "who_reference_number": entry.who_reference_number,
                            "inn": entry.inn,
                            "presentation": entry.presentation,
                        }
                    )
    return exact_inns, segments, segment_samples


def summarize_mychem_strategy(
    rows: list[dict[str, Any]],
    resolver: MyChemResolver,
    query_fn: Callable[[dict[str, Any]], str | None],
    strategy_name: str,
) -> dict[str, Any]:
    exact = 0
    phrase = 0
    any_hit = 0
    no_query = 0
    resolved_samples = []
    unresolved_samples = []
    for row in rows:
        query = clean_text(query_fn(row))
        if not query:
            no_query += 1
            continue
        report = resolve_query(resolver, query)
        if report["exact_match"]:
            exact += 1
        if report["phrase_match"]:
            phrase += 1
        if report["total_hits"] > 0:
            any_hit += 1
        sample = {
            "who_product_id": row["WHO Product ID"],
            "inn": row["INN"],
            "query": query,
            "best_hit": report["best_hit"],
            "top_hits": report["top_hits"][:2],
        }
        if report["phrase_match"] and len(resolved_samples) < 5:
            resolved_samples.append(sample)
        if not report["phrase_match"] and len(unresolved_samples) < 5:
            unresolved_samples.append(sample)

    total = len(rows)
    return {
        "strategy": strategy_name,
        "rows": total,
        "no_query_rows": no_query,
        "exact_match_rows": exact,
        "exact_match_rate": round(exact / total * 100, 2) if total else 0.0,
        "phrase_or_exact_rows": phrase,
        "phrase_or_exact_rate": round(phrase / total * 100, 2) if total else 0.0,
        "any_hit_rows": any_hit,
        "any_hit_rate": round(any_hit / total * 100, 2) if total else 0.0,
        "sample_resolved_rows": resolved_samples,
        "sample_unresolved_rows": unresolved_samples,
    }


def summarize_finished_overlap(
    rows: list[dict[str, Any]],
    exact_inns: set[str],
    segments: set[str],
    segment_samples: dict[str, list[dict[str, Any]]],
) -> dict[str, Any]:
    exact_links = 0
    component_links = 0
    exact_samples = []
    component_samples = []
    unresolved_samples = []

    for row in rows:
        normalized_inn = normalize_match_key(row.get("INN"))
        normalized_segments = split_normalized_segments(normalized_inn)
        exact_link = bool(normalized_inn and normalized_inn in exact_inns)
        component_link = bool(normalized_segments) and all(
            segment in segments for segment in normalized_segments
        )

        if exact_link:
            exact_links += 1
        if component_link:
            component_links += 1

        if exact_link and len(exact_samples) < 5:
            exact_samples.append(
                {
                    "who_product_id": row["WHO Product ID"],
                    "inn": row["INN"],
                    "matching_segments": normalized_segments,
                    "finished_pharma_examples": segment_samples.get(normalized_segments[0], []),
                }
            )
        elif component_link and len(component_samples) < 5:
            examples = []
            for segment in normalized_segments:
                examples.extend(segment_samples.get(segment, []))
            component_samples.append(
                {
                    "who_product_id": row["WHO Product ID"],
                    "inn": row["INN"],
                    "matching_segments": normalized_segments,
                    "finished_pharma_examples": examples[:3],
                }
            )
        elif not component_link and len(unresolved_samples) < 5:
            unresolved_samples.append(
                {
                    "who_product_id": row["WHO Product ID"],
                    "inn": row["INN"],
                    "normalized_inn": normalized_inn,
                }
            )

    total = len(rows)
    return {
        "rows": total,
        "exact_inn_link_rows": exact_links,
        "exact_inn_link_rate": round(exact_links / total * 100, 2) if total else 0.0,
        "component_link_rows": component_links,
        "component_link_rate": round(component_links / total * 100, 2) if total else 0.0,
        "sample_exact_links": exact_samples,
        "sample_component_links": component_samples,
        "sample_unresolved_rows": unresolved_samples,
    }


def build_api_linkage_probe_payload() -> dict[str, Any]:
    apis = load_apis().rows
    finished = load_finished_pharma().entries
    exact_inns, segments, segment_samples = build_finished_indexes(finished)
    resolver = MyChemResolver()
    try:
        mychem_results = {
            "raw_inn": summarize_mychem_strategy(apis, resolver, lambda row: row.get("INN"), "raw_inn"),
            "normalized_inn": summarize_mychem_strategy(
                apis,
                resolver,
                lambda row: normalize_match_key(row.get("INN")),
                "normalized_inn",
            ),
        }
        overlap_results = summarize_finished_overlap(apis, exact_inns, segments, segment_samples)
        return {
            "generated_at": iso_now(),
            "row_count": len(apis),
            "mychem_identity_results": mychem_results,
            "finished_pharma_overlap": overlap_results,
        }
    finally:
        resolver.flush()


def write_api_linkage_probe_result() -> dict[str, Any]:
    payload = build_api_linkage_probe_payload()
    write_json(API_LINKAGE_RESULT_PATH, payload)
    return payload


def build_metadata_probe_payload() -> dict[str, Any]:
    vaccines = load_vaccines().rows
    devices = load_devices()
    device_summary = device_schema_summary(devices)

    return {
        "generated_at": iso_now(),
        "vaccine_row_count": len(vaccines),
        "vaccine_field_completeness": {
            "date_of_prequalification": count_presence(vaccines, "Date of Prequalification"),
            "vaccine_type": count_presence(vaccines, "Vaccine Type"),
            "commercial_name": count_presence(vaccines, "Commercial Name"),
            "presentation": count_presence(vaccines, "Presentation"),
            "no_of_doses": count_presence(vaccines, "No. of doses"),
            "manufacturer": count_presence(vaccines, "Manufacturer"),
            "responsible_nra": count_presence(vaccines, "Responsible NRA"),
        },
        "vaccine_specific_field_assessment": {
            "target_or_pathogen_proxy": {
                "available": True,
                "source_field": "Vaccine Type",
                "completeness": count_presence(vaccines, "Vaccine Type"),
            },
            "immunization_schedule": {
                "available": False,
                "source_field": None,
                "completeness": {"present": 0, "total": len(vaccines), "percent": 0.0},
            },
            "cold_chain_or_storage": {
                "available": False,
                "source_field": None,
                "completeness": {"present": 0, "total": len(vaccines), "percent": 0.0},
            },
            "dose_count": {
                "available": True,
                "source_field": "No. of doses",
                "completeness": count_presence(vaccines, "No. of doses"),
            },
        },
        "ticket_validation_samples": ticket_validation_samples(vaccines),
        "immunization_device_catalog": device_summary,
    }


def write_metadata_probe_result() -> dict[str, Any]:
    payload = build_metadata_probe_payload()
    write_json(METADATA_RESULT_PATH, payload)
    return payload


PROBE_RUNNERS = {
    "schema_comparison": write_schema_probe_result,
    "vaccine_identity": write_vaccine_identity_probe_result,
    "api_linkage": write_api_linkage_probe_result,
    "metadata_and_devices": write_metadata_probe_result,
}


def write_probe_summary(
    probe_payloads: dict[str, dict[str, Any]], stage_timings: dict[str, float]
) -> dict[str, Any]:
    payload = {
        "generated_at": iso_now(),
        "stage_timings_seconds": stage_timings,
        "results": probe_payloads,
    }
    write_json(PROBE_SUMMARY_PATH, payload)
    return payload


def run_probe_suite() -> tuple[dict[str, dict[str, Any]], dict[str, float]]:
    probe_payloads: dict[str, dict[str, Any]] = {}
    stage_timings: dict[str, float] = {}
    for name, runner in PROBE_RUNNERS.items():
        payload, elapsed = time_call(runner)
        probe_payloads[name] = payload
        stage_timings[f"{name}_seconds"] = elapsed
    stage_timings["total_seconds"] = round(sum(stage_timings.values()), 4)
    write_probe_summary(probe_payloads, stage_timings)
    return probe_payloads, stage_timings
