use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use clap::Parser;

use super::super::test_support::{
    Mock, MockServer, ResponseTemplate, lock_env, method, mount_drug_lookup_miss,
    mount_gene_lookup_hit, mount_gene_lookup_miss, mount_ols_alias, path, query_param, set_env_var,
};
use super::super::{
    Cli, OutputStream, PaginationMeta, execute, execute_mcp, extract_json_from_sections,
    resolve_query_input, run_outcome, search_json, search_json_with_meta, search_meta,
    search_meta_with_suggestions,
};

#[test]
fn extract_json_from_sections_detects_trailing_long_flag() {
    let sections = vec!["all".to_string(), "--json".to_string()];
    let (cleaned, json_override) = extract_json_from_sections(&sections);
    assert_eq!(cleaned, vec!["all".to_string()]);
    assert!(json_override);
}

#[test]
fn extract_json_from_sections_detects_trailing_short_flag() {
    let sections = vec!["clinvar".to_string(), "-j".to_string()];
    let (cleaned, json_override) = extract_json_from_sections(&sections);
    assert_eq!(cleaned, vec!["clinvar".to_string()]);
    assert!(json_override);
}

#[test]
fn extract_json_from_sections_keeps_regular_sections() {
    let sections = vec!["eligibility".to_string(), "locations".to_string()];
    let (cleaned, json_override) = extract_json_from_sections(&sections);
    assert_eq!(cleaned, sections);
    assert!(!json_override);
}

#[test]
fn resolve_query_input_accepts_flag_or_positional() {
    let from_flag = resolve_query_input(Some("BRAF".into()), None, "--query").unwrap();
    assert_eq!(from_flag.as_deref(), Some("BRAF"));

    let from_positional = resolve_query_input(None, Some("melanoma".into()), "--query").unwrap();
    assert_eq!(from_positional.as_deref(), Some("melanoma"));
}

#[test]
fn resolve_query_input_rejects_dual_values() {
    let err = resolve_query_input(Some("BRAF".into()), Some("TP53".into()), "--query").unwrap_err();
    assert!(format!("{err}").contains("Use either positional QUERY or --query, not both"));

    let err_gene =
        resolve_query_input(Some("TP53".into()), Some("BRAF".into()), "--gene").unwrap_err();
    assert!(format!("{err_gene}").contains("Use either positional QUERY or --gene, not both"));
}

#[test]
fn phenotype_search_json_contract_unchanged() {
    let pagination = PaginationMeta::offset(0, 1, 1, Some(1));
    let json = search_json(
        vec![crate::entities::disease::PhenotypeSearchResult {
            disease_id: "MONDO:0100135".to_string(),
            disease_name: "Dravet syndrome".to_string(),
            score: 15.036,
        }],
        pagination,
    )
    .expect("phenotype search json");

    let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    assert_eq!(value["count"], 1);
    assert_eq!(value["results"][0]["disease_id"], "MONDO:0100135");
    assert_eq!(value["results"][0]["disease_name"], "Dravet syndrome");
    assert!(
        value.get("_meta").is_none(),
        "generic search json should not grow entity-style _meta"
    );
}

#[test]
fn search_meta_trims_empty_commands() {
    let meta = search_meta(vec![
        " biomcp get gene BRAF ".to_string(),
        String::new(),
        "   ".to_string(),
        "biomcp list gene".to_string(),
    ])
    .expect("search meta should be present");

    let value = serde_json::to_value(meta).expect("meta json");
    assert_eq!(
        value["next_commands"][0],
        serde_json::Value::String("biomcp get gene BRAF".into())
    );
    assert_eq!(
        value["next_commands"][1],
        serde_json::Value::String("biomcp list gene".into())
    );
    assert!(value.get("suggestions").is_none());
}

#[test]
fn search_meta_with_suggestions_keeps_empty_suggestions_array() {
    let meta = search_meta_with_suggestions(
        vec!["biomcp get article 12345".to_string()],
        Some(vec![String::new(), "   ".to_string()]),
    )
    .expect("search meta should be present");

    let value = serde_json::to_value(meta).expect("meta json");
    assert_eq!(value["suggestions"].as_array().map(Vec::len), Some(0));
}

