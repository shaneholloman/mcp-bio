mod next_commands_json_property;
mod next_commands_validity;

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use super::test_support::{
    Mock, MockServer, ResponseTemplate, TempDirGuard, lock_env, method, mount_drug_lookup_miss,
    mount_gene_lookup_hit, mount_gene_lookup_miss, mount_ols_alias, path, query_param, set_env_var,
};
use super::{
    ChartArgs, Cli, Commands, McpChartPass, OutputStream, PaginationMeta, StudyCommand,
    article_search_json, build_article_debug_plan, disease_search_json,
    drug_all_region_search_json, execute, execute_mcp, extract_json_from_sections,
    resolve_drug_search_region, resolve_query_input, rewrite_mcp_chart_args, run_outcome,
    search_json, should_try_pathway_trial_fallback, truncate_article_annotations,
};
use crate::entities::drug::{DrugRegion, DrugSearchFilters};
use clap::{CommandFactory, FromArgMatches, Parser};

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

#[tokio::test]
async fn get_drug_raw_rejects_non_label_sections() {
    let cli = Cli::try_parse_from(["biomcp", "get", "drug", "pembrolizumab", "targets", "--raw"])
        .expect("get drug --raw should parse");

    let err = run_outcome(cli)
        .await
        .expect_err("targets --raw should be rejected");
    assert!(
        err.to_string()
            .contains("--raw can only be used with label or all")
    );
}

#[test]
fn skill_help_examples_match_installed_surface() {
    let mut command = Cli::command();
    let skill = command
        .find_subcommand_mut("skill")
        .expect("skill subcommand should exist");
    let mut help = Vec::new();
    skill
        .write_long_help(&mut help)
        .expect("skill help should render");
    let help = String::from_utf8(help).expect("help should be utf-8");

    assert!(help.contains("biomcp skill            # show skill overview"));
    assert!(help.contains("biomcp skill install    # install skill to your agent config"));
    assert!(help.contains("Commands:\n  list"));
    assert!(!help.contains("biomcp skill 03"));
    assert!(!help.contains("variant-to-treatment"));
}

#[test]
fn runtime_help_hides_query_only_global_flags() {
    for subcommand_name in super::RUNTIME_HELP_SUBCOMMANDS {
        let mut command = super::build_cli();
        let runtime = command
            .find_subcommand_mut(subcommand_name)
            .expect("runtime subcommand should exist");
        let mut help = Vec::new();
        runtime
            .write_long_help(&mut help)
            .expect("runtime help should render");
        let help = String::from_utf8(help).expect("help should be utf-8");

        assert!(
            !help.contains("--json"),
            "{subcommand_name} help should not advertise --json"
        );
        assert!(
            !help.contains("--no-cache"),
            "{subcommand_name} help should not advertise --no-cache"
        );
    }
}

#[test]
fn runtime_commands_still_parse_hidden_global_flags() {
    let cli = parse_built_cli([
        "biomcp",
        "serve-http",
        "--json",
        "--no-cache",
        "--host",
        "127.0.0.1",
        "--port",
        "8080",
    ]);
    assert!(cli.json);
    assert!(cli.no_cache);
    assert!(matches!(
        cli.command,
        Commands::ServeHttp(super::system::ServeHttpArgs { host, port })
            if host == "127.0.0.1" && port == 8080
    ));

    for args in [
        ["biomcp", "mcp", "--json", "--no-cache"].as_slice(),
        ["biomcp", "serve", "--json", "--no-cache"].as_slice(),
        ["biomcp", "serve-sse", "--json", "--no-cache"].as_slice(),
    ] {
        let cli = parse_built_cli(args);
        assert!(cli.json);
        assert!(cli.no_cache);
    }
}

#[test]
fn serve_sse_help_stays_callable_and_deprecated() {
    let mut command = super::build_cli();
    let serve_sse = command
        .find_subcommand_mut("serve-sse")
        .expect("serve-sse subcommand should exist");
    let mut help = Vec::new();
    serve_sse
        .write_long_help(&mut help)
        .expect("serve-sse help should render");
    let help = String::from_utf8(help).expect("help should be utf-8");

    assert!(help.contains("serve-sse"));
    assert!(help.contains("removed"));
    assert!(help.contains("serve-http"));
    assert!(help.contains("/mcp"));
    assert!(!help.contains("--json"));
    assert!(!help.contains("--no-cache"));
}

#[test]
fn top_level_help_hides_serve_sse_but_keeps_serve_http() {
    let mut command = super::build_cli();
    let mut help = Vec::new();
    command
        .write_long_help(&mut help)
        .expect("top-level help should render");
    let help = String::from_utf8(help).expect("help should be utf-8");

    assert!(help.contains("serve-http"));
    assert!(!help.contains("serve-sse"));
}

#[test]
fn cache_path_command_parses() {
    Cli::try_parse_from(["biomcp", "cache", "path"]).expect("cache path should parse");
}

#[test]
fn cache_stats_command_parses() {
    Cli::try_parse_from(["biomcp", "cache", "stats"]).expect("cache stats should parse");
}

#[test]
fn cache_clean_command_parses_with_flags() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "cache",
        "clean",
        "--max-age",
        "30d",
        "--max-size",
        "500M",
        "--dry-run",
    ])
    .expect("cache clean should parse");

    let Cli {
        command:
            Commands::Cache {
                cmd:
                    crate::cli::cache::CacheCommand::Clean {
                        max_age,
                        max_size,
                        dry_run,
                    },
            },
        ..
    } = cli
    else {
        panic!("expected cache clean command");
    };

    assert_eq!(
        max_age,
        Some(std::time::Duration::from_secs(30 * 24 * 60 * 60))
    );
    assert_eq!(max_size, Some(500_000_000));
    assert!(dry_run);
}

#[test]
fn cache_clear_command_parses() {
    Cli::try_parse_from(["biomcp", "cache", "clear"]).expect("cache clear should parse");
}

#[test]
fn cache_clear_command_parses_with_yes_flag() {
    Cli::try_parse_from(["biomcp", "cache", "clear", "--yes"])
        .expect("cache clear --yes should parse");
}

