use super::super::DrugSearchFilters;
use super::*;
use crate::error::BioMcpError;

#[test]
fn build_mychem_query_requires_at_least_one_filter() {
    let filters = DrugSearchFilters::default();
    let err = build_mychem_query(&filters).unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
}

#[test]
fn build_mychem_query_includes_target_and_mechanism_filters() {
    let filters = DrugSearchFilters {
        query: Some("pembrolizumab".into()),
        target: Some("BRAF".into()),
        indication: None,
        mechanism: Some("inhibitor".into()),
        drug_type: Some("small molecule".into()),
        atc: None,
        pharm_class: None,
        interactions: None,
    };
    let q = build_mychem_query(&filters).unwrap();
    assert!(q.contains("pembrolizumab"));
    assert!(q.contains("gtopdb.interaction_targets.symbol:BRAF"));
    assert!(q.contains("chembl.drug_mechanisms.action_type:*inhibitor*"));
    assert!(q.contains("ndc.pharm_classes"));
    assert!(q.contains("chembl.molecule_type:\"Small molecule\""));
}

#[test]
fn build_mychem_query_includes_mechanism_of_action_field() {
    let filters = DrugSearchFilters {
        mechanism: Some("adenosine deaminase inhibitor".into()),
        ..Default::default()
    };

    let q = build_mychem_query(&filters).unwrap();
    assert!(q.contains("chembl.drug_mechanisms.mechanism_of_action"));
    assert!(
        q.contains(
            "chembl.drug_mechanisms.mechanism_of_action:*adenosine* AND chembl.drug_mechanisms.mechanism_of_action:*deaminase*"
        )
    );
}

#[test]
fn build_mychem_query_expands_purine_to_atc_codes() {
    let filters = DrugSearchFilters {
        mechanism: Some("purine analog".into()),
        ..Default::default()
    };

    let q = build_mychem_query(&filters).unwrap();
    assert!(q.contains("chembl.atc_classifications:L01BB*"));
    assert!(q.contains("chembl.atc_classifications:L01XX08"));
}

#[test]
fn build_mychem_query_keeps_atc_filter_exact() {
    let filters = DrugSearchFilters {
        atc: Some("L01BB".into()),
        ..Default::default()
    };

    let q = build_mychem_query(&filters).unwrap();
    assert!(q.contains("chembl.atc_classifications:L01BB"));
    assert!(!q.contains("chembl.atc_classifications:L01BB*"));
}

#[test]
fn build_mychem_query_escapes_free_text_query() {
    let filters = DrugSearchFilters {
        query: Some("EGFR:inhibitor (3rd-gen)".into()),
        target: None,
        indication: None,
        mechanism: None,
        drug_type: None,
        atc: None,
        pharm_class: None,
        interactions: None,
    };

    let q = build_mychem_query(&filters).unwrap();
    assert!(q.contains(r"EGFR\:inhibitor"));
    assert!(q.contains(r"\(3rd\-gen\)"));
}

#[test]
fn build_mychem_query_rejects_public_interaction_filter() {
    let filters = DrugSearchFilters {
        query: None,
        target: None,
        indication: None,
        mechanism: None,
        drug_type: None,
        atc: None,
        pharm_class: None,
        interactions: Some("warfarin".into()),
    };

    let err = build_mychem_query(&filters).unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
    assert!(
        err.to_string().contains(
            "Interaction-partner drug search is unavailable from the public data sources"
        )
    );
}

#[test]
fn drug_search_filters_detect_structured_filters() {
    let plain_name = DrugSearchFilters {
        query: Some("Keytruda".into()),
        ..Default::default()
    };
    assert!(!plain_name.has_structured_filters());

    let structured = DrugSearchFilters {
        target: Some("EGFR".into()),
        ..Default::default()
    };
    assert!(structured.has_structured_filters());
}

#[test]
fn mechanism_atc_expansions_returns_purine_mapping() {
    assert_eq!(
        mechanism_atc_expansions("purine analog"),
        vec![
            AtcExpansion::Prefix("L01BB"),
            AtcExpansion::Exact("L01XX08")
        ]
    );
    assert!(mechanism_atc_expansions("kinase inhibitor").is_empty());
}
