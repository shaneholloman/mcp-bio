from __future__ import annotations

import re
from typing import Any

from phenotype_spike_common import expected_overlap

from .common import compact_evidence, normalize_text, slugify, source_native_id
from .hpo_mapping import map_feature
from .medlineplus import direct_title_queries


BODY_SYSTEM_BY_DISEASE = {
    "uterine_fibroid": "reproductive",
    "endometriosis": "reproductive",
    "chronic_venous_insufficiency": "vascular",
}

EXTRA_EXTRACTION_PATTERNS = {
    "abdominal pain": ["lower abdomen"],
    "urinary frequency": ["urinating peeing often"],
}


def topic_score(topic: dict[str, str], disease: dict[str, Any]) -> dict[str, Any]:
    title_norm = normalize_text(topic.get("title", ""))
    summary_norm = normalize_text(topic.get("summary", ""))
    slug = slugify(topic.get("url", ""))
    direct_queries = direct_title_queries(disease)
    query_slugs = {slugify(query) for query in disease["source_queries"] + [disease["label"]]}

    score = 0.0
    reasons: list[str] = []
    if title_norm in direct_queries:
        score += 100.0
        reasons.append("exact_title")
    if source_native_id(topic.get("url", "")) in query_slugs or slug in query_slugs:
        score += 80.0
        reasons.append("exact_url_slug")

    label_tokens = {
        token
        for token in normalize_text(" ".join(disease["source_queries"])).split()
        if len(token) > 3
    }
    title_tokens = set(title_norm.split())
    if label_tokens:
        overlap = len(label_tokens & title_tokens) / len(label_tokens)
        score += round(overlap * 35.0, 2)
        if overlap:
            reasons.append(f"title_token_overlap:{overlap:.2f}")

    if any(normalize_text(query) in summary_norm for query in disease["source_queries"]):
        score += 15.0
        reasons.append("query_in_summary")

    relation = "direct" if score >= 90.0 else "related"
    return {
        "score": round(score, 2),
        "relation": relation,
        "reasons": reasons,
    }


def select_topics(disease: dict[str, Any], topics: list[dict[str, str]]) -> dict[str, Any]:
    scored: list[dict[str, Any]] = []
    for topic in topics:
        score = topic_score(topic, disease)
        row = dict(topic)
        row["selection_score"] = score["score"]
        row["selection_relation"] = score["relation"]
        row["selection_reasons"] = score["reasons"]
        scored.append(row)

    direct = [topic for topic in scored if topic["selection_relation"] == "direct"]
    if direct:
        selected = sorted(direct, key=lambda row: (-row["selection_score"], row.get("title", "")))
        policy = "direct_pages_only"
    else:
        selected = sorted(scored, key=lambda row: (-row["selection_score"], row.get("title", "")))[:3]
        policy = "top_related_when_no_direct_page"

    return {
        "selection_policy": policy,
        "candidate_topic_count": len(topics),
        "selected_topic_count": len(selected),
        "related_topic_count": sum(1 for row in selected if row["selection_relation"] != "direct"),
        "noise_reduction_count": max(0, len(topics) - len(selected)),
        "topics": selected,
    }


def _find_first_pattern(text: str, patterns: list[str]) -> str | None:
    text_norm = normalize_text(text)
    for pattern in patterns:
        pattern_norm = normalize_text(pattern)
        if not pattern_norm:
            continue
        if pattern_norm in text_norm:
            return pattern
    return None


