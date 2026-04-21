use super::{ArticleCommand, ArticleGetArgs, ArticleSearchArgs};
use crate::cli::CommandOutcome;

fn extract_pdf_from_sections(sections: &[String]) -> (Vec<String>, bool) {
    let mut allow_pdf = false;
    let cleaned = sections
        .iter()
        .filter_map(|raw| {
            let trimmed = raw.trim();
            let normalized = trimmed.to_ascii_lowercase();
            if normalized == "--pdf" {
                allow_pdf = true;
                return None;
            }
            if trimmed.is_empty() {
                return None;
            }
            Some(trimmed.to_string())
        })
        .collect();
    (cleaned, allow_pdf)
}

pub(in crate::cli) async fn handle_get(
    args: ArticleGetArgs,
    json: bool,
) -> anyhow::Result<CommandOutcome> {
    let (sections, json_override) = super::super::extract_json_from_sections(&args.sections);
    let (sections, pdf_from_sections) = extract_pdf_from_sections(&sections);
    let json_output = json || json_override;
    let article = crate::entities::article::get(
        &args.id,
        &sections,
        crate::entities::article::ArticleGetOptions {
            allow_pdf: args.pdf || pdf_from_sections,
        },
    )
    .await?;
    let text = if json_output {
        crate::render::json::to_entity_json(
            &article,
            crate::render::markdown::article_evidence_urls(&article),
            crate::render::markdown::related_article(&article),
            crate::render::provenance::article_section_sources(&article),
        )?
    } else {
        crate::render::markdown::article_markdown(&article, &sections)?
    };
    Ok(CommandOutcome::stdout(text))
}

pub(super) fn resolved_article_date_bounds(
    args: &ArticleSearchArgs,
) -> (Option<String>, Option<String>) {
    let date_from = args
        .date_from
        .clone()
        .or_else(|| args.year_min.map(|year| format!("{year:04}-01-01")));
    let date_to = args
        .date_to
        .clone()
        .or_else(|| args.year_max.map(|year| format!("{year:04}-12-31")));
    (date_from, date_to)
}

