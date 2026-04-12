use super::super::test_support::*;
use super::*;

#[test]
fn parse_sections_supports_new_disease_sections() {
    let flags = parse_sections(&[
        "phenotypes".to_string(),
        "variants".to_string(),
        "models".to_string(),
        "prevalence".to_string(),
        "survival".to_string(),
        "funding".to_string(),
        "disgenet".to_string(),
        "all".to_string(),
    ])
    .expect("sections should parse");
    assert!(flags.include_genes);
    assert!(flags.include_pathways);
    assert!(flags.include_phenotypes);
    assert!(flags.include_variants);
    assert!(flags.include_models);
    assert!(flags.include_prevalence);
    assert!(flags.include_survival);
    assert!(flags.include_funding);
    assert!(flags.include_civic);
    assert!(flags.include_disgenet);
}

#[test]
fn parse_sections_all_keeps_disgenet_opt_in() {
    let flags = parse_sections(&["all".to_string()]).expect("sections should parse");
    assert!(flags.include_survival);
    assert!(!flags.include_funding);
    assert!(!flags.include_disgenet);
}

#[tokio::test]
async fn get_disease_preserves_canonical_mondo_lookup_path() {
    let _guard = lock_env().await;
    with_no_http_cache(async {
        let server = MockServer::start().await;
        let _env = set_env_var(
            "BIOMCP_MYDISEASE_BASE",
            Some(&format!("{}/v1", server.uri())),
        );

        let body = r#"{
              "_id": "MONDO:0005105",
              "mondo": {
                "name": "melanoma",
                "definition": "Example disease."
              }
            }"#;

        Mock::given(method("GET"))
                .and(path("/v1/disease/MONDO:0005105"))
                .and(query_param(
                    "fields",
                    "_id,mondo.name,mondo.definition,mondo.parents,mondo.synonym,mondo.xrefs,disease_ontology.name,disease_ontology.doid,disease_ontology.def,disease_ontology.parents,disease_ontology.synonyms,disease_ontology.xrefs,umls.mesh,umls.nci,umls.snomed,umls.icd10am,disgenet.genes_related_to_disease,hpo.phenotype_related_to_disease.hpo_id,hpo.phenotype_related_to_disease.evidence,hpo.phenotype_related_to_disease.hp_freq,hpo.inheritance.hpo_id",
                ))
                .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
                .expect(1)
                .mount(&server)
                .await;

        let disease = get("MONDO:0005105", &[])
            .await
            .expect("canonical get should resolve");
        assert_eq!(disease.id, "MONDO:0005105");
        assert_eq!(disease.name, "melanoma");
    })
    .await;
}

#[tokio::test]
async fn get_disease_resolves_mesh_and_omim_crosswalk_ids_before_fetch() {
    let _guard = lock_env().await;
    with_no_http_cache(async {
        let server = MockServer::start().await;
        let _env = set_env_var(
            "BIOMCP_MYDISEASE_BASE",
            Some(&format!("{}/v1", server.uri())),
        );

        let melanoma_get = r#"{
              "_id": "MONDO:0005105",
              "mondo": {
                "name": "melanoma",
                "definition": "Example disease."
              },
              "disease_ontology": {
                "name": "melanoma"
              }
            }"#;
        let marfan_get = r#"{
              "_id": "MONDO:0007947",
              "mondo": {
                "name": "Marfan syndrome",
                "definition": "Example syndrome."
              },
              "disease_ontology": {
                "name": "Marfan syndrome"
              }
            }"#;

        Mock::given(method("GET"))
                .and(path("/v1/query"))
                .and(query_param(
                    "q",
                    "(mondo.xrefs.mesh:\"D008545\" OR disease_ontology.xrefs.mesh:\"D008545\" OR umls.mesh:\"D008545\")",
                ))
                .respond_with(ResponseTemplate::new(200).set_body_raw(
                    r#"{"total":2,"hits":[{"_id":"DOID:1909","disease_ontology":{"name":"melanoma"}},{"_id":"MONDO:0005105","mondo":{"name":"melanoma"}}]}"#,
                    "application/json",
                ))
                .expect(1)
                .mount(&server)
                .await;

        Mock::given(method("GET"))
            .and(path("/v1/query"))
            .and(query_param(
                "q",
                "(mondo.xrefs.omim:\"154700\" OR disease_ontology.xrefs.omim:\"154700\")",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{"total":1,"hits":[{"_id":"MONDO:0007947","mondo":{"name":"Marfan syndrome"}}]}"#,
                "application/json",
            ))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/v1/disease/MONDO:0005105"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(melanoma_get, "application/json"))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/v1/disease/MONDO:0007947"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(marfan_get, "application/json"))
            .expect(1)
            .mount(&server)
            .await;

        let mesh = get("MESH:D008545", &[])
            .await
            .expect("mesh crosswalk should resolve");
        assert_eq!(mesh.id, "MONDO:0005105");
        assert_eq!(mesh.name, "melanoma");

        let omim = get("OMIM:154700", &[])
            .await
            .expect("omim crosswalk should resolve");
        assert_eq!(omim.id, "MONDO:0007947");
        assert_eq!(omim.name, "Marfan syndrome");
    })
    .await;
}

