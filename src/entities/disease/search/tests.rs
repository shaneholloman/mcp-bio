use super::*;

#[test]
fn disease_search_request_records_normalized_filters_and_fetch_plan() {
    let filters = DiseaseSearchFilters {
        query: Some(" chronic myeloid leukemia ".into()),
        source: Some(" DOID ".into()),
        inheritance: Some(" autosomal dominant ".into()),
        phenotype: Some(" HP:0001250 ".into()),
        onset: Some(" childhood ".into()),
    };

    let request = DiseaseSearchRequest::new(&filters, 3, 2).expect("request");

    assert_eq!(request.query, "chronic myeloid leukemia");
    assert_eq!(request.source.as_deref(), Some("DOID"));
    assert_eq!(request.inheritance.as_deref(), Some("autosomal dominant"));
    assert_eq!(request.phenotype.as_deref(), Some("HP:0001250"));
    assert_eq!(request.onset.as_deref(), Some("childhood"));
    assert_eq!(request.limit, 3);
    assert_eq!(request.offset, 2);
    assert_eq!(request.fetch_size, 25);
    assert!(
        request
            .resolver_queries
            .iter()
            .any(|value| value == "chronic myeloid leukemia")
    );
    assert!(request.prefer_doid);
}

#[test]
fn ticket_400_request_command_disease_search_fields_drive_source_query_and_pagination() {
    let filters = DiseaseSearchFilters {
        query: Some(" chronic myeloid leukemia ".into()),
        source: Some(" doid ".into()),
        inheritance: Some(" autosomal dominant ".into()),
        phenotype: Some(" HP:0001250 ".into()),
        onset: Some(" childhood ".into()),
    };
    let request = DiseaseSearchRequest::new(&filters, 3, 2).expect("request");
    let client =
        crate::sources::mydisease::MyDiseaseClient::new_for_test("http://127.0.0.1/v1".into())
            .expect("mydisease client");
    let plan = client
        .query_request_plan(
            &request.resolver_queries[0],
            request.fetch_size,
            0,
            request.source.as_deref(),
            request.inheritance.as_deref(),
            request.phenotype.as_deref(),
            request.onset.as_deref(),
        )
        .expect("source query plan");

    assert_eq!(request.limit, 3);
    assert_eq!(request.offset, 2);
    assert_eq!(plan.path, "/query");
    assert!(plan.query_params.contains(&("size", "25".to_string())));
    assert!(plan.query_params.contains(&("from", "0".to_string())));
    assert!(plan.query_params.iter().any(|(key, value)| {
        *key == "q"
            && value.contains("chronic myeloid leukemia")
            && value.contains("disease_ontology.doid:*")
            && value.contains("hpo.inheritance.hpo_name:*autosomal dominant*")
            && value.contains("hpo.phenotype_related_to_disease.hpo_id:*HP\\:0001250*")
            && value.contains("hpo.clinical_course.hpo_name:*childhood*")
    }));
}

#[test]
fn disease_search_request_preserves_limit_and_query_validation() {
    let filters = DiseaseSearchFilters::default();
    let err = DiseaseSearchRequest::new(&filters, 0, 0).expect_err("limit should fail");
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));

    let err = DiseaseSearchRequest::new(&filters, 1, 0).expect_err("query should fail");
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
}

#[test]
fn parse_hpo_query_terms_requires_valid_ids() {
    let parsed = parse_hpo_query_terms("HP:0001250 HP:0001263").expect("valid terms");
    assert_eq!(parsed, vec!["HP:0001250", "HP:0001263"]);
    let comma_separated = parse_hpo_query_terms("hp:0001250, HP:0001263").expect("comma terms");
    assert_eq!(comma_separated, vec!["HP:0001250", "HP:0001263"]);
    assert!(parse_hpo_query_terms("NOT_AN_HPO").is_err());
}

#[test]
fn split_phenotype_queries_preserves_single_phrase_and_splits_commas() {
    assert_eq!(
        split_phenotype_queries("developmental delay"),
        vec!["developmental delay"]
    );
    assert_eq!(
        split_phenotype_queries("seizure, developmental delay,  hypotonia "),
        vec!["seizure", "developmental delay", "hypotonia"]
    );
}

#[tokio::test]
async fn resolve_phenotype_query_terms_empty_input_mentions_hpo_ids_and_symptom_phrases() {
    let err = resolve_phenotype_query_terms("   ")
        .await
        .expect_err("empty phenotype query should fail");

    match err {
        BioMcpError::InvalidArgument(message) => {
            assert!(message.contains("Use HPO IDs or symptom phrases"));
            assert!(message.contains("HP:0001250 HP:0001263"));
            assert!(message.contains("seizure, developmental delay"));
        }
        other => panic!("expected InvalidArgument, got: {other}"),
    }
}
