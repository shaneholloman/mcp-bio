from __future__ import annotations

from typing import Any


# Deterministic fixture for the architecture spike. Production mapping should
# use JAX HPO search plus reviewed allow/deny fixtures before a row is trusted.
HPO_MAPPING: dict[str, dict[str, Any]] = {
    "heavy menstrual bleeding": {
        "id": "HP:0000132",
        "label": "Menorrhagia",
        "confidence": 0.86,
        "method": "reviewed_fixture_exact_or_synonym",
    },
    "pelvic pain": {
        "id": "HP:0034267",
        "label": "Pelvic pain",
        "confidence": 0.95,
        "method": "reviewed_fixture_exact",
    },
    "lower back pain": {
        "id": "HP:0003419",
        "label": "Low back pain",
        "confidence": 0.92,
        "method": "reviewed_fixture_exact_or_synonym",
    },
    "fatigue": {
        "id": "HP:0012378",
        "label": "Fatigue",
        "confidence": 0.95,
        "method": "reviewed_fixture_exact",
    },
    "urinary frequency": {
        "id": "HP:0100515",
        "label": "Pollakisuria",
        "confidence": 0.78,
        "method": "reviewed_fixture_synonym",
    },
    "constipation": {
        "id": "HP:0002019",
        "label": "Constipation",
        "confidence": 0.95,
        "method": "reviewed_fixture_exact",
    },
    "infertility": {
        "id": "HP:0000789",
        "label": "Infertility",
        "confidence": 0.9,
        "method": "reviewed_fixture_exact",
    },
    "dyspareunia": {
        "id": "HP:0030016",
        "label": "Dyspareunia",
        "confidence": 0.95,
        "method": "reviewed_fixture_exact_or_synonym",
    },
    "dysmenorrhea": {
        "id": "HP:0100607",
        "label": "Dysmenorrhea",
        "confidence": 0.95,
        "method": "reviewed_fixture_exact_or_synonym",
    },
    "dyschezia": {
        "id": "HP:6000222",
        "label": "Painful defecation",
        "confidence": 0.78,
        "method": "reviewed_fixture_synonym",
    },
    "dysuria": {
        "id": "HP:0100518",
        "label": "Dysuria",
        "confidence": 0.9,
        "method": "reviewed_fixture_exact_or_synonym",
    },
    "abdominal pain": {
        "id": "HP:0002027",
        "label": "Abdominal pain",
        "confidence": 0.95,
        "method": "reviewed_fixture_exact",
    },
    "leg swelling": {
        "id": "HP:0010741",
        "label": "Pedal edema",
        "confidence": 0.68,
        "method": "reviewed_fixture_broader_lower_limb_edema",
    },
    "leg pain": {
        "id": "HP:0012514",
        "label": "Lower limb pain",
        "confidence": 0.86,
        "method": "reviewed_fixture_synonym",
    },
    "varicose veins": {
        "id": "HP:0002619",
        "label": "Varicose veins",
        "confidence": 0.95,
        "method": "reviewed_fixture_exact",
    },
    "venous ulcer": {
        "id": "HP:0200042",
        "label": "Skin ulcer",
        "confidence": 0.62,
        "method": "reviewed_fixture_broader_skin_ulcer",
    },
    "skin discoloration": {
        "id": "HP:0000953",
        "label": "Hyperpigmentation of the skin",
        "confidence": 0.62,
        "method": "reviewed_fixture_broader_discoloration",
    },
    "stasis dermatitis": {
        "id": "HP:0033564",
        "label": "Stasis dermatitis",
        "confidence": 0.95,
        "method": "reviewed_fixture_exact",
    },
    "itching": {
        "id": "HP:0000989",
        "label": "Pruritus",
        "confidence": 0.9,
        "method": "reviewed_fixture_synonym",
    },
}


def map_feature(label: str) -> dict[str, Any]:
    mapping = HPO_MAPPING.get(label)
    if not mapping:
        return {
            "normalized_hpo_id": None,
            "normalized_hpo_label": None,
            "mapping_confidence": 0.0,
            "mapping_method": "unmapped",
        }
    return {
        "normalized_hpo_id": mapping["id"],
        "normalized_hpo_label": mapping["label"],
        "mapping_confidence": mapping["confidence"],
        "mapping_method": mapping["method"],
    }
