use super::{AdverseEventGetArgs, AdverseEventSearchArgs};
use crate::cli::CommandOutcome;

pub(crate) async fn handle_get(
    args: AdverseEventGetArgs,
    json: bool,
) -> anyhow::Result<CommandOutcome> {
    let (sections, json_override) = super::super::extract_json_from_sections(&args.sections);
    let json_output = json || json_override;
    let event = crate::entities::adverse_event::get(&args.report_id).await?;
    let text = if json_output {
        match &event {
            crate::entities::adverse_event::AdverseEventReport::Faers(report) => {
                crate::render::json::to_entity_json(
                    &event,
                    crate::render::markdown::adverse_event_evidence_urls(report),
                    crate::render::markdown::related_adverse_event(report),
                    crate::render::provenance::adverse_event_report_section_sources(&event),
                )?
            }
            crate::entities::adverse_event::AdverseEventReport::Device(report) => {
                crate::render::json::to_entity_json(
                    &event,
                    crate::render::markdown::device_event_evidence_urls(report),
                    crate::render::markdown::related_device_event(report),
                    crate::render::provenance::adverse_event_report_section_sources(&event),
                )?
            }
        }
    } else {
        match &event {
            crate::entities::adverse_event::AdverseEventReport::Faers(report) => {
                crate::render::markdown::adverse_event_markdown(report, &sections)?
            }
            crate::entities::adverse_event::AdverseEventReport::Device(report) => {
                crate::render::markdown::device_event_markdown(report)?
            }
        }
    };
    Ok(CommandOutcome::stdout(text))
}

