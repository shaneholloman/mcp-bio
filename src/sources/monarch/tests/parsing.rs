//! Tier 3 - response parsing and local result shaping. Pure: feeds JSON bytes
//! into decode helpers and validates output. No network.

use reqwest::StatusCode;
use reqwest::header::HeaderValue;

use super::super::*;

fn association_response(value: serde_json::Value) -> MonarchAssociationResponse {
    serde_json::from_value(value).expect("association fixture should parse")
}

#[test]
fn map_gene_associations_maps_rows_and_relationships() {
    let resp = association_response(serde_json::json!({
        "total": 1,
        "items": [
            {
                "subject": "HGNC:4851",
                "subject_label": "HTT",
                "predicate": "biolink:gene_associated_with_condition",
                "primary_knowledge_source": "infores:orphanet",
                "object": "MONDO:0016621",
                "object_label": "juvenile Huntington disease"
            }
        ]
    }));

    let rows = MonarchClient::map_gene_associations(resp, 5);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].gene, "HTT");
    assert_eq!(
        rows[0].relationship.as_deref(),
        Some("gene associated with condition")
    );
    assert_eq!(rows[0].source.as_deref(), Some("infores:orphanet"));
}

#[test]
fn map_phenotype_associations_keeps_hpo_rows_and_qualifiers() {
    let resp = association_response(serde_json::json!({
        "items": [
            {
                "subject": "MONDO:0007739",
                "subject_label": "Disease",
                "object": "HP:0001250",
                "object_label": "Seizure",
                "predicate": "biolink:has_phenotype",
                "frequency_qualifier_label": "Frequent",
                "qualifiers_label": ["severe", "childhood"]
            },
            {"object": "MONDO:skip", "object_label": "Skip"}
        ]
    }));

    let rows = MonarchClient::map_phenotype_associations(resp, 5);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].hpo_id, "HP:0001250");
    assert_eq!(rows[0].frequency_qualifier.as_deref(), Some("Frequent"));
    assert_eq!(rows[0].qualifiers, vec!["severe", "childhood"]);
}

#[test]
fn map_model_associations_maps_genotype_rows() {
    let resp = association_response(serde_json::json!({
        "items": [
            {
                "subject": "MGI:3698752",
                "subject_label": "Htt tm1.1",
                "subject_taxon_label": "Mus musculus",
                "predicate": "biolink:model_of",
                "provided_by": "alliance_disease_edges"
            }
        ]
    }));

    let rows = MonarchClient::map_model_associations(resp, 5);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].organism.as_deref(), Some("Mus musculus"));
    assert_eq!(rows[0].source.as_deref(), Some("alliance_disease_edges"));
}

#[test]
fn map_phenotype_matches_maps_scores() {
    let rows: Vec<MonarchSemsimRow> = serde_json::from_value(serde_json::json!([
        {
            "subject": {
                "id": "MONDO:0010450",
                "name": "intellectual disability, X-linked 89"
            },
            "score": 13.302
        },
        {
            "subject": {"id": "HP:0001250", "name": "not a disease"},
            "score": 9.0
        }
    ]))
    .expect("semsim fixture should parse");

    let rows = MonarchClient::map_phenotype_matches(rows, 5);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].disease_id, "MONDO:0010450");
    assert!(rows[0].score > 0.0);
}

#[test]
fn decode_json_response_maps_5xx_to_source_unavailable() {
    let content_type = HeaderValue::from_static("application/json");
    let err = MonarchClient::decode_json_response::<Vec<MonarchSemsimRow>>(
        StatusCode::BAD_GATEWAY,
        Some(&content_type),
        b"bad gateway",
    )
    .expect_err("5xx should be classified as source unavailable");

    match err {
        BioMcpError::SourceUnavailable {
            source_name,
            reason,
            suggestion,
        } => {
            assert_eq!(source_name, "Monarch Initiative");
            assert!(reason.contains("HTTP 502"));
            assert!(suggestion.contains("Retry later"));
        }
        other => panic!("expected SourceUnavailable, got {other}"),
    }
}