#[test]
fn top_level_help_lists_cache_command() {
    let mut command = super::build_cli();
    let mut help = Vec::new();
    command
        .write_long_help(&mut help)
        .expect("top-level help should render");
    let help = String::from_utf8(help).expect("help should be utf-8");

    assert!(
        help.lines()
            .any(|line| line.trim_start().starts_with("cache")),
        "top-level help should list the cache family: {help}"
    );
}

#[test]
fn top_level_help_mentions_cache_path_json_exception() {
    let mut command = super::build_cli();
    let mut help = Vec::new();
    command
        .write_long_help(&mut help)
        .expect("top-level help should render");
    let help = String::from_utf8(help).expect("help should be utf-8");

    assert!(help.contains("except biomcp cache path"));
    assert!(help.contains("stays plain text"));
}

#[test]
fn cache_path_help_mentions_plain_text_and_ignored_json() {
    let help = render_cache_path_long_help();

    assert!(help.contains("plain text"));
    assert!(help.contains("--json"));
    assert!(help.contains("ignored"));
}

#[test]
fn cache_stats_help_mentions_json_and_cli_only() {
    let help = render_cache_stats_long_help();

    assert!(help.contains("cache statistics"));
    assert!(help.contains("--json"));
    assert!(help.contains("CLI-only"));
    assert!(help.contains("local filesystem paths"));
}

#[test]
fn cache_clean_help_mentions_dry_run_json_and_limits() {
    let help = render_cache_clean_long_help();

    assert!(help.contains("--max-age"));
    assert!(help.contains("--max-size"));
    assert!(help.contains("--dry-run"));
    assert!(help.contains("--json"));
    assert!(help.contains("orphan"));
}

#[test]
fn cache_clear_help_mentions_yes_tty_and_destructive_scope() {
    let help = render_cache_clear_long_help();

    assert!(help.contains("--yes"));
    assert!(help.contains("TTY"));
    assert!(help.contains("downloads"));
    assert!(help.contains("destructive"));
}

#[test]
fn cache_help_lists_clear_subcommand() {
    let help = render_cache_long_help();

    assert!(help.contains("clear"));
}

#[test]
fn top_level_help_describes_cache_family_not_path_only() {
    let mut command = super::build_cli();
    let mut help = Vec::new();
    command
        .write_long_help(&mut help)
        .expect("top-level help should render");
    let help = String::from_utf8(help).expect("help should be utf-8");

    assert!(help.contains(
        "Inspect the managed HTTP cache (CLI-only; cache commands reveal workstation-local filesystem paths)"
    ));
    assert!(
        !help
            .contains("Print the managed HTTP cache path (CLI-only; plain text; ignores `--json`)")
    );
}

fn render_cache_path_long_help() -> String {
    let mut command = Cli::command();
    let cache = command
        .find_subcommand_mut("cache")
        .expect("cache subcommand should exist");
    let path = cache
        .find_subcommand_mut("path")
        .expect("cache path subcommand should exist");
    let mut help = Vec::new();
    path.write_long_help(&mut help)
        .expect("cache path help should render");
    String::from_utf8(help).expect("help should be utf-8")
}

fn render_cache_long_help() -> String {
    let mut command = Cli::command();
    let cache = command
        .find_subcommand_mut("cache")
        .expect("cache subcommand should exist");
    let mut help = Vec::new();
    cache
        .write_long_help(&mut help)
        .expect("cache help should render");
    String::from_utf8(help).expect("help should be utf-8")
}

fn render_cache_stats_long_help() -> String {
    let mut command = Cli::command();
    let cache = command
        .find_subcommand_mut("cache")
        .expect("cache subcommand should exist");
    let stats = cache
        .find_subcommand_mut("stats")
        .expect("cache stats subcommand should exist");
    let mut help = Vec::new();
    stats
        .write_long_help(&mut help)
        .expect("cache stats help should render");
    String::from_utf8(help).expect("help should be utf-8")
}

fn render_cache_clean_long_help() -> String {
    let mut command = Cli::command();
    let cache = command
        .find_subcommand_mut("cache")
        .expect("cache subcommand should exist");
    let clean = cache
        .find_subcommand_mut("clean")
        .expect("cache clean subcommand should exist");
    let mut help = Vec::new();
    clean
        .write_long_help(&mut help)
        .expect("cache clean help should render");
    String::from_utf8(help).expect("help should be utf-8")
}

fn render_cache_clear_long_help() -> String {
    let mut command = Cli::command();
    let cache = command
        .find_subcommand_mut("cache")
        .expect("cache subcommand should exist");
    let clear = cache
        .find_subcommand_mut("clear")
        .expect("cache clear subcommand should exist");
    let mut help = Vec::new();
    clear
        .write_long_help(&mut help)
        .expect("cache clear help should render");
    String::from_utf8(help).expect("help should be utf-8")
}

fn parse_built_cli<I, T>(args: I) -> Cli
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let matches = super::build_cli()
        .try_get_matches_from(args)
        .expect("args should parse with canonical CLI");
    Cli::from_arg_matches(&matches).expect("matches should decode into Cli")
}

