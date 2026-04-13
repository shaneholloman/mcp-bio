//! Top-level CLI parsing and command execution.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use futures::StreamExt;
use tracing::warn;

use crate::cli::debug_plan::{DebugPlan, DebugPlanLeg};
use crate::entities::drug::DrugRegion;

mod adverse_event;
mod article;
pub mod cache;
pub mod chart;
mod commands;
pub mod debug_plan;
pub mod discover;
mod disease;
mod drug;
mod gene;
mod gwas;
pub mod health;
pub mod list;
mod outcome;
mod pathway;
mod pgx;
mod phenotype;
mod protein;
pub mod search_all;
mod search_all_command;
mod shared;
pub mod skill;
mod study;
mod system;
#[cfg(test)]
mod test_support;
mod trial;
mod types;
pub mod update;
mod variant;

pub use self::article::ArticleCommand;
pub use self::commands::{Commands, GetEntity, SearchEntity};
pub use self::disease::DiseaseCommand;
pub use self::drug::DrugCommand;
pub use self::gene::GeneCommand;
#[cfg(test)]
use self::outcome::{McpChartPass, rewrite_mcp_chart_args};
pub use self::outcome::{execute, execute_mcp, run, run_outcome};
pub use self::pathway::PathwayCommand;
pub use self::protein::ProteinCommand;
pub use self::shared::{build_cli, parse_cli_from_env};
pub use self::study::StudyCommand;
pub use self::system::{EmaCommand, WhoCommand};
pub use self::types::{
    ChartArgs, ChartType, Cli, CliOutput, CommandOutcome, DrugRegionArg, OutputStream,
};
pub use self::variant::VariantCommand;

#[cfg(test)]
use self::shared::RUNTIME_HELP_SUBCOMMANDS;
use self::shared::{
    PaginationMeta, empty_sections, extract_json_from_sections, log_pagination_truncation,
    normalize_cli_query, normalize_cli_tokens, paged_fetch_limit, paginate_results,
    pagination_footer_cursor, pagination_footer_offset, related_article_filters, render_batch_json,
    resolve_query_input, search_json, try_alias_fallback_outcome,
};

const DRUG_SEARCH_EMA_STRUCTURED_FILTER_ERROR: &str = "EMA and all-region search currently support name/alias lookups only; use --region us for structured MyChem filters or --region who to filter structured U.S. hits through WHO prequalification.";

