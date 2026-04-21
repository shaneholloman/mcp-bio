use super::{DiagnosticGetArgs, DiagnosticSearchArgs};
use crate::cli::CommandOutcome;

pub(in crate::cli) async fn handle_get(
    args: DiagnosticGetArgs,
    json: bool,
) -> anyhow::Result<CommandOutcome> {
    let (sections, json_override) = super::super::extract_json_from_sections(&args.sections);
    let json_output = json || json_override;
    let diagnostic = crate::entities::diagnostic::get(&args.accession, &sections).await?;
    let text = if json_output {
        crate::render::json::to_entity_json(
            &diagnostic,
            crate::render::markdown::diagnostic_evidence_urls(&diagnostic),
            crate::render::markdown::diagnostic_next_commands(&diagnostic, &sections),
            crate::render::provenance::diagnostic_section_sources(&diagnostic),
        )?
    } else {
        crate::render::markdown::diagnostic_markdown(&diagnostic, &sections)?
    };
    Ok(CommandOutcome::stdout(text))
}

pub(in crate::cli) async fn handle_search(
    args: DiagnosticSearchArgs,
    json: bool,
) -> anyhow::Result<CommandOutcome> {
    let filters = crate::entities::diagnostic::DiagnosticSearchFilters {
        source: args.source.into(),
        gene: args.gene,
        disease: args.disease,
        test_type: args.test_type,
        manufacturer: args.manufacturer,
    };
    let mut query_summary = crate::entities::diagnostic::search_query_summary(&filters);
    if args.offset > 0 {
        query_summary = if query_summary.is_empty() {
            format!("offset={}", args.offset)
        } else {
            format!("{query_summary}, offset={}", args.offset)
        };
    }
    let page = crate::entities::diagnostic::search_page(&filters, args.limit, args.offset).await?;
    let total = page.total;
    let results = page.results;
    let true_zero_result = results.is_empty() && total == Some(0);
    let pagination =
        super::super::PaginationMeta::offset(args.offset, args.limit, results.len(), total);
    let text = if json {
        let next_commands = crate::render::markdown::search_next_commands_diagnostic(&results);
        let suggestions = true_zero_result
            .then(crate::render::markdown::diagnostic_zero_result_recovery_commands);
        super::super::search_json_with_meta_and_suggestions(
            results,
            pagination,
            next_commands,
            suggestions,
        )?
    } else {
        let footer = super::super::pagination_footer_offset(&pagination);
        crate::render::markdown::diagnostic_search_markdown_with_footer(
            &query_summary,
            &results,
            total,
            &footer,
        )?
    };
    Ok(CommandOutcome::stdout(text))
}
