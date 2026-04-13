use super::{PgxGetArgs, PgxSearchArgs};
use crate::cli::CommandOutcome;

pub(in crate::cli) async fn handle_get(
    args: PgxGetArgs,
    json: bool,
) -> anyhow::Result<CommandOutcome> {
    let (sections, json_override) = super::super::extract_json_from_sections(&args.sections);
    let json_output = json || json_override;
    let pgx = crate::entities::pgx::get(&args.query, &sections).await?;
    let text = if json_output {
        crate::render::json::to_entity_json(
            &pgx,
            crate::render::markdown::pgx_evidence_urls(&pgx),
            crate::render::markdown::related_pgx(&pgx),
            crate::render::provenance::pgx_section_sources(&pgx),
        )?
    } else {
        crate::render::markdown::pgx_markdown(&pgx, &sections)?
    };
    Ok(CommandOutcome::stdout(text))
}

pub(in crate::cli) async fn handle_search(
    args: PgxSearchArgs,
    json: bool,
) -> anyhow::Result<CommandOutcome> {
    let gene = super::super::resolve_query_input(args.gene, args.positional_query, "--gene")?;
    let filters = crate::entities::pgx::PgxSearchFilters {
        gene,
        drug: args.drug,
        cpic_level: args.cpic_level,
        pgx_testing: args.pgx_testing,
        evidence: args.evidence,
    };
    let mut query_summary = crate::entities::pgx::search_query_summary(&filters);
    if args.offset > 0 {
        query_summary = format!("{query_summary}, offset={}", args.offset);
    }
    let page = crate::entities::pgx::search_page(&filters, args.limit, args.offset).await?;
    let results = page.results;
    let pagination =
        super::super::PaginationMeta::offset(args.offset, args.limit, results.len(), page.total);
    let text = if json {
        super::super::search_json(results, pagination)?
    } else {
        let footer = super::super::pagination_footer_offset(&pagination);
        crate::render::markdown::pgx_search_markdown_with_footer(&query_summary, &results, &footer)?
    };
    Ok(CommandOutcome::stdout(text))
}
