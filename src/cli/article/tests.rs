use chrono::NaiveDate;
use clap::{CommandFactory, Parser};

use super::dispatch::{
    ArticleEntitySuggestion, ArticleSearchJsonPage, article_debug_filters,
    article_entity_suggestion, article_query_summary, article_search_json,
    build_article_debug_plan, is_exact_article_keyword_lookup_eligible,
    resolved_article_date_bounds, truncate_article_annotations,
};
use crate::cli::{Cli, Commands, GetEntity, PaginationMeta, SearchEntity};
use crate::entities::discover::{DiscoverType, ExactArticleKeywordEntity};
use crate::test_support::{env_lock, set_env_var};
use wiremock::matchers::{body_string_contains, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn render_article_search_long_help() -> String {
    let mut command = Cli::command();
    let search = command
        .find_subcommand_mut("search")
        .expect("search subcommand should exist");
    let article = search
        .find_subcommand_mut("article")
        .expect("article subcommand should exist");
    let mut help = Vec::new();
    article
        .write_long_help(&mut help)
        .expect("article help should render");
    String::from_utf8(help).expect("help should be utf-8")
}

fn render_article_get_long_help() -> String {
    let mut command = Cli::command();
    let get = command
        .find_subcommand_mut("get")
        .expect("get subcommand should exist");
    let article = get
        .find_subcommand_mut("article")
        .expect("article subcommand should exist");
    let mut help = Vec::new();
    article
        .write_long_help(&mut help)
        .expect("help should render");
    String::from_utf8(help).expect("help should be utf-8")
}

#[test]
fn search_article_help_includes_when_to_use_guidance() {
    let help = render_article_search_long_help();

    assert!(help.contains("When to use:"));
    assert!(help.contains("keyword search to scan a topic"));
    assert!(help.contains("Prefer --type review"));
}

#[test]
fn search_article_help_includes_query_formulation_guidance() {
    let help = render_article_search_long_help();

    assert!(help.contains("QUERY FORMULATION:"));
    assert!(help.contains(
        "Known gene/disease/drug anchors belong in `-g/--gene`, `-d/--disease`, or `--drug`."
    ));
    assert!(help.contains(
        "Use `-k/--keyword` for mechanisms, phenotypes, datasets, outcomes, and other free-text concepts."
    ));
    assert!(
        help.contains(
            "Unknown-entity questions should stay keyword-first or start with `discover`."
        )
    );
    assert!(help.contains(
        "Keyword-only result pages can suggest typed `get gene`, `get drug`, or `get disease` follow-ups when the whole `-k/--keyword` exactly matches a vocabulary label or alias."
    ));
    assert!(help.contains(
        "Multi-concept phrases and searches that already use `-g/--gene`, `-d/--disease`, or `--drug` do not get direct entity suggestions."
    ));
    assert!(help.contains(
        "Adding `-k/--keyword` on the default route brings in LitSense2 and default `hybrid` relevance."
    ));
    assert!(help.contains(
        "`semantic` sorts by the LitSense2-derived semantic signal and falls back to lexical ties."
    ));
    assert!(help.contains(
        "Hybrid score = `0.4*semantic + 0.3*lexical + 0.2*citations + 0.1*position` by default, using the same LitSense2-derived semantic signal and `semantic=0` when LitSense2 did not match."
    ));
    assert!(
        help.contains("biomcp search article -g TP53 -k \"apoptosis gene regulation\" --limit 5")
    );
    assert!(help.contains(
        "biomcp search article -k '\"cafe-au-lait spots\" neurofibromas disease' --type review --limit 5"
    ));
}

#[test]
fn article_date_help_advertises_shared_accepted_formats() {
    let help = render_article_search_long_help();

    assert!(help.contains("Published after date (YYYY, YYYY-MM, or YYYY-MM-DD)"));
    assert!(help.contains("Published before date (YYYY, YYYY-MM, or YYYY-MM-DD)"));
    assert!(help.contains("--year-min <YYYY>"));
    assert!(help.contains("--year-max <YYYY>"));
    assert!(help.contains("Published from year (YYYY)"));
    assert!(help.contains("Published through year (YYYY)"));
    assert!(help.contains("[aliases: --since]"));
    assert!(help.contains("[aliases: --until]"));
    assert!(help.contains("--max-per-source <N>"));
    assert!(help.contains(
        "Cap each federated source's contribution after deduplication and before ranking."
    ));
    assert!(help.contains(
        "Default: 40% of `--limit` on federated pools with at least three surviving primary sources."
    ));
    assert!(help.contains("`0` uses the default cap."));
    assert!(help.contains("Setting it equal to `--limit` disables capping."));
}

#[test]
fn get_article_help_includes_opt_in_pdf_guidance() {
    let help = render_article_get_long_help();

    assert!(help.contains("--pdf"));
    assert!(help.contains("Allow Semantic Scholar PDF as a final fulltext fallback"));
    assert!(help.contains("`--pdf` requires the fulltext section."));
    assert!(help.contains("biomcp get article 22663011 fulltext --pdf"));
}

#[test]
fn article_get_pdf_modifier_parses_before_fulltext() {
    let cli = Cli::try_parse_from(["biomcp", "get", "article", "22663011", "--pdf", "fulltext"])
        .expect("article get should accept --pdf before fulltext");

    let Cli {
        command: Commands::Get {
            entity: GetEntity::Article(args),
        },
        ..
    } = cli
    else {
        panic!("expected article get command");
    };

    assert_eq!(args.id, "22663011");
    assert_eq!(args.sections, vec!["fulltext"]);
}

#[test]
fn article_get_pdf_modifier_parses_after_fulltext() {
    let cli = Cli::try_parse_from(["biomcp", "get", "article", "22663011", "fulltext", "--pdf"])
        .expect("article get should accept --pdf after fulltext");

    let Cli {
        command: Commands::Get {
            entity: GetEntity::Article(args),
        },
        ..
    } = cli
    else {
        panic!("expected article get command");
    };

    assert_eq!(args.id, "22663011");
    assert_eq!(args.sections.first().map(String::as_str), Some("fulltext"));
}

#[test]
fn article_year_flags_parse_and_expand_to_date_bounds() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "search",
        "article",
        "-g",
        "BRAF",
        "--year-min",
        "2000",
        "--year-max",
        "2013",
        "--limit",
        "1",
    ])
    .expect("article year flags should parse");

    let Cli {
        command: Commands::Search {
            entity: SearchEntity::Article(args),
        },
        ..
    } = cli
    else {
        panic!("expected article search command");
    };

    assert_eq!(args.year_min, Some(2000));
    assert_eq!(args.year_max, Some(2013));
    let (date_from, date_to) = resolved_article_date_bounds(&args);
    assert_eq!(date_from.as_deref(), Some("2000-01-01"));
    assert_eq!(date_to.as_deref(), Some("2013-12-31"));
}