def extract_features(
    disease: dict[str, Any],
    selected_topics: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    features: list[dict[str, Any]] = []
    topic_texts = [
        (topic, " ".join([topic.get("title", ""), topic.get("summary", "")]).strip())
        for topic in selected_topics
    ]
    rank = 1
    for concept, patterns in disease["expected_symptoms"].items():
        extraction_patterns = patterns + EXTRA_EXTRACTION_PATTERNS.get(concept, [])
        best: dict[str, Any] | None = None
        best_pattern: str | None = None
        for topic, text in topic_texts:
            pattern = _find_first_pattern(text, extraction_patterns)
            if not pattern:
                continue
            if best is None or topic.get("selection_score", 0) > best.get("selection_score", 0):
                best = topic
                best_pattern = pattern
        if not best or not best_pattern:
            continue

        text = " ".join([best.get("title", ""), best.get("summary", "")]).strip()
        mapping = map_feature(concept)
        features.append(
            {
                "rank": rank,
                "label": concept,
                "feature_type": "symptom",
                "source": "MedlinePlus",
                "source_url": best.get("url"),
                "source_native_id": source_native_id(best.get("url", "")),
                "evidence_tier": "clinical_summary",
                "evidence_text": compact_evidence(text, best_pattern),
                "evidence_match": best_pattern,
                "body_system": BODY_SYSTEM_BY_DISEASE.get(disease["key"]),
                "topic_title": best.get("title"),
                "topic_relation": best.get("selection_relation"),
                "topic_selection_score": best.get("selection_score"),
                **mapping,
            }
        )
        rank += 1
    return features


def phenotype_coverage(
    disease: dict[str, Any],
    hpo_phenotypes: list[dict[str, Any]],
    clinical_features: list[dict[str, Any]],
) -> dict[str, Any]:
    expected = disease["expected_symptoms"]
    feature_labels = feature_label_set(clinical_features)
    missing = [
        concept
        for concept in expected
        if normalize_text(concept) not in feature_labels
    ]
    matched_total = len(expected) - len(missing)
    mapped = [row for row in clinical_features if row.get("normalized_hpo_id")]
    labels_for_checksum = [
        {
            "label": row["label"],
            "hpo": row.get("normalized_hpo_id"),
            "source": row.get("source_native_id"),
        }
        for row in clinical_features
    ]
    return {
        "curated_hpo_count": len(hpo_phenotypes),
        "clinical_feature_count": len(clinical_features),
        "mapped_feature_count": len(mapped),
        "unmapped_feature_count": len(clinical_features) - len(mapped),
        "expected_symptom_total": len(expected),
        "expected_symptom_matched": matched_total,
        "expected_symptom_missing": missing,
        "expected_symptom_recall": round(matched_total / len(expected), 3) if expected else None,
        "feature_label_checksum_input": labels_for_checksum,
        "coverage_note": (
            "Clinical features include source-native MedlinePlus terms and "
            "confidence-scored HPO mappings; low-confidence mappings should "
            "remain auditable rather than replacing the native term."
        ),
    }


def blind_topic_recall(disease: dict[str, Any], topics: list[dict[str, str]]) -> dict[str, Any]:
    text_terms: list[str] = []
    for topic in topics:
        text_terms.append(topic.get("title", ""))
        text_terms.append(topic.get("summary", ""))
    return expected_overlap(text_terms, disease["expected_symptoms"])


def selected_feature_recall(disease: dict[str, Any], features: list[dict[str, Any]]) -> dict[str, Any]:
    return expected_overlap([feature["label"] for feature in features], disease["expected_symptoms"])


def feature_label_set(features: list[dict[str, Any]]) -> set[str]:
    return {normalize_text(feature["label"]) for feature in features}


def simple_mismatch_count(disease: dict[str, Any], features: list[dict[str, Any]]) -> int:
    labels = feature_label_set(features)
    missing = 0
    for concept in disease["expected_symptoms"]:
        if normalize_text(concept) not in labels:
            missing += 1
    return missing


def excerpt_contains_extraction_anchor(feature: dict[str, Any]) -> bool:
    match = normalize_text(str(feature.get("evidence_match") or ""))
    evidence = normalize_text(str(feature.get("evidence_text") or ""))
    if not match:
        return False
    return bool(re.search(re.escape(match), evidence))
