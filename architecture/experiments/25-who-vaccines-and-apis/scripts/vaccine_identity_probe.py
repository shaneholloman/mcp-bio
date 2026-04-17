#!/usr/bin/env python3
from __future__ import annotations

import re

from who_vaccines_apis_lib import (
    MyChemResolver,
    RESULTS_DIR,
    classify_hits,
    clean_text,
    iso_now,
    load_vaccines,
    split_vaccine_components,
    strip_parentheticals,
    write_json,
)

RESULT_PATH = RESULTS_DIR / "vaccine_identity_probe.json"
VACCINE_COMPONENT_TYPE_RE = re.compile(
    r"\btypes?\s+[0-9]+(?:\s+and\s+[0-9]+)*\b", re.IGNORECASE
)
VACCINE_COMPONENT_NOISE_RE = re.compile(
    r"\b(?:seasonal|pandemic|trivalent|quadrivalent|bivalent|monovalent|oral|inactivated|"
    r"live|attenuated|paediatric|conjugate|reduced antigen content|whole cell|acellular|"
    r"recombinant|novel|sabin)\b",
    re.IGNORECASE,
)


def match_score(report: dict) -> tuple[int, int]:
    if report["exact_match"]:
        return (2, report["total_hits"])
    if report["phrase_match"]:
        return (1, report["total_hits"])
    return (0, report["total_hits"])


def resolve_query(resolver: MyChemResolver, query: str) -> dict:
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
    name: str, rows: list[dict], query_fn, resolver: MyChemResolver
) -> dict:
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


def summarize_component_strategy(rows: list[dict], resolver: MyChemResolver) -> dict:
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


def summarize_component_brand_fallback_strategy(rows: list[dict], resolver: MyChemResolver) -> dict:
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


def run() -> dict:
    vaccines = load_vaccines()["rows"]
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
        payload = {
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
        write_json(RESULT_PATH, payload)
        return payload
    finally:
        resolver.flush()


def main() -> None:
    run()


if __name__ == "__main__":
    main()