#[test]
fn article_year_flags_reject_non_yyyy_values() {
    let err = Cli::try_parse_from([
        "biomcp",
        "search",
        "article",
        "-g",
        "BRAF",
        "--year-min",
        "200",
    ])
    .expect_err("non-YYYY year should fail to parse");

    let message = err.to_string();
    assert!(message.contains("invalid value '200' for '--year-min <YYYY>'"));
    assert!(message.contains("expected YYYY"));
}

#[test]
fn article_year_flags_conflict_with_explicit_dates() {
    let err = Cli::try_parse_from([
        "biomcp",
        "search",
        "article",
        "-g",
        "BRAF",
        "--year-min",
        "2000",
        "--date-from",
        "2000-01-01",
    ])
    .expect_err("year-min and date-from should conflict");

    assert!(err.to_string().contains(
        "the argument '--year-min <YYYY>' cannot be used with '--date-from <DATE_FROM>'"
    ));
}

#[test]
fn article_year_max_conflicts_with_date_to() {
    let err = Cli::try_parse_from([
        "biomcp",
        "search",
        "article",
        "-g",
        "BRAF",
        "--year-max",
        "2013",
        "--date-to",
        "2013-12-31",
    ])
    .expect_err("year-max and date-to should conflict");

    assert!(
        err.to_string()
            .contains("the argument '--year-max <YYYY>' cannot be used with '--date-to <DATE_TO>'")
    );
}

