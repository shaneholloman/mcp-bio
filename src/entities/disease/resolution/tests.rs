use super::super::test_support::*;
use super::*;

#[test]
fn normalize_disease_id_basic() {
    assert_eq!(
        normalize_disease_id("MONDO:0005105"),
        Some("MONDO:0005105".into())
    );
    assert_eq!(
        normalize_disease_id("mondo:0005105"),
        Some("MONDO:0005105".into())
    );
    assert_eq!(
        normalize_disease_id(" DOID:1909 "),
        Some("DOID:1909".into())
    );
    assert_eq!(normalize_disease_id("lung cancer"), None);
    assert_eq!(normalize_disease_id("MONDO:"), None);
    assert_eq!(normalize_disease_id("HP:0002861"), None);
}

#[test]
fn parse_disease_lookup_input_distinguishes_canonical_crosswalk_and_text() {
    assert_eq!(
        parse_disease_lookup_input("MONDO:0005105"),
        DiseaseLookupInput::CanonicalOntologyId("MONDO:0005105".into())
    );
    assert_eq!(
        parse_disease_lookup_input("mesh:D008545"),
        DiseaseLookupInput::CrosswalkId(DiseaseXrefKind::Mesh, "D008545".into())
    );
    assert_eq!(
        parse_disease_lookup_input("OMIM:155600"),
        DiseaseLookupInput::CrosswalkId(DiseaseXrefKind::Omim, "155600".into())
    );
    assert_eq!(
        parse_disease_lookup_input("ICD10CM:Q07.0"),
        DiseaseLookupInput::CrosswalkId(DiseaseXrefKind::Icd10Cm, "Q07.0".into())
    );
    assert_eq!(
        parse_disease_lookup_input("Arnold Chiari syndrome"),
        DiseaseLookupInput::FreeText
    );
}

#[test]
fn preferred_crosswalk_hit_prefers_mondo_then_doid_then_lexicographic_id() {
    let best = preferred_crosswalk_hit(vec![
        test_disease_hit("DOID:1909", "melanoma", &[], &[]),
        test_disease_hit("MONDO:0005105", "melanoma", &[], &[]),
        test_disease_hit("MESH:D008545", "melanoma", &[], &[]),
    ])
    .expect("a best hit should be selected");
    assert_eq!(best.id, "MONDO:0005105");
}

#[test]
fn resolver_queries_adds_cml_fallback_variant() {
    let queries = resolver_queries("chronic myeloid leukemia");
    assert!(
        queries
            .iter()
            .any(|query| query == "chronic myelogenous leukemia")
    );
    assert!(
        queries
            .iter()
            .any(|query| query == "chronic myelogenous leukemia, bcr-abl1 positive")
    );
}

#[test]
fn resolver_queries_adds_hodgkin_alias_variants() {
    let queries = resolver_queries("Hodgkin lymphoma");
    assert!(queries.iter().any(|query| query == "hodgkins lymphoma"));
    assert!(queries.iter().any(|query| query == "hodgkin disease"));
}

#[tokio::test]
async fn resolve_disease_hit_by_name_direct_rejects_weak_contains_only_match() {
    let _guard = lock_env().await;
    with_no_http_cache(async {
        let server = MockServer::start().await;
        let _env = set_env_var(
            "BIOMCP_MYDISEASE_BASE",
            Some(&format!("{}/v1", server.uri())),
        );

        Mock::given(method("GET"))
            .and(path("/v1/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "total": 1,
                "hits": [{
                    "_id": "MONDO:0015760",
                    "mondo": {"name": "T-cell non-Hodgkin lymphoma"}
                }]
            })))
            .mount(&server)
            .await;

        let client = MyDiseaseClient::new().expect("client");
        let best = resolve_disease_hit_by_name_direct(&client, "Hodgkin lymphoma")
            .await
            .expect("weak direct match should not error");

        assert!(best.is_none());
    })
    .await;
}

#[test]
fn disease_candidate_score_prefers_canonical_colorectal_match_over_subtype() {
    let broad = disease_candidate_score("colorectal cancer", "colorectal carcinoma");
    let subtype = disease_candidate_score(
        "colorectal cancer",
        "hereditary nonpolyposis colorectal cancer type 6",
    );
    assert!(broad > subtype);
}

#[test]
fn scored_best_candidate_for_queries_prefers_hodgkin_alias_over_non_hodgkin_contains_match() {
    let queries = resolver_queries("Hodgkin lymphoma");
    let best = scored_best_candidate_for_queries(
        &queries,
        vec![
            test_disease_hit("MONDO:0015760", "T-cell non-Hodgkin lymphoma", &[], &[]),
            test_disease_hit(
                "MONDO:0004952",
                "Hodgkins lymphoma",
                &["Hodgkin disease"],
                &[],
            ),
        ],
    )
    .expect("a best hit should be selected");

    assert_eq!(best.id, "MONDO:0004952");
}
#[test]
fn rerank_disease_search_hits_prefers_canonical_exact_candidate_across_query_variants() {
    let canonical = test_disease_hit(
        "MONDO:0024331",
        "colorectal carcinoma",
        &["colorectal cancer"],
        &["colorectal cancer"],
    );

    let ranked = rerank_disease_search_hits(
        "colorectal cancer",
        vec![
            (
                0,
                vec![test_disease_hit(
                    "MONDO:0101010",
                    "hereditary nonpolyposis colorectal cancer type 6",
                    &[],
                    &[],
                )],
            ),
            (
                1,
                vec![
                    canonical,
                    test_disease_hit(
                        "MONDO:0101010",
                        "hereditary nonpolyposis colorectal cancer type 6",
                        &[],
                        &[],
                    ),
                ],
            ),
        ],
    );

    let ids = ranked.iter().map(|hit| hit.id.as_str()).collect::<Vec<_>>();
    assert_eq!(ids, vec!["MONDO:0024331", "MONDO:0101010"]);
}

#[test]
fn disease_exact_rank_prefers_exact_then_prefix_then_contains() {
    assert!(
        disease_exact_rank("colorectal cancer", "colorectal cancer")
            > disease_exact_rank("colorectal cancer syndrome", "colorectal cancer")
    );
    assert!(
        disease_exact_rank("colorectal cancer syndrome", "colorectal cancer")
            > disease_exact_rank("metastatic colorectal cancer", "colorectal cancer")
    );
}

#[test]
fn resolver_queries_adds_carcinoma_fallback_for_cancer_terms() {
    let queries = resolver_queries("breast cancer");
    assert!(queries.iter().any(|q| q == "breast cancer"));
    assert!(queries.iter().any(|q| q == "breast carcinoma"));
}
