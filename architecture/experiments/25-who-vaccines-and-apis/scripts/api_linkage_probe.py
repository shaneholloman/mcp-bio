#!/usr/bin/env python3
from __future__ import annotations

from who_vaccines_apis_lib import (
    MyChemResolver,
    RESULTS_DIR,
    classify_hits,
    clean_text,
    iso_now,
    load_apis,
    load_finished_pharma,
    normalize_match_key,
    split_normalized_segments,
    write_json,
)

RESULT_PATH = RESULTS_DIR / "api_linkage_probe.json"


def build_finished_indexes(entries: list[dict]) -> tuple[set[str], set[str], dict[str, list[dict]]]:
    exact_inns: set[str] = set()
    segments: set[str] = set()
    segment_samples: dict[str, list[dict]] = {}
    for entry in entries:
        normalized_inn = entry.get("normalized_inn")
        if normalized_inn:
            exact_inns.add(normalized_inn)
        for normalized_value in (entry.get("normalized_inn"), entry.get("normalized_presentation")):
            for segment in split_normalized_segments(normalized_value):
                segments.add(segment)
                bucket = segment_samples.setdefault(segment, [])
                if len(bucket) < 3:
                    bucket.append(
                        {
                            "who_reference_number": entry["who_reference_number"],
                            "inn": entry["inn"],
                            "presentation": entry["presentation"],
                        }
                    )
    return exact_inns, segments, segment_samples


def resolve_query(resolver: MyChemResolver, query: str) -> dict:
    response = resolver.search(query, size=10)
    report = classify_hits(query, response["hits"])
    report["mychem_total"] = response["total"]
    return report


def summarize_mychem_strategy(
    rows: list[dict], resolver: MyChemResolver, query_fn, strategy_name: str
) -> dict:
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


def summarize_finished_overlap(rows: list[dict], exact_inns: set[str], segments: set[str], segment_samples) -> dict:
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


def run() -> dict:
    apis = load_apis()["rows"]
    finished = load_finished_pharma()["entries"]
    exact_inns, segments, segment_samples = build_finished_indexes(finished)
    resolver = MyChemResolver()

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
    payload = {
        "generated_at": iso_now(),
        "row_count": len(apis),
        "mychem_identity_results": mychem_results,
        "finished_pharma_overlap": overlap_results,
    }
    write_json(RESULT_PATH, payload)
    return payload


def main() -> None:
    run()


if __name__ == "__main__":
    main()