#[test]
fn article_search_json_includes_query_and_ranking_context() {
    let pagination = PaginationMeta::offset(0, 3, 1, Some(1));
    let mut filters = super::related_article_filters();
    filters.gene = Some("BRAF".into());
    let query = super::article_query_summary(
        &filters,
        crate::entities::article::ArticleSourceFilter::All,
        false,
        3,
        0,
    );
    let json = article_search_json(
        &query,
        &filters,
        true,
        Some(
            "Note: --type restricts article search to Europe PMC and PubMed. PubTator3, LitSense2, and Semantic Scholar do not support publication-type filtering.".into(),
        ),
        None,
        vec![crate::entities::article::ArticleSearchResult {
            pmid: "22663011".into(),
            pmcid: Some("PMC9984800".into()),
            doi: Some("10.1056/NEJMoa1203421".into()),
            title: "BRAF melanoma review".into(),
            journal: Some("Journal".into()),
            date: Some("2025-01-01".into()),
            citation_count: Some(12),
            influential_citation_count: Some(4),
            source: crate::entities::article::ArticleSource::EuropePmc,
            matched_sources: vec![
                crate::entities::article::ArticleSource::EuropePmc,
                crate::entities::article::ArticleSource::SemanticScholar,
            ],
            score: None,
            is_retracted: Some(false),
            abstract_snippet: Some("Abstract".into()),
            ranking: Some(crate::entities::article::ArticleRankingMetadata {
                directness_tier: 3,
                anchor_count: 2,
                title_anchor_hits: 2,
                abstract_anchor_hits: 0,
                combined_anchor_hits: 2,
                all_anchors_in_title: true,
                all_anchors_in_text: true,
                study_or_review_cue: true,
                pubmed_rescue: false,
                pubmed_rescue_kind: None,
                pubmed_source_position: None,
                mode: Some(crate::entities::article::ArticleRankingMode::Lexical),
                semantic_score: None,
                lexical_score: None,
                citation_score: None,
                position_score: None,
                composite_score: None,
                avg_source_rank: None,
            }),
            normalized_title: "braf melanoma review".into(),
            normalized_abstract: "abstract".into(),
            publication_type: Some("Review".into()),
            source_local_position: 0,
        }],
        pagination,
    )
    .expect("article search json should render");

    let value: serde_json::Value =
        serde_json::from_str(&json).expect("json should parse successfully");
    assert_eq!(value["query"], query);
    assert_eq!(value["sort"], "relevance");
    assert_eq!(value["semantic_scholar_enabled"], true);
    assert_eq!(
        value["ranking_policy"],
        crate::entities::article::ARTICLE_RELEVANCE_RANKING_POLICY
    );
    assert_eq!(
        value["note"],
        "Note: --type restricts article search to Europe PMC and PubMed. PubTator3, LitSense2, and Semantic Scholar do not support publication-type filtering."
    );
    assert_eq!(value["results"][0]["ranking"]["directness_tier"], 3);
    assert_eq!(value["results"][0]["ranking"]["pubmed_rescue"], false);
    assert!(value["results"][0]["ranking"]["pubmed_rescue_kind"].is_null());
    assert!(value["results"][0]["ranking"]["pubmed_source_position"].is_null());
    assert_eq!(
        value["results"][0]["matched_sources"][1],
        serde_json::Value::String("semanticscholar".into())
    );
}

#[test]
fn disease_search_json_includes_fallback_meta_and_provenance() {
    let pagination = PaginationMeta::offset(0, 10, 1, Some(1));
    let json = disease_search_json(
        vec![crate::entities::disease::DiseaseSearchResult {
            id: "MONDO:0000115".into(),
            name: "Arnold-Chiari malformation".into(),
            synonyms_preview: Some("Chiari malformation".into()),
            resolved_via: Some("MESH crosswalk".into()),
            source_id: Some("MESH:D001139".into()),
        }],
        pagination,
        true,
    )
    .expect("disease search json should render");

    let value: serde_json::Value =
        serde_json::from_str(&json).expect("json should parse successfully");
    assert_eq!(value["results"][0]["resolved_via"], "MESH crosswalk");
    assert_eq!(value["results"][0]["source_id"], "MESH:D001139");
    assert_eq!(value["_meta"]["fallback_used"], true);
}

#[test]
fn disease_search_json_omits_meta_for_direct_hits() {
    let pagination = PaginationMeta::offset(0, 10, 1, Some(1));
    let json = disease_search_json(
        vec![crate::entities::disease::DiseaseSearchResult {
            id: "MONDO:0005105".into(),
            name: "melanoma".into(),
            synonyms_preview: Some("malignant melanoma".into()),
            resolved_via: None,
            source_id: None,
        }],
        pagination,
        false,
    )
    .expect("disease search json should render");

    let value: serde_json::Value =
        serde_json::from_str(&json).expect("json should parse successfully");
    assert!(value.get("_meta").is_none());
    assert!(value["results"][0].get("resolved_via").is_none());
    assert!(value["results"][0].get("source_id").is_none());
}

#[test]
fn build_article_debug_plan_includes_article_type_limitation_note() {
    let filters = crate::entities::article::ArticleSearchFilters {
        gene: Some("BRAF".into()),
        gene_anchored: false,
        disease: None,
        drug: None,
        author: None,
        keyword: None,
        date_from: None,
        date_to: None,
        article_type: Some("review".into()),
        journal: None,
        open_access: false,
        no_preprints: false,
        exclude_retracted: false,
        max_per_source: None,
        sort: crate::entities::article::ArticleSort::Relevance,
        ranking: crate::entities::article::ArticleRankingOptions::default(),
    };
    let pagination = PaginationMeta::offset(0, 3, 0, Some(0));

    let plan = build_article_debug_plan(
        "gene=BRAF, type=review",
        &filters,
        crate::entities::article::ArticleSourceFilter::All,
        3,
        &[],
        &pagination,
    )
    .expect("debug plan should build");

    assert_eq!(plan.legs.len(), 1);
    assert!(
        plan.legs[0]
            .note
            .as_deref()
            .is_some_and(|value: &str| value.contains("Europe PMC and PubMed"))
    );
}

#[test]
fn pathway_trial_fallback_allows_no_match_on_first_page() {
    assert!(should_try_pathway_trial_fallback(0, 0, Some(0)));
    assert!(should_try_pathway_trial_fallback(0, 0, None));
}

