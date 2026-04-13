use super::{DrugCommand, DrugGetArgs, DrugSearchArgs};
use crate::cli::CommandOutcome;
use crate::entities::drug::DrugRegion;

pub(crate) async fn handle_get(
    args: DrugGetArgs,
    json: bool,
    alias_suggestions_as_json: bool,
) -> anyhow::Result<CommandOutcome> {
    let (sections, json_override) = super::super::extract_json_from_sections(&args.sections);
    let region = args.region.map(DrugRegion::from);
    let json_output = json || json_override;
    super::super::render_drug_card_outcome(
        &args.name,
        &sections,
        region,
        args.raw,
        json_output,
        alias_suggestions_as_json,
    )
    .await
}

pub(crate) async fn handle_search(
    args: DrugSearchArgs,
    json: bool,
) -> anyhow::Result<CommandOutcome> {
    let query = super::super::resolve_query_input(args.query, args.positional_query, "--query")?;
    let filters = crate::entities::drug::DrugSearchFilters {
        query,
        target: args.target,
        indication: args.indication,
        mechanism: args.mechanism,
        drug_type: args.drug_type,
        atc: args.atc,
        pharm_class: args.pharm_class,
        interactions: args.interactions,
    };
    let region = super::super::resolve_drug_search_region(args.region, &filters)?;
    let mut query_summary = crate::entities::drug::search_query_summary(&filters);
    if args.offset > 0 {
        query_summary = format!("{query_summary}, offset={}", args.offset);
    }
    let text = match crate::entities::drug::search_page_with_region(
        &filters,
        args.limit,
        args.offset,
        region,
    )
    .await?
    {
        crate::entities::drug::DrugSearchPageWithRegion::Us(page) => {
            let results = page.results;
            let pagination = super::super::PaginationMeta::offset(
                args.offset,
                args.limit,
                results.len(),
                page.total,
            );
            if json {
                return super::super::search_json(results, pagination).map(CommandOutcome::stdout);
            }
            let footer = super::super::pagination_footer_offset(&pagination);
            crate::render::markdown::drug_search_markdown_with_region(
                &query_summary,
                region,
                &results,
                pagination.total,
                &[],
                None,
                &[],
                None,
                &footer,
            )?
        }
        crate::entities::drug::DrugSearchPageWithRegion::Eu(page) => {
            let results = page.results;
            let pagination = super::super::PaginationMeta::offset(
                args.offset,
                args.limit,
                results.len(),
                page.total,
            );
            if json {
                return super::super::search_json(results, pagination).map(CommandOutcome::stdout);
            }
            let footer = super::super::pagination_footer_offset(&pagination);
            crate::render::markdown::drug_search_markdown_with_region(
                &query_summary,
                region,
                &[],
                None,
                &results,
                pagination.total,
                &[],
                None,
                &footer,
            )?
        }
        crate::entities::drug::DrugSearchPageWithRegion::Who(page) => {
            let results = page.results;
            let pagination = super::super::PaginationMeta::offset(
                args.offset,
                args.limit,
                results.len(),
                page.total,
            );
            if json {
                return super::super::search_json(results, pagination).map(CommandOutcome::stdout);
            }
            let footer = super::super::pagination_footer_offset(&pagination);
            crate::render::markdown::drug_search_markdown_with_region(
                &query_summary,
                region,
                &[],
                None,
                &[],
                None,
                &results,
                pagination.total,
                &footer,
            )?
        }
        crate::entities::drug::DrugSearchPageWithRegion::All { us, eu, who } => {
            if json {
                return super::super::drug_all_region_search_json(&query_summary, us, eu, who)
                    .map(CommandOutcome::stdout);
            }
            crate::render::markdown::drug_search_markdown_with_region(
                &query_summary,
                region,
                &us.results,
                us.total,
                &eu.results,
                eu.total,
                &who.results,
                who.total,
                "",
            )?
        }
    };

    Ok(CommandOutcome::stdout(text))
}

pub(crate) async fn handle_command(
    cmd: DrugCommand,
    json: bool,
    alias_suggestions_as_json: bool,
) -> anyhow::Result<CommandOutcome> {
    match cmd {
        DrugCommand::External(args) => {
            let name = args.join(" ");
            super::super::render_drug_card_outcome(
                &name,
                super::super::empty_sections(),
                None,
                false,
                json,
                alias_suggestions_as_json,
            )
            .await
        }
        other => {
            let text = match other {
                DrugCommand::Trials {
                    name,
                    limit,
                    offset,
                    source,
                } => {
                    let trial_source = crate::entities::trial::TrialSource::from_flag(&source)?;
                    let filters = crate::entities::trial::TrialSearchFilters {
                        intervention: Some(name.clone()),
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
                            format!("intervention={name}, offset={offset}")
                        } else {
                            format!("intervention={name}")
                        };
                        crate::render::markdown::trial_search_markdown(&query, &results, total)?
                    }
                }
                DrugCommand::AdverseEvents {
                    name,
                    limit,
                    offset,
                    serious,
                } => {
                    let filters = crate::entities::adverse_event::AdverseEventSearchFilters {
                        drug: Some(name.clone()),
                        serious: serious.then_some("any".to_string()),
                        ..Default::default()
                    };
                    let query_summary =
                        crate::entities::adverse_event::search_query_summary(&filters);
                    let fetch_limit = super::super::paged_fetch_limit(limit, offset, 50)?;
                    let response = crate::entities::adverse_event::search_with_summary(
                        &filters,
                        fetch_limit,
                        0,
                    )
                    .await?;
                    let (results, observed_total) =
                        super::super::paginate_results(response.results, offset, limit);
                    super::super::log_pagination_truncation(observed_total, offset, results.len());
                    let summary = crate::entities::adverse_event::summarize_search_results(
                        response.summary.total_reports,
                        &results,
                    );
                    if json {
                        #[derive(serde::Serialize)]
                        struct SearchResponse {
                            total: Option<usize>,
                            count: usize,
                            summary: crate::entities::adverse_event::AdverseEventSearchSummary,
                            results: Vec<crate::entities::adverse_event::AdverseEventSearchResult>,
                        }

                        crate::render::json::to_pretty(&SearchResponse {
                            total: Some(summary.total_reports),
                            count: results.len(),
                            summary,
                            results,
                        })?
                    } else {
                        crate::render::markdown::adverse_event_search_markdown(
                            &query_summary,
                            &results,
                            &summary,
                        )?
                    }
                }
                DrugCommand::External(_) => unreachable!("handled above"),
            };

            Ok(CommandOutcome::stdout(text))
        }
    }
}
