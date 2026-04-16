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
    render_drug_card_outcome(
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
    let region = resolve_drug_search_region(args.region, &filters)?;
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
                let next_commands = crate::render::markdown::search_next_commands_drug(
                    &results,
                    filters.query.as_deref(),
                );
                return super::super::search_json_with_meta(results, pagination, next_commands)
                    .map(CommandOutcome::stdout);
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
                let next_commands = crate::render::markdown::search_next_commands_drug_eu(
                    &results,
                    filters.query.as_deref(),
                );
                return super::super::search_json_with_meta(results, pagination, next_commands)
                    .map(CommandOutcome::stdout);
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
                let next_commands = crate::render::markdown::search_next_commands_drug_who(
                    &results,
                    filters.query.as_deref(),
                );
                return super::super::search_json_with_meta(results, pagination, next_commands)
                    .map(CommandOutcome::stdout);
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
                let next_commands =
                    if us.results.is_empty() && eu.results.is_empty() && who.results.is_empty() {
                        Vec::new()
                    } else {
                        filters
                            .query
                            .as_deref()
                            .map(crate::render::markdown::search_next_commands_drug_all)
                            .unwrap_or_default()
                    };
                return drug_all_region_search_json(&query_summary, us, eu, who, next_commands)
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
            render_drug_card_outcome(
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
                    no_alias_expand,
                } => {
                    let trial_source = crate::entities::trial::TrialSource::from_flag(&source)?;
                    let filters = crate::entities::trial::TrialSearchFilters {
                        intervention: Some(name.clone()),
                        no_alias_expand,
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
                        let mut query_parts = vec![format!("intervention={name}")];
                        if no_alias_expand
                            && matches!(
                                trial_source,
                                crate::entities::trial::TrialSource::ClinicalTrialsGov
                            )
                        {
                            query_parts.push("alias_expand=off".to_string());
                        }
                        if offset > 0 {
                            query_parts.push(format!("offset={offset}"));
                        }
                        let query = query_parts.join(", ");
                        crate::render::markdown::trial_search_markdown(&query, &results, total)?
                    }
                }
                DrugCommand::AdverseEvents {
                    name,
                    limit,
                    offset,
                    serious,
                } => {
                    #[derive(serde::Serialize)]
                    struct SearchResponse {
                        total: Option<usize>,
                        count: usize,
                        summary: crate::entities::adverse_event::AdverseEventSearchSummary,
                        results: Vec<crate::entities::adverse_event::AdverseEventSearchResult>,
                        faers_not_found: bool,
                        #[serde(skip_serializing_if = "Option::is_none")]
                        trial_adverse_events:
                            Option<Vec<crate::entities::adverse_event::TrialAdverseEventTerm>>,
                    }

                    let filters = crate::entities::adverse_event::AdverseEventSearchFilters {
                        drug: Some(name.clone()),
                        serious: serious.then_some("any".to_string()),
                        ..Default::default()
                    };
                    let query_summary =
                        crate::entities::adverse_event::search_query_summary(&filters);
                    let fetch_limit = super::super::paged_fetch_limit(limit, offset, 50)?;
                    let status = crate::entities::adverse_event::search_with_status(
                        &filters,
                        fetch_limit,
                        0,
                    )
                    .await?;
                    match status {
                        crate::entities::adverse_event::FaersSearchStatus::Results(response) => {
                            let (results, observed_total) =
                                super::super::paginate_results(response.results, offset, limit);
                            super::super::log_pagination_truncation(
                                observed_total,
                                offset,
                                results.len(),
                            );
                            let summary = crate::entities::adverse_event::summarize_search_results(
                                response.summary.total_reports,
                                &results,
                            );
                            if json {
                                crate::render::json::to_pretty(&SearchResponse {
                                    total: Some(summary.total_reports),
                                    count: results.len(),
                                    summary,
                                    results,
                                    faers_not_found: false,
                                    trial_adverse_events: None,
                                })?
                            } else {
                                let empty_state_message = results.is_empty().then_some(
                                    "Drug found in FAERS, but no events matched your filters. Try broadening the search.",
                                );
                                crate::render::markdown::adverse_event_search_markdown_with_context(
                                    &query_summary,
                                    &results,
                                    &summary,
                                    "",
                                    empty_state_message,
                                    &[],
                                    None,
                                )?
                            }
                        }
                        crate::entities::adverse_event::FaersSearchStatus::NotFound => {
                            let trial_adverse_events =
                                match crate::entities::adverse_event::trial_adverse_events(&name)
                                    .await
                                {
                                    Ok(crate::entities::adverse_event::TrialAdverseEventOutcome::Found(
                                        rows,
                                    )) => Some(rows),
                                    Ok(crate::entities::adverse_event::TrialAdverseEventOutcome::Empty) => {
                                        None
                                    }
                                    Err(err) => {
                                        return Err(anyhow::anyhow!(
                                            "drug not found in FAERS and ClinicalTrials.gov fallback failed: {err}"
                                        ));
                                    }
                                };
                            let summary =
                                crate::entities::adverse_event::AdverseEventSearchSummary {
                                    total_reports: 0,
                                    returned_report_count: 0,
                                    top_reactions: Vec::new(),
                                };
                            let results = Vec::new();
                            if json {
                                crate::render::json::to_pretty(&SearchResponse {
                                    total: Some(0),
                                    count: 0,
                                    summary,
                                    results,
                                    faers_not_found: true,
                                    trial_adverse_events,
                                })?
                            } else {
                                let empty_state_message = if trial_adverse_events.is_some() {
                                    "Drug not found in FAERS. FAERS is a post-marketing database; expect no records for investigational, newly approved, or name-variant drugs. Falling back to ClinicalTrials.gov trial-reported adverse events."
                                } else {
                                    "Drug not found in FAERS. FAERS is a post-marketing database; expect no records for investigational, newly approved, or name-variant drugs. Falling back to ClinicalTrials.gov trial-reported adverse events. ClinicalTrials.gov did not return posted trial adverse events for this drug."
                                };
                                let trial_rows = trial_adverse_events.unwrap_or_default();
                                crate::render::markdown::adverse_event_search_markdown_with_context(
                                    &query_summary,
                                    &results,
                                    &summary,
                                    "",
                                    Some(empty_state_message),
                                    &trial_rows,
                                    Some(&name),
                                )?
                            }
                        }
                    }
                }
                DrugCommand::External(_) => unreachable!("handled above"),
            };

            Ok(CommandOutcome::stdout(text))
        }
    }
}