pub(in crate::cli) async fn handle_search(
    args: ArticleSearchArgs,
    json: bool,
) -> anyhow::Result<CommandOutcome> {
    let (date_from, date_to) = resolved_article_date_bounds(&args);
    let disease = super::super::normalize_cli_tokens(args.disease);
    let drug = super::super::normalize_cli_tokens(args.drug);
    let author = super::super::normalize_cli_tokens(args.author);
    let keyword = super::super::resolve_query_input(
        super::super::normalize_cli_tokens(args.keyword),
        args.positional_query,
        "--keyword/--query",
    )?;
    let journal = super::super::normalize_cli_tokens(args.journal);
    let sort = crate::entities::article::ArticleSort::from_flag(&args.sort)?;
    let source_filter = crate::entities::article::ArticleSourceFilter::from_flag(&args.source)?;
    let exclude_retracted = args.exclude_retracted || !args.include_retracted;
    let ranking = crate::entities::article::ArticleRankingOptions::from_inputs(
        args.ranking_mode.as_deref(),
        args.weight_semantic,
        args.weight_lexical,
        args.weight_citations,
        args.weight_position,
    )?;
    let gene_anchored = args
        .gene
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
        && disease.as_deref().map(str::trim).is_none_or(str::is_empty)
        && drug.as_deref().map(str::trim).is_none_or(str::is_empty)
        && author.as_deref().map(str::trim).is_none_or(str::is_empty)
        && keyword.as_deref().map(str::trim).is_none_or(str::is_empty);
    let filters = crate::entities::article::ArticleSearchFilters {
        gene: args.gene,
        gene_anchored,
        disease,
        drug,
        author,
        keyword,
        date_from,
        date_to,
        article_type: args.article_type,
        journal,
        open_access: args.open_access,
        no_preprints: args.no_preprints,
        exclude_retracted,
        max_per_source: args.max_per_source,
        sort,
        ranking,
    };

    let query = article_query_summary(
        &filters,
        source_filter,
        args.include_retracted,
        args.limit,
        args.offset,
    );

    let page =
        crate::entities::article::search_page(&filters, args.limit, args.offset, source_filter)
            .await?;
    let results = page.results;
    let pagination =
        super::super::PaginationMeta::offset(args.offset, args.limit, results.len(), page.total);
    let semantic_scholar_enabled =
        crate::entities::article::semantic_scholar_search_enabled(&filters, source_filter);
    let debug_plan = if args.debug_plan {
        Some(build_article_debug_plan(
            &query,
            &filters,
            source_filter,
            args.limit,
            &results,
            &pagination,
        )?)
    } else {
        None
    };
    let suggestions =
        crate::render::markdown::related_article_search_results(&results, &filters, source_filter);
    let next_commands =
        crate::render::markdown::search_next_commands_article(&results, &filters, source_filter);

    let text = if json {
        article_search_json(
            &query,
            &filters,
            semantic_scholar_enabled,
            crate::entities::article::article_type_limitation_note(&filters, source_filter),
            debug_plan,
            ArticleSearchJsonPage {
                results,
                pagination,
                next_commands,
                suggestions,
            },
        )?
    } else {
        let footer = super::super::pagination_footer_offset(&pagination);
        crate::render::markdown::article_search_markdown_with_footer_and_context(
            &query,
            &results,
            &footer,
            &filters,
            crate::render::markdown::ArticleSearchRenderContext {
                source_filter,
                semantic_scholar_enabled,
                note: crate::entities::article::article_type_limitation_note(
                    &filters,
                    source_filter,
                )
                .as_deref(),
                debug_plan: debug_plan.as_ref(),
            },
        )?
    };

    Ok(CommandOutcome::stdout(text))
}

pub(in crate::cli) async fn handle_command(
    cmd: ArticleCommand,
    json: bool,
) -> anyhow::Result<CommandOutcome> {
    let text = match cmd {
        ArticleCommand::Entities { pmid, limit } => {
            let limit = super::super::paged_fetch_limit(limit, 0, 50)?;
            let sections = vec!["annotations".to_string()];
            let article = crate::entities::article::get(
                &pmid,
                &sections,
                crate::entities::article::ArticleGetOptions::default(),
            )
            .await?;
            let annotations = article
                .annotations
                .clone()
                .map(|value| truncate_article_annotations(value, limit));
            if json {
                #[derive(serde::Serialize)]
                struct ArticleEntitiesResponse {
                    pmid: String,
                    annotations: Option<crate::entities::article::ArticleAnnotations>,
                }

                crate::render::json::to_pretty(&ArticleEntitiesResponse { pmid, annotations })?
            } else {
                crate::render::markdown::article_entities_markdown(
                    article.pmid.as_deref().unwrap_or(&pmid),
                    annotations.as_ref(),
                    Some(limit),
                )?
            }
        }
        ArticleCommand::Batch { ids } => {
            let results = crate::entities::article::get_batch_compact(&ids).await?;
            if json {
                crate::render::json::to_pretty(&results)?
            } else {
                crate::render::markdown::article_batch_markdown(&results)?
            }
        }
        ArticleCommand::Citations { id, limit } => {
            let limit = super::super::paged_fetch_limit(limit, 0, 100)?;
            let graph = crate::entities::article::citations(&id, limit).await?;
            if json {
                crate::render::json::to_pretty(&graph)?
            } else {
                crate::render::markdown::article_graph_markdown("Citations", &graph)?
            }
        }
        ArticleCommand::References { id, limit } => {
            let limit = super::super::paged_fetch_limit(limit, 0, 100)?;
            let graph = crate::entities::article::references(&id, limit).await?;
            if json {
                crate::render::json::to_pretty(&graph)?
            } else {
                crate::render::markdown::article_graph_markdown("References", &graph)?
            }
        }
        ArticleCommand::Recommendations {
            ids,
            negative,
            limit,
        } => {
            let limit = super::super::paged_fetch_limit(limit, 0, 100)?;
            let recommendations =
                crate::entities::article::recommendations(&ids, &negative, limit).await?;
            if json {
                crate::render::json::to_pretty(&recommendations)?
            } else {
                crate::render::markdown::article_recommendations_markdown(&recommendations)?
            }
        }
    };

    Ok(CommandOutcome::stdout(text))
}