#[test]
fn pathway_trial_fallback_skips_offset_or_known_matches() {
    assert!(!should_try_pathway_trial_fallback(0, 5, Some(2)));
    assert!(!should_try_pathway_trial_fallback(0, 0, Some(7)));
    assert!(!should_try_pathway_trial_fallback(1, 0, Some(1)));
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
fn search_drug_region_defaults_to_all_for_name_only_queries() {
    let filters = DrugSearchFilters {
        query: Some("Keytruda".into()),
        ..Default::default()
    };

    let region = resolve_drug_search_region(None, &filters).expect("name-only default");
    assert_eq!(region, DrugRegion::All);
}

#[test]
fn search_drug_region_defaults_to_us_for_structured_queries() {
    let filters = DrugSearchFilters {
        target: Some("EGFR".into()),
        ..Default::default()
    };

    let region = resolve_drug_search_region(None, &filters).expect("structured default");
    assert_eq!(region, DrugRegion::Us);
}

#[test]
fn search_drug_region_rejects_explicit_non_us_for_structured_queries() {
    let filters = DrugSearchFilters {
        target: Some("EGFR".into()),
        ..Default::default()
    };

    let err = resolve_drug_search_region(Some(super::DrugRegionArg::Eu), &filters)
        .expect_err("explicit eu should be rejected");
    assert!(format!("{err}").contains(
        "EMA and all-region search currently support name/alias lookups only; use --region us for structured MyChem filters or --region who to filter structured U.S. hits through WHO prequalification."
    ));

    let err = resolve_drug_search_region(Some(super::DrugRegionArg::All), &filters)
        .expect_err("explicit all should be rejected");
    assert!(format!("{err}").contains(
        "EMA and all-region search currently support name/alias lookups only; use --region us for structured MyChem filters or --region who to filter structured U.S. hits through WHO prequalification."
    ));
}

#[test]
fn search_drug_region_allows_explicit_who_for_structured_queries() {
    let filters = DrugSearchFilters {
        indication: Some("malaria".into()),
        ..Default::default()
    };

    let region =
        resolve_drug_search_region(Some(super::DrugRegionArg::Who), &filters).expect("who");
    assert_eq!(region, DrugRegion::Who);
}

#[test]
fn search_json_preserves_who_search_fields() {
    let pagination = PaginationMeta::offset(0, 5, 1, Some(1));
    let json = search_json(
        vec![crate::entities::drug::WhoPrequalificationSearchResult {
            inn: "Trastuzumab".to_string(),
            therapeutic_area: "Oncology".to_string(),
            dosage_form: "Powder for concentrate for solution for infusion".to_string(),
            applicant: "Samsung Bioepis NL B.V.".to_string(),
            who_reference_number: "BT-ON001".to_string(),
            listing_basis: "Prequalification - Abridged".to_string(),
            prequalification_date: Some("2019-12-18".to_string()),
        }],
        pagination,
    )
    .expect("WHO search json");

    let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    assert_eq!(value["count"], 1);
    assert_eq!(value["results"][0]["who_reference_number"], "BT-ON001");
    assert_eq!(
        value["results"][0]["listing_basis"],
        "Prequalification - Abridged"
    );
    assert_eq!(value["results"][0]["prequalification_date"], "2019-12-18");
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
fn drug_all_region_search_json_includes_who_bucket() {
    let json = drug_all_region_search_json(
        "trastuzumab",
        crate::entities::SearchPage::offset(
            vec![crate::entities::drug::DrugSearchResult {
                name: "trastuzumab".to_string(),
                drugbank_id: None,
                drug_type: None,
                mechanism: None,
                target: Some("ERBB2".to_string()),
            }],
            Some(1),
        ),
        crate::entities::SearchPage::offset(
            vec![crate::entities::drug::EmaDrugSearchResult {
                name: "Herzuma".to_string(),
                active_substance: "trastuzumab".to_string(),
                ema_product_number: "EMEA/H/C/004123".to_string(),
                status: "Authorised".to_string(),
            }],
            Some(1),
        ),
        crate::entities::SearchPage::offset(
            vec![crate::entities::drug::WhoPrequalificationSearchResult {
                inn: "Trastuzumab".to_string(),
                therapeutic_area: "Oncology".to_string(),
                dosage_form: "Powder for concentrate for solution for infusion".to_string(),
                applicant: "Samsung Bioepis NL B.V.".to_string(),
                who_reference_number: "BT-ON001".to_string(),
                listing_basis: "Prequalification - Abridged".to_string(),
                prequalification_date: Some("2019-12-18".to_string()),
            }],
            Some(1),
        ),
    )
    .expect("all-region drug search json");

    let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    assert_eq!(value["region"], "all");
    assert_eq!(value["who"]["count"], 1);
    assert_eq!(value["who"]["total"], 1);
    assert_eq!(
        value["who"]["results"][0]["who_reference_number"],
        "BT-ON001"
    );
    assert_eq!(
        value["eu"]["results"][0]["ema_product_number"],
        "EMEA/H/C/004123"
    );
}

#[test]
fn related_article_filters_default_to_relevance_and_safety_flags() {
    let filters = super::related_article_filters();

    assert_eq!(
        filters.sort,
        crate::entities::article::ArticleSort::Relevance
    );
    assert!(!filters.open_access);
    assert!(filters.no_preprints);
    assert!(filters.exclude_retracted);
    assert_eq!(filters.max_per_source, None);
}

#[test]
fn article_query_and_debug_filters_include_effective_ranking_context() {
    let mut filters = super::related_article_filters();
    filters.keyword = Some("melanoma".into());
    filters.max_per_source = Some(10);

    let summary = super::article_query_summary(
        &filters,
        crate::entities::article::ArticleSourceFilter::All,
        false,
        25,
        0,
    );
    assert!(summary.contains("ranking_mode=hybrid"));
    assert!(summary.contains("max_per_source=10"));
    assert!(summary.contains(
        "ranking_policy=hybrid relevance (score = 0.4*semantic + 0.3*lexical + 0.2*citations + 0.1*position)"
    ));

    let debug_filters = super::article_debug_filters(
        &filters,
        crate::entities::article::ArticleSourceFilter::All,
        25,
    );
    assert!(
        debug_filters
            .iter()
            .any(|entry| entry == "ranking_mode=hybrid")
    );
    assert!(
        debug_filters
            .iter()
            .any(|entry| entry == "max_per_source=10")
    );
    assert!(debug_filters.iter().any(|entry| {
        entry
            == "ranking_policy=hybrid relevance (score = 0.4*semantic + 0.3*lexical + 0.2*citations + 0.1*position)"
    }));
}

#[test]
fn article_query_and_debug_filters_render_default_and_disabled_max_per_source_modes() {
    let mut filters = super::related_article_filters();
    filters.gene = Some("BRAF".into());
    filters.max_per_source = Some(0);

    let summary = super::article_query_summary(
        &filters,
        crate::entities::article::ArticleSourceFilter::All,
        false,
        25,
        0,
    );
    assert!(summary.contains("max_per_source=default"));

    let debug_filters = super::article_debug_filters(
        &filters,
        crate::entities::article::ArticleSourceFilter::All,
        25,
    );
    assert!(
        debug_filters
            .iter()
            .any(|entry| entry == "max_per_source=default")
    );

    filters.max_per_source = Some(25);
    let disabled_summary = super::article_query_summary(
        &filters,
        crate::entities::article::ArticleSourceFilter::All,
        false,
        25,
        0,
    );
    assert!(disabled_summary.contains("max_per_source=disabled"));

    let disabled_debug_filters = super::article_debug_filters(
        &filters,
        crate::entities::article::ArticleSourceFilter::All,
        25,
    );
    assert!(
        disabled_debug_filters
            .iter()
            .any(|entry| entry == "max_per_source=disabled")
    );
}

#[test]
fn chart_args_default_to_no_chart() {
    let args = ChartArgs {
        chart: None,
        terminal: false,
        output: None,
        title: None,
        theme: None,
        palette: None,
        cols: None,
        rows: None,
        width: None,
        height: None,
        scale: None,
        mcp_inline: false,
    };
    assert_eq!(args.chart, None);
    assert!(!args.terminal);
    assert!(!args.mcp_inline);
    assert_eq!(args.cols, None);
    assert_eq!(args.rows, None);
    assert_eq!(args.width, None);
    assert_eq!(args.height, None);
    assert_eq!(args.scale, None);
}

#[test]
fn chart_dimension_flags_validate_positive_values() {
    let cols_err = Cli::try_parse_from([
        "biomcp",
        "study",
        "query",
        "--study",
        "msk_impact_2017",
        "--gene",
        "TP53",
        "--type",
        "mutations",
        "--chart",
        "bar",
        "--cols",
        "0",
    ])
    .expect_err("zero columns should fail");
    assert!(cols_err.to_string().contains("--cols must be >= 1"));

    let scale_err = Cli::try_parse_from([
        "biomcp",
        "study",
        "query",
        "--study",
        "msk_impact_2017",
        "--gene",
        "TP53",
        "--type",
        "mutations",
        "--chart",
        "bar",
        "--scale",
        "0",
    ])
    .expect_err("zero scale should fail");
    assert!(scale_err.to_string().contains("--scale must be > 0"));

    let nan_err = Cli::try_parse_from([
        "biomcp",
        "study",
        "query",
        "--study",
        "msk_impact_2017",
        "--gene",
        "TP53",
        "--type",
        "mutations",
        "--chart",
        "bar",
        "--scale",
        "NaN",
        "-o",
        "chart.png",
    ])
    .expect_err("non-finite scale should fail");
    assert!(
        nan_err
            .to_string()
            .contains("--scale must be a finite number > 0")
    );
}

#[test]
fn rewrite_mcp_chart_args_preserves_svg_sizing_flags() {
    let args = vec![
        "biomcp".to_string(),
        "study".to_string(),
        "query".to_string(),
        "--study".to_string(),
        "demo".to_string(),
        "--gene".to_string(),
        "TP53".to_string(),
        "--type".to_string(),
        "mutations".to_string(),
        "--chart".to_string(),
        "bar".to_string(),
        "--width".to_string(),
        "1200".to_string(),
        "--height".to_string(),
        "600".to_string(),
        "--title".to_string(),
        "Example".to_string(),
    ];

    let text = rewrite_mcp_chart_args(&args, McpChartPass::Text).expect("text rewrite");
    assert!(!text.iter().any(|value| value == "--chart"));
    assert!(!text.iter().any(|value| value == "--width"));
    assert!(!text.iter().any(|value| value == "--height"));

    let svg = rewrite_mcp_chart_args(&args, McpChartPass::Svg).expect("svg rewrite");
    assert!(svg.iter().any(|value| value == "--chart"));
    assert!(svg.iter().any(|value| value == "--width"));
    assert!(svg.iter().any(|value| value == "--height"));
    assert!(svg.iter().any(|value| value == "--mcp-inline"));
}

#[test]
fn rewrite_mcp_chart_args_rejects_terminal_and_png_only_flags() {
    let cols_err = rewrite_mcp_chart_args(
        &[
            "biomcp".to_string(),
            "study".to_string(),
            "query".to_string(),
            "--study".to_string(),
            "demo".to_string(),
            "--gene".to_string(),
            "TP53".to_string(),
            "--type".to_string(),
            "mutations".to_string(),
            "--chart".to_string(),
            "bar".to_string(),
            "--cols".to_string(),
            "80".to_string(),
        ],
        McpChartPass::Svg,
    )
    .expect_err("mcp svg rewrite should reject terminal sizing");
    assert!(
        cols_err
            .to_string()
            .contains("--cols/--rows require terminal chart output"),
        "{cols_err}"
    );

    let scale_err = rewrite_mcp_chart_args(
        &[
            "biomcp".to_string(),
            "study".to_string(),
            "query".to_string(),
            "--study".to_string(),
            "demo".to_string(),
            "--gene".to_string(),
            "TP53".to_string(),
            "--type".to_string(),
            "mutations".to_string(),
            "--chart".to_string(),
            "bar".to_string(),
            "--scale".to_string(),
            "2.0".to_string(),
        ],
        McpChartPass::Svg,
    )
    .expect_err("mcp svg rewrite should reject png scale");
    assert!(
        scale_err
            .to_string()
            .contains("--scale requires PNG chart output"),
        "{scale_err}"
    );
}

#[test]
fn study_survival_parses_endpoint_flag() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "study",
        "survival",
        "--study",
        "brca_tcga_pan_can_atlas_2018",
        "--gene",
        "TP53",
        "--endpoint",
        "dfs",
    ])
    .expect("study survival should parse");
    match cli.command {
        Commands::Study {
            cmd:
                StudyCommand::Survival {
                    study,
                    gene,
                    endpoint,
                    ..
                },
        } => {
            assert_eq!(study, "brca_tcga_pan_can_atlas_2018");
            assert_eq!(gene, "TP53");
            assert_eq!(endpoint, "dfs");
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn study_compare_parses_type_and_target() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "study",
        "compare",
        "--study",
        "brca_tcga_pan_can_atlas_2018",
        "--gene",
        "TP53",
        "--type",
        "expression",
        "--target",
        "ERBB2",
    ])
    .expect("study compare should parse");
    match cli.command {
        Commands::Study {
            cmd:
                StudyCommand::Compare {
                    study,
                    gene,
                    compare_type,
                    target,
                    ..
                },
        } => {
            assert_eq!(study, "brca_tcga_pan_can_atlas_2018");
            assert_eq!(gene, "TP53");
            assert_eq!(compare_type, "expression");
            assert_eq!(target, "ERBB2");
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn study_filter_parses_all_flags_and_repeated_values() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "study",
        "filter",
        "--study",
        "brca_tcga_pan_can_atlas_2018",
        "--mutated",
        "TP53",
        "--mutated",
        "PIK3CA",
        "--amplified",
        "ERBB2",
        "--deleted",
        "PTEN",
        "--expression-above",
        "MYC:1.5",
        "--expression-above",
        "ERBB2:-0.5",
        "--expression-below",
        "ESR1:0.5",
        "--cancer-type",
        "Breast Cancer",
        "--cancer-type",
        "Lung Cancer",
    ])
    .expect("study filter should parse");
    match cli.command {
        Commands::Study {
            cmd:
                StudyCommand::Filter {
                    study,
                    mutated,
                    amplified,
                    deleted,
                    expression_above,
                    expression_below,
                    cancer_type,
                },
        } => {
            assert_eq!(study, "brca_tcga_pan_can_atlas_2018");
            assert_eq!(mutated, vec!["TP53", "PIK3CA"]);
            assert_eq!(amplified, vec!["ERBB2"]);
            assert_eq!(deleted, vec!["PTEN"]);
            assert_eq!(expression_above, vec!["MYC:1.5", "ERBB2:-0.5"]);
            assert_eq!(expression_below, vec!["ESR1:0.5"]);
            assert_eq!(cancer_type, vec!["Breast Cancer", "Lung Cancer"]);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn study_co_occurrence_parses_gene_list() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "study",
        "co-occurrence",
        "--study",
        "brca_tcga_pan_can_atlas_2018",
        "--genes",
        "TP53,PIK3CA,GATA3",
    ])
    .expect("study co-occurrence should parse");
    match cli.command {
        Commands::Study {
            cmd: StudyCommand::CoOccurrence { study, genes, .. },
        } => {
            assert_eq!(study, "brca_tcga_pan_can_atlas_2018");
            assert_eq!(genes, "TP53,PIK3CA,GATA3");
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn truncate_article_annotations_applies_limit_per_bucket() {
    let annotations = crate::entities::article::ArticleAnnotations {
        genes: vec![
            crate::entities::article::AnnotationCount {
                text: "BRAF".into(),
                count: 2,
            },
            crate::entities::article::AnnotationCount {
                text: "TP53".into(),
                count: 1,
            },
        ],
        diseases: vec![
            crate::entities::article::AnnotationCount {
                text: "melanoma".into(),
                count: 2,
            },
            crate::entities::article::AnnotationCount {
                text: "glioma".into(),
                count: 1,
            },
        ],
        chemicals: vec![
            crate::entities::article::AnnotationCount {
                text: "vemurafenib".into(),
                count: 1,
            },
            crate::entities::article::AnnotationCount {
                text: "dabrafenib".into(),
                count: 1,
            },
        ],
        mutations: vec![
            crate::entities::article::AnnotationCount {
                text: "V600E".into(),
                count: 1,
            },
            crate::entities::article::AnnotationCount {
                text: "L858R".into(),
                count: 1,
            },
        ],
    };
    let truncated = truncate_article_annotations(annotations, 1);
    assert_eq!(truncated.genes.len(), 1);
    assert_eq!(truncated.diseases.len(), 1);
    assert_eq!(truncated.chemicals.len(), 1);
    assert_eq!(truncated.mutations.len(), 1);
}

#[tokio::test]
async fn enrich_rejects_zero_limit_before_api_call() {
    let err = execute(vec![
        "biomcp".to_string(),
        "enrich".to_string(),
        "BRCA1,TP53".to_string(),
        "--limit".to_string(),
        "0".to_string(),
    ])
    .await
    .expect_err("enrich should reject --limit 0");
    assert!(err.to_string().contains("--limit must be between 1 and 50"));
}

#[tokio::test]
async fn enrich_rejects_limit_above_max_before_api_call() {
    let err = execute(vec![
        "biomcp".to_string(),
        "enrich".to_string(),
        "BRCA1,TP53".to_string(),
        "--limit".to_string(),
        "51".to_string(),
    ])
    .await
    .expect_err("enrich should reject --limit > 50");
    assert!(err.to_string().contains("--limit must be between 1 and 50"));
}

#[tokio::test]
async fn search_adverse_event_device_rejects_positional_drug_alias() {
    let err = execute(vec![
        "biomcp".to_string(),
        "search".to_string(),
        "adverse-event".to_string(),
        "pembrolizumab".to_string(),
        "--type".to_string(),
        "device".to_string(),
    ])
    .await
    .expect_err("device query should reject positional drug alias");
    assert!(
        err.to_string()
            .contains("--drug cannot be used with --type device")
    );
}

#[tokio::test]
async fn search_all_requires_at_least_one_typed_slot() {
    let err = execute(vec![
        "biomcp".to_string(),
        "search".to_string(),
        "all".to_string(),
    ])
    .await
    .expect_err("search all should require typed slots");
    assert!(err.to_string().contains("at least one typed slot"));
    assert!(err.to_string().contains("--gene"));
}

#[tokio::test]
async fn search_pathway_requires_query_unless_top_level() {
    let err = execute(vec![
        "biomcp".to_string(),
        "search".to_string(),
        "pathway".to_string(),
    ])
    .await
    .expect_err("search pathway should require query unless --top-level");
    assert!(
        err.to_string()
            .contains("Query is required. Example: biomcp search pathway -q \"MAPK signaling\"")
    );
}

#[tokio::test]
async fn study_co_occurrence_requires_2_to_10_genes() {
    let err = execute(vec![
        "biomcp".to_string(),
        "study".to_string(),
        "co-occurrence".to_string(),
        "--study".to_string(),
        "msk_impact_2017".to_string(),
        "--genes".to_string(),
        "TP53".to_string(),
    ])
    .await
    .expect_err("study co-occurrence should validate gene count");
    assert!(err.to_string().contains("--genes must contain 2 to 10"));
}

#[tokio::test]
async fn study_filter_requires_at_least_one_criterion() {
    let err = execute(vec![
        "biomcp".to_string(),
        "study".to_string(),
        "filter".to_string(),
        "--study".to_string(),
        "brca_tcga_pan_can_atlas_2018".to_string(),
    ])
    .await
    .expect_err("study filter should require criteria");
    assert!(
        err.to_string()
            .contains("At least one filter criterion is required")
    );
}

#[tokio::test]
async fn study_filter_rejects_malformed_expression_threshold() {
    let err = execute(vec![
        "biomcp".to_string(),
        "study".to_string(),
        "filter".to_string(),
        "--study".to_string(),
        "brca_tcga_pan_can_atlas_2018".to_string(),
        "--expression-above".to_string(),
        "MYC:not-a-number".to_string(),
    ])
    .await
    .expect_err("study filter should validate threshold format");
    assert!(err.to_string().contains("--expression-above"));
    assert!(err.to_string().contains("GENE:THRESHOLD"));
}

#[tokio::test]
async fn study_survival_rejects_unknown_endpoint() {
    let err = execute(vec![
        "biomcp".to_string(),
        "study".to_string(),
        "survival".to_string(),
        "--study".to_string(),
        "msk_impact_2017".to_string(),
        "--gene".to_string(),
        "TP53".to_string(),
        "--endpoint".to_string(),
        "foo".to_string(),
    ])
    .await
    .expect_err("study survival should validate endpoint");
    assert!(err.to_string().contains("Unknown survival endpoint"));
}

#[tokio::test]
async fn study_compare_rejects_unknown_type() {
    let err = execute(vec![
        "biomcp".to_string(),
        "study".to_string(),
        "compare".to_string(),
        "--study".to_string(),
        "msk_impact_2017".to_string(),
        "--gene".to_string(),
        "TP53".to_string(),
        "--type".to_string(),
        "foo".to_string(),
        "--target".to_string(),
        "ERBB2".to_string(),
    ])
    .await
    .expect_err("study compare should validate type");
    assert!(err.to_string().contains("Unknown comparison type"));
}

#[tokio::test]
async fn study_co_occurrence_invalid_chart_lists_heatmap() {
    let err = execute(vec![
        "biomcp".to_string(),
        "study".to_string(),
        "co-occurrence".to_string(),
        "--study".to_string(),
        "msk_impact_2017".to_string(),
        "--genes".to_string(),
        "TP53,KRAS".to_string(),
        "--chart".to_string(),
        "violin".to_string(),
        "--terminal".to_string(),
    ])
    .await
    .expect_err("study co-occurrence should reject violin");
    let msg = err.to_string();
    assert!(msg.contains("study co-occurrence"));
    assert!(msg.contains("bar"));
    assert!(msg.contains("pie"));
    assert!(msg.contains("heatmap"));
}

#[tokio::test]
async fn study_query_mutations_invalid_chart_lists_waterfall() {
    let err = execute(vec![
        "biomcp".to_string(),
        "study".to_string(),
        "query".to_string(),
        "--study".to_string(),
        "msk_impact_2017".to_string(),
        "--gene".to_string(),
        "TP53".to_string(),
        "--type".to_string(),
        "mutations".to_string(),
        "--chart".to_string(),
        "violin".to_string(),
        "--terminal".to_string(),
    ])
    .await
    .expect_err("study query mutations should reject violin");
    let msg = err.to_string();
    assert!(msg.contains("study query --type mutations"));
    assert!(msg.contains("bar"));
    assert!(msg.contains("pie"));
    assert!(msg.contains("waterfall"));
}

#[tokio::test]
async fn study_compare_mutations_invalid_chart_lists_stacked_bar() {
    let err = execute(vec![
        "biomcp".to_string(),
        "study".to_string(),
        "compare".to_string(),
        "--study".to_string(),
        "msk_impact_2017".to_string(),
        "--gene".to_string(),
        "TP53".to_string(),
        "--type".to_string(),
        "mutations".to_string(),
        "--target".to_string(),
        "KRAS".to_string(),
        "--chart".to_string(),
        "violin".to_string(),
        "--terminal".to_string(),
    ])
    .await
    .expect_err("mutation compare should reject violin");
    let msg = err.to_string();
    assert!(msg.contains("study compare --type mutations"));
    assert!(msg.contains("bar"));
    assert!(msg.contains("stacked-bar"));
}

#[tokio::test]
async fn study_compare_expression_invalid_chart_lists_scatter() {
    let err = execute(vec![
        "biomcp".to_string(),
        "study".to_string(),
        "compare".to_string(),
        "--study".to_string(),
        "msk_impact_2017".to_string(),
        "--gene".to_string(),
        "TP53".to_string(),
        "--type".to_string(),
        "expression".to_string(),
        "--target".to_string(),
        "ERBB2".to_string(),
        "--chart".to_string(),
        "pie".to_string(),
        "--terminal".to_string(),
    ])
    .await
    .expect_err("expression compare should reject pie");
    let msg = err.to_string();
    assert!(msg.contains("study compare --type expression"));
    assert!(msg.contains("box"));
    assert!(msg.contains("violin"));
    assert!(msg.contains("ridgeline"));
    assert!(msg.contains("scatter"));
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

#[tokio::test]
async fn json_cache_path_still_returns_plain_text() {
    let _guard = lock_env().await;
    let root = TempDirGuard::new("cache-path-json");
    let cache_home = root.path().join("cache-home");
    let config_home = root.path().join("config-home");
    std::fs::create_dir_all(&cache_home).expect("create cache home");
    std::fs::create_dir_all(&config_home).expect("create config home");
    let _cache_home = set_env_var("XDG_CACHE_HOME", Some(&cache_home.to_string_lossy()));
    let _config_home = set_env_var("XDG_CONFIG_HOME", Some(&config_home.to_string_lossy()));
    let _cache_dir = set_env_var("BIOMCP_CACHE_DIR", None);
    let _cache_size = set_env_var("BIOMCP_CACHE_MAX_SIZE", None);

    let output = execute(vec![
        "biomcp".to_string(),
        "--json".to_string(),
        "cache".to_string(),
        "path".to_string(),
    ])
    .await
    .expect("cache path should execute");

    assert_eq!(
        output.trim(),
        cache_home.join("biomcp").join("http").display().to_string()
    );
    assert!(!output.trim_start().starts_with('{'));
}

#[tokio::test]
async fn cache_stats_execute_returns_markdown_table() {
    let _guard = lock_env().await;
    let root = TempDirGuard::new("cache-stats-text");
    let cache_home = root.path().join("cache-home");
    let config_home = root.path().join("config-home");
    std::fs::create_dir_all(&cache_home).expect("create cache home");
    std::fs::create_dir_all(&config_home).expect("create config home");
    let _cache_home = set_env_var("XDG_CACHE_HOME", Some(&cache_home.to_string_lossy()));
    let _config_home = set_env_var("XDG_CONFIG_HOME", Some(&config_home.to_string_lossy()));
    let _cache_dir = set_env_var("BIOMCP_CACHE_DIR", None);
    let _cache_size = set_env_var("BIOMCP_CACHE_MAX_SIZE", None);
    let _cache_age = set_env_var("BIOMCP_CACHE_MAX_AGE", None);

    let output = execute(vec![
        "biomcp".to_string(),
        "cache".to_string(),
        "stats".to_string(),
    ])
    .await
    .expect("cache stats should execute");

    for row in [
        "| Path |",
        "| Blob bytes |",
        "| Blob files |",
        "| Orphan blobs |",
        "| Age range |",
        "| Max size |",
        "| Max age |",
    ] {
        assert!(output.contains(row), "missing row {row}: {output}");
    }
    assert!(!output.trim_start().starts_with('{'));
}

#[tokio::test]
async fn cache_stats_execute_json_returns_structured_report() {
    let _guard = lock_env().await;
    let root = TempDirGuard::new("cache-stats-json");
    let cache_home = root.path().join("cache-home");
    let config_home = root.path().join("config-home");
    std::fs::create_dir_all(&cache_home).expect("create cache home");
    std::fs::create_dir_all(&config_home).expect("create config home");
    let _cache_home = set_env_var("XDG_CACHE_HOME", Some(&cache_home.to_string_lossy()));
    let _config_home = set_env_var("XDG_CONFIG_HOME", Some(&config_home.to_string_lossy()));
    let _cache_dir = set_env_var("BIOMCP_CACHE_DIR", None);
    let _cache_size = set_env_var("BIOMCP_CACHE_MAX_SIZE", None);
    let _cache_age = set_env_var("BIOMCP_CACHE_MAX_AGE", None);

    let output = execute(vec![
        "biomcp".to_string(),
        "--json".to_string(),
        "cache".to_string(),
        "stats".to_string(),
    ])
    .await
    .expect("cache stats json should execute");

    let value: serde_json::Value =
        serde_json::from_str(&output).expect("cache stats json should be valid");
    for key in [
        "path",
        "blob_bytes",
        "blob_count",
        "orphan_count",
        "age_range",
        "max_size_bytes",
        "max_size_origin",
        "max_age_secs",
        "max_age_origin",
    ] {
        assert!(value.get(key).is_some(), "missing key {key}: {value}");
    }
    assert!(!output.contains("| Path |"));
    assert!(!output.contains("| Blob bytes |"));
}

#[tokio::test]
async fn cache_clean_execute_returns_single_line_summary() {
    let _guard = lock_env().await;
    let root = TempDirGuard::new("cache-clean-text");
    let cache_home = root.path().join("cache-home");
    let config_home = root.path().join("config-home");
    std::fs::create_dir_all(&cache_home).expect("create cache home");
    std::fs::create_dir_all(&config_home).expect("create config home");
    let _cache_home = set_env_var("XDG_CACHE_HOME", Some(&cache_home.to_string_lossy()));
    let _config_home = set_env_var("XDG_CONFIG_HOME", Some(&config_home.to_string_lossy()));
    let _cache_dir = set_env_var("BIOMCP_CACHE_DIR", None);
    let _cache_size = set_env_var("BIOMCP_CACHE_MAX_SIZE", None);

    let output = execute(vec![
        "biomcp".to_string(),
        "cache".to_string(),
        "clean".to_string(),
    ])
    .await
    .expect("cache clean should execute");

    assert!(output.starts_with("Cache clean:"));
    assert!(output.contains("dry_run=false"));
    assert_eq!(output.lines().count(), 1);
}

#[tokio::test]
async fn cache_clean_execute_json_returns_structured_report() {
    let _guard = lock_env().await;
    let root = TempDirGuard::new("cache-clean-json");
    let cache_home = root.path().join("cache-home");
    let config_home = root.path().join("config-home");
    std::fs::create_dir_all(&cache_home).expect("create cache home");
    std::fs::create_dir_all(&config_home).expect("create config home");
    let _cache_home = set_env_var("XDG_CACHE_HOME", Some(&cache_home.to_string_lossy()));
    let _config_home = set_env_var("XDG_CONFIG_HOME", Some(&config_home.to_string_lossy()));
    let _cache_dir = set_env_var("BIOMCP_CACHE_DIR", None);
    let _cache_size = set_env_var("BIOMCP_CACHE_MAX_SIZE", None);

    let output = execute(vec![
        "biomcp".to_string(),
        "--json".to_string(),
        "cache".to_string(),
        "clean".to_string(),
    ])
    .await
    .expect("cache clean json should execute");

    let value: serde_json::Value =
        serde_json::from_str(&output).expect("cache clean json should be valid");
    for key in [
        "dry_run",
        "orphans_removed",
        "entries_removed",
        "bytes_freed",
        "errors",
    ] {
        assert!(value.get(key).is_some(), "missing key {key}: {value}");
    }
}