#[test]
fn search_json_with_meta_includes_next_commands() {
    let pagination = PaginationMeta::offset(0, 1, 1, Some(1));
    let json = search_json_with_meta(
        vec![crate::entities::gene::GeneSearchResult {
            symbol: "BRAF".to_string(),
            name: "B-Raf proto-oncogene".to_string(),
            entrez_id: "673".to_string(),
            genomic_coordinates: None,
            uniprot_id: None,
            omim_id: None,
        }],
        pagination,
        vec![
            "biomcp get gene BRAF".to_string(),
            "biomcp list gene".to_string(),
        ],
    )
    .expect("search json with meta");

    let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    assert_eq!(value["count"], 1);
    assert_eq!(
        value["_meta"]["next_commands"][0],
        serde_json::Value::String("biomcp get gene BRAF".into())
    );
    assert_eq!(
        value["_meta"]["next_commands"][1],
        serde_json::Value::String("biomcp list gene".into())
    );
}

#[test]
fn search_json_with_meta_omits_meta_when_empty() {
    let pagination = PaginationMeta::offset(0, 1, 1, Some(1));
    let json = search_json_with_meta(
        vec![crate::entities::gene::GeneSearchResult {
            symbol: "BRAF".to_string(),
            name: "B-Raf proto-oncogene".to_string(),
            entrez_id: "673".to_string(),
            genomic_coordinates: None,
            uniprot_id: None,
            omim_id: None,
        }],
        pagination,
        vec![String::new(), "   ".to_string()],
    )
    .expect("search json with empty meta");

    let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    assert!(value.get("_meta").is_none());
}

#[tokio::test]
async fn gene_alias_fallback_returns_exit_1_markdown_suggestion() {
    let _guard = lock_env().await;
    let mygene = MockServer::start().await;
    let ols = MockServer::start().await;
    let _mygene_base = set_env_var("BIOMCP_MYGENE_BASE", Some(&format!("{}/v3", mygene.uri())));
    let _ols_base = set_env_var("BIOMCP_OLS4_BASE", Some(&ols.uri()));
    let _umls_base = set_env_var("BIOMCP_UMLS_BASE", None);
    let _umls_key = set_env_var("UMLS_API_KEY", None);

    mount_gene_lookup_miss(&mygene, "ERBB1").await;
    mount_ols_alias(&ols, "ERBB1", "hgnc", "HGNC:3236", "EGFR", &["ERBB1"], 1).await;

    let cli = Cli::try_parse_from(["biomcp", "get", "gene", "ERBB1"]).expect("parse");
    let outcome = run_outcome(cli).await.expect("alias outcome");

    assert_eq!(outcome.stream, OutputStream::Stderr);
    assert_eq!(outcome.exit_code, 1);
    assert!(outcome.text.contains("Error: gene 'ERBB1' not found."));
    assert!(
        outcome
            .text
            .contains("Did you mean: `biomcp get gene EGFR`")
    );
}

#[tokio::test]
async fn gene_alias_fallback_json_writes_stdout_and_exit_1() {
    let _guard = lock_env().await;
    let mygene = MockServer::start().await;
    let ols = MockServer::start().await;
    let _mygene_base = set_env_var("BIOMCP_MYGENE_BASE", Some(&format!("{}/v3", mygene.uri())));
    let _ols_base = set_env_var("BIOMCP_OLS4_BASE", Some(&ols.uri()));
    let _umls_base = set_env_var("BIOMCP_UMLS_BASE", None);
    let _umls_key = set_env_var("UMLS_API_KEY", None);

    mount_gene_lookup_miss(&mygene, "ERBB1").await;
    mount_ols_alias(&ols, "ERBB1", "hgnc", "HGNC:3236", "EGFR", &["ERBB1"], 1).await;

    let cli = Cli::try_parse_from(["biomcp", "--json", "get", "gene", "ERBB1"]).expect("parse");
    let outcome = run_outcome(cli).await.expect("alias json outcome");

    assert_eq!(outcome.stream, OutputStream::Stdout);
    assert_eq!(outcome.exit_code, 1);
    let value: serde_json::Value = serde_json::from_str(&outcome.text).expect("valid alias json");
    assert_eq!(
        value["_meta"]["alias_resolution"]["canonical"], "EGFR",
        "json={value}"
    );
    assert_eq!(value["_meta"]["next_commands"][0], "biomcp get gene EGFR");
}