#[test]
fn exact_article_keyword_lookup_eligibility_is_keyword_only_and_short() {
    let mut filters = super::super::related_article_filters();
    filters.keyword = Some("BRAF".into());
    assert!(is_exact_article_keyword_lookup_eligible(&filters));

    filters.keyword = Some("non-small cell lung cancer".into());
    assert!(!is_exact_article_keyword_lookup_eligible(&filters));

    filters.keyword = Some("BRAF".into());
    filters.gene = Some("BRAF".into());
    assert!(!is_exact_article_keyword_lookup_eligible(&filters));

    filters.gene = None;
    filters.disease = Some("melanoma".into());
    assert!(!is_exact_article_keyword_lookup_eligible(&filters));

    filters.disease = None;
    filters.drug = Some("imatinib".into());
    assert!(!is_exact_article_keyword_lookup_eligible(&filters));
}

#[test]
fn article_entity_suggestion_uses_alias_reason_and_valid_sections() {
    let suggestion = article_entity_suggestion(&ExactArticleKeywordEntity {
        entity_type: DiscoverType::Drug,
        label: "imatinib mesylate".into(),
        primary_id: Some("CHEBI:45783".into()),
        matched_query: "Gleevec".into(),
        matched_alias: true,
    });

    assert_eq!(suggestion.command, "biomcp get drug \"imatinib mesylate\"");
    assert_eq!(
        suggestion.reason,
        "Exact drug alias match for article keyword \"Gleevec\"; suggested canonical drug \"imatinib mesylate\"."
    );
    assert_eq!(suggestion.sections, vec!["label", "targets", "indications"]);
}

#[tokio::test]
async fn handle_command_rejects_zero_limit_before_backend_lookup() {
    let cli = Cli::try_parse_from(["biomcp", "article", "citations", "22663011", "--limit", "0"])
        .expect("article citations should parse");

    let Cli {
        command: Commands::Article { cmd },
        json,
        ..
    } = cli
    else {
        panic!("expected article command");
    };

    let err = super::handle_command(cmd, json)
        .await
        .expect_err("zero article citations limit should fail fast");
    assert!(
        err.to_string()
            .contains("--limit must be between 1 and 100")
    );
}

#[tokio::test]
async fn handle_search_json_fails_open_when_exact_entity_lookup_errors() {
    let _guard = env_lock().lock().await;
    let pubtator = MockServer::start().await;
    let semantic_scholar = MockServer::start().await;
    let ols4 = MockServer::start().await;
    let _pubtator_base = set_env_var("BIOMCP_PUBTATOR_BASE", Some(&pubtator.uri()));
    let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&semantic_scholar.uri()));
    let _s2_key = set_env_var("S2_API_KEY", None);
    let _ols4_base = set_env_var("BIOMCP_OLS4_BASE", Some(&ols4.uri()));

    Mock::given(method("GET"))
        .and(path("/search/"))
        .and(query_param("page", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "results": [{
                "_id": "pt-1",
                "pmid": 22663011,
                "title": "BRAF melanoma review",
                "journal": "Journal",
                "date": "2025-01-01",
                "score": 42.0
            }],
            "count": 1,
            "total_pages": 1,
            "current": 1,
            "page_size": 25,
            "facets": {}
        })))
        .expect(1)
        .mount(&pubtator)
        .await;

    Mock::given(method("POST"))
        .and(path("/graph/v1/paper/batch"))
        .and(query_param(
            "fields",
            "paperId,externalIds,citationCount,influentialCitationCount,abstract",
        ))
        .and(body_string_contains("\"PMID:22663011\""))
        .respond_with(ResponseTemplate::new(429).set_body_string("shared rate limit"))
        .expect(1)
        .mount(&semantic_scholar)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/search"))
        .and(query_param("q", "BRAF"))
        .respond_with(ResponseTemplate::new(500).set_body_string("ols unavailable"))
        .mount(&ols4)
        .await;

    let cli = Cli::try_parse_from([
        "biomcp", "--json", "search", "article", "-k", "BRAF", "--source", "pubtator", "--sort",
        "date", "--limit", "1",
    ])
    .expect("article search should parse");

    let Cli {
        command: Commands::Search {
            entity: SearchEntity::Article(args),
        },
        json,
        ..
    } = cli
    else {
        panic!("expected article search command");
    };

    let outcome = super::handle_search(args, json)
        .await
        .expect("article search should fail open on OLS4 errors");
    let value: serde_json::Value =
        serde_json::from_str(&outcome.text).expect("json should parse successfully");
    let ols_requests = ols4
        .received_requests()
        .await
        .expect("OLS4 mock should record requests");
    assert!(
        !ols_requests.is_empty(),
        "article keyword lookup should call the failing OLS4 mock"
    );
    assert_eq!(value["count"], 1);
    assert!(
        value
            .get("_meta")
            .and_then(|meta| meta.get("suggestions"))
            .is_none()
    );
    assert!(
        !value["_meta"]["next_commands"]
            .as_array()
            .expect("next commands should be present")
            .iter()
            .any(|command| command.as_str() == Some("biomcp get gene BRAF"))
    );
}

