use super::{ArticleCommand, ArticleGetArgs, ArticleSearchArgs};
use crate::cli::CommandOutcome;

pub(in crate::cli) async fn handle_get(
    args: ArticleGetArgs,
    json: bool,
) -> anyhow::Result<CommandOutcome> {
    let (sections, json_override) = super::super::extract_json_from_sections(&args.sections);
    let json_output = json || json_override;
    let article = crate::entities::article::get(&args.id, &sections).await?;
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

pub(in crate::cli) async fn handle_search(
    args: ArticleSearchArgs,
    json: bool,
) -> anyhow::Result<CommandOutcome> {
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
        date_from: args.date_from,
        date_to: args.date_to,
        article_type: args.article_type,
        journal,
        open_access: args.open_access,
        no_preprints: args.no_preprints,
        exclude_retracted,
        max_per_source: args.max_per_source,
        sort,
        ranking,
    };

    let query = super::super::article_query_summary(
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
        Some(super::super::build_article_debug_plan(
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

    let text = if json {
        super::super::article_search_json(
            &query,
            &filters,
            semantic_scholar_enabled,
            crate::entities::article::article_type_limitation_note(&filters, source_filter),
            debug_plan,
            results,
            pagination,
        )?
    } else {
        let footer = super::super::pagination_footer_offset(&pagination);
        crate::render::markdown::article_search_markdown_with_footer_and_context(
            &query,
            &results,
            &footer,
            &filters,
            semantic_scholar_enabled,
            crate::entities::article::article_type_limitation_note(&filters, source_filter)
                .as_deref(),
            debug_plan.as_ref(),
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
            let article = crate::entities::article::get(&pmid, &sections).await?;
            let annotations = article
                .annotations
                .clone()
                .map(|value| super::super::truncate_article_annotations(value, limit));
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