#[tokio::test]
async fn canonical_gene_lookup_skips_discovery() {
    let _guard = lock_env().await;
    let mygene = MockServer::start().await;
    let ols = MockServer::start().await;
    let _mygene_base = set_env_var("BIOMCP_MYGENE_BASE", Some(&format!("{}/v3", mygene.uri())));
    let _ols_base = set_env_var("BIOMCP_OLS4_BASE", Some(&ols.uri()));
    let _umls_base = set_env_var("BIOMCP_UMLS_BASE", None);
    let _umls_key = set_env_var("UMLS_API_KEY", None);

    mount_gene_lookup_hit(&mygene, "TP53", "tumor protein p53", "7157").await;
    mount_ols_alias(&ols, "TP53", "hgnc", "HGNC:11998", "TP53", &["P53"], 0).await;

    let cli = Cli::try_parse_from(["biomcp", "get", "gene", "TP53"]).expect("parse");
    let outcome = run_outcome(cli).await.expect("success outcome");

    assert_eq!(outcome.stream, OutputStream::Stdout);
    assert_eq!(outcome.exit_code, 0);
    assert!(outcome.text.contains("# TP53"));
}

#[test]
fn batch_gene_json_includes_meta_per_item() {
    std::thread::Builder::new()
        .name("batch-gene-json-test".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime")
                .block_on(async {
                    let _guard = lock_env().await;
                    let mygene = MockServer::start().await;
                    let _mygene_base = set_env_var(
                        "BIOMCP_MYGENE_BASE",
                        Some(&format!("{}/v3", mygene.uri())),
                    );

                    mount_gene_lookup_hit(&mygene, "BRAF", "B-Raf proto-oncogene", "673").await;
                    mount_gene_lookup_hit(&mygene, "TP53", "tumor protein p53", "7157").await;

                    let output = execute(vec![
                        "biomcp".to_string(),
                        "--json".to_string(),
                        "batch".to_string(),
                        "gene".to_string(),
                        "BRAF,TP53".to_string(),
                    ])
                    .await
                    .expect("batch outcome");
                    let value: serde_json::Value =
                        serde_json::from_str(&output).expect("valid batch json");
                    let items = value.as_array().expect("batch root should stay an array");
                    assert_eq!(items.len(), 2, "json={value}");
                    assert_eq!(items[0]["symbol"], "BRAF", "json={value}");
                    assert_eq!(items[1]["symbol"], "TP53", "json={value}");
                    assert!(
                        items.iter().all(|item| item["_meta"]["evidence_urls"]
                            .as_array()
                            .is_some_and(|urls| !urls.is_empty())),
                        "each batch item should include non-empty _meta.evidence_urls: {value}"
                    );
                    assert!(
                        items.iter().all(|item| item["_meta"]["next_commands"]
                            .as_array()
                            .is_some_and(|cmds| !cmds.is_empty())),
                        "each batch item should include non-empty _meta.next_commands: {value}"
                    );
                    assert!(
                        items.iter().any(|item| item["_meta"]["section_sources"]
                            .as_array()
                            .is_some_and(|sources| !sources.is_empty())),
                        "at least one batch item should include non-empty _meta.section_sources: {value}"
                    );
                });
        })
        .expect("spawn")
        .join()
        .expect("thread should complete");
}