#[tokio::test]
async fn get_disease_returns_not_found_for_unresolved_crosswalk_without_name_fallback() {
    let _guard = lock_env().await;
    with_no_http_cache(async {
        let server = MockServer::start().await;
        let _env = set_env_var(
            "BIOMCP_MYDISEASE_BASE",
            Some(&format!("{}/v1", server.uri())),
        );

        Mock::given(method("GET"))
            .and(path("/v1/query"))
            .and(query_param(
                "q",
                "(mondo.xrefs.omim:\"000000\" OR disease_ontology.xrefs.omim:\"000000\")",
            ))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_raw(r#"{"total":0,"hits":[]}"#, "application/json"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let err = get("OMIM:000000", &[])
            .await
            .expect_err("unresolved crosswalk should return not found");
        match err {
            BioMcpError::NotFound {
                entity,
                id,
                suggestion,
            } => {
                assert_eq!(entity, "disease");
                assert_eq!(id, "OMIM:000000");
                assert!(suggestion.contains("biomcp discover"));
            }
            other => panic!("expected not found, got {other:?}"),
        }
    })
    .await;
}

pub(crate) async fn proof_get_disease_genes_promotes_opentargets_rows_for_cll() {
    let _guard = lock_env().await;
    with_no_http_cache(async {
        let mydisease = MockServer::start().await;
        let opentargets = MockServer::start().await;
        let monarch = MockServer::start().await;
        let civic = MockServer::start().await;
        let mychem = MockServer::start().await;
        let ctgov = MockServer::start().await;
        let _mydisease_env = set_env_var(
            "BIOMCP_MYDISEASE_BASE",
            Some(&format!("{}/v1", mydisease.uri())),
        );
        let _opentargets_env = set_env_var("BIOMCP_OPENTARGETS_BASE", Some(&opentargets.uri()));
        let _monarch_env = set_env_var("BIOMCP_MONARCH_BASE", Some(&monarch.uri()));
        let _civic_env = set_env_var("BIOMCP_CIVIC_BASE", Some(&civic.uri()));
        let _mychem_env =
            set_env_var("BIOMCP_MYCHEM_BASE", Some(&format!("{}/v1", mychem.uri())));
        let _ctgov_env = set_env_var(
            "BIOMCP_CTGOV_BASE",
            Some(&format!("{}/api/v2", ctgov.uri())),
        );

        Mock::given(method("GET"))
            .and(path("/v1/disease/MONDO:0003864"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "_id": "MONDO:0003864",
                "mondo": {
                    "name": "chronic lymphocytic leukemia",
                    "synonym": ["CLL"]
                },
                "disease_ontology": {
                    "name": "chronic lymphocytic leukemia"
                }
            })))
            .mount(&mydisease)
            .await;

    Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(body_string_contains("SearchDisease"))
            .and(body_string_contains("\"query\":\"chronic lymphocytic leukemia\""))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "search": {
                        "hits": [
                            {"id": "EFO_0000095", "name": "chronic lymphocytic leukemia", "entity": "disease"}
                        ]
                    }
                }
            })))
            .mount(&opentargets)
            .await;

    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_string_contains("DiseaseGenes"))
        .and(body_string_contains("\"efoId\":\"EFO_0000095\""))
        .and(body_string_contains("\"size\":20"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "disease": {
                    "associatedTargets": {
                        "rows": [
                            {
                                "score": 0.99,
                                "datatypeScores": [{"id": "somatic_mutation", "score": 0.88}],
                                "datasourceScores": [],
                                "target": {"approvedSymbol": "TP53"}
                            },
                            {
                                "score": 0.94,
                                "datatypeScores": [{"id": "somatic_mutation", "score": 0.71}],
                                "datasourceScores": [],
                                "target": {"approvedSymbol": "ATM"}
                            },
                            {
                                "score": 0.91,
                                "datatypeScores": [{"id": "somatic_mutation", "score": 0.69}],
                                "datasourceScores": [],
                                "target": {"approvedSymbol": "NOTCH1"}
                            },
                            {
                                "score": 0.89,
                                "datatypeScores": [{"id": "somatic_mutation", "score": 0.66}],
                                "datasourceScores": [],
                                "target": {"approvedSymbol": "XPO1"}
                            },
                            {
                                "score": 0.86,
                                "datatypeScores": [{"id": "somatic_mutation", "score": 0.62}],
                                "datasourceScores": [],
                                "target": {"approvedSymbol": "MYD88"}
                            },
                            {
                                "score": 0.85,
                                "datatypeScores": [{"id": "somatic_mutation", "score": 0.61}],
                                "datasourceScores": [],
                                "target": {"approvedSymbol": "SF3B1"}
                            },
                            {
                                "score": 0.82,
                                "datatypeScores": [{"id": "somatic_mutation", "score": 0.58}],
                                "datasourceScores": [],
                                "target": {"approvedSymbol": "FBXW7"}
                            },
                            {
                                "score": 0.81,
                                "datatypeScores": [{"id": "somatic_mutation", "score": 0.57}],
                                "datasourceScores": [],
                                "target": {"approvedSymbol": "BCL2"}
                            }
                        ]
                    }
                }
            }
        })))
        .mount(&opentargets)
        .await;

    mock_empty_monarch(&monarch).await;

    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_string_contains("CivicContext"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "evidenceItems": {
                    "totalCount": 1,
                    "nodes": [
                        {
                            "id": 1,
                            "name": "BCL2 evidence",
                            "status": "ACCEPTED",
                            "evidenceType": "PREDICTIVE",
                            "evidenceLevel": "A",
                            "significance": "SUPPORTS",
                            "molecularProfile": {"name": "BCL2 amplification"},
                            "disease": {"displayName": "chronic lymphocytic leukemia"},
                            "therapies": [],
                            "source": {
                                "citation": "PMID:1",
                                "sourceType": "PUBMED",
                                "publicationYear": 2024
                            }
                        }
                    ]
                },
                "assertions": {
                    "totalCount": 0,
                    "nodes": []
                }
            }
        })))
        .mount(&civic)
        .await;

    mock_empty_mychem(&mychem).await;
    mock_empty_ctgov(&ctgov).await;

        let disease = get("MONDO:0003864", &["genes".to_string()])
            .await
            .expect("CLL should resolve");

        let genes = disease
            .gene_associations
            .iter()
            .map(|row| row.gene.as_str())
            .collect::<Vec<_>>();
        let cll_gold = [
            "TP53", "ATM", "NOTCH1", "XPO1", "MYD88", "SF3B1", "FBXW7", "BCL2",
        ];
        let matched = cll_gold.iter().filter(|gene| genes.contains(gene)).count();
        assert!(
            matched >= 8,
            "expected >=8 CLL gold genes, got {matched}: {genes:?}"
        );
        assert!(disease.gene_associations.iter().any(|row| {
            row.gene == "TP53"
                && row.source.as_deref() == Some("OpenTargets")
                && row.opentargets_score.is_some()
        }));
        assert!(disease.gene_associations.iter().any(|row| {
            row.gene == "BCL2"
                && row.source.as_deref() == Some("CIViC; OpenTargets")
                && row.opentargets_score.is_some()
        }));
    })
    .await;
}