pub(super) fn article_query_summary(
    filters: &crate::entities::article::ArticleSearchFilters,
    source_filter: crate::entities::article::ArticleSourceFilter,
    include_retracted: bool,
    limit: usize,
    offset: usize,
) -> String {
    let mut query = vec![
        filters.gene.as_deref().map(|v| format!("gene={v}")),
        filters.disease.as_deref().map(|v| format!("disease={v}")),
        filters.drug.as_deref().map(|v| format!("drug={v}")),
        filters.author.as_deref().map(|v| format!("author={v}")),
        filters.keyword.as_deref().map(|v| format!("keyword={v}")),
        filters.article_type.as_deref().map(|v| format!("type={v}")),
        filters
            .date_from
            .as_deref()
            .map(|v| format!("date_from={v}")),
        filters.date_to.as_deref().map(|v| format!("date_to={v}")),
        filters.journal.as_deref().map(|v| format!("journal={v}")),
        filters.open_access.then(|| "open_access=true".to_string()),
        filters
            .no_preprints
            .then(|| "no_preprints=true".to_string()),
        if include_retracted {
            Some("include_retracted=true".to_string())
        } else {
            filters
                .exclude_retracted
                .then(|| "exclude_retracted=true".to_string())
        },
        Some(format!("sort={}", filters.sort.as_str())),
        (source_filter != crate::entities::article::ArticleSourceFilter::All)
            .then(|| format!("source={}", source_filter.as_str())),
        article_max_per_source_summary(filters.max_per_source, limit),
        (offset > 0).then(|| format!("offset={offset}")),
    ];
    if let Some(mode) = crate::entities::article::article_effective_ranking_mode(filters) {
        query.push(Some(format!("ranking_mode={}", mode.as_str())));
        query.push(
            crate::entities::article::article_relevance_ranking_policy(filters)
                .map(|policy| format!("ranking_policy={policy}")),
        );
    }
    query.into_iter().flatten().collect::<Vec<_>>().join(", ")
}

pub(super) fn article_max_per_source_summary(
    max_per_source: Option<usize>,
    limit: usize,
) -> Option<String> {
    match max_per_source {
        None => None,
        Some(0) => Some("max_per_source=default".to_string()),
        Some(value) if value == limit => Some("max_per_source=disabled".to_string()),
        Some(value) => Some(format!("max_per_source={value}")),
    }
}

pub(super) fn article_debug_filters(
    filters: &crate::entities::article::ArticleSearchFilters,
    source_filter: crate::entities::article::ArticleSourceFilter,
    limit: usize,
) -> Vec<String> {
    let mut values = vec![
        filters.gene.as_deref().map(|v| format!("gene={v}")),
        filters.disease.as_deref().map(|v| format!("disease={v}")),
        filters.drug.as_deref().map(|v| format!("drug={v}")),
        filters.author.as_deref().map(|v| format!("author={v}")),
        filters.keyword.as_deref().map(|v| format!("keyword={v}")),
        filters
            .date_from
            .as_deref()
            .map(|v| format!("date_from={v}")),
        filters.date_to.as_deref().map(|v| format!("date_to={v}")),
        filters.article_type.as_deref().map(|v| format!("type={v}")),
        filters.journal.as_deref().map(|v| format!("journal={v}")),
        filters.open_access.then(|| "open_access=true".to_string()),
        filters
            .no_preprints
            .then(|| "no_preprints=true".to_string()),
        Some(format!("exclude_retracted={}", filters.exclude_retracted)),
        Some(format!("sort={}", filters.sort.as_str())),
        Some(format!("source={}", source_filter.as_str())),
        article_max_per_source_summary(filters.max_per_source, limit),
    ];
    if let Some(mode) = crate::entities::article::article_effective_ranking_mode(filters) {
        values.push(Some(format!("ranking_mode={}", mode.as_str())));
        values.push(
            crate::entities::article::article_relevance_ranking_policy(filters)
                .map(|policy| format!("ranking_policy={policy}")),
        );
    }
    values.into_iter().flatten().collect()
}

