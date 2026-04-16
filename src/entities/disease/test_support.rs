//! Shared disease test helpers used by sidecar test modules.

use std::future::Future;

#[allow(unused_imports)]
pub(super) use std::collections::{HashMap, HashSet};

#[allow(unused_imports)]
pub(super) use super::{
    Disease, DiseaseAssociationScoreSummary, DiseaseGeneAssociation, DiseaseSearchResult,
    DiseaseTargetScore, DiseaseVariantAssociation,
};
#[allow(unused_imports)]
pub(super) use crate::entities::SearchPage;
#[allow(unused_imports)]
pub(super) use crate::error::BioMcpError;
#[allow(unused_imports)]
pub(super) use crate::sources::mydisease::MyDiseaseHit;
#[allow(unused_imports)]
pub(super) use crate::test_support::{EnvVarGuard, set_env_var};
#[allow(unused_imports)]
pub(super) use wiremock::matchers::{body_string_contains, method, path, query_param};
#[allow(unused_imports)]
pub(super) use wiremock::{Mock, MockServer, ResponseTemplate};

pub(super) async fn lock_env() -> tokio::sync::MutexGuard<'static, ()> {
    crate::test_support::env_lock().lock().await
}

pub(super) async fn with_no_http_cache<R, Fut>(future: Fut) -> R
where
    Fut: Future<Output = R>,
{
    crate::sources::with_no_cache(true, future).await
}

pub(super) fn test_disease(id: &str, name: &str) -> Disease {
    Disease {
        id: id.to_string(),
        name: name.to_string(),
        definition: None,
        synonyms: Vec::new(),
        parents: Vec::new(),
        associated_genes: Vec::new(),
        gene_associations: Vec::new(),
        top_genes: Vec::new(),
        top_gene_scores: Vec::new(),
        treatment_landscape: Vec::new(),
        recruiting_trial_count: None,
        pathways: Vec::new(),
        phenotypes: Vec::new(),
        key_features: Vec::new(),
        variants: Vec::new(),
        top_variant: None,
        models: Vec::new(),
        prevalence: Vec::new(),
        prevalence_note: None,
        survival: None,
        survival_note: None,
        funding: None,
        funding_note: None,
        civic: None,
        disgenet: None,
        xrefs: HashMap::new(),
    }
}

pub(super) async fn mock_empty_monarch(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/v3/api/association"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "items": []
        })))
        .mount(server)
        .await;
}

pub(super) async fn mock_empty_civic(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "evidenceItems": {
                    "totalCount": 0,
                    "nodes": []
                },
                "assertions": {
                    "totalCount": 0,
                    "nodes": []
                }
            }
        })))
        .mount(server)
        .await;
}

pub(super) async fn mock_empty_mychem(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total": 0,
            "hits": []
        })))
        .mount(server)
        .await;
}

pub(super) async fn mock_empty_ctgov(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/studies"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "studies": [],
            "nextPageToken": null,
            "totalCount": 0
        })))
        .mount(server)
        .await;
}

pub(super) async fn mock_seer_catalog(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/get_var_formats.php"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "VariableFormats": {
                "site": {
                    "1": "All Cancer Sites Combined",
                    "83": "Hodgkin Lymphoma",
                    "97": "Chronic Myeloid Leukemia (CML)"
                },
                "sex": {
                    "1": "Both Sexes",
                    "2": "Male",
                    "3": "Female"
                },
                "race": {
                    "1": "All Races / Ethnicities"
                },
                "age_range": {
                    "1": "All Ages"
                }
            },
            "CancerSites": [
                {"value": 1, "active": true},
                {"value": 83, "active": true},
                {"value": 97, "active": true}
            ]
        })))
        .mount(server)
        .await;
}

pub(super) fn test_disease_hit(
    id: &str,
    disease_name: &str,
    mondo_synonyms: &[&str],
    do_synonyms: &[&str],
) -> MyDiseaseHit {
    serde_json::from_value(serde_json::json!({
        "_id": id,
        "mondo": {
            "name": disease_name,
            "synonym": mondo_synonyms,
        },
        "disease_ontology": {
            "name": disease_name,
            "synonyms": do_synonyms,
        }
    }))
    .expect("valid disease hit")
}

pub(super) fn test_discover_disease_concept(
    label: &str,
    primary_id: Option<&str>,
    synonyms: &[&str],
    xrefs: &[(&str, &str)],
    match_tier: crate::entities::discover::MatchTier,
    confidence: crate::entities::discover::DiscoverConfidence,
) -> crate::entities::discover::DiscoverConcept {
    crate::entities::discover::DiscoverConcept {
        label: label.to_string(),
        primary_id: primary_id.map(str::to_string),
        primary_type: crate::entities::discover::DiscoverType::Disease,
        synonyms: synonyms.iter().map(|value| (*value).to_string()).collect(),
        xrefs: xrefs
            .iter()
            .map(|(source, id)| crate::entities::discover::ConceptXref {
                source: (*source).to_string(),
                id: (*id).to_string(),
            })
            .collect(),
        sources: Vec::new(),
        match_tier,
        confidence,
    }
}
