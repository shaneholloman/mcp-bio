use clap::{CommandFactory, Parser};

use super::dispatch::{
    ArticleSearchJsonPage, article_debug_filters, article_query_summary, article_search_json,
    build_article_debug_plan, truncate_article_annotations,
};
use crate::cli::{Cli, Commands, PaginationMeta};

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
    let next_commands = crate::render::markdown::search_next_commands_article(&results);
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
    assert_eq!(
        value["_meta"]["next_commands"][0],
        serde_json::Value::String("biomcp get article 22663011".into())
    );
    assert_eq!(
        value["_meta"]["next_commands"][1],
        serde_json::Value::String("biomcp list article".into())
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
