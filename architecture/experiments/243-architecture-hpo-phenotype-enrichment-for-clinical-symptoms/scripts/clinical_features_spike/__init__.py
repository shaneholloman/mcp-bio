"""Clinical feature enrichment proof for spike 243."""

from __future__ import annotations

from .api import (
    extract_clinical_feature_dataset,
    extract_disease_clinical_features,
    load_hpo_rows_by_disease,
    summarize_clinical_feature_dataset,
)
from .medlineplus import all_diseases
from .types import ClinicalFeature, DiseaseClinicalFeatures, TopicRow, TopicSelection

__all__ = [
    "ClinicalFeature",
    "DiseaseClinicalFeatures",
    "TopicRow",
    "TopicSelection",
    "all_diseases",
    "extract_clinical_feature_dataset",
    "extract_disease_clinical_features",
    "load_hpo_rows_by_disease",
    "summarize_clinical_feature_dataset",
]
