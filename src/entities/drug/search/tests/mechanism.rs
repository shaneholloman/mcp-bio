//! Search mechanism and target-match helper coverage.

use super::*;

#[test]
fn mechanism_match_uses_mechanism_fields_not_drug_name() {
    let hit: MyChemHit = serde_json::from_value(serde_json::json!({
        "_id": "x",
        "_score": 1.0,
        "drugbank": {"name": "alpha.1-proteinase inhibitor human"},
        "chembl": {
            "drug_mechanisms": [{"action_type": "protease inhibitor", "target_name": "ELANE"}]
        }
    }))
    .expect("valid hit");

    assert!(!hit_mentions_mechanism(&hit, "kinase inhibitor"));
    assert!(hit_mentions_mechanism(&hit, "protease inhibitor"));
}

#[test]
fn hit_mentions_mechanism_matches_atc_purine_hits() {
    let hit: MyChemHit = serde_json::from_value(serde_json::json!({
        "_id": "x",
        "_score": 1.0,
        "chembl": {
            "atc_classifications": ["L01BB07"],
            "drug_mechanisms": []
        }
    }))
    .expect("valid hit");

    assert!(hit_mentions_mechanism(&hit, "purine"));
    assert!(hit_mentions_mechanism(&hit, "purine analog"));
}

#[test]
fn hit_mentions_mechanism_matches_mechanism_of_action_text() {
    let hit: MyChemHit = serde_json::from_value(serde_json::json!({
        "_id": "x",
        "_score": 1.0,
        "chembl": {
            "drug_mechanisms": [{
                "mechanism_of_action": "Adenosine deaminase inhibitor"
            }]
        }
    }))
    .expect("valid hit");

    assert!(hit_mentions_mechanism(
        &hit,
        "adenosine deaminase inhibitor"
    ));
    assert!(hit_mentions_mechanism(&hit, "deaminase inhibitor"));
}