#[tokio::test]
async fn handle_search_json_typed_filter_skips_exact_lookup_and_suggestions() {
    let _guard = env_lock().lock().await;
    let europepmc = MockServer::start().await;
    let ols4 = MockServer::start().await;
    let _europepmc_base = set_env_var("BIOMCP_EUROPEPMC_BASE", Some(&europepmc.uri()));
    let _ols4_base = set_env_var("BIOMCP_OLS4_BASE", Some(&ols4.uri()));

    Mock::given(method("GET"))
        .and(path("/search"))
        .and(query_param("page", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "hitCount": 0,
            "resultList": {
                "result": []
            }
        })))
        .expect(1)
        .mount(&europepmc)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "response": {
                "docs": []
            }
        })))
        .expect(0)
        .mount(&ols4)
        .await;

    let cli = Cli::try_parse_from([
        "biomcp",
        "--json",
        "search",
        "article",
        "-k",
        "BRAF",
        "-g",
        "BRAF",
        "--source",
        "europepmc",
        "--sort",
        "date",
        "--limit",
        "1",
    ])
    .expect("article search should parse");

    let Cli {
        command: Commands::Search {
            entity: SearchEntity::Article(args),
        },
        json,
        ..
    } = cli
    else {
        panic!("expected article search command");
    };

    let outcome = super::handle_search(args, json)
        .await
        .expect("typed-filter article search should succeed");
    let value: serde_json::Value =
        serde_json::from_str(&outcome.text).expect("json should parse successfully");
    assert_eq!(value["count"], 0);
    assert!(
        value
            .get("_meta")
            .and_then(|meta| meta.get("suggestions"))
            .is_none()
    );
    assert!(!outcome.text.contains("biomcp get gene BRAF"));
}