#[tokio::test]
async fn ambiguous_gene_miss_points_to_discover() {
    let _guard = lock_env().await;
    let mygene = MockServer::start().await;
    let ols = MockServer::start().await;
    let _mygene_base = set_env_var("BIOMCP_MYGENE_BASE", Some(&format!("{}/v3", mygene.uri())));
    let _ols_base = set_env_var("BIOMCP_OLS4_BASE", Some(&ols.uri()));
    let _umls_base = set_env_var("BIOMCP_UMLS_BASE", None);
    let _umls_key = set_env_var("UMLS_API_KEY", None);

    mount_gene_lookup_miss(&mygene, "V600E").await;
    mount_ols_alias(&ols, "V600E", "so", "SO:0001583", "V600E", &["V600E"], 1).await;

    let cli = Cli::try_parse_from(["biomcp", "get", "gene", "V600E"]).expect("parse");
    let outcome = run_outcome(cli).await.expect("ambiguous outcome");

    assert_eq!(outcome.stream, OutputStream::Stderr);
    assert_eq!(outcome.exit_code, 1);
    assert!(
        outcome
            .text
            .contains("BioMCP could not map 'V600E' to a single gene.")
    );
    assert!(outcome.text.contains("1. biomcp discover V600E"));
    assert!(outcome.text.contains("2. biomcp search gene -q V600E"));
}

#[tokio::test]
async fn alias_fallback_ols_failure_preserves_original_not_found() {
    let _guard = lock_env().await;
    let mygene = MockServer::start().await;
    let ols = MockServer::start().await;
    let _mygene_base = set_env_var("BIOMCP_MYGENE_BASE", Some(&format!("{}/v3", mygene.uri())));
    let _ols_base = set_env_var("BIOMCP_OLS4_BASE", Some(&ols.uri()));
    let _umls_base = set_env_var("BIOMCP_UMLS_BASE", None);
    let _umls_key = set_env_var("UMLS_API_KEY", None);

    mount_gene_lookup_miss(&mygene, "ERBB1").await;
    let ols_calls = Arc::new(AtomicUsize::new(0));
    let ols_calls_for_responder = Arc::clone(&ols_calls);
    Mock::given(method("GET"))
        .and(path("/api/search"))
        .and(query_param("q", "ERBB1"))
        .respond_with(move |_request: &wiremock::Request| {
            let call_index = ols_calls_for_responder.fetch_add(1, Ordering::SeqCst);
            if call_index == 0 {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "response": {
                        "docs": [{
                            "iri": "http://example.org/hgnc/HGNC_3236",
                            "ontology_name": "hgnc",
                            "ontology_prefix": "hgnc",
                            "short_form": "hgnc:3236",
                            "obo_id": "HGNC:3236",
                            "label": "EGFR",
                            "description": [],
                            "exact_synonyms": ["ERBB1"],
                            "type": "class"
                        }]
                    }
                }))
            } else {
                ResponseTemplate::new(500).set_body_raw("upstream down", "text/plain")
            }
        })
        .expect(2u64..)
        .mount(&ols)
        .await;

    crate::entities::discover::resolve_query(
        "ERBB1",
        crate::entities::discover::DiscoverMode::Command,
    )
    .await
    .expect("warm cache with a successful discover lookup");

    let cli = Cli::try_parse_from(["biomcp", "get", "gene", "ERBB1"]).expect("parse");
    let err = run_outcome(cli)
        .await
        .expect_err("should preserve not found");
    let rendered = err.to_string();

    assert!(
        ols_calls.load(Ordering::SeqCst) >= 2,
        "alias fallback should re-query OLS after the cache warm-up"
    );
    assert!(rendered.contains("gene 'ERBB1' not found"));
    assert!(rendered.contains("Try searching: biomcp search gene -q ERBB1"));
}