fn parse_batch_sections(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn parse_expression_filter(
    value: &str,
    flag: &str,
    make_criterion: impl FnOnce(String, f64) -> crate::entities::study::FilterCriterion,
) -> Result<crate::entities::study::FilterCriterion, crate::error::BioMcpError> {
    let trimmed = value.trim();
    let invalid = || {
        crate::error::BioMcpError::InvalidArgument(format!(
            "Invalid value '{trimmed}' for {flag}. Expected GENE:THRESHOLD."
        ))
    };

    let (gene, threshold) = trimmed.split_once(':').ok_or_else(invalid)?;
    let gene = gene.trim();
    let threshold = threshold.trim();
    if gene.is_empty() || threshold.is_empty() {
        return Err(invalid());
    }
    let threshold = threshold.parse::<f64>().map_err(|_| invalid())?;
    Ok(make_criterion(gene.to_string(), threshold))
}

fn chart_json_conflict(
    chart: &ChartArgs,
    json_output: bool,
) -> Result<(), crate::error::BioMcpError> {
    if json_output && chart.chart.is_some() {
        return Err(crate::error::BioMcpError::InvalidArgument(
            "--json cannot be combined with --chart. Use standard study output for JSON, or remove --json for chart rendering.".into(),
        ));
    }
    Ok(())
}

fn resolve_drug_search_region(
    region_arg: Option<DrugRegionArg>,
    filters: &crate::entities::drug::DrugSearchFilters,
) -> Result<DrugRegion, crate::error::BioMcpError> {
    match (region_arg, filters.has_structured_filters()) {
        (None, false) => Ok(DrugRegion::All),
        (None, true) | (Some(DrugRegionArg::Us), _) => Ok(DrugRegion::Us),
        (Some(DrugRegionArg::Who), _) => Ok(DrugRegion::Who),
        (Some(DrugRegionArg::Eu), false) => Ok(DrugRegion::Eu),
        (Some(DrugRegionArg::All), false) => Ok(DrugRegion::All),
        (Some(DrugRegionArg::Eu | DrugRegionArg::All), true) => {
            Err(crate::error::BioMcpError::InvalidArgument(
                DRUG_SEARCH_EMA_STRUCTURED_FILTER_ERROR.into(),
            ))
        }
    }
}

async fn render_gene_card_outcome(
    symbol: &str,
    sections: &[String],
    json_output: bool,
    alias_suggestions_as_json: bool,
) -> anyhow::Result<CommandOutcome> {
    match crate::entities::gene::get(symbol, sections).await {
        Ok(gene) => {
            let text = if json_output {
                crate::render::json::to_entity_json(
                    &gene,
                    crate::render::markdown::gene_evidence_urls(&gene),
                    crate::render::markdown::related_gene(&gene),
                    crate::render::provenance::gene_section_sources(&gene),
                )?
            } else {
                crate::render::markdown::gene_markdown(&gene, sections)?
            };
            Ok(CommandOutcome::stdout(text))
        }
        Err(err @ crate::error::BioMcpError::NotFound { .. }) => {
            if let Some(outcome) = try_alias_fallback_outcome(
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

async fn render_drug_card_outcome(
    name: &str,
    sections: &[String],
    region: Option<DrugRegion>,
    raw_label: bool,
    json_output: bool,
    alias_suggestions_as_json: bool,
) -> anyhow::Result<CommandOutcome> {
    let effective_region = region.unwrap_or(DrugRegion::Us);
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
            if let Some(outcome) = try_alias_fallback_outcome(
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
struct RegionResults<T: serde::Serialize> {
    count: usize,
    total: Option<usize>,
    results: Vec<T>,
}

#[derive(serde::Serialize)]
struct DrugAllRegionSearchResponse<T: serde::Serialize, U: serde::Serialize, V: serde::Serialize> {
    region: &'static str,
    query: String,
    us: RegionResults<T>,
    eu: RegionResults<U>,
    who: RegionResults<V>,
}

#[derive(serde::Serialize)]
struct DiseaseSearchMeta {
    fallback_used: bool,
}

#[derive(serde::Serialize)]
struct DiseaseSearchJsonResponse<T: serde::Serialize> {
    pagination: PaginationMeta,
    count: usize,
    results: Vec<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    _meta: Option<DiseaseSearchMeta>,
}

fn to_region_results<T: serde::Serialize>(
    page: crate::entities::SearchPage<T>,
) -> RegionResults<T> {
    RegionResults {
        count: page.results.len(),
        total: page.total,
        results: page.results,
    }
}

fn drug_all_region_search_json(
    query: &str,
    us: crate::entities::SearchPage<crate::entities::drug::DrugSearchResult>,
    eu: crate::entities::SearchPage<crate::entities::drug::EmaDrugSearchResult>,
    who: crate::entities::SearchPage<crate::entities::drug::WhoPrequalificationSearchResult>,
) -> anyhow::Result<String> {
    crate::render::json::to_pretty(&DrugAllRegionSearchResponse {
        region: crate::entities::drug::DrugRegion::All.as_str(),
        query: query.to_string(),
        us: to_region_results(us),
        eu: to_region_results(eu),
        who: to_region_results(who),
    })
    .map_err(Into::into)
}

fn disease_search_json(
    results: Vec<crate::entities::disease::DiseaseSearchResult>,
    pagination: PaginationMeta,
    fallback_used: bool,
) -> anyhow::Result<String> {
    let count = results.len();
    crate::render::json::to_pretty(&DiseaseSearchJsonResponse {
        pagination,
        count,
        results,
        _meta: fallback_used.then_some(DiseaseSearchMeta { fallback_used }),
    })
    .map_err(Into::into)
}

fn article_query_summary(
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

fn article_max_per_source_summary(max_per_source: Option<usize>, limit: usize) -> Option<String> {
    match max_per_source {
        None => None,
        Some(0) => Some("max_per_source=default".to_string()),
        Some(value) if value == limit => Some("max_per_source=disabled".to_string()),
        Some(value) => Some(format!("max_per_source={value}")),
    }
}

fn article_debug_filters(
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

fn build_article_debug_plan(
    query: &str,
    filters: &crate::entities::article::ArticleSearchFilters,
    source_filter: crate::entities::article::ArticleSourceFilter,
    limit: usize,
    results: &[crate::entities::article::ArticleSearchResult],
    pagination: &PaginationMeta,
) -> Result<DebugPlan, crate::error::BioMcpError> {
    let summary = crate::entities::article::summarize_debug_plan(filters, source_filter, results)?;
    Ok(DebugPlan {
        surface: "search_article",
        query: query.to_string(),
        anchor: None,
        legs: vec![DebugPlanLeg {
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

fn article_search_json(
    query: &str,
    filters: &crate::entities::article::ArticleSearchFilters,
    semantic_scholar_enabled: bool,
    note: Option<String>,
    debug_plan: Option<DebugPlan>,
    results: Vec<crate::entities::article::ArticleSearchResult>,
    pagination: PaginationMeta,
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
        pagination: PaginationMeta,
        count: usize,
        results: Vec<crate::entities::article::ArticleSearchResult>,
        #[serde(skip_serializing_if = "Option::is_none")]
        debug_plan: Option<DebugPlan>,
    }

    let count = results.len();
    crate::render::json::to_pretty(&ArticleSearchResponse {
        query: query.to_string(),
        sort: filters.sort.as_str().to_string(),
        semantic_scholar_enabled,
        ranking_policy: crate::entities::article::article_relevance_ranking_policy(filters),
        note,
        pagination,
        count,
        results,
        debug_plan,
    })
    .map_err(Into::into)
}

fn truncate_article_annotations(
    mut annotations: crate::entities::article::ArticleAnnotations,
    limit: usize,
) -> crate::entities::article::ArticleAnnotations {
    annotations.genes.truncate(limit);
    annotations.diseases.truncate(limit);
    annotations.chemicals.truncate(limit);
    annotations.mutations.truncate(limit);
    annotations
}

fn version_output(verbose: bool) -> String {
    let cargo_version = env!("CARGO_PKG_VERSION");
    let git_tag = option_env!("BIOMCP_BUILD_GIT_TAG");
    let git = option_env!("BIOMCP_BUILD_GIT_SHA").unwrap_or("unknown");
    let build = option_env!("BIOMCP_BUILD_DATE").unwrap_or("unknown");
    let version = git_tag
        .filter(|t| t.starts_with('v') && !t.contains('-'))
        .map(|t| &t[1..])
        .unwrap_or(cargo_version);
    let base = format!("biomcp {version} (git {git}, build {build})");
    if !verbose {
        return base;
    }

    let executable = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let path_hits = find_biomcp_on_path();
    let active = std::env::current_exe()
        .ok()
        .as_deref()
        .and_then(canonical_for_compare);
    let mut out = Vec::new();
    out.push(base);
    out.push(format!("Executable: {executable}"));
    out.push(format!("Build: version={version}, git={git}, date={build}"));
    out.push("PATH:".to_string());
    if path_hits.is_empty() {
        out.push("- (no biomcp binaries found on PATH)".to_string());
    } else {
        for hit in &path_hits {
            let canonical = canonical_for_compare(hit);
            let marker = if active.is_some() && active == canonical {
                " (active)"
            } else {
                ""
            };
            out.push(format!("- {}{}", hit.display(), marker));
        }
    }
    if executable.contains("/.venv/") || executable.contains("\\.venv\\") {
        out.push("Warning: active executable appears to come from a virtualenv path.".to_string());
    }
    if path_hits.len() > 1 {
        out.push(format!(
            "Warning: multiple biomcp binaries found on PATH ({}).",
            path_hits.len()
        ));
    }
    out.join("\n")
}

fn find_biomcp_on_path() -> Vec<PathBuf> {
    #[cfg(windows)]
    let binary_name = "biomcp.exe";
    #[cfg(not(windows))]
    let binary_name = "biomcp";

    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let Some(path_var) = std::env::var_os("PATH") else {
        return out;
    };
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(binary_name);
        if !candidate.is_file() {
            continue;
        }
        let canonical = canonical_for_compare(&candidate);
        let key = canonical
            .as_deref()
            .unwrap_or(candidate.as_path())
            .display()
            .to_string();
        if seen.insert(key) {
            out.push(candidate);
        }
    }
    out
}

fn canonical_for_compare(path: &Path) -> Option<PathBuf> {
    std::fs::canonicalize(path).ok()
}

fn should_try_pathway_trial_fallback(
    results_len: usize,
    offset: usize,
    total: Option<u32>,
) -> bool {
    if results_len != 0 || offset > 0 {
        return false;
    }
    total.is_none_or(|value| value == 0)
}

async fn pathway_drug_results(
    id: &str,
    fetch_limit: usize,
) -> Result<Vec<crate::entities::drug::DrugSearchResult>, crate::error::BioMcpError> {
    let sections = vec!["genes".to_string()];
    let pathway = crate::entities::pathway::get(id, &sections).await?;

    let search_limit = fetch_limit.clamp(1, 10);
    let mut stream = futures::stream::iter(pathway.genes.into_iter().map(|gene| async move {
        let filters = crate::entities::drug::DrugSearchFilters {
            target: Some(gene.clone()),
            ..Default::default()
        };
        let result = crate::entities::drug::search(&filters, search_limit).await;
        (gene, result)
    }))
    .buffer_unordered(5);

    let mut results: Vec<Vec<crate::entities::drug::DrugSearchResult>> = Vec::new();
    let mut attempted: usize = 0;
    let mut failures: usize = 0;
    while let Some((gene, next)) = stream.next().await {
        attempted += 1;
        match next {
            Ok(rows) => results.push(rows),
            Err(err) => {
                failures += 1;
                warn!(gene = %gene, "pathway drug lookup failed: {err}");
            }
        }
    }

    if attempted > 0 && failures.saturating_mul(2) > attempted {
        return Err(crate::error::BioMcpError::Api {
            api: "pathway-drugs".into(),
            message: format!(
                "Failed to resolve {failures} of {attempted} pathway gene target lookups while collecting drugs"
            ),
        });
    }

    let mut out: Vec<crate::entities::drug::DrugSearchResult> = Vec::new();
    for rows in results {
        for row in rows {
            if out.iter().any(|v| v.name.eq_ignore_ascii_case(&row.name)) {
                continue;
            }
            out.push(row);
            if out.len() >= fetch_limit {
                return Ok(out);
            }
        }
    }

    Ok(out)
}

fn uninstall_self() -> Result<String, crate::error::BioMcpError> {
    let current = std::env::current_exe()?;
    match std::fs::remove_file(&current) {
        Ok(()) => Ok(format!("Uninstalled biomcp from {}", current.display())),
        Err(err) => Ok(format!(
            "Unable to remove running binary automatically ({err}).\nRemove manually:\n  rm {}",
            current.display()
        )),
    }
}

fn enrich_markdown(genes: &[String], terms: &[crate::sources::gprofiler::GProfilerTerm]) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Enrichment: {}\n\n", genes.join(", ")));
    if terms.is_empty() {
        out.push_str("No enriched terms found.\n");
        return out;
    }

    out.push_str("| Source | ID | Name | p-value |\n");
    out.push_str("|--------|----|------|---------|\n");
    for row in terms {
        let source = row.source.as_deref().unwrap_or("-");
        let id = row.native.as_deref().unwrap_or("-");
        let name = row.name.as_deref().unwrap_or("-");
        let p = row
            .p_value
            .map(|v| format!("{v:.3e}"))
            .unwrap_or_else(|| "-".to_string());
        out.push_str(&format!("| {source} | {id} | {name} | {p} |\n"));
    }
    out
}

#[cfg(test)]
mod tests;
