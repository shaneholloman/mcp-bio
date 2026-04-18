use super::{GeneCommand, GeneGetArgs, GeneSearchArgs};
use crate::cli::CommandOutcome;

pub(crate) async fn handle_get(
    args: GeneGetArgs,
    json: bool,
    alias_suggestions_as_json: bool,
) -> anyhow::Result<CommandOutcome> {
    let (sections, json_override) = super::super::extract_json_from_sections(&args.sections);
    let json_output = json || json_override;
    render_gene_card_outcome(
        &args.symbol,
        &sections,
        json_output,
        alias_suggestions_as_json,
    )
    .await
}

pub(crate) async fn handle_search(
    args: GeneSearchArgs,
    json: bool,
) -> anyhow::Result<CommandOutcome> {
    let query = super::super::resolve_query_input(args.query, args.positional_query, "--query")?;
    let filters = crate::entities::gene::GeneSearchFilters {
        query,
        gene_type: args.gene_type,
        chromosome: args.chromosome,
        region: args.region,
        pathway: args.pathway,
        go_term: args.go_term,
    };
    let mut query_summary = crate::entities::gene::search_query_summary(&filters);
    if args.offset > 0 {
        query_summary = format!("{query_summary}, offset={}", args.offset);
    }
    let page = crate::entities::gene::search_page(&filters, args.limit, args.offset).await?;
    let results = page.results;
    let pagination =
        super::super::PaginationMeta::offset(args.offset, args.limit, results.len(), page.total);
    let text = if json {
        let next_commands = crate::render::markdown::search_next_commands_gene(&results);
        return super::super::search_json_with_meta(results, pagination, next_commands)
            .map(CommandOutcome::stdout);
    } else {
        let footer = super::super::pagination_footer_offset(&pagination);
        crate::render::markdown::gene_search_markdown_with_footer(
            &query_summary,
            &results,
            &footer,
        )?
    };
    Ok(CommandOutcome::stdout(text))
}