#[tokio::test]
async fn get_disease_genes_promotes_opentargets_rows_for_cll() {
    proof_get_disease_genes_promotes_opentargets_rows_for_cll().await;
}

pub(crate) async fn proof_get_disease_genes_uses_ols4_label_fallback_for_sparse_mondo_identity() {
    let _guard = lock_env().await;
    with_no_http_cache(async {
        let mydisease = MockServer::start().await;
        let opentargets = MockServer::start().await;
        let monarch = MockServer::start().await;
        let civic = MockServer::start().await;
        let ols4 = MockServer::start().await;
        let mychem = MockServer::start().await;
        let ctgov = MockServer::start().await;
        let _mydisease_env = set_env_var(
            "BIOMCP_MYDISEASE_BASE",
            Some(&format!("{}/v1", mydisease.uri())),
        );
        let _opentargets_env = set_env_var("BIOMCP_OPENTARGETS_BASE", Some(&opentargets.uri()));
        let _monarch_env = set_env_var("BIOMCP_MONARCH_BASE", Some(&monarch.uri()));
        let _civic_env = set_env_var("BIOMCP_CIVIC_BASE", Some(&civic.uri()));
        let _ols4_env = set_env_var("BIOMCP_OLS4_BASE", Some(&ols4.uri()));
        let _mychem_env =
            set_env_var("BIOMCP_MYCHEM_BASE", Some(&format!("{}/v1", mychem.uri())));
        let _ctgov_env = set_env_var(
            "BIOMCP_CTGOV_BASE",
            Some(&format!("{}/api/v2", ctgov.uri())),
        );

        Mock::given(method("GET"))
            .and(path("/v1/disease/MONDO:0019468"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "_id": "MONDO:0019468",
                "mondo": {
                    "name": "MONDO:0019468"
                }
            })))
            .mount(&mydisease)
            .await;

    Mock::given(method("GET"))
        .and(path("/api/search"))
        .and(query_param("q", "MONDO:0019468"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "response": {
                "docs": [
                    {
                        "iri": "http://purl.obolibrary.org/obo/MONDO_0019468",
                        "ontology_name": "mondo",
                        "ontology_prefix": "mondo",
                        "short_form": "MONDO_0019468",
                        "obo_id": "MONDO:0019468",
                        "label": "T-cell prolymphocytic leukemia",
                        "description": [],
                        "exact_synonyms": ["T-PLL"],
                        "type": "class"
                    }
                ]
            }
        })))
        .mount(&ols4)
        .await;

    Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(body_string_contains("SearchDisease"))
            .and(body_string_contains("\"query\":\"T-cell prolymphocytic leukemia\""))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "search": {
                        "hits": [
                            {"id": "EFO_1000560", "name": "T-cell prolymphocytic leukemia", "entity": "disease"}
                        ]
                    }
                }
            })))
            .expect(1)
            .mount(&opentargets)
            .await;

    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_string_contains("DiseaseGenes"))
        .and(body_string_contains("\"efoId\":\"EFO_1000560\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "disease": {
                    "associatedTargets": {
                        "rows": [
                            {
                                "score": 0.95,
                                "datatypeScores": [{"id": "somatic_mutation", "score": 0.82}],
                                "datasourceScores": [],
                                "target": {"approvedSymbol": "ATM"}
                            },
                            {
                                "score": 0.88,
                                "datatypeScores": [{"id": "somatic_mutation", "score": 0.77}],
                                "datasourceScores": [],
                                "target": {"approvedSymbol": "JAK3"}
                            },
                            {
                                "score": 0.81,
                                "datatypeScores": [{"id": "somatic_mutation", "score": 0.72}],
                                "datasourceScores": [],
                                "target": {"approvedSymbol": "STAT5B"}
                            }
                        ]
                    }
                }
            }
        })))
        .mount(&opentargets)
        .await;

    mock_empty_monarch(&monarch).await;
    mock_empty_civic(&civic).await;
    mock_empty_mychem(&mychem).await;
    mock_empty_ctgov(&ctgov).await;

        let disease = get("MONDO:0019468", &["genes".to_string()])
            .await
            .expect("T-PLL should resolve");

        assert_eq!(disease.name, "T-cell prolymphocytic leukemia");
        assert!(disease.synonyms.iter().any(|value| value == "T-PLL"));
        let genes = disease
            .gene_associations
            .iter()
            .map(|row| row.gene.as_str())
            .collect::<Vec<_>>();
        assert!(genes.contains(&"ATM"));
        assert!(genes.contains(&"JAK3"));
        assert!(genes.contains(&"STAT5B"));
    })
    .await;
}

#[tokio::test]
async fn get_disease_genes_uses_ols4_label_fallback_for_sparse_mondo_identity() {
    proof_get_disease_genes_uses_ols4_label_fallback_for_sparse_mondo_identity().await;
}
