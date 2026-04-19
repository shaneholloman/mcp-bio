from __future__ import annotations

from typing import Any, TypedDict


DiseaseInput = dict[str, Any]
HpoPhenotypeRow = dict[str, Any]
CoveragePayload = dict[str, Any]


class TopicRow(TypedDict, total=False):
    url: str
    title: str
    summary: str
    source_native_id: str
    selection_score: float
    selection_relation: str
    selection_reasons: list[str]


class TopicSelection(TypedDict):
    selection_policy: str
    candidate_topic_count: int
    selected_topic_count: int
    related_topic_count: int
    noise_reduction_count: int
    topics: list[TopicRow]


class ClinicalFeature(TypedDict, total=False):
    rank: int
    label: str
    feature_type: str
    source: str
    source_url: str | None
    source_native_id: str
    evidence_tier: str
    evidence_text: str
    evidence_match: str
    body_system: str | None
    topic_title: str | None
    topic_relation: str | None
    topic_selection_score: float | None
    normalized_hpo_id: str | None
    normalized_hpo_label: str | None
    mapping_confidence: float
    mapping_method: str


class DiseaseClinicalFeatures(TypedDict):
    disease_key: str
    label: str
    biomcp_query: str
    source_mode: str
    fallback_used: bool
    work_dir: str
    attempts: list[dict[str, Any]]
    topic_selection: TopicSelection
    phenotypes: list[HpoPhenotypeRow]
    clinical_features: list[ClinicalFeature]
    phenotype_coverage: CoveragePayload
    feature_checksum: str