pub(crate) async fn handle_command(
    cmd: GeneCommand,
    json: bool,
    alias_suggestions_as_json: bool,
) -> anyhow::Result<CommandOutcome> {
    match cmd {
        GeneCommand::Definition { symbol } => {
            render_gene_card_outcome(
                &symbol,
                super::super::empty_sections(),
                json,
                alias_suggestions_as_json,
            )
            .await
        }
        GeneCommand::External(args) => {
            let symbol = args.join(" ");
            render_gene_card_outcome(
                &symbol,
                super::super::empty_sections(),
                json,
                alias_suggestions_as_json,
            )
            .await
        }
        other => {
            let text = match other {
                GeneCommand::Trials {
                    symbol,
                    limit,
                    offset,
                    source,
                } => {
                    let trial_source = crate::entities::trial::TrialSource::from_flag(&source)?;
                    let filters = crate::entities::trial::TrialSearchFilters {
                        biomarker: Some(symbol.clone()),
                        source: trial_source,
                        ..Default::default()
                    };
                    let (results, total) =
                        crate::entities::trial::search(&filters, limit, offset).await?;
                    if let Some(total) = total {
                        super::super::log_pagination_truncation(
                            total as usize,
                            offset,
                            results.len(),
                        );
                    }
                    if json {
                        #[derive(serde::Serialize)]
                        struct SearchResponse {
                            count: usize,
                            total: Option<u32>,
                            results: Vec<crate::entities::trial::TrialSearchResult>,
                        }

                        crate::render::json::to_pretty(&SearchResponse {
                            count: results.len(),
                            total,
                            results,
                        })?
                    } else {
                        let query = if offset > 0 {
                            format!("biomarker={symbol}, offset={offset}")
                        } else {
                            format!("biomarker={symbol}")
                        };
                        crate::render::markdown::trial_search_markdown(&query, &results, total)?
                    }
                }
                GeneCommand::Drugs {
                    symbol,
                    limit,
                    offset,
                } => {
                    let filters = crate::entities::drug::DrugSearchFilters {
                        target: Some(symbol.clone()),
                        ..Default::default()
                    };
                    let mut query_summary = crate::entities::drug::search_query_summary(&filters);
                    if offset > 0 {
                        query_summary = format!("{query_summary}, offset={offset}");
                    }
                    let fetch_limit = super::super::paged_fetch_limit(limit, offset, 50)?;
                    let rows = crate::entities::drug::search(&filters, fetch_limit).await?;
                    let (results, total) = super::super::paginate_results(rows, offset, limit);
                    super::super::log_pagination_truncation(total, offset, results.len());
                    if json {
                        #[derive(serde::Serialize)]
                        struct SearchResponse {
                            total: Option<usize>,
                            count: usize,
                            results: Vec<crate::entities::drug::DrugSearchResult>,
                        }

                        crate::render::json::to_pretty(&SearchResponse {
                            total: Some(total),
                            count: results.len(),
                            results,
                        })?
                    } else {
                        crate::render::markdown::drug_search_markdown(&query_summary, &results)?
                    }
                }
                GeneCommand::Articles {
                    symbol,
                    limit,
                    offset,
                } => {
                    let filters = crate::entities::article::ArticleSearchFilters {
                        gene: Some(symbol.clone()),
                        gene_anchored: true,
                        ..super::super::related_article_filters()
                    };
                    let query = if offset > 0 {
                        format!("gene={symbol}, offset={offset}")
                    } else {
                        format!("gene={symbol}")
                    };
                    let fetch_limit = super::super::paged_fetch_limit(limit, offset, 50)?;
                    let rows = crate::entities::article::search(&filters, fetch_limit).await?;
                    let (results, total) = super::super::paginate_results(rows, offset, limit);
                    super::super::log_pagination_truncation(total, offset, results.len());
                    if json {
                        #[derive(serde::Serialize)]
                        struct SearchResponse {
                            total: Option<usize>,
                            count: usize,
                            results: Vec<crate::entities::article::ArticleSearchResult>,
                        }

                        crate::render::json::to_pretty(&SearchResponse {
                            total: Some(total),
                            count: results.len(),
                            results,
                        })?
                    } else {
                        crate::render::markdown::article_search_markdown_with_footer_and_context(
                            &query,
                            &results,
                            "",
                            &filters,
                            crate::render::markdown::ArticleSearchRenderContext {
                                source_filter: crate::entities::article::ArticleSourceFilter::All,
                                semantic_scholar_enabled:
                                    crate::entities::article::semantic_scholar_search_enabled(
                                        &filters,
                                        crate::entities::article::ArticleSourceFilter::All,
                                    ),
                                note: None,
                                debug_plan: None,
                            },
                        )?
                    }
                }
                GeneCommand::Pathways {
                    symbol,
                    limit,
                    offset,
                } => {
                    let fetch_limit = super::super::paged_fetch_limit(limit, offset, 25)?;
                    let sections = vec!["pathways".to_string()];
                    let mut gene = crate::entities::gene::get(&symbol, &sections).await?;
                    if let Some(pathways) = gene.pathways.take() {
                        let fetched = pathways.into_iter().take(fetch_limit).collect::<Vec<_>>();
                        let (results, observed_total) =
                            super::super::paginate_results(fetched, offset, limit);
                        super::super::log_pagination_truncation(
                            observed_total,
                            offset,
                            results.len(),
                        );
                        gene.pathways = (!results.is_empty()).then_some(results);
                    }
                    if json {
                        crate::render::json::to_pretty(&gene)?
                    } else {
                        crate::render::markdown::gene_markdown(&gene, &sections)?
                    }
                }
                GeneCommand::Definition { .. } | GeneCommand::External(_) => {
                    unreachable!("handled above")
                }
            };

            Ok(CommandOutcome::stdout(text))
        }
    }
}

pub(super) async fn render_gene_card_outcome(
    symbol: &str,
    sections: &[String],
    json_output: bool,
    alias_suggestions_as_json: bool,
) -> anyhow::Result<CommandOutcome> {
    match crate::entities::gene::get(symbol, sections).await {
        Ok(gene) => {
            let text = if json_output {
                crate::render::json::to_entity_json_with_suggestions(
                    &gene,
                    crate::render::markdown::gene_evidence_urls(&gene),
                    crate::render::markdown::gene_next_commands(&gene, sections),
                    crate::render::markdown::related_gene(&gene),
                    crate::render::provenance::gene_section_sources(&gene),
                )?
            } else {
                crate::render::markdown::gene_markdown(&gene, sections)?
            };
            Ok(CommandOutcome::stdout(text))
        }
        Err(err @ crate::error::BioMcpError::NotFound { .. }) => {
            if let Some(outcome) = super::super::try_alias_fallback_outcome(
                symbol,
                crate::entities::discover::DiscoverType::Gene,
                json_output || alias_suggestions_as_json,
            )
            .await?
            {
                Ok(outcome)
            } else {
                Err(err.into())
            }
        }
        Err(err) => Err(err.into()),
    }
}