pub(crate) async fn handle_search(
    args: AdverseEventSearchArgs,
    json: bool,
) -> anyhow::Result<CommandOutcome> {
    let drug = super::super::resolve_query_input(args.drug, args.positional_query, "--drug")?;
    let query_type =
        crate::entities::adverse_event::AdverseEventQueryType::from_flag(&args.r#type)?;

    let text = match query_type {
        crate::entities::adverse_event::AdverseEventQueryType::Faers => {
            if args.device.is_some() {
                return Err(crate::error::BioMcpError::InvalidArgument(
                    "--device can only be used with --type device".into(),
                )
                .into());
            }
            if args.manufacturer.is_some() {
                return Err(crate::error::BioMcpError::InvalidArgument(
                    "--manufacturer can only be used with --type device".into(),
                )
                .into());
            }
            if args.product_code.is_some() {
                return Err(crate::error::BioMcpError::InvalidArgument(
                    "--product-code can only be used with --type device".into(),
                )
                .into());
            }

            let filters = crate::entities::adverse_event::AdverseEventSearchFilters {
                drug,
                reaction: args.reaction,
                outcome: args.outcome,
                serious: args.serious,
                since: args.date_from,
                date_to: args.date_to,
                suspect_only: args.suspect_only,
                sex: args.sex,
                age_min: args.age_min,
                age_max: args.age_max,
                reporter: args.reporter,
            };
            let mut query_summary = crate::entities::adverse_event::search_query_summary(&filters);
            if let Some(count_field) = args
                .count
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
            {
                if query_summary.is_empty() {
                    query_summary = format!("count={count_field}");
                } else {
                    query_summary = format!("{query_summary}, count={count_field}");
                }
            }
            if args.offset > 0 {
                query_summary = format!("{query_summary}, offset={}", args.offset);
            }

            if let Some(count_field) = args
                .count
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
            {
                let response =
                    crate::entities::adverse_event::search_count(&filters, count_field, args.limit)
                        .await?;
                if json {
                    #[derive(serde::Serialize)]
                    struct CountResponse {
                        query: String,
                        count_field: String,
                        buckets: Vec<crate::entities::adverse_event::AdverseEventCountBucket>,
                    }

                    crate::render::json::to_pretty(&CountResponse {
                        query: query_summary,
                        count_field: response.count_field,
                        buckets: response.buckets,
                    })?
                } else {
                    crate::render::markdown::adverse_event_count_markdown(
                        &query_summary,
                        &response.count_field,
                        &response.buckets,
                    )?
                }
            } else {
                let response = crate::entities::adverse_event::search_with_summary(
                    &filters,
                    args.limit,
                    args.offset,
                )
                .await?;
                let summary = response.summary;
                let results = response.results;
                let pagination = super::super::PaginationMeta::offset(
                    args.offset,
                    args.limit,
                    results.len(),
                    Some(summary.total_reports),
                );
                if json {
                    #[derive(serde::Serialize)]
                    struct SearchResponse {
                        pagination: super::super::PaginationMeta,
                        count: usize,
                        summary: crate::entities::adverse_event::AdverseEventSearchSummary,
                        results: Vec<crate::entities::adverse_event::AdverseEventSearchResult>,
                        #[serde(skip_serializing_if = "Option::is_none")]
                        _meta: Option<crate::cli::SearchJsonMeta>,
                    }

                    let next_commands =
                        crate::render::markdown::search_next_commands_faers(&results);
                    crate::render::json::to_pretty(&SearchResponse {
                        pagination,
                        count: results.len(),
                        summary,
                        results,
                        _meta: crate::cli::search_meta(next_commands),
                    })?
                } else {
                    let footer = super::super::pagination_footer_offset(&pagination);
                    crate::render::markdown::adverse_event_search_markdown_with_footer(
                        &query_summary,
                        &results,
                        &summary,
                        &footer,
                    )?
                }
            }
        }
        crate::entities::adverse_event::AdverseEventQueryType::Recall => {
            if args.date_from.is_some()
                || args.date_to.is_some()
                || args.suspect_only
                || args.sex.is_some()
                || args.age_min.is_some()
                || args.age_max.is_some()
                || args.reporter.is_some()
                || args.count.is_some()
            {
                return Err(crate::error::BioMcpError::InvalidArgument(
                    "--date-from/--date-to/--suspect-only/--sex/--age-min/--age-max/--reporter/--count are only valid for --type faers".into(),
                )
                .into());
            }
            if args.device.is_some() {
                return Err(crate::error::BioMcpError::InvalidArgument(
                    "--device can only be used with --type device".into(),
                )
                .into());
            }
            if args.manufacturer.is_some() {
                return Err(crate::error::BioMcpError::InvalidArgument(
                    "--manufacturer can only be used with --type device".into(),
                )
                .into());
            }
            if args.product_code.is_some() {
                return Err(crate::error::BioMcpError::InvalidArgument(
                    "--product-code can only be used with --type device".into(),
                )
                .into());
            }
            if args.outcome.is_some() {
                return Err(crate::error::BioMcpError::InvalidArgument(
                    "--outcome is only valid for --type faers".into(),
                )
                .into());
            }
            let filters = crate::entities::adverse_event::RecallSearchFilters {
                drug,
                classification: args.classification,
            };
            let mut query_summary = crate::entities::adverse_event::recall_query_summary(&filters);
            if args.offset > 0 {
                query_summary = format!("{query_summary}, offset={}", args.offset);
            }
            let page = crate::entities::adverse_event::search_recalls_page(
                &filters,
                args.limit,
                args.offset,
            )
            .await?;
            let results = page.results;
            let pagination = super::super::PaginationMeta::offset(
                args.offset,
                args.limit,
                results.len(),
                page.total,
            );
            if json {
                let next_commands = crate::render::markdown::search_next_commands_recalls(&results);
                return super::super::search_json_with_meta(results, pagination, next_commands)
                    .map(CommandOutcome::stdout);
            }
            let footer = super::super::pagination_footer_offset(&pagination);
            crate::render::markdown::recall_search_markdown_with_footer(
                &query_summary,
                &results,
                &footer,
            )?
        }
        crate::entities::adverse_event::AdverseEventQueryType::Device => {
            if drug.is_some() {
                return Err(crate::error::BioMcpError::InvalidArgument(
                    "--drug cannot be used with --type device (use --device)".into(),
                )
                .into());
            }
            if args.reaction.is_some() {
                return Err(crate::error::BioMcpError::InvalidArgument(
                    "--reaction is not supported with --type device".into(),
                )
                .into());
            }
            if args.outcome.is_some() {
                return Err(crate::error::BioMcpError::InvalidArgument(
                    "--outcome is only valid for --type faers".into(),
                )
                .into());
            }
            if args.classification.is_some() {
                return Err(crate::error::BioMcpError::InvalidArgument(
                    "--classification is only valid for --type recall".into(),
                )
                .into());
            }
            if args.date_to.is_some()
                || args.suspect_only
                || args.sex.is_some()
                || args.age_min.is_some()
                || args.age_max.is_some()
                || args.reporter.is_some()
                || args.count.is_some()
            {
                return Err(crate::error::BioMcpError::InvalidArgument(
                    "--date-to/--suspect-only/--sex/--age-min/--age-max/--reporter/--count are only valid for --type faers".into(),
                )
                .into());
            }

            let filters = crate::entities::adverse_event::DeviceEventSearchFilters {
                device: args.device,
                manufacturer: args.manufacturer,
                product_code: args.product_code,
                serious: args.serious.is_some(),
                since: args.date_from,
            };
            let mut query_summary = crate::entities::adverse_event::device_query_summary(&filters);
            if args.offset > 0 {
                query_summary = format!("{query_summary}, offset={}", args.offset);
            }
            let page = crate::entities::adverse_event::search_device_page(
                &filters,
                args.limit,
                args.offset,
            )
            .await?;
            let results = page.results;
            let pagination = super::super::PaginationMeta::offset(
                args.offset,
                args.limit,
                results.len(),
                page.total,
            );
            if json {
                let next_commands =
                    crate::render::markdown::search_next_commands_device_events(&results);
                return super::super::search_json_with_meta(results, pagination, next_commands)
                    .map(CommandOutcome::stdout);
            }
            let footer = super::super::pagination_footer_offset(&pagination);
            crate::render::markdown::device_event_search_markdown_with_footer(
                &query_summary,
                &results,
                &footer,
            )?
        }
    };

    Ok(CommandOutcome::stdout(text))
}