#[tokio::test]
async fn drug_alias_fallback_returns_exit_1_markdown_suggestion() {
    let _guard = lock_env().await;
    let mychem = MockServer::start().await;
    let ols = MockServer::start().await;
    let _mychem_base = set_env_var("BIOMCP_MYCHEM_BASE", Some(&format!("{}/v1", mychem.uri())));
    let _ols_base = set_env_var("BIOMCP_OLS4_BASE", Some(&ols.uri()));
    let _umls_base = set_env_var("BIOMCP_UMLS_BASE", None);
    let _umls_key = set_env_var("UMLS_API_KEY", None);

    mount_drug_lookup_miss(&mychem, "Keytruda").await;
    mount_ols_alias(
        &ols,
        "Keytruda",
        "mesh",
        "MESH:C582435",
        "pembrolizumab",
        &["Keytruda"],
        1,
    )
    .await;

    let cli = Cli::try_parse_from(["biomcp", "get", "drug", "Keytruda"]).expect("parse");
    let outcome = run_outcome(cli).await.expect("drug alias outcome");

    assert_eq!(outcome.stream, OutputStream::Stderr);
    assert_eq!(outcome.exit_code, 1);
    assert!(outcome.text.contains("Error: drug 'Keytruda' not found."));
    assert!(
        outcome
            .text
            .contains("Did you mean: `biomcp get drug pembrolizumab`")
    );
}

#[tokio::test]
async fn drug_alias_fallback_json_writes_stdout_and_exit_1() {
    let _guard = lock_env().await;
    let mychem = MockServer::start().await;
    let ols = MockServer::start().await;
    let _mychem_base = set_env_var("BIOMCP_MYCHEM_BASE", Some(&format!("{}/v1", mychem.uri())));
    let _ols_base = set_env_var("BIOMCP_OLS4_BASE", Some(&ols.uri()));
    let _umls_base = set_env_var("BIOMCP_UMLS_BASE", None);
    let _umls_key = set_env_var("UMLS_API_KEY", None);

    mount_drug_lookup_miss(&mychem, "Keytruda").await;
    mount_ols_alias(
        &ols,
        "Keytruda",
        "mesh",
        "MESH:C582435",
        "pembrolizumab",
        &["Keytruda"],
        1,
    )
    .await;

    let cli = Cli::try_parse_from(["biomcp", "--json", "get", "drug", "Keytruda"]).expect("parse");
    let outcome = run_outcome(cli).await.expect("drug alias json outcome");

    assert_eq!(outcome.stream, OutputStream::Stdout);
    assert_eq!(outcome.exit_code, 1);
    let value: serde_json::Value = serde_json::from_str(&outcome.text).expect("valid alias json");
    assert_eq!(
        value["_meta"]["alias_resolution"]["canonical"],
        "pembrolizumab"
    );
    assert_eq!(
        value["_meta"]["next_commands"][0],
        "biomcp get drug pembrolizumab"
    );
}

#[tokio::test]
async fn execute_mcp_alias_suggestion_returns_structured_json_text() {
    let _guard = lock_env().await;
    let mygene = MockServer::start().await;
    let ols = MockServer::start().await;
    let _mygene_base = set_env_var("BIOMCP_MYGENE_BASE", Some(&format!("{}/v3", mygene.uri())));
    let _ols_base = set_env_var("BIOMCP_OLS4_BASE", Some(&ols.uri()));
    let _umls_base = set_env_var("BIOMCP_UMLS_BASE", None);
    let _umls_key = set_env_var("UMLS_API_KEY", None);

    mount_gene_lookup_miss(&mygene, "ERBB1").await;
    mount_ols_alias(&ols, "ERBB1", "hgnc", "HGNC:3236", "EGFR", &["ERBB1"], 1).await;

    let output = execute_mcp(vec![
        "biomcp".to_string(),
        "get".to_string(),
        "gene".to_string(),
        "ERBB1".to_string(),
    ])
    .await
    .expect("mcp alias outcome");

    let value: serde_json::Value =
        serde_json::from_str(&output.text).expect("valid mcp alias json");
    assert_eq!(value["_meta"]["alias_resolution"]["kind"], "canonical");
    assert_eq!(value["_meta"]["alias_resolution"]["canonical"], "EGFR");
}