#[test]
fn article_search_json_includes_query_and_ranking_context() {
    let pagination = PaginationMeta::offset(0, 3, 1, Some(1));
    let mut filters = super::super::related_article_filters();
    filters.gene = Some("BRAF".into());
    let query = article_query_summary(
        &filters,
        crate::entities::article::ArticleSourceFilter::All,
        false,
        3,
        0,
    );
    let results = vec![crate::entities::article::ArticleSearchResult {
        pmid: "22663011".into(),
        pmcid: Some("PMC9984800".into()),
        doi: Some("10.1056/NEJMoa1203421".into()),
        title: "BRAF melanoma review".into(),
        journal: Some("Journal".into()),
        date: Some("2025-01-01".into()),
        first_index_date: Some(NaiveDate::from_ymd_opt(2025, 1, 15).expect("valid date")),
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
    }];
    let next_commands = crate::render::markdown::search_next_commands_article(
        &results,
        &filters,
        crate::entities::article::ArticleSourceFilter::All,
        &[],
    );
    let related = crate::render::markdown::related_article_search_results(
        &results,
        &filters,
        crate::entities::article::ArticleSourceFilter::All,
        &[],
    );
    let json = article_search_json(
        &query,
        &filters,
        true,
        Some(
            "Note: --type restricts article search to Europe PMC and PubMed. PubTator3, LitSense2, and Semantic Scholar do not support publication-type filtering.".into(),
        ),
        None,
        ArticleSearchJsonPage {
            results,
            pagination,
            next_commands,
            suggestions: Vec::new(),
        },
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
    assert_eq!(value["results"][0]["first_index_date"], "2025-01-15");
    assert_eq!(
        value["_meta"]["next_commands"][0],
        serde_json::Value::String("biomcp get article 22663011".into())
    );
    assert!(
        value["_meta"]["next_commands"]
            .as_array()
            .is_some_and(|commands| commands.contains(&serde_json::Value::String(
                "biomcp search article -g BRAF --year-min 2025 --year-max 2025 --limit 5".into()
            )))
    );
    assert!(
        value["_meta"]["next_commands"]
            .as_array()
            .is_some_and(|commands| commands
                .contains(&serde_json::Value::String("biomcp list article".into())))
    );
    assert!(
        value
            .get("_meta")
            .and_then(|meta| meta.get("suggestions"))
            .is_none()
    );
    assert!(!related.contains(&"biomcp list article".to_string()));
}

#[test]
fn article_search_json_emits_structured_exact_entity_suggestions() {
    let pagination = PaginationMeta::offset(0, 1, 1, Some(1));
    let mut filters = super::super::related_article_filters();
    filters.keyword = Some("BRAF".into());
    let query = article_query_summary(
        &filters,
        crate::entities::article::ArticleSourceFilter::All,
        false,
        1,
        0,
    );
    let results = vec![crate::entities::article::ArticleSearchResult {
        pmid: "22663011".into(),
        pmcid: Some("PMC9984800".into()),
        doi: Some("10.1056/NEJMoa1203421".into()),
        title: "BRAF article".into(),
        journal: Some("Journal".into()),
        date: Some("2025-01-01".into()),
        first_index_date: None,
        citation_count: Some(12),
        influential_citation_count: Some(4),
        source: crate::entities::article::ArticleSource::EuropePmc,
        matched_sources: vec![crate::entities::article::ArticleSource::EuropePmc],
        score: None,
        is_retracted: Some(false),
        abstract_snippet: Some("Abstract".into()),
        ranking: None,
        normalized_title: "braf article".into(),
        normalized_abstract: "abstract".into(),
        publication_type: None,
        source_local_position: 0,
    }];
    let suggestions = vec![ArticleEntitySuggestion {
        command: "biomcp get gene BRAF".into(),
        reason: "Exact gene vocabulary match for article keyword \"BRAF\".".into(),
        sections: vec!["protein".into(), "diseases".into(), "expression".into()],
    }];
    let exact_commands = suggestions
        .iter()
        .map(|suggestion| suggestion.command.clone())
        .collect::<Vec<_>>();
    let next_commands = crate::render::markdown::search_next_commands_article(
        &results,
        &filters,
        crate::entities::article::ArticleSourceFilter::All,
        &exact_commands,
    );
    let related = crate::render::markdown::related_article_search_results(
        &results,
        &filters,
        crate::entities::article::ArticleSourceFilter::All,
        &exact_commands,
    );
    let json = article_search_json(
        &query,
        &filters,
        true,
        None,
        None,
        ArticleSearchJsonPage {
            results,
            pagination,
            next_commands,
            suggestions,
        },
    )
    .expect("article search json should render");

    let value: serde_json::Value =
        serde_json::from_str(&json).expect("json should parse successfully");
    assert!(value["results"][0].get("first_index_date").is_none());
    assert_eq!(
        value["_meta"]["next_commands"][0],
        serde_json::Value::String("biomcp get article 22663011".into())
    );
    assert_eq!(
        value["_meta"]["next_commands"][1],
        serde_json::Value::String("biomcp get gene BRAF".into())
    );
    assert!(
        value["_meta"]["next_commands"]
            .as_array()
            .is_some_and(|commands| commands.contains(&serde_json::Value::String(
                "biomcp search article -k BRAF --year-min 2025 --year-max 2025 --limit 5".into()
            )))
    );
    assert!(
        value["_meta"]["next_commands"]
            .as_array()
            .is_some_and(|commands| commands
                .contains(&serde_json::Value::String("biomcp list article".into())))
    );
    assert_eq!(
        value["_meta"]["suggestions"][0]["command"],
        serde_json::Value::String("biomcp get gene BRAF".into())
    );
    assert_eq!(
        value["_meta"]["suggestions"][0]["reason"],
        serde_json::Value::String(
            "Exact gene vocabulary match for article keyword \"BRAF\".".into()
        )
    );
    assert_eq!(
        value["_meta"]["suggestions"][0]["sections"],
        serde_json::json!(["protein", "diseases", "expression"])
    );
    assert!(related.contains(&"biomcp get gene BRAF".to_string()));
    assert!(
        !related
            .iter()
            .any(|command| command.contains("search article -g BRAF -k"))
    );
}

#[test]
fn article_search_json_next_commands_preserve_source_filter() {
    let pagination = PaginationMeta::offset(0, 1, 1, Some(1));
    let mut filters = super::super::related_article_filters();
    filters.keyword = Some("BRAF melanoma".into());
    let query = article_query_summary(
        &filters,
        crate::entities::article::ArticleSourceFilter::PubMed,
        false,
        1,
        0,
    );
    let results = vec![crate::entities::article::ArticleSearchResult {
        pmid: "22663011".into(),
        pmcid: None,
        doi: None,
        title: "BRAF melanoma review".into(),
        journal: Some("Journal".into()),
        date: Some("2013-05-12".into()),
        first_index_date: None,
        citation_count: Some(12),
        influential_citation_count: Some(4),
        source: crate::entities::article::ArticleSource::PubMed,
        matched_sources: vec![crate::entities::article::ArticleSource::PubMed],
        score: None,
        is_retracted: Some(false),
        abstract_snippet: Some("Abstract".into()),
        ranking: None,
        normalized_title: "braf melanoma review".into(),
        normalized_abstract: "abstract".into(),
        publication_type: None,
        source_local_position: 0,
    }];
    let next_commands = crate::render::markdown::search_next_commands_article(
        &results,
        &filters,
        crate::entities::article::ArticleSourceFilter::PubMed,
        &[],
    );
    let json = article_search_json(
        &query,
        &filters,
        true,
        None,
        None,
        ArticleSearchJsonPage {
            results,
            pagination,
            next_commands,
            suggestions: Vec::new(),
        },
    )
    .expect("article search json should render");

    let value: serde_json::Value =
        serde_json::from_str(&json).expect("json should parse successfully");
    assert!(
        value["_meta"]["next_commands"]
            .as_array()
            .is_some_and(|commands| commands.contains(&serde_json::Value::String(
                "biomcp search article -k \"BRAF melanoma\" --source pubmed --year-min 2013 --year-max 2013 --limit 5".into()
            )))
    );
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
fn related_article_filters_default_to_relevance_and_safety_flags() {
    let filters = super::super::related_article_filters();

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
    let mut filters = super::super::related_article_filters();
    filters.keyword = Some("melanoma".into());
    filters.max_per_source = Some(10);

    let summary = article_query_summary(
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

    let debug_filters = article_debug_filters(
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
    let mut filters = super::super::related_article_filters();
    filters.gene = Some("BRAF".into());
    filters.max_per_source = Some(0);

    let summary = article_query_summary(
        &filters,
        crate::entities::article::ArticleSourceFilter::All,
        false,
        25,
        0,
    );
    assert!(summary.contains("max_per_source=default"));

    let debug_filters = article_debug_filters(
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
    let disabled_summary = article_query_summary(
        &filters,
        crate::entities::article::ArticleSourceFilter::All,
        false,
        25,
        0,
    );
    assert!(disabled_summary.contains("max_per_source=disabled"));

    let disabled_debug_filters = article_debug_filters(
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
