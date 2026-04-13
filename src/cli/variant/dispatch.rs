use super::{VariantCommand, VariantGetArgs, VariantSearchArgs};
use crate::cli::CommandOutcome;

pub(crate) async fn handle_get(
    args: VariantGetArgs,
    json: bool,
    alias_suggestions_as_json: bool,
) -> anyhow::Result<CommandOutcome> {
    let (sections, json_override) = super::super::extract_json_from_sections(&args.sections);
    let json_output = json || json_override;
    super::super::render_variant_card_outcome(
        &args.id,
        &sections,
        json_output,
        alias_suggestions_as_json,
    )
    .await
}

pub(crate) async fn handle_search(
    args: VariantSearchArgs,
    json: bool,
    alias_suggestions_as_json: bool,
) -> anyhow::Result<CommandOutcome> {
    super::super::render_variant_search_outcome(
        json,
        alias_suggestions_as_json,
        super::super::VariantSearchRequest {
            gene: args.gene,
            positional_query: args.positional_query,
            hgvsp: args.hgvsp,
            significance: args.significance,
            max_frequency: args.max_frequency,
            min_cadd: args.min_cadd,
            consequence: args.consequence,
            review_status: args.review_status,
            population: args.population,
            revel_min: args.revel_min,
            gerp_min: args.gerp_min,
            tumor_site: args.tumor_site,
            condition: args.condition,
            impact: args.impact,
            lof: args.lof,
            has: args.has,
            missing: args.missing,
            therapy: args.therapy,
            limit: args.limit,
            offset: args.offset,
        },
    )
    .await
}

pub(crate) async fn handle_command(
    cmd: VariantCommand,
    json: bool,
) -> anyhow::Result<CommandOutcome> {
    let text = match cmd {
        VariantCommand::Trials {
            id,
            limit,
            offset,
            source,
        } => {
            let _ = crate::entities::variant::parse_variant_id(&id)?;
            let mutation_query = super::super::variant_trial_mutation_query(&id).await;
            let trial_source = crate::entities::trial::TrialSource::from_flag(&source)?;
            let filters = crate::entities::trial::TrialSearchFilters {
                mutation: Some(mutation_query.clone()),
                source: trial_source,
                ..Default::default()
            };
            let (results, total) = crate::entities::trial::search(&filters, limit, offset).await?;
            if let Some(total) = total {
                super::super::log_pagination_truncation(total as usize, offset, results.len());
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
                let mut query_parts = vec![format!("mutation={mutation_query}")];
                if matches!(trial_source, crate::entities::trial::TrialSource::NciCts) {
                    query_parts.push("source=nci".to_string());
                }
                if offset > 0 {
                    query_parts.push(format!("offset={offset}"));
                }
                let query = query_parts.join(", ");
                crate::render::markdown::trial_search_markdown(&query, &results, total)?
            }
        }
        VariantCommand::Articles { id, limit, offset } => {
            let id_format = crate::entities::variant::parse_variant_id(&id)?;
            let (gene, keyword) = match id_format {
                crate::entities::variant::VariantIdFormat::RsId(rsid) => (None, Some(rsid)),
                crate::entities::variant::VariantIdFormat::HgvsGenomic(hgvs) => (None, Some(hgvs)),
                crate::entities::variant::VariantIdFormat::GeneProteinChange { gene, change } => {
                    (Some(gene), Some(change))
                }
            };

            let filters = crate::entities::article::ArticleSearchFilters {
                gene,
                gene_anchored: true,
                keyword,
                ..super::super::related_article_filters()
            };

            let query = vec![
                filters.gene.as_deref().map(|value| format!("gene={value}")),
                filters
                    .keyword
                    .as_deref()
                    .map(|value| format!("keyword={value}")),
                (offset > 0).then(|| format!("offset={offset}")),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(", ");

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
                    crate::entities::article::semantic_scholar_search_enabled(
                        &filters,
                        crate::entities::article::ArticleSourceFilter::All,
                    ),
                    None,
                    None,
                )?
            }
        }
        VariantCommand::Oncokb { id } => {
            let result = crate::entities::variant::oncokb(&id).await?;
            if json {
                crate::render::json::to_pretty(&result)?
            } else {
                crate::render::markdown::variant_oncokb_markdown(&result)
            }
        }
        VariantCommand::External(args) => {
            let id = args.join(" ");
            let variant =
                crate::entities::variant::get(&id, super::super::empty_sections()).await?;
            if json {
                crate::render::json::to_entity_json(
                    &variant,
                    crate::render::markdown::variant_evidence_urls(&variant),
                    crate::render::markdown::related_variant(&variant),
                    crate::render::provenance::variant_section_sources(&variant),
                )?
            } else {
                crate::render::markdown::variant_markdown(&variant, super::super::empty_sections())?
            }
        }
    };

    Ok(CommandOutcome::stdout(text))
}