pub(super) fn build_article_debug_plan(
    query: &str,
    filters: &crate::entities::article::ArticleSearchFilters,
    source_filter: crate::entities::article::ArticleSourceFilter,
    limit: usize,
    results: &[crate::entities::article::ArticleSearchResult],
    pagination: &crate::cli::PaginationMeta,
) -> Result<crate::cli::debug_plan::DebugPlan, crate::error::BioMcpError> {
    let summary = crate::entities::article::summarize_debug_plan(filters, source_filter, results)?;
    Ok(crate::cli::debug_plan::DebugPlan {
        surface: "search_article",
        query: query.to_string(),
        anchor: None,
        legs: vec![crate::cli::debug_plan::DebugPlanLeg {
            leg: "article".to_string(),
            entity: "article".to_string(),
            filters: article_debug_filters(filters, source_filter, limit),
            routing: summary.routing,
            sources: summary.sources,
            matched_sources: summary.matched_sources,
            count: results.len(),
            total: pagination.total,
            note: crate::entities::article::article_type_limitation_note(filters, source_filter),
            error: None,
        }],
    })
}

pub(super) struct ArticleSearchJsonPage {
    pub results: Vec<crate::entities::article::ArticleSearchResult>,
    pub pagination: crate::cli::PaginationMeta,
    pub next_commands: Vec<String>,
    pub suggestions: Vec<String>,
}

pub(super) fn article_search_json(
    query: &str,
    filters: &crate::entities::article::ArticleSearchFilters,
    semantic_scholar_enabled: bool,
    note: Option<String>,
    debug_plan: Option<crate::cli::debug_plan::DebugPlan>,
    page: ArticleSearchJsonPage,
) -> anyhow::Result<String> {
    #[derive(serde::Serialize)]
    struct ArticleSearchResponse {
        query: String,
        sort: String,
        semantic_scholar_enabled: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        ranking_policy: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        note: Option<String>,
        pagination: crate::cli::PaginationMeta,
        count: usize,
        results: Vec<crate::entities::article::ArticleSearchResult>,
        #[serde(skip_serializing_if = "Option::is_none")]
        debug_plan: Option<crate::cli::debug_plan::DebugPlan>,
        #[serde(skip_serializing_if = "Option::is_none")]
        _meta: Option<crate::cli::SearchJsonMeta>,
    }

    let count = page.results.len();
    crate::render::json::to_pretty(&ArticleSearchResponse {
        query: query.to_string(),
        sort: filters.sort.as_str().to_string(),
        semantic_scholar_enabled,
        ranking_policy: crate::entities::article::article_relevance_ranking_policy(filters),
        note,
        pagination: page.pagination,
        count,
        results: page.results,
        debug_plan,
        _meta: crate::cli::search_meta_with_suggestions(page.next_commands, Some(page.suggestions)),
    })
    .map_err(Into::into)
}

pub(super) fn truncate_article_annotations(
    mut annotations: crate::entities::article::ArticleAnnotations,
    limit: usize,
) -> crate::entities::article::ArticleAnnotations {
    annotations.genes.truncate(limit);
    annotations.diseases.truncate(limit);
    annotations.chemicals.truncate(limit);
    annotations.mutations.truncate(limit);
    annotations
}