pub(super) const DRUG_SEARCH_EMA_STRUCTURED_FILTER_ERROR: &str = "EMA and all-region search currently support name/alias lookups only; use --region us for structured MyChem filters or --region who to filter structured U.S. hits through WHO prequalification.";

pub(super) fn resolve_drug_search_region(
    region_arg: Option<crate::cli::DrugRegionArg>,
    filters: &crate::entities::drug::DrugSearchFilters,
) -> Result<DrugRegion, crate::error::BioMcpError> {
    match (region_arg, filters.has_structured_filters()) {
        (None, false) => Ok(DrugRegion::All),
        (None, true) | (Some(crate::cli::DrugRegionArg::Us), _) => Ok(DrugRegion::Us),
        (Some(crate::cli::DrugRegionArg::Who), _) => Ok(DrugRegion::Who),
        (Some(crate::cli::DrugRegionArg::Eu), false) => Ok(DrugRegion::Eu),
        (Some(crate::cli::DrugRegionArg::All), false) => Ok(DrugRegion::All),
        (Some(crate::cli::DrugRegionArg::Eu | crate::cli::DrugRegionArg::All), true) => {
            Err(crate::error::BioMcpError::InvalidArgument(
                DRUG_SEARCH_EMA_STRUCTURED_FILTER_ERROR.into(),
            ))
        }
    }
}

pub(super) fn resolve_drug_get_region(
    sections: &[String],
    region: Option<DrugRegion>,
) -> DrugRegion {
    if let Some(region) = region {
        return region;
    }

    if matches!(sections, [section] if section.eq_ignore_ascii_case("regulatory")) {
        DrugRegion::All
    } else {
        DrugRegion::Us
    }
}

pub(super) async fn render_drug_card_outcome(
    name: &str,
    sections: &[String],
    region: Option<DrugRegion>,
    raw_label: bool,
    json_output: bool,
    alias_suggestions_as_json: bool,
) -> anyhow::Result<CommandOutcome> {
    let effective_region = resolve_drug_get_region(sections, region);
    match crate::entities::drug::get_with_region(
        name,
        sections,
        effective_region,
        region.is_some(),
        raw_label,
    )
    .await
    {
        Ok(drug) => {
            let text = if json_output {
                crate::render::json::to_entity_json(
                    &drug,
                    crate::render::markdown::drug_evidence_urls(&drug),
                    crate::render::markdown::related_drug(&drug),
                    crate::render::provenance::drug_section_sources(&drug),
                )?
            } else {
                crate::render::markdown::drug_markdown_with_region(
                    &drug,
                    sections,
                    effective_region,
                    raw_label,
                )?
            };
            Ok(CommandOutcome::stdout(text))
        }
        Err(err @ crate::error::BioMcpError::NotFound { .. }) => {
            if let Some(outcome) = super::super::try_alias_fallback_outcome(
                name,
                crate::entities::discover::DiscoverType::Drug,
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

#[derive(serde::Serialize)]
pub(super) struct RegionResults<T: serde::Serialize> {
    count: usize,
    total: Option<usize>,
    results: Vec<T>,
}

#[derive(serde::Serialize)]
pub(super) struct DrugAllRegionSearchResponse<
    T: serde::Serialize,
    U: serde::Serialize,
    V: serde::Serialize,
> {
    region: &'static str,
    query: String,
    us: RegionResults<T>,
    eu: RegionResults<U>,
    who: RegionResults<V>,
    #[serde(skip_serializing_if = "Option::is_none")]
    _meta: Option<crate::cli::SearchJsonMeta>,
}

pub(super) fn to_region_results<T: serde::Serialize>(
    page: crate::entities::SearchPage<T>,
) -> RegionResults<T> {
    RegionResults {
        count: page.results.len(),
        total: page.total,
        results: page.results,
    }
}

pub(super) fn drug_all_region_search_json(
    query: &str,
    us: crate::entities::SearchPage<crate::entities::drug::DrugSearchResult>,
    eu: crate::entities::SearchPage<crate::entities::drug::EmaDrugSearchResult>,
    who: crate::entities::SearchPage<crate::entities::drug::WhoPrequalificationSearchResult>,
    next_commands: Vec<String>,
) -> anyhow::Result<String> {
    crate::render::json::to_pretty(&DrugAllRegionSearchResponse {
        region: crate::entities::drug::DrugRegion::All.as_str(),
        query: query.to_string(),
        us: to_region_results(us),
        eu: to_region_results(eu),
        who: to_region_results(who),
        _meta: crate::cli::search_meta(next_commands),
    })
    .map_err(Into::into)
}
