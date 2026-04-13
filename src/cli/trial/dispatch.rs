use super::{TrialGetArgs, TrialSearchArgs};
use crate::cli::CommandOutcome;

pub(in crate::cli) async fn handle_get(
    args: TrialGetArgs,
    json: bool,
) -> anyhow::Result<CommandOutcome> {
    let (sections, location_offset, location_limit) =
        super::super::parse_trial_location_paging(&args.sections)?;
    let (sections, json_override) = super::super::extract_json_from_sections(&sections);
    let json_output = json || json_override;
    let trial_source = crate::entities::trial::TrialSource::from_flag(&args.source)?;
    let includes_locations = sections
        .iter()
        .any(|section| section.trim().eq_ignore_ascii_case("locations"));
    if !includes_locations && (location_offset.is_some() || location_limit.is_some()) {
        return Err(crate::error::BioMcpError::InvalidArgument(
            "--offset and --limit are only valid with the 'locations' section".into(),
        )
        .into());
    }

    let mut trial = crate::entities::trial::get(&args.nct_id, &sections, trial_source).await?;
    let mut location_pagination = None;
    if includes_locations {
        let offset = location_offset.unwrap_or(0);
        let limit = location_limit.unwrap_or(20);
        location_pagination = Some(super::super::paginate_trial_locations(
            &mut trial, offset, limit,
        ));
    }

    let text = if json_output {
        if let Some(loc_page) = location_pagination {
            super::super::trial_locations_json(&trial, loc_page)?
        } else {
            crate::render::json::to_entity_json(
                &trial,
                crate::render::markdown::trial_evidence_urls(&trial),
                crate::render::markdown::related_trial(&trial),
                crate::render::provenance::trial_section_sources(&trial),
            )?
        }
    } else {
        let mut md = crate::render::markdown::trial_markdown(&trial, &sections)?;
        if let Some(loc_page) = location_pagination {
            md.push_str(&format!(
                "\n\n---\n*Locations: showing {} of {} (offset {}, limit {}{})*",
                trial.locations.as_ref().map_or(0, |value| value.len()),
                loc_page.total,
                loc_page.offset,
                loc_page.limit,
                if loc_page.has_more {
                    ", more available"
                } else {
                    ""
                },
            ));
        }
        md
    };

    Ok(CommandOutcome::stdout(text))
}

pub(in crate::cli) async fn handle_search(
    args: TrialSearchArgs,
    json: bool,
) -> anyhow::Result<CommandOutcome> {
    let positional_trial_query = args
        .positional_query
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let condition = super::super::resolve_query_input(
        super::super::normalize_cli_tokens(args.condition),
        args.positional_query,
        "--condition",
    )?;
    let intervention = super::super::normalize_cli_tokens(args.intervention);
    let facility = super::super::normalize_cli_tokens(args.facility);
    let mutation = super::super::normalize_cli_tokens(args.mutation);
    let criteria = super::super::normalize_cli_tokens(args.criteria);
    let biomarker = super::super::normalize_cli_tokens(args.biomarker);
    let prior_therapies = super::super::normalize_cli_tokens(args.prior_therapies);
    let progression_on = super::super::normalize_cli_tokens(args.progression_on);
    let sponsor = super::super::normalize_cli_tokens(args.sponsor);
    let trial_source = crate::entities::trial::TrialSource::from_flag(&args.source)?;
    let filters = crate::entities::trial::TrialSearchFilters {
        condition,
        intervention,
        facility,
        status: args.status,
        phase: args.phase,
        study_type: args.study_type,
        age: args.age,
        sex: args.sex,
        sponsor,
        sponsor_type: args.sponsor_type,
        date_from: args.date_from,
        date_to: args.date_to,
        mutation,
        criteria,
        biomarker,
        prior_therapies,
        progression_on,
        line_of_therapy: args.line_of_therapy,
        lat: args.lat,
        lon: args.lon,
        distance: args.distance,
        results_available: args.results_available,
        source: trial_source,
    };

    if args
        .next_page
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
        && args.offset > 0
    {
        return Err(crate::error::BioMcpError::InvalidArgument(
            "--next-page cannot be used together with --offset".into(),
        )
        .into());
    }

    let query =
        super::super::trial_search_query_summary(&filters, args.offset, args.next_page.as_deref());
    let text = if args.count_only {
        let count = crate::entities::trial::count_all(&filters).await?;
        if json {
            use crate::entities::trial::TrialCount;

            #[derive(serde::Serialize)]
            struct TrialCountOnlyJson {
                total: Option<usize>,
                #[serde(skip_serializing_if = "Option::is_none")]
                approximate: Option<bool>,
            }

            let (total, approximate) = match count {
                TrialCount::Exact(total) => (Some(total), None),
                TrialCount::Approximate(total) => (Some(total), Some(true)),
                TrialCount::Unknown => (None, None),
            };
            crate::render::json::to_pretty(&TrialCountOnlyJson { total, approximate })?
        } else {
            match count {
                crate::entities::trial::TrialCount::Exact(total) => format!("Total: {total}"),
                crate::entities::trial::TrialCount::Approximate(total) => {
                    format!("Total: {total} (approximate, age post-filtered)")
                }
                crate::entities::trial::TrialCount::Unknown => {
                    "Total: unknown (traversal limit reached)".to_string()
                }
            }
        }
    } else {
        let page =
            crate::entities::trial::search_page(&filters, args.limit, args.offset, args.next_page)
                .await?;
        let results = page.results;
        let pagination = super::super::PaginationMeta::cursor(
            args.offset,
            args.limit,
            results.len(),
            page.total,
            page.next_page_token,
        );
        if json {
            return super::super::search_json(results, pagination).map(CommandOutcome::stdout);
        }

        let footer = if matches!(
            trial_source,
            crate::entities::trial::TrialSource::ClinicalTrialsGov
        ) {
            super::super::pagination_footer_cursor(&pagination)
        } else {
            super::super::pagination_footer_offset(&pagination)
        };
        let total = pagination.total.and_then(|value| u32::try_from(value).ok());
        let show_zero_result_nickname_hint =
            super::super::should_show_trial_zero_result_nickname_hint(
                positional_trial_query.as_deref(),
                trial_source,
                results.len(),
            );
        crate::render::markdown::trial_search_markdown_with_footer(
            &query,
            &results,
            total,
            &footer,
            show_zero_result_nickname_hint,
            positional_trial_query.as_deref(),
        )?
    };

    Ok(CommandOutcome::stdout(text))
}
