//! Top-level CLI parsing and command execution.

use std::collections::HashSet;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use clap::{CommandFactory, FromArgMatches, Parser};
use futures::{StreamExt, future::try_join_all};
use tracing::{debug, warn};

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
pub use self::pathway::PathwayCommand;
pub use self::protein::ProteinCommand;
pub use self::study::StudyCommand;
pub use self::system::{EmaCommand, WhoCommand};
pub use self::types::{
    ChartArgs, ChartType, Cli, CliOutput, CommandOutcome, DrugRegionArg, OutputStream,
};
pub use self::variant::VariantCommand;

const DRUG_SEARCH_EMA_STRUCTURED_FILTER_ERROR: &str = "EMA and all-region search currently support name/alias lookups only; use --region us for structured MyChem filters or --region who to filter structured U.S. hits through WHO prequalification.";
const RUNTIME_HELP_SUBCOMMANDS: [&str; 4] = ["mcp", "serve", "serve-http", "serve-sse"];

fn hide_runtime_help_globals(
    command: clap::Command,
    subcommand_name: &'static str,
    json_arg: &clap::Arg,
    no_cache_arg: &clap::Arg,
) -> clap::Command {
    command.mut_subcommand(subcommand_name, |runtime| {
        runtime.arg(json_arg.clone()).arg(no_cache_arg.clone())
    })
}

pub fn build_cli() -> clap::Command {
    let mut command = Cli::command();
    let json_arg = command
        .get_arguments()
        .find(|arg| arg.get_id() == "json")
        .cloned()
        .expect("json arg should exist")
        .hide(true);
    let no_cache_arg = command
        .get_arguments()
        .find(|arg| arg.get_id() == "no_cache")
        .cloned()
        .expect("no_cache arg should exist")
        .hide(true);

    for subcommand_name in RUNTIME_HELP_SUBCOMMANDS {
        command = hide_runtime_help_globals(command, subcommand_name, &json_arg, &no_cache_arg);
    }
    command
}

pub fn parse_cli_from_env() -> Cli {
    let matches = build_cli().get_matches();
    Cli::from_arg_matches(&matches).unwrap_or_else(|err| err.exit())
}

fn empty_sections() -> &'static [String] {
    &[]
}

fn related_article_filters() -> crate::entities::article::ArticleSearchFilters {
    crate::entities::article::ArticleSearchFilters {
        gene: None,
        gene_anchored: false,
        disease: None,
        drug: None,
        author: None,
        keyword: None,
        date_from: None,
        date_to: None,
        article_type: None,
        journal: None,
        open_access: false,
        no_preprints: true,
        exclude_retracted: true,
        max_per_source: None,
        sort: crate::entities::article::ArticleSort::Relevance,
        ranking: crate::entities::article::ArticleRankingOptions::default(),
    }
}

fn parse_batch_sections(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn extract_json_from_sections(sections: &[String]) -> (Vec<String>, bool) {
    let mut json_override = false;
    let cleaned = sections
        .iter()
        .filter_map(|raw| {
            let trimmed = raw.trim();
            let normalized = trimmed.to_ascii_lowercase();
            if normalized == "--json" || normalized == "-j" {
                json_override = true;
                return None;
            }
            if trimmed.is_empty() {
                return None;
            }
            Some(trimmed.to_string())
        })
        .collect();
    (cleaned, json_override)
}

fn parse_usize_arg(flag: &str, value: &str) -> Result<usize, crate::error::BioMcpError> {
    value.parse::<usize>().map_err(|_| {
        crate::error::BioMcpError::InvalidArgument(format!("{flag} must be a non-negative integer"))
    })
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

type LocationPaging = (Vec<String>, Option<usize>, Option<usize>);

fn parse_trial_location_paging(
    sections: &[String],
) -> Result<LocationPaging, crate::error::BioMcpError> {
    let mut cleaned: Vec<String> = Vec::new();
    let mut location_offset: Option<usize> = None;
    let mut location_limit: Option<usize> = None;
    let mut i = 0usize;
    while i < sections.len() {
        let token = sections[i].trim();
        if token.is_empty() {
            i += 1;
            continue;
        }

        if let Some(value) = token.strip_prefix("--offset=") {
            location_offset = Some(parse_usize_arg("--offset", value)?);
            i += 1;
            continue;
        }
        if token == "--offset" {
            let value = sections.get(i + 1).ok_or_else(|| {
                crate::error::BioMcpError::InvalidArgument(
                    "--offset requires a value for trial location pagination".into(),
                )
            })?;
            location_offset = Some(parse_usize_arg("--offset", value.trim())?);
            i += 2;
            continue;
        }
        if let Some(value) = token.strip_prefix("--limit=") {
            location_limit = Some(parse_usize_arg("--limit", value)?);
            i += 1;
            continue;
        }
        if token == "--limit" {
            let value = sections.get(i + 1).ok_or_else(|| {
                crate::error::BioMcpError::InvalidArgument(
                    "--limit requires a value for trial location pagination".into(),
                )
            })?;
            location_limit = Some(parse_usize_arg("--limit", value.trim())?);
            i += 2;
            continue;
        }
        cleaned.push(sections[i].clone());
        i += 1;
    }

    if location_limit.is_some_and(|value| value == 0) {
        return Err(crate::error::BioMcpError::InvalidArgument(
            "--limit must be >= 1 for trial location pagination".into(),
        ));
    }

    Ok((cleaned, location_offset, location_limit))
}

fn normalize_cli_query(value: Option<String>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
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

fn mcp_output_flag_error() -> crate::error::BioMcpError {
    crate::error::BioMcpError::InvalidArgument(
        "MCP chart responses do not support --output/-o. Omit file output and consume the inline SVG image content instead.".into(),
    )
}

fn is_charted_mcp_study_command(cli: &Cli) -> Result<bool, crate::error::BioMcpError> {
    let chart = match &cli.command {
        Commands::Study {
            cmd:
                StudyCommand::Query { chart, .. }
                | StudyCommand::Survival { chart, .. }
                | StudyCommand::Compare { chart, .. }
                | StudyCommand::CoOccurrence { chart, .. },
        } => chart,
        _ => return Ok(false),
    };

    if chart.chart.is_none() || cli.json {
        return Ok(false);
    }
    if chart.output.is_some() {
        return Err(mcp_output_flag_error());
    }
    Ok(true)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum McpChartPass {
    Text,
    Svg,
}

fn require_flag_value(
    args: &[String],
    index: usize,
    flag: &str,
) -> Result<String, crate::error::BioMcpError> {
    args.get(index + 1).cloned().ok_or_else(|| {
        crate::error::BioMcpError::InvalidArgument(format!("{flag} requires a value"))
    })
}

fn rewrite_mcp_chart_args(
    args: &[String],
    pass: McpChartPass,
) -> Result<Vec<String>, crate::error::BioMcpError> {
    let mut rewritten = Vec::with_capacity(args.len() + 1);
    rewritten.push(
        args.first()
            .cloned()
            .unwrap_or_else(|| "biomcp".to_string()),
    );

    let mut i = 1usize;
    let mut saw_inline_flag = false;
    while i < args.len() {
        let token = &args[i];
        match token.as_str() {
            "--chart" => {
                let value = require_flag_value(args, i, "--chart")?;
                if pass == McpChartPass::Svg {
                    rewritten.push(token.clone());
                    rewritten.push(value);
                }
                i += 2;
            }
            "--terminal" => {
                i += 1;
            }
            "--output" => {
                if pass == McpChartPass::Svg {
                    return Err(mcp_output_flag_error());
                }
                let _ = require_flag_value(args, i, "--output")?;
                i += 2;
            }
            "-o" => {
                if pass == McpChartPass::Svg {
                    return Err(mcp_output_flag_error());
                }
                let _ = require_flag_value(args, i, "-o")?;
                i += 2;
            }
            "--title" | "--theme" | "--palette" => {
                let value = require_flag_value(args, i, token)?;
                if pass == McpChartPass::Svg {
                    rewritten.push(token.clone());
                    rewritten.push(value);
                }
                i += 2;
            }
            "--width" | "--height" => {
                let value = require_flag_value(args, i, token)?;
                if pass == McpChartPass::Svg {
                    rewritten.push(token.clone());
                    rewritten.push(value);
                }
                i += 2;
            }
            "--cols" | "--rows" => {
                let _ = require_flag_value(args, i, token)?;
                if pass == McpChartPass::Svg {
                    return Err(crate::error::BioMcpError::InvalidArgument(
                        crate::render::chart::TERMINAL_SIZE_FLAGS_ERROR.into(),
                    ));
                }
                i += 2;
            }
            "--scale" => {
                let _ = require_flag_value(args, i, token)?;
                if pass == McpChartPass::Svg {
                    return Err(crate::error::BioMcpError::InvalidArgument(
                        crate::render::chart::PNG_SCALE_FLAGS_ERROR.into(),
                    ));
                }
                i += 2;
            }
            "--mcp-inline" => {
                if pass == McpChartPass::Svg {
                    rewritten.push(token.clone());
                }
                saw_inline_flag = true;
                i += 1;
            }
            _ => {
                if token.starts_with("--chart=") {
                    if pass == McpChartPass::Svg {
                        rewritten.push(token.clone());
                    }
                    i += 1;
                    continue;
                }
                if token.starts_with("--output=") || token.starts_with("-o=") {
                    if pass == McpChartPass::Svg {
                        return Err(mcp_output_flag_error());
                    }
                    i += 1;
                    continue;
                }
                if token.starts_with("-o") && token.len() > 2 {
                    if pass == McpChartPass::Svg {
                        return Err(mcp_output_flag_error());
                    }
                    i += 1;
                    continue;
                }
                if token.starts_with("--title=")
                    || token.starts_with("--theme=")
                    || token.starts_with("--palette=")
                {
                    if pass == McpChartPass::Svg {
                        rewritten.push(token.clone());
                    }
                    i += 1;
                    continue;
                }
                if token.starts_with("--width=") || token.starts_with("--height=") {
                    if pass == McpChartPass::Svg {
                        rewritten.push(token.clone());
                    }
                    i += 1;
                    continue;
                }
                if token.starts_with("--cols=") || token.starts_with("--rows=") {
                    if pass == McpChartPass::Svg {
                        return Err(crate::error::BioMcpError::InvalidArgument(
                            crate::render::chart::TERMINAL_SIZE_FLAGS_ERROR.into(),
                        ));
                    }
                    i += 1;
                    continue;
                }
                if token.starts_with("--scale=") {
                    if pass == McpChartPass::Svg {
                        return Err(crate::error::BioMcpError::InvalidArgument(
                            crate::render::chart::PNG_SCALE_FLAGS_ERROR.into(),
                        ));
                    }
                    i += 1;
                    continue;
                }
                rewritten.push(token.clone());
                i += 1;
            }
        }
    }

    if pass == McpChartPass::Svg && !saw_inline_flag {
        rewritten.push("--mcp-inline".to_string());
    }
    Ok(rewritten)
}

fn normalize_cli_tokens(values: Vec<String>) -> Option<String> {
    let joined = values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    normalize_cli_query(Some(joined))
}

fn resolve_query_input(
    flag_query: Option<String>,
    positional_query: Option<String>,
    flag_names: &str,
) -> Result<Option<String>, crate::error::BioMcpError> {
    let flag_query = normalize_cli_query(flag_query);
    let positional_query = normalize_cli_query(positional_query);
    match (flag_query, positional_query) {
        (Some(_), Some(_)) => Err(crate::error::BioMcpError::InvalidArgument(format!(
            "Use either positional QUERY or {flag_names}, not both"
        ))),
        (Some(value), None) | (None, Some(value)) => Ok(Some(value)),
        (None, None) => Ok(None),
    }
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

fn parse_simple_gene_change(query: &str) -> Option<(String, String)> {
    let parts = query.split_whitespace().collect::<Vec<_>>();
    if parts.len() != 2 {
        return None;
    }

    let gene = parts[0].trim();
    let change = parts[1]
        .trim()
        .trim_start_matches("p.")
        .trim_start_matches("P.");
    if gene.is_empty() || change.is_empty() {
        return None;
    }

    let candidate = format!("{gene} {change}");
    match crate::entities::variant::parse_variant_id(&candidate).ok()? {
        crate::entities::variant::VariantIdFormat::GeneProteinChange { gene, change } => {
            Some((gene, change))
        }
        _ => None,
    }
}

fn parse_gene_c_hgvs(query: &str) -> Option<(String, String)> {
    let parts = query.split_whitespace().collect::<Vec<_>>();
    if parts.len() != 2 {
        return None;
    }

    let gene = parts[0].trim();
    let change = parts[1].trim();
    if gene.is_empty() || change.is_empty() || !crate::sources::is_valid_gene_symbol(gene) {
        return None;
    }
    if !change.starts_with("c.") && !change.starts_with("C.") {
        return None;
    }
    Some((gene.to_string(), format!("c.{}", change[2..].trim())))
}

fn parse_exon_deletion_phrase(query: &str) -> Option<(String, String)> {
    let parts = query.split_whitespace().collect::<Vec<_>>();
    if parts.len() != 4 {
        return None;
    }

    let gene = parts[0].trim();
    if !crate::sources::is_valid_gene_symbol(gene)
        || !parts[1].eq_ignore_ascii_case("exon")
        || parts[2].parse::<u32>().ok().is_none()
        || !parts[3].eq_ignore_ascii_case("deletion")
    {
        return None;
    }

    Some((gene.to_string(), "inframe_deletion".to_string()))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ResolvedVariantQuery {
    gene: Option<String>,
    hgvsp: Option<String>,
    hgvsc: Option<String>,
    rsid: Option<String>,
    protein_alias: Option<crate::entities::variant::VariantProteinAlias>,
    consequence: Option<String>,
    condition: Option<String>,
}

#[derive(Debug, Clone)]
struct VariantSearchRequest {
    gene: Option<String>,
    positional_query: Vec<String>,
    hgvsp: Option<String>,
    significance: Option<String>,
    max_frequency: Option<f64>,
    min_cadd: Option<f64>,
    consequence: Option<String>,
    review_status: Option<String>,
    population: Option<String>,
    revel_min: Option<f64>,
    gerp_min: Option<f64>,
    tumor_site: Option<String>,
    condition: Option<String>,
    impact: Option<String>,
    lof: bool,
    has: Option<String>,
    missing: Option<String>,
    therapy: Option<String>,
    limit: usize,
    offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum VariantSearchPlan {
    Standard(ResolvedVariantQuery),
    Guidance(crate::entities::variant::VariantGuidance),
}

fn resolve_variant_query(
    gene_flag: Option<String>,
    hgvsp_flag: Option<String>,
    consequence_flag: Option<String>,
    condition_flag: Option<String>,
    positional_tokens: Vec<String>,
) -> Result<VariantSearchPlan, crate::error::BioMcpError> {
    let gene_flag = normalize_cli_query(gene_flag);
    let hgvsp_flag = normalize_cli_query(hgvsp_flag).map(|value| normalize_search_hgvsp(&value));
    let consequence_flag = normalize_cli_query(consequence_flag);
    let condition_flag = normalize_cli_query(condition_flag);

    let positional = positional_tokens
        .iter()
        .map(|token| token.trim())
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let positional = normalize_cli_query(Some(positional));

    let Some(query) = positional else {
        return Ok(VariantSearchPlan::Standard(ResolvedVariantQuery {
            gene: gene_flag,
            hgvsp: hgvsp_flag,
            consequence: consequence_flag,
            condition: condition_flag,
            ..Default::default()
        }));
    };

    let token_count = query.split_whitespace().count();
    if token_count <= 1 {
        if let Ok(crate::entities::variant::VariantIdFormat::RsId(rsid)) =
            crate::entities::variant::parse_variant_id(&query)
        {
            if gene_flag.is_some() {
                return Err(crate::error::BioMcpError::InvalidArgument(
                    "Use either positional QUERY or --gene, not both".into(),
                ));
            }
            return Ok(VariantSearchPlan::Standard(ResolvedVariantQuery {
                rsid: Some(rsid),
                hgvsp: hgvsp_flag,
                consequence: consequence_flag,
                condition: condition_flag,
                ..Default::default()
            }));
        }

        if let Some(gene) = gene_flag.clone() {
            if let Some(protein_alias) =
                crate::entities::variant::parse_variant_protein_alias(&query)
            {
                if hgvsp_flag.is_some() {
                    return Err(crate::error::BioMcpError::InvalidArgument(
                        "Positional residue alias conflicts with --hgvsp".into(),
                    ));
                }
                return Ok(VariantSearchPlan::Standard(ResolvedVariantQuery {
                    gene: Some(gene),
                    protein_alias: Some(protein_alias),
                    consequence: consequence_flag,
                    condition: condition_flag,
                    ..Default::default()
                }));
            }
            if let crate::entities::variant::VariantInputKind::Shorthand(
                crate::entities::variant::VariantShorthand::ProteinChangeOnly { change },
            ) = crate::entities::variant::classify_variant_input(&query)
            {
                if hgvsp_flag.is_some() {
                    return Err(crate::error::BioMcpError::InvalidArgument(
                        "Positional protein change conflicts with --hgvsp".into(),
                    ));
                }
                return Ok(VariantSearchPlan::Standard(ResolvedVariantQuery {
                    gene: Some(gene),
                    hgvsp: Some(normalize_search_hgvsp(&change)),
                    consequence: consequence_flag,
                    condition: condition_flag,
                    ..Default::default()
                }));
            }
            return Err(crate::error::BioMcpError::InvalidArgument(
                "Use either positional QUERY or --gene, not both".into(),
            ));
        }

        if let Some(guidance) = crate::entities::variant::variant_guidance(&query) {
            return Ok(VariantSearchPlan::Guidance(guidance));
        }
        return Ok(VariantSearchPlan::Standard(ResolvedVariantQuery {
            gene: Some(query),
            hgvsp: hgvsp_flag,
            consequence: consequence_flag,
            condition: condition_flag,
            ..Default::default()
        }));
    }

    if let Some((gene, change)) = parse_simple_gene_change(&query) {
        if gene_flag.is_some() {
            return Err(crate::error::BioMcpError::InvalidArgument(
                "Positional \"GENE CHANGE\" conflicts with --gene".into(),
            ));
        }
        if hgvsp_flag.is_some() {
            return Err(crate::error::BioMcpError::InvalidArgument(
                "Positional \"GENE CHANGE\" conflicts with --hgvsp".into(),
            ));
        }
        return Ok(VariantSearchPlan::Standard(ResolvedVariantQuery {
            gene: Some(gene),
            hgvsp: Some(normalize_search_hgvsp(&change)),
            consequence: consequence_flag,
            condition: condition_flag,
            ..Default::default()
        }));
    }

    if let crate::entities::variant::VariantInputKind::Shorthand(
        crate::entities::variant::VariantShorthand::GeneResidueAlias {
            gene,
            position,
            residue,
            ..
        },
    ) = crate::entities::variant::classify_variant_input(&query)
    {
        if gene_flag.is_some() {
            return Err(crate::error::BioMcpError::InvalidArgument(
                "Positional residue alias conflicts with --gene".into(),
            ));
        }
        if hgvsp_flag.is_some() {
            return Err(crate::error::BioMcpError::InvalidArgument(
                "Positional residue alias conflicts with --hgvsp".into(),
            ));
        }
        return Ok(VariantSearchPlan::Standard(ResolvedVariantQuery {
            gene: Some(gene),
            protein_alias: Some(crate::entities::variant::VariantProteinAlias {
                position,
                residue,
            }),
            consequence: consequence_flag,
            condition: condition_flag,
            ..Default::default()
        }));
    }

    if let Some((gene, hgvsc)) = parse_gene_c_hgvs(&query) {
        if gene_flag.is_some() {
            return Err(crate::error::BioMcpError::InvalidArgument(
                "Positional \"GENE c.HGVS\" conflicts with --gene".into(),
            ));
        }
        return Ok(VariantSearchPlan::Standard(ResolvedVariantQuery {
            gene: Some(gene),
            hgvsp: hgvsp_flag,
            hgvsc: Some(hgvsc),
            consequence: consequence_flag,
            condition: condition_flag,
            ..Default::default()
        }));
    }

    if let Some((gene, consequence)) = parse_exon_deletion_phrase(&query) {
        if gene_flag.is_some() {
            return Err(crate::error::BioMcpError::InvalidArgument(
                "Positional exon-deletion query conflicts with --gene".into(),
            ));
        }
        if consequence_flag.is_some() {
            return Err(crate::error::BioMcpError::InvalidArgument(
                "Positional exon-deletion query conflicts with --consequence".into(),
            ));
        }
        return Ok(VariantSearchPlan::Standard(ResolvedVariantQuery {
            gene: Some(gene),
            hgvsp: hgvsp_flag,
            consequence: Some(consequence),
            condition: condition_flag,
            ..Default::default()
        }));
    }

    if condition_flag.is_some() {
        return Err(crate::error::BioMcpError::InvalidArgument(
            "Use either positional QUERY or --condition, not both".into(),
        ));
    }
    Ok(VariantSearchPlan::Standard(ResolvedVariantQuery {
        gene: gene_flag,
        hgvsp: hgvsp_flag,
        consequence: consequence_flag,
        condition: Some(query),
        ..Default::default()
    }))
}

async fn render_gene_card(
    symbol: &str,
    sections: &[String],
    json_output: bool,
) -> anyhow::Result<String> {
    let gene = crate::entities::gene::get(symbol, sections).await?;
    if json_output {
        Ok(crate::render::json::to_entity_json(
            &gene,
            crate::render::markdown::gene_evidence_urls(&gene),
            crate::render::markdown::related_gene(&gene),
            crate::render::provenance::gene_section_sources(&gene),
        )?)
    } else {
        Ok(crate::render::markdown::gene_markdown(&gene, sections)?)
    }
}

fn alias_suggestion_markdown(
    query: &str,
    requested_entity: crate::entities::discover::DiscoverType,
    decision: &crate::entities::discover::AliasFallbackDecision,
) -> String {
    let err = crate::error::BioMcpError::NotFound {
        entity: requested_entity.cli_name().to_string(),
        id: query.trim().to_string(),
        suggestion: crate::render::markdown::alias_fallback_suggestion(decision),
    };
    format!("Error: {err}")
}

fn alias_suggestion_outcome(
    query: &str,
    requested_entity: crate::entities::discover::DiscoverType,
    decision: &crate::entities::discover::AliasFallbackDecision,
    json_output: bool,
) -> anyhow::Result<CommandOutcome> {
    if json_output {
        return Ok(CommandOutcome::stdout_with_exit(
            crate::render::json::to_alias_suggestion_json(decision)?,
            1,
        ));
    }
    Ok(CommandOutcome::stderr_with_exit(
        alias_suggestion_markdown(query, requested_entity, decision),
        1,
    ))
}

fn variant_guidance_markdown(guidance: &crate::entities::variant::VariantGuidance) -> String {
    let err = crate::error::BioMcpError::NotFound {
        entity: "variant".into(),
        id: guidance.query.clone(),
        suggestion: crate::render::markdown::variant_guidance_suggestion(guidance),
    };
    format!("Error: {err}")
}

fn variant_guidance_outcome(
    guidance: &crate::entities::variant::VariantGuidance,
    json_output: bool,
) -> anyhow::Result<CommandOutcome> {
    if json_output {
        return Ok(CommandOutcome::stdout_with_exit(
            crate::render::json::to_variant_guidance_json(guidance)?,
            1,
        ));
    }
    Ok(CommandOutcome::stderr_with_exit(
        variant_guidance_markdown(guidance),
        1,
    ))
}

async fn try_alias_fallback_outcome(
    query: &str,
    requested_entity: crate::entities::discover::DiscoverType,
    json_output: bool,
) -> anyhow::Result<Option<CommandOutcome>> {
    match crate::entities::discover::resolve_query(
        query,
        crate::entities::discover::DiscoverMode::AliasFallback,
    )
    .await
    {
        Ok(result) => {
            let decision =
                crate::entities::discover::classify_alias_fallback(&result, requested_entity);
            match decision {
                crate::entities::discover::AliasFallbackDecision::None => Ok(None),
                other => Ok(Some(alias_suggestion_outcome(
                    query,
                    requested_entity,
                    &other,
                    json_output,
                )?)),
            }
        }
        Err(err) => {
            warn!(
                query = query.trim(),
                entity = requested_entity.cli_name(),
                "alias fallback discovery unavailable: {err}"
            );
            Ok(None)
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

async fn render_variant_card_outcome(
    id: &str,
    sections: &[String],
    json_output: bool,
    guidance_as_json: bool,
) -> anyhow::Result<CommandOutcome> {
    if let Some(guidance) = crate::entities::variant::variant_guidance(id) {
        return variant_guidance_outcome(&guidance, json_output || guidance_as_json);
    }

    match crate::entities::variant::get(id, sections).await {
        Ok(variant) => {
            let text = if json_output {
                crate::render::json::to_entity_json(
                    &variant,
                    crate::render::markdown::variant_evidence_urls(&variant),
                    crate::render::markdown::related_variant(&variant),
                    crate::render::provenance::variant_section_sources(&variant),
                )?
            } else {
                crate::render::markdown::variant_markdown(&variant, sections)?
            };
            Ok(CommandOutcome::stdout(text))
        }
        Err(err) => Err(err.into()),
    }
}

async fn render_variant_search_outcome(
    json_output: bool,
    guidance_as_json: bool,
    request: VariantSearchRequest,
) -> anyhow::Result<CommandOutcome> {
    let VariantSearchRequest {
        gene,
        positional_query,
        hgvsp,
        significance,
        max_frequency,
        min_cadd,
        consequence,
        review_status,
        population,
        revel_min,
        gerp_min,
        tumor_site,
        condition,
        impact,
        lof,
        has,
        missing,
        therapy,
        limit,
        offset,
    } = request;

    let resolved =
        match resolve_variant_query(gene, hgvsp, consequence, condition, positional_query)? {
            VariantSearchPlan::Standard(resolved) => resolved,
            VariantSearchPlan::Guidance(guidance) => {
                return variant_guidance_outcome(&guidance, json_output || guidance_as_json);
            }
        };

    let filters = crate::entities::variant::VariantSearchFilters {
        gene: resolved.gene,
        hgvsp: resolved.hgvsp,
        hgvsc: resolved.hgvsc,
        rsid: resolved.rsid,
        protein_alias: resolved.protein_alias,
        significance,
        max_frequency,
        min_cadd,
        consequence: resolved.consequence,
        review_status,
        population,
        revel_min,
        gerp_min,
        tumor_site,
        condition: resolved.condition,
        impact,
        lof,
        has,
        missing,
        therapy,
    };

    let mut query = crate::entities::variant::search_query_summary(&filters);
    if offset > 0 {
        query = if query.is_empty() {
            format!("offset={offset}")
        } else {
            format!("{query}, offset={offset}")
        };
    }

    let page = crate::entities::variant::search_page(&filters, limit, offset).await?;
    let results = page.results;
    let pagination = PaginationMeta::offset(offset, limit, results.len(), page.total);
    if json_output {
        return Ok(CommandOutcome::stdout(search_json(results, pagination)?));
    }

    let footer = pagination_footer_offset(&pagination);
    Ok(CommandOutcome::stdout(
        crate::render::markdown::variant_search_markdown_with_context(
            &query,
            &results,
            &footer,
            filters.gene.as_deref(),
            filters.condition.as_deref(),
        )?,
    ))
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

#[derive(Debug, Clone, serde::Serialize)]
struct LocationPaginationMeta {
    total: usize,
    offset: usize,
    limit: usize,
    has_more: bool,
}

fn trial_locations_json(
    trial: &crate::entities::trial::Trial,
    location_pagination: LocationPaginationMeta,
) -> anyhow::Result<String> {
    #[derive(serde::Serialize)]
    struct TrialWithLocationPagination<'a> {
        #[serde(flatten)]
        trial: &'a crate::entities::trial::Trial,
        location_pagination: LocationPaginationMeta,
    }

    crate::render::json::to_entity_json(
        &TrialWithLocationPagination {
            trial,
            location_pagination,
        },
        crate::render::markdown::trial_evidence_urls(trial),
        crate::render::markdown::related_trial(trial),
        crate::render::provenance::trial_section_sources(trial),
    )
    .map_err(Into::into)
}

fn render_batch_json<T, F>(results: &[T], wrap: F) -> Result<String, crate::error::BioMcpError>
where
    F: Fn(&T) -> Result<serde_json::Value, crate::error::BioMcpError>,
{
    let items = results.iter().map(wrap).collect::<Result<Vec<_>, _>>()?;
    crate::render::json::to_pretty(&items)
}

fn paginate_trial_locations(
    trial: &mut crate::entities::trial::Trial,
    offset: usize,
    limit: usize,
) -> LocationPaginationMeta {
    let locations = trial.locations.take().unwrap_or_default();
    let total = locations.len();
    let paged: Vec<_> = locations.into_iter().skip(offset).take(limit).collect();
    let has_more = offset.saturating_add(paged.len()) < total;
    trial.locations = Some(paged);
    LocationPaginationMeta {
        total,
        offset,
        limit,
        has_more,
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PaginationMeta {
    pub offset: usize,
    pub limit: usize,
    pub returned: usize,
    pub total: Option<usize>,
    pub has_more: bool,
    pub next_page_token: Option<String>,
}

impl PaginationMeta {
    fn offset(offset: usize, limit: usize, returned: usize, total: Option<usize>) -> Self {
        let has_more = total
            .map(|value| offset.saturating_add(returned) < value)
            .unwrap_or(returned == limit);
        Self {
            offset,
            limit,
            returned,
            total,
            has_more,
            next_page_token: None,
        }
    }

    fn cursor(
        offset: usize,
        limit: usize,
        returned: usize,
        total: Option<usize>,
        next_page_token: Option<String>,
    ) -> Self {
        let has_token = next_page_token
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty());
        let has_more = match total {
            Some(value) => offset.saturating_add(returned) < value || has_token,
            None => has_token,
        };
        Self {
            offset,
            limit,
            returned,
            total,
            has_more,
            next_page_token,
        }
    }
}

#[derive(serde::Serialize)]
struct SearchJsonResponse<T: serde::Serialize> {
    pagination: PaginationMeta,
    count: usize,
    results: Vec<T>,
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

fn search_json<T: serde::Serialize>(
    results: Vec<T>,
    pagination: PaginationMeta,
) -> anyhow::Result<String> {
    let count = results.len();
    crate::render::json::to_pretty(&SearchJsonResponse {
        pagination,
        count,
        results,
    })
    .map_err(Into::into)
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

fn pagination_footer_offset(meta: &PaginationMeta) -> String {
    crate::render::markdown::pagination_footer(
        crate::render::markdown::PaginationFooterMode::Offset,
        meta.offset,
        meta.limit,
        meta.returned,
        meta.total,
        None,
    )
}

fn pagination_footer_cursor(meta: &PaginationMeta) -> String {
    crate::render::markdown::pagination_footer(
        crate::render::markdown::PaginationFooterMode::Cursor,
        meta.offset,
        meta.limit,
        meta.returned,
        meta.total,
        meta.next_page_token.as_deref(),
    )
}

fn paged_fetch_limit(
    limit: usize,
    offset: usize,
    max_limit: usize,
) -> Result<usize, crate::error::BioMcpError> {
    if limit == 0 || limit > max_limit {
        return Err(crate::error::BioMcpError::InvalidArgument(format!(
            "--limit must be between 1 and {max_limit}"
        )));
    }
    Ok(limit.saturating_add(offset).min(max_limit))
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

fn paginate_results<T>(rows: Vec<T>, offset: usize, limit: usize) -> (Vec<T>, usize) {
    let total = rows.len();
    let paged = rows.into_iter().skip(offset).take(limit).collect();
    (paged, total)
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

fn log_pagination_truncation(observed_total: usize, offset: usize, returned: usize) {
    if offset.saturating_add(returned) < observed_total {
        debug!(
            total = observed_total,
            offset, returned, "Results truncated by --limit"
        );
    }
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

fn trial_search_query_summary(
    filters: &crate::entities::trial::TrialSearchFilters,
    offset: usize,
    next_page: Option<&str>,
) -> String {
    vec![
        filters
            .condition
            .as_deref()
            .map(|v| format!("condition={v}")),
        filters
            .intervention
            .as_deref()
            .map(|v| format!("intervention={v}")),
        filters.facility.as_deref().map(|v| format!("facility={v}")),
        filters.age.map(|v| format!("age={v}")),
        filters.sex.as_deref().map(|v| format!("sex={v}")),
        filters.status.as_deref().map(|v| format!("status={v}")),
        filters.phase.as_deref().map(|v| format!("phase={v}")),
        filters
            .study_type
            .as_deref()
            .map(|v| format!("study_type={v}")),
        filters.sponsor.as_deref().map(|v| format!("sponsor={v}")),
        filters
            .sponsor_type
            .as_deref()
            .map(|v| format!("sponsor_type={v}")),
        filters
            .date_from
            .as_deref()
            .map(|v| format!("date_from={v}")),
        filters.date_to.as_deref().map(|v| format!("date_to={v}")),
        filters.mutation.as_deref().map(|v| format!("mutation={v}")),
        filters.criteria.as_deref().map(|v| format!("criteria={v}")),
        filters
            .biomarker
            .as_deref()
            .map(|v| format!("biomarker={v}")),
        filters
            .prior_therapies
            .as_deref()
            .map(|v| format!("prior_therapies={v}")),
        filters
            .progression_on
            .as_deref()
            .map(|v| format!("progression_on={v}")),
        filters
            .line_of_therapy
            .as_deref()
            .map(|v| format!("line_of_therapy={v}")),
        filters.lat.map(|v| format!("lat={v}")),
        filters.lon.map(|v| format!("lon={v}")),
        filters.distance.map(|v| format!("distance={v}")),
        matches!(filters.source, crate::entities::trial::TrialSource::NciCts)
            .then(|| "source=nci".to_string()),
        filters
            .results_available
            .then(|| "has_results=true".to_string()),
        (offset > 0).then(|| format!("offset={offset}")),
        next_page
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| format!("next_page={value}")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(", ")
}

fn should_show_trial_zero_result_nickname_hint(
    positional_query: Option<&str>,
    source: crate::entities::trial::TrialSource,
    result_count: usize,
) -> bool {
    positional_query
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
        && matches!(
            source,
            crate::entities::trial::TrialSource::ClinicalTrialsGov
        )
        && result_count == 0
}

fn trim_protein_change_prefix(value: &str) -> &str {
    value
        .trim()
        .trim_start_matches("p.")
        .trim_start_matches("P.")
}

fn normalize_search_hgvsp(value: &str) -> String {
    let normalized = crate::entities::variant::normalize_protein_change(value)
        .unwrap_or_else(|| trim_protein_change_prefix(value).to_string());
    normalized
        .strip_suffix('*')
        .map(|prefix| format!("{prefix}X"))
        .unwrap_or(normalized)
}

async fn variant_trial_mutation_query(id: &str) -> String {
    let id = id.trim();
    if id.is_empty() {
        return String::new();
    }

    if let Ok(crate::entities::variant::VariantIdFormat::GeneProteinChange { gene, change }) =
        crate::entities::variant::parse_variant_id(id)
    {
        let normalized = crate::entities::variant::normalize_protein_change(&change)
            .unwrap_or_else(|| trim_protein_change_prefix(&change).to_string());
        if !normalized.is_empty() {
            return format!("{gene} {normalized}");
        }
    }

    if let Ok(variant) = crate::entities::variant::get(id, empty_sections()).await {
        let gene = variant.gene.trim();
        let protein = variant
            .hgvs_p
            .as_deref()
            .map(|value| {
                crate::entities::variant::normalize_protein_change(value)
                    .unwrap_or_else(|| trim_protein_change_prefix(value).to_string())
            })
            .unwrap_or_default();
        if !gene.is_empty() && !protein.is_empty() {
            return format!("{gene} {protein}");
        }
    }

    id.to_string()
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

/// Executes one parsed CLI command and returns rendered output.
///
/// # Errors
///
/// Returns an error if argument validation fails, downstream entity operations fail,
/// rendering fails, or external API requests fail.
pub async fn run(cli: Cli) -> anyhow::Result<String> {
    let no_cache = cli.no_cache;
    crate::sources::with_no_cache(no_cache, async move {
        match cli.command {
            Commands::Get {
                entity: GetEntity::Gene(gene::GeneGetArgs { symbol, sections }),
            } => {
                let (sections, json_override) = extract_json_from_sections(&sections);
                let json_output = cli.json || json_override;
                render_gene_card(&symbol, &sections, json_output).await
            }
            Commands::Get {
                entity: GetEntity::Article(article::ArticleGetArgs { id, sections }),
            } => {
                let (sections, json_override) = extract_json_from_sections(&sections);
                let json_output = cli.json || json_override;
                let article = crate::entities::article::get(&id, &sections).await?;
                if json_output {
                    Ok(crate::render::json::to_entity_json(
                        &article,
                        crate::render::markdown::article_evidence_urls(&article),
                        crate::render::markdown::related_article(&article),
                        crate::render::provenance::article_section_sources(&article),
                    )?)
                } else {
                    Ok(crate::render::markdown::article_markdown(&article, &sections)?)
                }
            }
            Commands::Get {
                entity: GetEntity::Disease(disease::DiseaseGetArgs {
                    name_or_id,
                    sections,
                }),
            } => {
                let (sections, json_override) = extract_json_from_sections(&sections);
                let json_output = cli.json || json_override;
                let disease = crate::entities::disease::get(&name_or_id, &sections).await?;
                if json_output {
                    Ok(crate::render::json::to_entity_json(
                        &disease,
                        crate::render::markdown::disease_evidence_urls(&disease),
                        crate::render::markdown::related_disease(&disease),
                        crate::render::provenance::disease_section_sources(&disease),
                    )?)
                } else {
                    Ok(crate::render::markdown::disease_markdown(&disease, &sections)?)
                }
            }
            Commands::Get {
                entity: GetEntity::Pgx(pgx::PgxGetArgs { query, sections }),
            } => {
                let (sections, json_override) = extract_json_from_sections(&sections);
                let json_output = cli.json || json_override;
                let pgx = crate::entities::pgx::get(&query, &sections).await?;
                if json_output {
                    Ok(crate::render::json::to_entity_json(
                        &pgx,
                        crate::render::markdown::pgx_evidence_urls(&pgx),
                        crate::render::markdown::related_pgx(&pgx),
                        crate::render::provenance::pgx_section_sources(&pgx),
                    )?)
                } else {
                    Ok(crate::render::markdown::pgx_markdown(&pgx, &sections)?)
                }
            }
            Commands::Get {
                entity: GetEntity::Trial(trial::TrialGetArgs {
                    nct_id,
                    sections,
                    source,
                }),
            } => {
                let (sections, location_offset, location_limit) =
                    parse_trial_location_paging(&sections)?;
                let (sections, json_override) = extract_json_from_sections(&sections);
                let json_output = cli.json || json_override;
                let trial_source = crate::entities::trial::TrialSource::from_flag(&source)?;
                let includes_locations = sections
                    .iter()
                    .any(|section| section.trim().eq_ignore_ascii_case("locations"));
                if !includes_locations
                    && (location_offset.is_some() || location_limit.is_some())
                {
                    return Err(crate::error::BioMcpError::InvalidArgument(
                        "--offset and --limit are only valid with the 'locations' section".into(),
                    )
                    .into());
                }
                let mut trial =
                    crate::entities::trial::get(&nct_id, &sections, trial_source).await?;
                let mut location_pagination: Option<LocationPaginationMeta> = None;
                if includes_locations {
                    let offset = location_offset.unwrap_or(0);
                    let limit = location_limit.unwrap_or(20);
                    location_pagination = Some(paginate_trial_locations(&mut trial, offset, limit));
                }
                if json_output {
                    if let Some(loc_page) = location_pagination {
                        trial_locations_json(&trial, loc_page)
                    } else {
                        Ok(crate::render::json::to_entity_json(
                            &trial,
                            crate::render::markdown::trial_evidence_urls(&trial),
                            crate::render::markdown::related_trial(&trial),
                            crate::render::provenance::trial_section_sources(&trial),
                        )?)
                    }
                } else {
                    let mut md =
                        crate::render::markdown::trial_markdown(&trial, &sections)?;
                    if let Some(loc_page) = location_pagination {
                        md.push_str(&format!(
                            "\n\n---\n*Locations: showing {} of {} (offset {}, limit {}{})*",
                            trial.locations.as_ref().map_or(0, |v| v.len()),
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
                    Ok(md)
                }
            }
            Commands::Get {
                entity: GetEntity::Variant(variant::VariantGetArgs { id, sections }),
            } => {
                let (sections, json_override) = extract_json_from_sections(&sections);
                let json_output = cli.json || json_override;
                let variant = crate::entities::variant::get(&id, &sections).await?;
                if json_output {
                    Ok(crate::render::json::to_entity_json(
                        &variant,
                        crate::render::markdown::variant_evidence_urls(&variant),
                        crate::render::markdown::related_variant(&variant),
                        crate::render::provenance::variant_section_sources(&variant),
                    )?)
                } else {
                    Ok(crate::render::markdown::variant_markdown(&variant, &sections)?)
                }
            }
            Commands::Get {
                entity: GetEntity::Drug(drug::DrugGetArgs {
                    name,
                    sections,
                    region,
                    raw,
                }),
            } => {
                let (sections, json_override) = extract_json_from_sections(&sections);
                let region = region.map(DrugRegion::from);
                let json_output = cli.json || json_override;
                let effective_region = region.unwrap_or(DrugRegion::Us);
                let drug = crate::entities::drug::get_with_region(
                    &name,
                    &sections,
                    effective_region,
                    region.is_some(),
                    raw,
                )
                .await?;
                if json_output {
                    Ok(crate::render::json::to_entity_json(
                        &drug,
                        crate::render::markdown::drug_evidence_urls(&drug),
                        crate::render::markdown::related_drug(&drug),
                        crate::render::provenance::drug_section_sources(&drug),
                    )?)
                } else {
                    Ok(crate::render::markdown::drug_markdown_with_region(
                        &drug,
                        &sections,
                        effective_region,
                        raw,
                    )?)
                }
            }
            Commands::Get {
                entity: GetEntity::Pathway(pathway::PathwayGetArgs { id, sections }),
            } => {
                let (sections, json_override) = extract_json_from_sections(&sections);
                let json_output = cli.json || json_override;
                let pathway = crate::entities::pathway::get(&id, &sections).await?;
                if json_output {
                    Ok(crate::render::json::to_entity_json(
                        &pathway,
                        crate::render::markdown::pathway_evidence_urls(&pathway),
                        crate::render::markdown::related_pathway(&pathway),
                        crate::render::provenance::pathway_section_sources(&pathway),
                    )?)
                } else {
                    Ok(crate::render::markdown::pathway_markdown(&pathway, &sections)?)
                }
            }
            Commands::Get {
                entity: GetEntity::Protein(protein::ProteinGetArgs {
                    accession,
                    sections,
                }),
            } => {
                let (sections, json_override) = extract_json_from_sections(&sections);
                let json_output = cli.json || json_override;
                let protein = crate::entities::protein::get(&accession, &sections).await?;
                if json_output {
                    Ok(crate::render::json::to_entity_json(
                        &protein,
                        crate::render::markdown::protein_evidence_urls(&protein),
                        crate::render::markdown::related_protein(&protein, &sections),
                        crate::render::provenance::protein_section_sources(&protein),
                    )?)
                } else {
                    Ok(crate::render::markdown::protein_markdown(&protein, &sections)?)
                }
            }
            Commands::Get {
                entity: GetEntity::AdverseEvent(adverse_event::AdverseEventGetArgs {
                    report_id,
                    sections,
                }),
            } => {
                let (sections, json_override) = extract_json_from_sections(&sections);
                let json_output = cli.json || json_override;
                let event = crate::entities::adverse_event::get(&report_id).await?;
                if json_output {
                    return match &event {
                        crate::entities::adverse_event::AdverseEventReport::Faers(r) => {
                            Ok(crate::render::json::to_entity_json(
                                &event,
                                crate::render::markdown::adverse_event_evidence_urls(r),
                                crate::render::markdown::related_adverse_event(r),
                                crate::render::provenance::adverse_event_report_section_sources(
                                    &event,
                                ),
                            )?)
                        }
                        crate::entities::adverse_event::AdverseEventReport::Device(r) => {
                            Ok(crate::render::json::to_entity_json(
                                &event,
                                crate::render::markdown::device_event_evidence_urls(r),
                                crate::render::markdown::related_device_event(r),
                                crate::render::provenance::adverse_event_report_section_sources(
                                    &event,
                                ),
                            )?)
                        }
                    };
                }
                match event {
                    crate::entities::adverse_event::AdverseEventReport::Faers(ref r) => {
                        Ok(crate::render::markdown::adverse_event_markdown(r, &sections)?)
                    }
                    crate::entities::adverse_event::AdverseEventReport::Device(ref r) => {
                        Ok(crate::render::markdown::device_event_markdown(r)?)
                    }
                }
            }
            Commands::Variant { cmd } => match cmd {
                VariantCommand::Trials {
                    id,
                    limit,
                    offset,
                    source,
                } => {
                    let _ = crate::entities::variant::parse_variant_id(&id)?;
                    let mutation_query = variant_trial_mutation_query(&id).await;
                    let trial_source = crate::entities::trial::TrialSource::from_flag(&source)?;
                    let filters = crate::entities::trial::TrialSearchFilters {
                        mutation: Some(mutation_query.clone()),
                        source: trial_source,
                        ..Default::default()
                    };
                    let (results, total) =
                        crate::entities::trial::search(&filters, limit, offset).await?;
                    if let Some(total) = total {
                        log_pagination_truncation(total as usize, offset, results.len());
                    }
                    if cli.json {
                        #[derive(serde::Serialize)]
                        struct SearchResponse {
                            count: usize,
                            total: Option<u32>,
                            results: Vec<crate::entities::trial::TrialSearchResult>,
                        }

                        Ok(crate::render::json::to_pretty(&SearchResponse {
                            count: results.len(),
                            total,
                            results,
                        })?)
                    } else {
                        let mut query_parts = vec![format!("mutation={mutation_query}")];
                        if matches!(trial_source, crate::entities::trial::TrialSource::NciCts) {
                            query_parts.push("source=nci".to_string());
                        }
                        if offset > 0 {
                            query_parts.push(format!("offset={offset}"));
                        }
                        let query = query_parts.join(", ");
                        Ok(crate::render::markdown::trial_search_markdown(
                            &query, &results, total,
                        )?)
                    }
                }
                VariantCommand::Articles { id, limit, offset } => {
                    let id_format = crate::entities::variant::parse_variant_id(&id)?;
                    let (gene, keyword) = match id_format {
                        crate::entities::variant::VariantIdFormat::RsId(rsid) => (None, Some(rsid)),
                        crate::entities::variant::VariantIdFormat::HgvsGenomic(hgvs) => {
                            (None, Some(hgvs))
                        }
                        crate::entities::variant::VariantIdFormat::GeneProteinChange { gene, change } => {
                            (Some(gene), Some(change))
                        }
                    };

                    let filters = crate::entities::article::ArticleSearchFilters {
                        gene,
                        gene_anchored: true,
                        keyword,
                        ..related_article_filters()
                    };

                    let query = vec![
                        filters.gene.as_deref().map(|v| format!("gene={v}")),
                        filters.keyword.as_deref().map(|v| format!("keyword={v}")),
                        (offset > 0).then(|| format!("offset={offset}")),
                    ]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(", ");

                    let fetch_limit = paged_fetch_limit(limit, offset, 50)?;
                    let rows = crate::entities::article::search(&filters, fetch_limit).await?;
                    let (results, total) = paginate_results(rows, offset, limit);
                    log_pagination_truncation(total, offset, results.len());
                    if cli.json {
                        #[derive(serde::Serialize)]
                        struct SearchResponse {
                            total: Option<usize>,
                            count: usize,
                            results: Vec<crate::entities::article::ArticleSearchResult>,
                        }

                        Ok(crate::render::json::to_pretty(&SearchResponse {
                            total: Some(total),
                            count: results.len(),
                            results,
                        })?)
                    } else {
                        Ok(crate::render::markdown::article_search_markdown_with_footer_and_context(
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
                        )?)
                    }
                }
                VariantCommand::Oncokb { id } => {
                    let result = crate::entities::variant::oncokb(&id).await?;
                    if cli.json {
                        Ok(crate::render::json::to_pretty(&result)?)
                    } else {
                        Ok(crate::render::markdown::variant_oncokb_markdown(&result))
                    }
                }
                VariantCommand::External(args) => {
                    let id = args.join(" ");
                    let variant = crate::entities::variant::get(&id, empty_sections()).await?;
                    if cli.json {
                        Ok(crate::render::json::to_entity_json(
                            &variant,
                            crate::render::markdown::variant_evidence_urls(&variant),
                            crate::render::markdown::related_variant(&variant),
                            crate::render::provenance::variant_section_sources(&variant),
                        )?)
                    } else {
                        Ok(crate::render::markdown::variant_markdown(
                            &variant,
                            empty_sections(),
                        )?)
                    }
                }
            },
            Commands::Drug { cmd } => match cmd {
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
                        log_pagination_truncation(total as usize, offset, results.len());
                    }
                    if cli.json {
                        #[derive(serde::Serialize)]
                        struct SearchResponse {
                            count: usize,
                            total: Option<u32>,
                            results: Vec<crate::entities::trial::TrialSearchResult>,
                        }

                        Ok(crate::render::json::to_pretty(&SearchResponse {
                            count: results.len(),
                            total,
                            results,
                        })?)
                    } else {
                        let query = if offset > 0 {
                            format!("intervention={name}, offset={offset}")
                        } else {
                            format!("intervention={name}")
                        };
                        Ok(crate::render::markdown::trial_search_markdown(
                            &query, &results, total,
                        )?)
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
                    let query_summary = crate::entities::adverse_event::search_query_summary(&filters);
                    let fetch_limit = paged_fetch_limit(limit, offset, 50)?;
                    let response =
                        crate::entities::adverse_event::search_with_summary(
                            &filters,
                            fetch_limit,
                            0,
                        )
                        .await?;
                    let (results, observed_total) =
                        paginate_results(response.results, offset, limit);
                    log_pagination_truncation(observed_total, offset, results.len());
                    let summary = crate::entities::adverse_event::summarize_search_results(
                        response.summary.total_reports,
                        &results,
                    );
                    if cli.json {
                        #[derive(serde::Serialize)]
                        struct SearchResponse {
                            total: Option<usize>,
                            count: usize,
                            summary: crate::entities::adverse_event::AdverseEventSearchSummary,
                            results: Vec<crate::entities::adverse_event::AdverseEventSearchResult>,
                        }

                        Ok(crate::render::json::to_pretty(&SearchResponse {
                            total: Some(summary.total_reports),
                            count: results.len(),
                            summary,
                            results,
                        })?)
                    } else {
                        Ok(crate::render::markdown::adverse_event_search_markdown(
                            &query_summary,
                            &results,
                            &summary,
                        )?)
                    }
                }
                DrugCommand::External(args) => {
                    let name = args.join(" ");
                    let drug = crate::entities::drug::get(&name, empty_sections()).await?;
                    if cli.json {
                        Ok(crate::render::json::to_entity_json(
                            &drug,
                            crate::render::markdown::drug_evidence_urls(&drug),
                            crate::render::markdown::related_drug(&drug),
                            crate::render::provenance::drug_section_sources(&drug),
                        )?)
                    } else {
                        Ok(crate::render::markdown::drug_markdown(&drug, empty_sections())?)
                    }
                }
            },
            Commands::Disease { cmd } => match cmd {
                DiseaseCommand::Trials {
                    name,
                    limit,
                    offset,
                    source,
                } => {
                    let trial_source = crate::entities::trial::TrialSource::from_flag(&source)?;
                    let filters = crate::entities::trial::TrialSearchFilters {
                        condition: Some(name.clone()),
                        source: trial_source,
                        ..Default::default()
                    };
                    let (results, total) =
                        crate::entities::trial::search(&filters, limit, offset).await?;
                    if let Some(total) = total {
                        log_pagination_truncation(total as usize, offset, results.len());
                    }
                    if cli.json {
                        #[derive(serde::Serialize)]
                        struct SearchResponse {
                            count: usize,
                            total: Option<u32>,
                            results: Vec<crate::entities::trial::TrialSearchResult>,
                        }

                        Ok(crate::render::json::to_pretty(&SearchResponse {
                            count: results.len(),
                            total,
                            results,
                        })?)
                    } else {
                        let query = if offset > 0 {
                            format!("condition={name}, offset={offset}")
                        } else {
                            format!("condition={name}")
                        };
                        Ok(crate::render::markdown::trial_search_markdown(
                            &query, &results, total,
                        )?)
                    }
                }
                DiseaseCommand::Articles {
                    name,
                    limit,
                    offset,
                } => {
                    let filters = crate::entities::article::ArticleSearchFilters {
                        disease: Some(name.clone()),
                        ..related_article_filters()
                    };

                    let query = if offset > 0 {
                        format!("disease={name}, offset={offset}")
                    } else {
                        format!("disease={name}")
                    };
                    let fetch_limit = paged_fetch_limit(limit, offset, 50)?;
                    let rows = crate::entities::article::search(&filters, fetch_limit).await?;
                    let (results, total) = paginate_results(rows, offset, limit);
                    log_pagination_truncation(total, offset, results.len());
                    if cli.json {
                        #[derive(serde::Serialize)]
                        struct SearchResponse {
                            total: Option<usize>,
                            count: usize,
                            results: Vec<crate::entities::article::ArticleSearchResult>,
                        }

                        Ok(crate::render::json::to_pretty(&SearchResponse {
                            total: Some(total),
                            count: results.len(),
                            results,
                        })?)
                    } else {
                        Ok(crate::render::markdown::article_search_markdown_with_footer_and_context(
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
                        )?)
                    }
                }
                DiseaseCommand::Drugs {
                    name,
                    limit,
                    offset,
                } => {
                    let filters = crate::entities::drug::DrugSearchFilters {
                        indication: Some(name.clone()),
                        ..Default::default()
                    };
                    let mut query_summary = crate::entities::drug::search_query_summary(&filters);
                    if offset > 0 {
                        query_summary = format!("{query_summary}, offset={offset}");
                    }
                    let fetch_limit = paged_fetch_limit(limit, offset, 50)?;
                    let rows = crate::entities::drug::search(&filters, fetch_limit).await?;
                    let (results, total) = paginate_results(rows, offset, limit);
                    log_pagination_truncation(total, offset, results.len());
                    if cli.json {
                        #[derive(serde::Serialize)]
                        struct SearchResponse {
                            total: Option<usize>,
                            count: usize,
                            results: Vec<crate::entities::drug::DrugSearchResult>,
                        }

                        Ok(crate::render::json::to_pretty(&SearchResponse {
                            total: Some(total),
                            count: results.len(),
                            results,
                        })?)
                    } else {
                        Ok(crate::render::markdown::drug_search_markdown(
                            &query_summary,
                            &results,
                        )?)
                    }
                }
            },
            Commands::Article { cmd } => match cmd {
                ArticleCommand::Entities { pmid, limit } => {
                    let limit = paged_fetch_limit(limit, 0, 50)?;
                    let sections = vec!["annotations".to_string()];
                    let article = crate::entities::article::get(&pmid, &sections).await?;
                    let annotations = article
                        .annotations
                        .clone()
                        .map(|value| truncate_article_annotations(value, limit));
                    if cli.json {
                        #[derive(serde::Serialize)]
                        struct ArticleEntitiesResponse {
                            pmid: String,
                            annotations: Option<crate::entities::article::ArticleAnnotations>,
                        }
                        Ok(crate::render::json::to_pretty(&ArticleEntitiesResponse {
                            pmid,
                            annotations,
                        })?)
                    } else {
                        Ok(crate::render::markdown::article_entities_markdown(
                            article.pmid.as_deref().unwrap_or(&pmid),
                            annotations.as_ref(),
                            Some(limit),
                        )?)
                    }
                }
                ArticleCommand::Batch { ids } => {
                    let results = crate::entities::article::get_batch_compact(&ids).await?;
                    if cli.json {
                        Ok(crate::render::json::to_pretty(&results)?)
                    } else {
                        Ok(crate::render::markdown::article_batch_markdown(&results)?)
                    }
                }
                ArticleCommand::Citations { id, limit } => {
                    let limit = paged_fetch_limit(limit, 0, 100)?;
                    let graph = crate::entities::article::citations(&id, limit).await?;
                    if cli.json {
                        Ok(crate::render::json::to_pretty(&graph)?)
                    } else {
                        Ok(crate::render::markdown::article_graph_markdown(
                            "Citations",
                            &graph,
                        )?)
                    }
                }
                ArticleCommand::References { id, limit } => {
                    let limit = paged_fetch_limit(limit, 0, 100)?;
                    let graph = crate::entities::article::references(&id, limit).await?;
                    if cli.json {
                        Ok(crate::render::json::to_pretty(&graph)?)
                    } else {
                        Ok(crate::render::markdown::article_graph_markdown(
                            "References",
                            &graph,
                        )?)
                    }
                }
                ArticleCommand::Recommendations {
                    ids,
                    negative,
                    limit,
                } => {
                    let limit = paged_fetch_limit(limit, 0, 100)?;
                    let recommendations =
                        crate::entities::article::recommendations(&ids, &negative, limit).await?;
                    if cli.json {
                        Ok(crate::render::json::to_pretty(&recommendations)?)
                    } else {
                        Ok(crate::render::markdown::article_recommendations_markdown(
                            &recommendations,
                        )?)
                    }
                }
            },
            Commands::Gene { cmd } => match cmd {
                GeneCommand::Definition { symbol } => {
                    render_gene_card(&symbol, empty_sections(), cli.json).await
                }
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
                        log_pagination_truncation(total as usize, offset, results.len());
                    }
                    if cli.json {
                        #[derive(serde::Serialize)]
                        struct SearchResponse {
                            count: usize,
                            total: Option<u32>,
                            results: Vec<crate::entities::trial::TrialSearchResult>,
                        }

                        Ok(crate::render::json::to_pretty(&SearchResponse {
                            count: results.len(),
                            total,
                            results,
                        })?)
                    } else {
                        let query = if offset > 0 {
                            format!("biomarker={symbol}, offset={offset}")
                        } else {
                            format!("biomarker={symbol}")
                        };
                        Ok(crate::render::markdown::trial_search_markdown(
                            &query, &results, total,
                        )?)
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
                    let fetch_limit = paged_fetch_limit(limit, offset, 50)?;
                    let rows = crate::entities::drug::search(&filters, fetch_limit).await?;
                    let (results, total) = paginate_results(rows, offset, limit);
                    log_pagination_truncation(total, offset, results.len());
                    if cli.json {
                        #[derive(serde::Serialize)]
                        struct SearchResponse {
                            total: Option<usize>,
                            count: usize,
                            results: Vec<crate::entities::drug::DrugSearchResult>,
                        }

                        Ok(crate::render::json::to_pretty(&SearchResponse {
                            total: Some(total),
                            count: results.len(),
                            results,
                        })?)
                    } else {
                        Ok(crate::render::markdown::drug_search_markdown(
                            &query_summary,
                            &results,
                        )?)
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
                        ..related_article_filters()
                    };
                    let query = if offset > 0 {
                        format!("gene={symbol}, offset={offset}")
                    } else {
                        format!("gene={symbol}")
                    };
                    let fetch_limit = paged_fetch_limit(limit, offset, 50)?;
                    let rows = crate::entities::article::search(&filters, fetch_limit).await?;
                    let (results, total) = paginate_results(rows, offset, limit);
                    log_pagination_truncation(total, offset, results.len());
                    if cli.json {
                        #[derive(serde::Serialize)]
                        struct SearchResponse {
                            total: Option<usize>,
                            count: usize,
                            results: Vec<crate::entities::article::ArticleSearchResult>,
                        }

                        Ok(crate::render::json::to_pretty(&SearchResponse {
                            total: Some(total),
                            count: results.len(),
                            results,
                        })?)
                    } else {
                        Ok(crate::render::markdown::article_search_markdown_with_footer_and_context(
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
                        )?)
                    }
                }
                GeneCommand::Pathways {
                    symbol,
                    limit,
                    offset,
                } => {
                    let fetch_limit = paged_fetch_limit(limit, offset, 25)?;
                    let sections = vec!["pathways".to_string()];
                    let mut gene = crate::entities::gene::get(&symbol, &sections).await?;
                    if let Some(pathways) = gene.pathways.take() {
                        let fetched = pathways.into_iter().take(fetch_limit).collect::<Vec<_>>();
                        let (results, observed_total) = paginate_results(fetched, offset, limit);
                        log_pagination_truncation(observed_total, offset, results.len());
                        gene.pathways = (!results.is_empty()).then_some(results);
                    }
                    if cli.json {
                        Ok(crate::render::json::to_pretty(&gene)?)
                    } else {
                        Ok(crate::render::markdown::gene_markdown(&gene, &sections)?)
                    }
                }
                GeneCommand::External(args) => {
                    let symbol = args.join(" ");
                    render_gene_card(&symbol, empty_sections(), cli.json).await
                }
            },
            Commands::Pathway { cmd } => match cmd {
                PathwayCommand::Drugs { id, limit, offset } => {
                    let fetch_limit = paged_fetch_limit(limit, offset, 50)?;
                    let rows = pathway_drug_results(&id, fetch_limit).await?;
                    let (results, total) = paginate_results(rows, offset, limit);
                    log_pagination_truncation(total, offset, results.len());
                    if cli.json {
                        #[derive(serde::Serialize)]
                        struct SearchResponse {
                            total: Option<usize>,
                            count: usize,
                            results: Vec<crate::entities::drug::DrugSearchResult>,
                        }

                        Ok(crate::render::json::to_pretty(&SearchResponse {
                            total: Some(total),
                            count: results.len(),
                            results,
                        })?)
                    } else {
                        let query = if offset > 0 {
                            format!("pathway={id}, offset={offset}")
                        } else {
                            format!("pathway={id}")
                        };
                        Ok(crate::render::markdown::drug_search_markdown(&query, &results)?)
                    }
                }
                PathwayCommand::Articles { id, limit, offset } => {
                    let pathway = crate::entities::pathway::get(&id, empty_sections()).await?;
                    let pathway_name = pathway.name.trim();
                    let keyword = if pathway_name.is_empty() {
                        id.clone()
                    } else {
                        pathway_name.to_string()
                    };
                    let filters = crate::entities::article::ArticleSearchFilters {
                        keyword: Some(keyword.clone()),
                        ..related_article_filters()
                    };
                    let query = if offset > 0 {
                        format!("keyword={keyword}, offset={offset}")
                    } else {
                        format!("keyword={keyword}")
                    };
                    let fetch_limit = paged_fetch_limit(limit, offset, 50)?;
                    let rows = crate::entities::article::search(&filters, fetch_limit).await?;
                    let (results, total) = paginate_results(rows, offset, limit);
                    log_pagination_truncation(total, offset, results.len());
                    if cli.json {
                        #[derive(serde::Serialize)]
                        struct SearchResponse {
                            total: Option<usize>,
                            count: usize,
                            results: Vec<crate::entities::article::ArticleSearchResult>,
                        }

                        Ok(crate::render::json::to_pretty(&SearchResponse {
                            total: Some(total),
                            count: results.len(),
                            results,
                        })?)
                    } else {
                        Ok(crate::render::markdown::article_search_markdown_with_footer_and_context(
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
                        )?)
                    }
                }
                PathwayCommand::Trials {
                    id,
                    limit,
                    offset,
                    source,
                } => {
                    let pathway = crate::entities::pathway::get(&id, empty_sections()).await?;
                    let pathway_name = pathway.name.trim();
                    let condition = if pathway_name.is_empty() {
                        id.clone()
                    } else {
                        pathway_name.to_string()
                    };
                    let trial_source = crate::entities::trial::TrialSource::from_flag(&source)?;
                    let filters = crate::entities::trial::TrialSearchFilters {
                        condition: Some(condition.clone()),
                        source: trial_source,
                        ..Default::default()
                    };
                    let (mut results, mut total) =
                        crate::entities::trial::search(&filters, limit, offset).await?;
                    let mut query = if offset > 0 {
                        format!("condition={condition}, offset={offset}")
                    } else {
                        format!("condition={condition}")
                    };

                    if should_try_pathway_trial_fallback(results.len(), offset, total) {
                        let pathway_with_genes =
                            crate::entities::pathway::get(&id, &["genes".to_string()]).await?;
                        let fallback_limit = limit.saturating_add(offset).clamp(1, 50);

                        for gene in pathway_with_genes.genes.into_iter().take(10) {
                            let gene = gene.trim().to_string();
                            if gene.is_empty() {
                                continue;
                            }

                            let fallback_filters = crate::entities::trial::TrialSearchFilters {
                                biomarker: Some(gene.clone()),
                                source: trial_source,
                                ..Default::default()
                            };

                            match crate::entities::trial::search(&fallback_filters, fallback_limit, 0)
                                .await
                            {
                                Ok((fallback_rows, fallback_total)) if !fallback_rows.is_empty() => {
                                    debug!(
                                        pathway_id = %id,
                                        fallback_gene = %gene,
                                        "Pathway trial condition search returned no rows; using biomarker fallback",
                                    );
                                    results =
                                        fallback_rows.into_iter().skip(offset).take(limit).collect();
                                    total = fallback_total;
                                    query = if offset > 0 {
                                        format!(
                                            "condition={condition}, fallback_biomarker={gene}, offset={offset}"
                                        )
                                    } else {
                                        format!("condition={condition}, fallback_biomarker={gene}")
                                    };
                                    break;
                                }
                                Ok(_) => {}
                                Err(err) => {
                                    warn!(pathway_id = %id, fallback_gene = %gene, "Pathway trial fallback failed: {err}");
                                }
                            }
                        }
                    }

                    if let Some(total) = total {
                        log_pagination_truncation(total as usize, offset, results.len());
                    }
                    if cli.json {
                        #[derive(serde::Serialize)]
                        struct SearchResponse {
                            count: usize,
                            total: Option<u32>,
                            results: Vec<crate::entities::trial::TrialSearchResult>,
                        }

                        Ok(crate::render::json::to_pretty(&SearchResponse {
                            count: results.len(),
                            total,
                            results,
                        })?)
                    } else {
                        Ok(crate::render::markdown::trial_search_markdown(
                            &query, &results, total,
                        )?)
                    }
                }
            },
            Commands::Protein { cmd } => match cmd {
                ProteinCommand::Structures {
                    accession,
                    limit,
                    offset,
                } => {
                    let sections = vec!["structures".to_string()];
                    let protein = crate::entities::protein::get_with_structure_limit(
                        &accession,
                        &sections,
                        Some(limit),
                        Some(offset),
                    )
                    .await?;
                    if cli.json {
                        Ok(crate::render::json::to_pretty(&protein)?)
                    } else {
                        Ok(crate::render::markdown::protein_markdown(&protein, &sections)?)
                    }
                }
            },
            Commands::Study { cmd } => match cmd {
                StudyCommand::List => {
                    let studies = crate::entities::study::list_studies().await?;
                    if cli.json {
                        Ok(crate::render::json::to_pretty(&studies)?)
                    } else {
                        Ok(crate::render::markdown::study_list_markdown(&studies))
                    }
                }
                StudyCommand::Download { list, study_id } => {
                    if list {
                        let result = crate::entities::study::list_downloadable_studies().await?;
                        if cli.json {
                            Ok(crate::render::json::to_pretty(&result)?)
                        } else {
                            Ok(crate::render::markdown::study_download_catalog_markdown(
                                &result,
                            ))
                        }
                    } else {
                        let study_id = study_id.expect("clap should require study_id");
                        let result = crate::entities::study::download_study(&study_id).await?;
                        if cli.json {
                            Ok(crate::render::json::to_pretty(&result)?)
                        } else {
                            Ok(crate::render::markdown::study_download_markdown(&result))
                        }
                    }
                }
                StudyCommand::Query {
                    study,
                    gene,
                    query_type,
                    chart,
                } => {
                    let query_type = crate::entities::study::StudyQueryType::from_flag(&query_type)?;
                    chart_json_conflict(&chart, cli.json)?;
                    if let Some(chart_type) = chart.chart {
                        crate::render::chart::validate_query_chart_type(query_type, chart_type)?;
                        let options = crate::render::chart::ChartRenderOptions::from(&chart);
                        match query_type {
                            crate::entities::study::StudyQueryType::Mutations => {
                                match chart_type {
                                    ChartType::Waterfall => {
                                        let sample_counts =
                                            crate::entities::study::mutation_counts_by_sample(
                                                &study, &gene,
                                            )
                                            .await?;
                                        Ok(crate::render::chart::render_mutation_waterfall_chart(
                                            &study,
                                            &gene,
                                            &sample_counts,
                                            &options,
                                        )?)
                                    }
                                    ChartType::Bar | ChartType::Pie => {
                                        let result = crate::entities::study::query_study(
                                            &study, &gene, query_type,
                                        )
                                        .await?;
                                        let crate::entities::study::StudyQueryResult::MutationFrequency(
                                            result,
                                        ) = result
                                        else {
                                            unreachable!(
                                                "mutation query should return mutation result"
                                            );
                                        };
                                        Ok(crate::render::chart::render_mutation_frequency_chart(
                                            &result,
                                            chart_type,
                                            &options,
                                        )?)
                                    }
                                    other => {
                                        Err(crate::error::BioMcpError::InvalidArgument(
                                            format!("Invalid chart type: {other}"),
                                        )
                                        .into())
                                    }
                                }
                            }
                            crate::entities::study::StudyQueryType::Cna => {
                                let result =
                                    crate::entities::study::query_study(&study, &gene, query_type)
                                        .await?;
                                let crate::entities::study::StudyQueryResult::CnaDistribution(
                                    result,
                                ) = result
                                else {
                                    unreachable!("cna query should return cna result");
                                };
                                Ok(crate::render::chart::render_cna_chart(
                                    &result,
                                    chart_type,
                                    &options,
                                )?)
                            }
                            crate::entities::study::StudyQueryType::Expression => Ok(
                                match chart_type {
                                    ChartType::Histogram => {
                                        let values =
                                            crate::entities::study::expression_values(&study, &gene)
                                                .await?;
                                        crate::render::chart::render_expression_histogram_chart(
                                            &study, &gene, &values, &options,
                                        )?
                                    }
                                    ChartType::Density => {
                                        let values =
                                            crate::entities::study::expression_values(&study, &gene)
                                                .await?;
                                        crate::render::chart::render_expression_density_chart(
                                            &study, &gene, &values, &options,
                                        )?
                                    }
                                    other => {
                                        return Err(crate::error::BioMcpError::InvalidArgument(
                                            format!("Invalid chart type: {other}"),
                                        )
                                        .into());
                                    }
                                },
                            ),
                        }
                    } else {
                        let result =
                            crate::entities::study::query_study(&study, &gene, query_type).await?;
                        if cli.json {
                            Ok(crate::render::json::to_pretty(&result)?)
                        } else {
                            Ok(crate::render::markdown::study_query_markdown(&result))
                        }
                    }
                }
                StudyCommand::TopMutated { study, limit } => {
                    let result = crate::entities::study::top_mutated_genes(&study, limit).await?;
                    if cli.json {
                        Ok(crate::render::json::to_pretty(&result)?)
                    } else {
                        Ok(crate::render::markdown::study_top_mutated_markdown(&result))
                    }
                }
                StudyCommand::Filter {
                    study,
                    mutated,
                    amplified,
                    deleted,
                    expression_above,
                    expression_below,
                    cancer_type,
                } => {
                    let mut criteria = Vec::new();
                    for gene in mutated {
                        criteria.push(crate::entities::study::FilterCriterion::Mutated(gene));
                    }
                    for gene in amplified {
                        criteria.push(crate::entities::study::FilterCriterion::Amplified(gene));
                    }
                    for gene in deleted {
                        criteria.push(crate::entities::study::FilterCriterion::Deleted(gene));
                    }
                    for value in expression_above {
                        criteria.push(parse_expression_filter(
                            &value,
                            "--expression-above",
                            crate::entities::study::FilterCriterion::ExpressionAbove,
                        )?);
                    }
                    for value in expression_below {
                        criteria.push(parse_expression_filter(
                            &value,
                            "--expression-below",
                            crate::entities::study::FilterCriterion::ExpressionBelow,
                        )?);
                    }
                    for value in cancer_type {
                        criteria.push(crate::entities::study::FilterCriterion::CancerType(value));
                    }
                    if criteria.is_empty() {
                        return Err(crate::error::BioMcpError::InvalidArgument(
                            crate::entities::study::filter_required_message().to_string(),
                        )
                        .into());
                    }

                    let result = crate::entities::study::filter(&study, criteria).await?;
                    if cli.json {
                        Ok(crate::render::json::to_pretty(&result)?)
                    } else {
                        Ok(crate::render::markdown::study_filter_markdown(&result))
                    }
                }
                StudyCommand::Cohort { study, gene } => {
                    let result = crate::entities::study::cohort(&study, &gene).await?;
                    if cli.json {
                        Ok(crate::render::json::to_pretty(&result)?)
                    } else {
                        Ok(crate::render::markdown::study_cohort_markdown(&result))
                    }
                }
                StudyCommand::Survival {
                    study,
                    gene,
                    endpoint,
                    chart,
                } => {
                    let endpoint = crate::entities::study::SurvivalEndpoint::from_flag(&endpoint)?;
                    chart_json_conflict(&chart, cli.json)?;
                    if let Some(chart_type) = chart.chart {
                        crate::render::chart::validate_standalone_chart_type(
                            "study survival",
                            chart_type,
                            &[ChartType::Bar, ChartType::Survival],
                        )?;
                        let result = crate::entities::study::survival(&study, &gene, endpoint).await?;
                        let options = crate::render::chart::ChartRenderOptions::from(&chart);
                        Ok(crate::render::chart::render_survival_chart(
                            &result,
                            chart_type,
                            &options,
                        )?)
                    } else {
                        let result = crate::entities::study::survival(&study, &gene, endpoint).await?;
                        if cli.json {
                            Ok(crate::render::json::to_pretty(&result)?)
                        } else {
                            Ok(crate::render::markdown::study_survival_markdown(&result))
                        }
                    }
                }
                StudyCommand::Compare {
                    study,
                    gene,
                    compare_type,
                    target,
                    chart,
                } => {
                    chart_json_conflict(&chart, cli.json)?;
                    match compare_type.trim().to_ascii_lowercase().as_str() {
                        "expression" | "expr" => {
                            if let Some(chart_type) = chart.chart {
                                crate::render::chart::validate_compare_chart_type(
                                    "expression",
                                    chart_type,
                                )?;
                                let options = crate::render::chart::ChartRenderOptions::from(&chart);
                                match chart_type {
                                    ChartType::Scatter => {
                                        let points =
                                            crate::entities::study::expression_pairs_by_sample(
                                                &study, &gene, &target,
                                            )
                                            .await?;
                                        Ok(crate::render::chart::render_expression_scatter_chart(
                                            &study,
                                            &gene,
                                            &target,
                                            &points,
                                            &options,
                                        )?)
                                    }
                                    ChartType::Box | ChartType::Violin | ChartType::Ridgeline => {
                                        let groups =
                                            crate::entities::study::compare_expression_values(
                                                &study, &gene, &target,
                                            )
                                            .await?;
                                        Ok(crate::render::chart::render_expression_compare_chart(
                                            &study,
                                            &gene,
                                            &target,
                                            &groups,
                                            chart_type,
                                            &options,
                                        )?)
                                    }
                                    other => {
                                        Err(crate::error::BioMcpError::InvalidArgument(
                                            format!("Invalid chart type: {other}"),
                                        )
                                        .into())
                                    }
                                }
                            } else {
                                let result =
                                    crate::entities::study::compare_expression(&study, &gene, &target)
                                        .await?;
                                if cli.json {
                                    Ok(crate::render::json::to_pretty(&result)?)
                                } else {
                                    Ok(crate::render::markdown::study_compare_expression_markdown(
                                        &result,
                                    ))
                                }
                            }
                        }
                        "mutations" | "mutation" => {
                            if let Some(chart_type) = chart.chart {
                                crate::render::chart::validate_compare_chart_type(
                                    "mutations",
                                    chart_type,
                                )?;
                                let result =
                                    crate::entities::study::compare_mutations(&study, &gene, &target)
                                        .await?;
                                let options = crate::render::chart::ChartRenderOptions::from(&chart);
                                Ok(crate::render::chart::render_mutation_compare_chart(
                                    &result,
                                    chart_type,
                                    &options,
                                )?)
                            } else {
                                let result =
                                    crate::entities::study::compare_mutations(&study, &gene, &target)
                                        .await?;
                                if cli.json {
                                    Ok(crate::render::json::to_pretty(&result)?)
                                } else {
                                    Ok(crate::render::markdown::study_compare_mutations_markdown(
                                        &result,
                                    ))
                                }
                            }
                        }
                        other => Err(crate::error::BioMcpError::InvalidArgument(format!(
                            "Unknown comparison type '{other}'. Expected: expression, mutations."
                        ))
                        .into()),
                    }
                }
                StudyCommand::CoOccurrence { study, genes, chart } => {
                    chart_json_conflict(&chart, cli.json)?;
                    let genes = genes
                        .split(',')
                        .map(str::trim)
                        .filter(|gene| !gene.is_empty())
                        .map(str::to_string)
                        .collect::<Vec<_>>();
                    if genes.len() < 2 || genes.len() > 10 {
                        return Err(crate::error::BioMcpError::InvalidArgument(
                            "--genes must contain 2 to 10 comma-separated symbols".into(),
                        )
                        .into());
                    }
                    if let Some(chart_type) = chart.chart {
                        crate::render::chart::validate_standalone_chart_type(
                            "study co-occurrence",
                            chart_type,
                            &[ChartType::Bar, ChartType::Pie, ChartType::Heatmap],
                        )?;
                        let result = crate::entities::study::co_occurrence(&study, &genes).await?;
                        let options = crate::render::chart::ChartRenderOptions::from(&chart);
                        Ok(crate::render::chart::render_co_occurrence_chart(
                            &result,
                            chart_type,
                            &options,
                        )?)
                    } else {
                        let result = crate::entities::study::co_occurrence(&study, &genes).await?;
                        if cli.json {
                            Ok(crate::render::json::to_pretty(&result)?)
                        } else {
                            Ok(crate::render::markdown::study_co_occurrence_markdown(&result))
                        }
                    }
                }
            },
            Commands::Batch(system::BatchArgs {
                entity,
                ids,
                sections,
                source,
            }) => {
                let entity = entity.trim().to_ascii_lowercase();
                let parsed_ids = ids
                    .split(',')
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .collect::<Vec<_>>();
                let batch_sections = parse_batch_sections(sections.as_deref());

                if parsed_ids.is_empty() {
                    return Err(crate::error::BioMcpError::InvalidArgument(
                        "Batch IDs are required. Example: biomcp batch gene BRAF,TP53".into(),
                    )
                    .into());
                }
                if parsed_ids.len() > 10 {
                    return Err(crate::error::BioMcpError::InvalidArgument(
                        "Batch is limited to 10 IDs".into(),
                    )
                    .into());
                }

                match entity.as_str() {
                    "gene" => {
                        let futs = parsed_ids
                            .iter()
                            .map(|id| crate::entities::gene::get(id, &batch_sections));
                        let results = try_join_all(futs).await?;
                        if cli.json {
                            Ok(render_batch_json(&results, |item| {
                                crate::render::json::to_entity_json_value(
                                    item,
                                    crate::render::markdown::gene_evidence_urls(item),
                                    crate::render::markdown::related_gene(item),
                                    crate::render::provenance::gene_section_sources(item),
                                )
                            })?)
                        } else {
                            let mut out = String::new();
                            out.push_str(&format!("# Batch: gene ({})\n\n", results.len()));
                            for (idx, item) in results.iter().enumerate() {
                                if idx > 0 {
                                    out.push_str("\n\n---\n\n");
                                }
                                out.push_str(&crate::render::markdown::gene_markdown(
                                    item,
                                    &batch_sections,
                                )?);
                            }
                            Ok(out)
                        }
                    }
                    "variant" => {
                        let futs = parsed_ids
                            .iter()
                            .map(|id| crate::entities::variant::get(id, &batch_sections));
                        let results = try_join_all(futs).await?;
                        if cli.json {
                            Ok(render_batch_json(&results, |item| {
                                crate::render::json::to_entity_json_value(
                                    item,
                                    crate::render::markdown::variant_evidence_urls(item),
                                    crate::render::markdown::related_variant(item),
                                    crate::render::provenance::variant_section_sources(item),
                                )
                            })?)
                        } else {
                            let mut out = String::new();
                            out.push_str(&format!("# Batch: variant ({})\n\n", results.len()));
                            for (idx, item) in results.iter().enumerate() {
                                if idx > 0 {
                                    out.push_str("\n\n---\n\n");
                                }
                                out.push_str(&crate::render::markdown::variant_markdown(
                                    item,
                                    &batch_sections,
                                )?);
                            }
                            Ok(out)
                        }
                    }
                    "article" => {
                        let futs = parsed_ids
                            .iter()
                            .map(|id| crate::entities::article::get(id, &batch_sections));
                        let results = try_join_all(futs).await?;
                        if cli.json {
                            Ok(render_batch_json(&results, |item| {
                                crate::render::json::to_entity_json_value(
                                    item,
                                    crate::render::markdown::article_evidence_urls(item),
                                    crate::render::markdown::related_article(item),
                                    crate::render::provenance::article_section_sources(item),
                                )
                            })?)
                        } else {
                            let mut out = String::new();
                            out.push_str(&format!("# Batch: article ({})\n\n", results.len()));
                            for (idx, item) in results.iter().enumerate() {
                                if idx > 0 {
                                    out.push_str("\n\n---\n\n");
                                }
                                out.push_str(&crate::render::markdown::article_markdown(
                                    item,
                                    &batch_sections,
                                )?);
                            }
                            Ok(out)
                        }
                    }
                    "trial" => {
                        let trial_source = crate::entities::trial::TrialSource::from_flag(&source)?;
                        let futs = parsed_ids.iter().map(|id| {
                            crate::entities::trial::get(id, &batch_sections, trial_source)
                        });
                        let results = try_join_all(futs).await?;
                        if cli.json {
                            Ok(render_batch_json(&results, |item| {
                                crate::render::json::to_entity_json_value(
                                    item,
                                    crate::render::markdown::trial_evidence_urls(item),
                                    crate::render::markdown::related_trial(item),
                                    crate::render::provenance::trial_section_sources(item),
                                )
                            })?)
                        } else {
                            let mut out = String::new();
                            out.push_str(&format!("# Batch: trial ({})\n\n", results.len()));
                            for (idx, item) in results.iter().enumerate() {
                                if idx > 0 {
                                    out.push_str("\n\n---\n\n");
                                }
                                out.push_str(&crate::render::markdown::trial_markdown(
                                    item,
                                    &batch_sections,
                                )?);
                            }
                            Ok(out)
                        }
                    }
                    "drug" => {
                        let futs = parsed_ids
                            .iter()
                            .map(|id| crate::entities::drug::get(id, &batch_sections));
                        let results = try_join_all(futs).await?;
                        if cli.json {
                            Ok(render_batch_json(&results, |item| {
                                crate::render::json::to_entity_json_value(
                                    item,
                                    crate::render::markdown::drug_evidence_urls(item),
                                    crate::render::markdown::related_drug(item),
                                    crate::render::provenance::drug_section_sources(item),
                                )
                            })?)
                        } else {
                            let mut out = String::new();
                            out.push_str(&format!("# Batch: drug ({})\n\n", results.len()));
                            for (idx, item) in results.iter().enumerate() {
                                if idx > 0 {
                                    out.push_str("\n\n---\n\n");
                                }
                                out.push_str(&crate::render::markdown::drug_markdown(
                                    item,
                                    &batch_sections,
                                )?);
                            }
                            Ok(out)
                        }
                    }
                    "disease" => {
                        let futs = parsed_ids
                            .iter()
                            .map(|id| crate::entities::disease::get(id, &batch_sections));
                        let results = try_join_all(futs).await?;
                        if cli.json {
                            Ok(render_batch_json(&results, |item| {
                                crate::render::json::to_entity_json_value(
                                    item,
                                    crate::render::markdown::disease_evidence_urls(item),
                                    crate::render::markdown::related_disease(item),
                                    crate::render::provenance::disease_section_sources(item),
                                )
                            })?)
                        } else {
                            let mut out = String::new();
                            out.push_str(&format!("# Batch: disease ({})\n\n", results.len()));
                            for (idx, item) in results.iter().enumerate() {
                                if idx > 0 {
                                    out.push_str("\n\n---\n\n");
                                }
                                out.push_str(&crate::render::markdown::disease_markdown(
                                    item,
                                    &batch_sections,
                                )?);
                            }
                            Ok(out)
                        }
                    }
                    "pgx" => {
                        let futs = parsed_ids
                            .iter()
                            .map(|id| crate::entities::pgx::get(id, &batch_sections));
                        let results = try_join_all(futs).await?;
                        if cli.json {
                            Ok(render_batch_json(&results, |item| {
                                crate::render::json::to_entity_json_value(
                                    item,
                                    crate::render::markdown::pgx_evidence_urls(item),
                                    crate::render::markdown::related_pgx(item),
                                    crate::render::provenance::pgx_section_sources(item),
                                )
                            })?)
                        } else {
                            let mut out = String::new();
                            out.push_str(&format!("# Batch: pgx ({})\n\n", results.len()));
                            for (idx, item) in results.iter().enumerate() {
                                if idx > 0 {
                                    out.push_str("\n\n---\n\n");
                                }
                                out.push_str(&crate::render::markdown::pgx_markdown(
                                    item,
                                    &batch_sections,
                                )?);
                            }
                            Ok(out)
                        }
                    }
                    "pathway" => {
                        let futs = parsed_ids
                            .iter()
                            .map(|id| crate::entities::pathway::get(id, &batch_sections));
                        let results = try_join_all(futs).await?;
                        if cli.json {
                            Ok(render_batch_json(&results, |item| {
                                crate::render::json::to_entity_json_value(
                                    item,
                                    crate::render::markdown::pathway_evidence_urls(item),
                                    crate::render::markdown::related_pathway(item),
                                    crate::render::provenance::pathway_section_sources(item),
                                )
                            })?)
                        } else {
                            let mut out = String::new();
                            out.push_str(&format!("# Batch: pathway ({})\n\n", results.len()));
                            for (idx, item) in results.iter().enumerate() {
                                if idx > 0 {
                                    out.push_str("\n\n---\n\n");
                                }
                                out.push_str(&crate::render::markdown::pathway_markdown(
                                    item,
                                    &batch_sections,
                                )?);
                            }
                            Ok(out)
                        }
                    }
                    "protein" => {
                        let futs = parsed_ids
                            .iter()
                            .map(|id| crate::entities::protein::get(id, &batch_sections));
                        let results = try_join_all(futs).await?;
                        if cli.json {
                            Ok(render_batch_json(&results, |item| {
                                crate::render::json::to_entity_json_value(
                                    item,
                                    crate::render::markdown::protein_evidence_urls(item),
                                    crate::render::markdown::related_protein(item, &batch_sections),
                                    crate::render::provenance::protein_section_sources(item),
                                )
                            })?)
                        } else {
                            let mut out = String::new();
                            out.push_str(&format!("# Batch: protein ({})\n\n", results.len()));
                            for (idx, item) in results.iter().enumerate() {
                                if idx > 0 {
                                    out.push_str("\n\n---\n\n");
                                }
                                out.push_str(&crate::render::markdown::protein_markdown(
                                    item,
                                    &batch_sections,
                                )?);
                            }
                            Ok(out)
                        }
                    }
                    "adverse-event" | "adverse_event" | "adverseevent" => {
                        if !batch_sections.is_empty() {
                            return Err(crate::error::BioMcpError::InvalidArgument(
                                "Batch sections are not supported for adverse-event".into(),
                            )
                            .into());
                        }
                        let futs = parsed_ids.iter().map(|id| crate::entities::adverse_event::get(id));
                        let results = try_join_all(futs).await?;
                        if cli.json {
                            Ok(render_batch_json(&results, |item| match item {
                                crate::entities::adverse_event::AdverseEventReport::Faers(report) => {
                                    crate::render::json::to_entity_json_value(
                                        item,
                                        crate::render::markdown::adverse_event_evidence_urls(report),
                                        crate::render::markdown::related_adverse_event(report),
                                        crate::render::provenance::adverse_event_report_section_sources(
                                            item,
                                        ),
                                    )
                                }
                                crate::entities::adverse_event::AdverseEventReport::Device(report) => {
                                    crate::render::json::to_entity_json_value(
                                        item,
                                        crate::render::markdown::device_event_evidence_urls(report),
                                        crate::render::markdown::related_device_event(report),
                                        crate::render::provenance::adverse_event_report_section_sources(
                                            item,
                                        ),
                                    )
                                }
                            })?)
                        } else {
                            let mut out = String::new();
                            out.push_str(&format!("# Batch: adverse-event ({})\n\n", results.len()));
                            for (idx, item) in results.iter().enumerate() {
                                if idx > 0 {
                                    out.push_str("\n\n---\n\n");
                                }
                                match item {
                                    crate::entities::adverse_event::AdverseEventReport::Faers(r) => {
                                        out.push_str(
                                            &crate::render::markdown::adverse_event_markdown(
                                                r,
                                                empty_sections(),
                                            )?,
                                        );
                                    }
                                    crate::entities::adverse_event::AdverseEventReport::Device(r) => {
                                        out.push_str(
                                            &crate::render::markdown::device_event_markdown(r)?,
                                        );
                                    }
                                }
                            }
                            Ok(out)
                        }
                    }
                    other => Err(crate::error::BioMcpError::InvalidArgument(format!(
                        "Unknown batch entity '{other}'. Expected one of: gene, variant, article, trial, drug, disease, pgx, pathway, protein, adverse-event"
                    ))
                    .into()),
                }
            }
            Commands::Search { entity } => {
                match entity {
                SearchEntity::All(search_all_command::SearchAllArgs {
                    gene,
                    variant,
                    disease,
                    drug,
                    keyword,
                    positional_query,
                    since,
                    limit,
                    counts_only,
                    debug_plan,
                }) => {
                    let keyword = resolve_query_input(keyword, positional_query, "--keyword")?;
                    let input = crate::cli::search_all::SearchAllInput {
                        gene,
                        variant,
                        disease,
                        drug,
                        keyword,
                        since,
                        limit,
                        counts_only,
                        debug_plan,
                    };
                    let results = crate::cli::search_all::dispatch(&input).await?;
                    if cli.json {
                        Ok(crate::render::json::to_pretty(&results)?)
                    } else {
                        Ok(crate::render::markdown::search_all_markdown(
                            &results,
                            input.counts_only,
                        )?)
                    }
                }
                SearchEntity::Gene(gene::GeneSearchArgs {
                    query,
                    positional_query,
                    gene_type,
                    chromosome,
                    region,
                    pathway,
                    go_term,
                    limit,
                    offset,
                }) => {
                    let query = resolve_query_input(query, positional_query, "--query")?;
                    let filters = crate::entities::gene::GeneSearchFilters {
                        query,
                        gene_type,
                        chromosome,
                        region,
                        pathway,
                        go_term,
                    };
                    let mut query_summary = crate::entities::gene::search_query_summary(&filters);
                    if offset > 0 {
                        query_summary = format!("{query_summary}, offset={offset}");
                    }
                    let page = crate::entities::gene::search_page(&filters, limit, offset).await?;
                    let results = page.results;
                    let pagination =
                        PaginationMeta::offset(offset, limit, results.len(), page.total);
                    if cli.json {
                        search_json(results, pagination)
                    } else {
                        let footer = pagination_footer_offset(&pagination);
                        Ok(crate::render::markdown::gene_search_markdown_with_footer(
                            &query_summary,
                            &results,
                            &footer,
                        )?)
                    }
                }
                SearchEntity::Disease(disease::DiseaseSearchArgs {
                    query,
                    positional_query,
                    source,
                    inheritance,
                    phenotype,
                    onset,
                    no_fallback,
                    limit,
                    offset,
                }) => {
                    let query = resolve_query_input(query, positional_query, "--query")?;
                    let filters = crate::entities::disease::DiseaseSearchFilters {
                        query,
                        source,
                        inheritance,
                        phenotype,
                        onset,
                    };
                    let mut query_summary = crate::entities::disease::search_query_summary(&filters);
                    if offset > 0 {
                        query_summary = format!("{query_summary}, offset={offset}");
                    }
                    let mut page =
                        crate::entities::disease::search_page(&filters, limit, offset).await?;
                    let mut fallback_used = false;
                    if page.results.is_empty() && !no_fallback
                        && let Some(fallback_page) =
                            crate::entities::disease::fallback_search_page(&filters, limit, offset)
                                .await?
                    {
                        page = fallback_page;
                        fallback_used = true;
                    }
                    let results = page.results;
                    let pagination = PaginationMeta::offset(offset, limit, results.len(), page.total);
                    if cli.json {
                        disease_search_json(results, pagination, fallback_used)
                    } else {
                        let footer = pagination_footer_offset(&pagination);
                        Ok(crate::render::markdown::disease_search_markdown_with_footer(
                            filters.query.as_deref().map(str::trim).unwrap_or_default(),
                            &query_summary,
                            &results,
                            fallback_used,
                            &footer,
                        )?)
                    }
                }
                SearchEntity::Pgx(pgx::PgxSearchArgs {
                    gene,
                    positional_query,
                    drug,
                    cpic_level,
                    pgx_testing,
                    evidence,
                    limit,
                    offset,
                }) => {
                    let gene = resolve_query_input(gene, positional_query, "--gene")?;
                    let filters = crate::entities::pgx::PgxSearchFilters {
                        gene,
                        drug,
                        cpic_level,
                        pgx_testing,
                        evidence,
                    };
                    let mut query_summary = crate::entities::pgx::search_query_summary(&filters);
                    if offset > 0 {
                        query_summary = format!("{query_summary}, offset={offset}");
                    }
                    let page = crate::entities::pgx::search_page(&filters, limit, offset).await?;
                    let results = page.results;
                    let pagination =
                        PaginationMeta::offset(offset, limit, results.len(), page.total);
                    if cli.json {
                        search_json(results, pagination)
                    } else {
                        let footer = pagination_footer_offset(&pagination);
                        Ok(crate::render::markdown::pgx_search_markdown_with_footer(
                            &query_summary,
                            &results,
                            &footer,
                        )?)
                    }
                }
                SearchEntity::Phenotype(phenotype::PhenotypeSearchArgs {
                    terms,
                    limit,
                    offset,
                }) => {
                    let mut query_summary = terms.trim().to_string();
                    if offset > 0 {
                        query_summary = format!("{query_summary}, offset={offset}");
                    }
                    let page =
                        crate::entities::disease::search_phenotype_page(&terms, limit, offset)
                            .await?;
                    let results = page.results;
                    let pagination =
                        PaginationMeta::offset(offset, limit, results.len(), page.total);
                    if cli.json {
                        search_json(results, pagination)
                    } else {
                        let footer = pagination_footer_offset(&pagination);
                        Ok(crate::render::markdown::phenotype_search_markdown_with_footer(
                            &query_summary,
                            &results,
                            &footer,
                        )?)
                    }
                }
                SearchEntity::Gwas(gwas::GwasSearchArgs {
                    gene,
                    positional_query,
                    trait_query,
                    region,
                    p_value,
                    limit,
                    offset,
                }) => {
                    let gene = resolve_query_input(gene, positional_query, "--gene")?;
                    let filters = crate::entities::variant::GwasSearchFilters {
                        gene,
                        trait_query,
                        region,
                        p_value,
                    };
                    let mut query_summary = crate::entities::variant::gwas_search_query_summary(&filters);
                    if offset > 0 {
                        query_summary = format!("{query_summary}, offset={offset}");
                    }
                    let page =
                        crate::entities::variant::search_gwas_page(&filters, limit, offset)
                            .await?;
                    let results = page.results;
                    let pagination =
                        PaginationMeta::offset(offset, limit, results.len(), page.total);
                    if cli.json {
                        search_json(results, pagination)
                    } else {
                        let footer = pagination_footer_offset(&pagination);
                        Ok(crate::render::markdown::gwas_search_markdown_with_footer(
                            &query_summary,
                            &results,
                            &footer,
                        )?)
                    }
                }
                SearchEntity::Article(article::ArticleSearchArgs {
                    gene,
                    disease,
                    drug,
                    author,
                    keyword,
                    positional_query,
                    date_from,
                    date_to,
                    article_type,
                    journal,
                    open_access,
                    no_preprints,
                    exclude_retracted,
                    include_retracted,
                    sort,
                    ranking_mode,
                    weight_semantic,
                    weight_lexical,
                    weight_citations,
                    weight_position,
                    source,
                    max_per_source,
                    limit,
                    offset,
                    debug_plan,
                }) => {
                    let disease = normalize_cli_tokens(disease);
                    let drug = normalize_cli_tokens(drug);
                    let author = normalize_cli_tokens(author);
                    let keyword = resolve_query_input(
                        normalize_cli_tokens(keyword),
                        positional_query,
                        "--keyword/--query",
                    )?;
                    let journal = normalize_cli_tokens(journal);
                    let sort = crate::entities::article::ArticleSort::from_flag(&sort)?;
                    let source_filter =
                        crate::entities::article::ArticleSourceFilter::from_flag(&source)?;
                    let exclude_retracted = exclude_retracted || !include_retracted;
                    let ranking = crate::entities::article::ArticleRankingOptions::from_inputs(
                        ranking_mode.as_deref(),
                        weight_semantic,
                        weight_lexical,
                        weight_citations,
                        weight_position,
                    )?;
                    let gene_anchored = gene
                        .as_deref()
                        .map(str::trim)
                        .is_some_and(|value| !value.is_empty())
                        && disease
                            .as_deref()
                            .map(str::trim)
                            .is_none_or(str::is_empty)
                        && drug
                            .as_deref()
                            .map(str::trim)
                            .is_none_or(str::is_empty)
                        && author
                            .as_deref()
                            .map(str::trim)
                            .is_none_or(str::is_empty)
                        && keyword
                            .as_deref()
                            .map(str::trim)
                            .is_none_or(str::is_empty);
                    let filters = crate::entities::article::ArticleSearchFilters {
                        gene,
                        gene_anchored,
                        disease,
                        drug,
                        author,
                        keyword,
                        date_from,
                        date_to,
                        article_type,
                        journal,
                        open_access,
                        no_preprints,
                        exclude_retracted,
                        max_per_source,
                        sort,
                        ranking,
                    };

                    let query = article_query_summary(
                        &filters,
                        source_filter,
                        include_retracted,
                        limit,
                        offset,
                    );

                    let page =
                        crate::entities::article::search_page(&filters, limit, offset, source_filter)
                            .await?;
                    let results = page.results;
                    let pagination =
                        PaginationMeta::offset(offset, limit, results.len(), page.total);
                    let semantic_scholar_enabled =
                        crate::entities::article::semantic_scholar_search_enabled(
                            &filters,
                            source_filter,
                        );
                    let debug_plan = if debug_plan {
                        Some(build_article_debug_plan(
                            &query,
                            &filters,
                            source_filter,
                            limit,
                            &results,
                            &pagination,
                        )?)
                    } else {
                        None
                    };
                    if cli.json {
                        article_search_json(
                            &query,
                            &filters,
                            semantic_scholar_enabled,
                            crate::entities::article::article_type_limitation_note(
                                &filters,
                                source_filter,
                            ),
                            debug_plan,
                            results,
                            pagination,
                        )
                    } else {
                        let footer = pagination_footer_offset(&pagination);
                        Ok(crate::render::markdown::article_search_markdown_with_footer_and_context(
                            &query,
                            &results,
                            &footer,
                            &filters,
                            semantic_scholar_enabled,
                            crate::entities::article::article_type_limitation_note(
                                &filters,
                                source_filter,
                            )
                            .as_deref(),
                            debug_plan.as_ref(),
                        )?)
                    }
                }
                SearchEntity::Trial(trial::TrialSearchArgs {
                    condition,
                    positional_query,
                    intervention,
                    facility,
                    phase,
                    study_type,
                    age,
                    sex,
                    status,
                    mutation,
                    criteria,
                    biomarker,
                    prior_therapies,
                    progression_on,
                    line_of_therapy,
                    sponsor,
                    sponsor_type,
                    date_from,
                    date_to,
                    lat,
                    lon,
                    distance,
                    results_available,
                    count_only,
                    source,
                    offset,
                    next_page,
                    limit,
                }) => {
                    let positional_trial_query = positional_query
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string);
                    let condition = resolve_query_input(
                        normalize_cli_tokens(condition),
                        positional_query,
                        "--condition",
                    )?;
                    let intervention = normalize_cli_tokens(intervention);
                    let facility = normalize_cli_tokens(facility);
                    let mutation = normalize_cli_tokens(mutation);
                    let criteria = normalize_cli_tokens(criteria);
                    let biomarker = normalize_cli_tokens(biomarker);
                    let prior_therapies = normalize_cli_tokens(prior_therapies);
                    let progression_on = normalize_cli_tokens(progression_on);
                    let sponsor = normalize_cli_tokens(sponsor);
                    let trial_source = crate::entities::trial::TrialSource::from_flag(&source)?;
                    let filters = crate::entities::trial::TrialSearchFilters {
                        condition,
                        intervention,
                        facility,
                        status,
                        phase,
                        study_type,
                        age,
                        sex,
                        sponsor,
                        sponsor_type,
                        date_from,
                        date_to,
                        mutation,
                        criteria,
                        biomarker,
                        prior_therapies,
                        progression_on,
                        line_of_therapy,
                        lat,
                        lon,
                        distance,
                        results_available,
                        source: trial_source,
                    };

                    if next_page
                        .as_deref()
                        .map(str::trim)
                        .is_some_and(|value| !value.is_empty())
                        && offset > 0
                    {
                        return Err(crate::error::BioMcpError::InvalidArgument(
                            "--next-page cannot be used together with --offset".into(),
                        )
                        .into());
                    }

                    let query =
                        trial_search_query_summary(&filters, offset, next_page.as_deref());
                    if count_only {
                        let count = crate::entities::trial::count_all(&filters).await?;
                        if cli.json {
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
                            return Ok(crate::render::json::to_pretty(&TrialCountOnlyJson {
                                total,
                                approximate,
                            })?);
                        }
                        return Ok(match count {
                            crate::entities::trial::TrialCount::Exact(total) => {
                                format!("Total: {total}")
                            }
                            crate::entities::trial::TrialCount::Approximate(total) => {
                                format!("Total: {total} (approximate, age post-filtered)")
                            }
                            crate::entities::trial::TrialCount::Unknown => {
                                "Total: unknown (traversal limit reached)".to_string()
                            }
                        });
                    }
                    let page = crate::entities::trial::search_page(
                        &filters,
                        limit,
                        offset,
                        next_page.clone(),
                    )
                    .await?;
                    let results = page.results;
                    let pagination = PaginationMeta::cursor(
                        offset,
                        limit,
                        results.len(),
                        page.total,
                        page.next_page_token,
                    );
                    if cli.json {
                        search_json(results, pagination)
                    } else {
                        let footer = if matches!(
                            trial_source,
                            crate::entities::trial::TrialSource::ClinicalTrialsGov
                        ) {
                            pagination_footer_cursor(&pagination)
                        } else {
                            pagination_footer_offset(&pagination)
                        };
                        let total = pagination.total.and_then(|value| u32::try_from(value).ok());
                        let show_zero_result_nickname_hint =
                            should_show_trial_zero_result_nickname_hint(
                                positional_trial_query.as_deref(),
                                trial_source,
                                results.len(),
                            );
                        Ok(crate::render::markdown::trial_search_markdown_with_footer(
                            &query,
                            &results,
                            total,
                            &footer,
                            show_zero_result_nickname_hint,
                            positional_trial_query.as_deref(),
                        )?)
                    }
                }
                SearchEntity::Variant(variant::VariantSearchArgs {
                    gene,
                    positional_query,
                    hgvsp,
                    significance,
                    max_frequency,
                    min_cadd,
                    consequence,
                    review_status,
                    population,
                    revel_min,
                    gerp_min,
                    tumor_site,
                    condition,
                    impact,
                    lof,
                    has,
                    missing,
                    therapy,
                    limit,
                    offset,
                }) => {
                    let outcome = render_variant_search_outcome(
                        cli.json,
                        false,
                        VariantSearchRequest {
                            gene,
                            positional_query,
                            hgvsp,
                            significance,
                            max_frequency,
                            min_cadd,
                            consequence,
                            review_status,
                            population,
                            revel_min,
                            gerp_min,
                            tumor_site,
                            condition,
                            impact,
                            lof,
                            has,
                            missing,
                            therapy,
                            limit,
                            offset,
                        },
                    )
                    .await?;
                    if outcome.exit_code == 0 {
                        Ok(outcome.text)
                    } else {
                        anyhow::bail!("{}", outcome.text)
                    }
                }
                SearchEntity::Drug(drug::DrugSearchArgs {
                    query,
                    positional_query,
                    target,
                    indication,
                    mechanism,
                    drug_type,
                    atc,
                    pharm_class,
                    interactions,
                    limit,
                    offset,
                    region,
                }) => {
                    let query = resolve_query_input(query, positional_query, "--query")?;
                    let filters = crate::entities::drug::DrugSearchFilters {
                        query,
                        target,
                        indication,
                        mechanism,
                        drug_type,
                        atc,
                        pharm_class,
                        interactions,
                    };
                    let region = resolve_drug_search_region(region, &filters)?;
                    let mut query_summary = crate::entities::drug::search_query_summary(&filters);
                    if offset > 0 {
                        query_summary = format!("{query_summary}, offset={offset}");
                    }
                    match crate::entities::drug::search_page_with_region(&filters, limit, offset, region)
                        .await?
                    {
                        crate::entities::drug::DrugSearchPageWithRegion::Us(page) => {
                            let results = page.results;
                            let pagination =
                                PaginationMeta::offset(offset, limit, results.len(), page.total);
                            if cli.json {
                                search_json(results, pagination)
                            } else {
                                let footer = pagination_footer_offset(&pagination);
                                Ok(crate::render::markdown::drug_search_markdown_with_region(
                                    &query_summary,
                                    region,
                                    &results,
                                    pagination.total,
                                    &[],
                                    None,
                                    &[],
                                    None,
                                    &footer,
                                )?)
                            }
                        }
                        crate::entities::drug::DrugSearchPageWithRegion::Eu(page) => {
                            let results = page.results;
                            let pagination =
                                PaginationMeta::offset(offset, limit, results.len(), page.total);
                            if cli.json {
                                search_json(results, pagination)
                            } else {
                                let footer = pagination_footer_offset(&pagination);
                                Ok(crate::render::markdown::drug_search_markdown_with_region(
                                    &query_summary,
                                    region,
                                    &[],
                                    None,
                                    &results,
                                    pagination.total,
                                    &[],
                                    None,
                                    &footer,
                                )?)
                            }
                        }
                        crate::entities::drug::DrugSearchPageWithRegion::Who(page) => {
                            let results = page.results;
                            let pagination =
                                PaginationMeta::offset(offset, limit, results.len(), page.total);
                            if cli.json {
                                search_json(results, pagination)
                            } else {
                                let footer = pagination_footer_offset(&pagination);
                                Ok(crate::render::markdown::drug_search_markdown_with_region(
                                    &query_summary,
                                    region,
                                    &[],
                                    None,
                                    &[],
                                    None,
                                    &results,
                                    pagination.total,
                                    &footer,
                                )?)
                            }
                        }
                        crate::entities::drug::DrugSearchPageWithRegion::All { us, eu, who } => {
                            if cli.json {
                                drug_all_region_search_json(&query_summary, us, eu, who)
                            } else {
                                Ok(crate::render::markdown::drug_search_markdown_with_region(
                                    &query_summary,
                                    region,
                                    &us.results,
                                    us.total,
                                    &eu.results,
                                    eu.total,
                                    &who.results,
                                    who.total,
                                    "",
                                )?)
                            }
                        }
                    }
                }
                SearchEntity::Pathway(pathway::PathwaySearchArgs {
                    query,
                    positional_query,
                    pathway_type,
                    top_level,
                    limit,
                    offset,
                }) => {
                    let query = resolve_query_input(query, positional_query, "--query")?;
                    let filters = crate::entities::pathway::PathwaySearchFilters {
                        query,
                        pathway_type,
                        top_level,
                    };
                    let fetch_limit = paged_fetch_limit(limit, offset, 25)?;
                    let mut query_summary = crate::entities::pathway::search_query_summary(&filters);
                    if offset > 0 {
                        query_summary = if query_summary.is_empty() {
                            format!("offset={offset}")
                        } else {
                            format!("{query_summary}, offset={offset}")
                        };
                    }
                    let (rows, total) =
                        crate::entities::pathway::search_with_filters(&filters, fetch_limit).await?;
                    let (results, observed_total) = paginate_results(rows, offset, limit);
                    log_pagination_truncation(observed_total, offset, results.len());
                    let total = total.or(Some(observed_total));
                    let pagination =
                        PaginationMeta::offset(offset, limit, results.len(), total);
                    if cli.json {
                        search_json(results, pagination)
                    } else {
                        let footer = pagination_footer_offset(&pagination);
                        Ok(crate::render::markdown::pathway_search_markdown_with_footer(
                            &query_summary,
                            &results,
                            total,
                            &footer,
                        )?)
                    }
                }
                SearchEntity::Protein(protein::ProteinSearchArgs {
                    query,
                    positional_query,
                    all_species,
                    reviewed,
                    disease,
                    existence,
                    limit,
                    offset,
                    next_page,
                }) => {
                    let query =
                        resolve_query_input(query, positional_query, "--query")?.unwrap_or_default();
                    if next_page
                        .as_deref()
                        .map(str::trim)
                        .is_some_and(|value| !value.is_empty())
                        && offset > 0
                    {
                        return Err(crate::error::BioMcpError::InvalidArgument(
                            "--next-page cannot be used together with --offset".into(),
                        )
                        .into());
                    }
                    let mut query_summary = crate::entities::protein::search_query_summary(
                        &query,
                        reviewed,
                        disease.as_deref(),
                        existence,
                        all_species,
                    );
                    if offset > 0 {
                        query_summary = if query_summary.is_empty() {
                            format!("offset={offset}")
                        } else {
                            format!("{query_summary}, offset={offset}")
                        };
                    }
                    let page = crate::entities::protein::search_page(
                        &query,
                        limit,
                        offset,
                        next_page.clone(),
                        all_species,
                        reviewed,
                        disease.as_deref(),
                        existence,
                    )
                    .await?;
                    let results = page.results;
                    let pagination = PaginationMeta::cursor(
                        offset,
                        limit,
                        results.len(),
                        page.total,
                        page.next_page_token,
                    );
                    if cli.json {
                        search_json(results, pagination)
                    } else {
                        let footer = pagination_footer_cursor(&pagination);
                        Ok(crate::render::markdown::protein_search_markdown_with_footer(
                            &query_summary,
                            &results,
                            &footer,
                        )?)
                    }
                }
                SearchEntity::AdverseEvent(adverse_event::AdverseEventSearchArgs {
                    drug,
                    positional_query,
                    device,
                    manufacturer,
                    product_code,
                    reaction,
                    outcome,
                    serious,
                    date_from,
                    date_to,
                    suspect_only,
                    sex,
                    age_min,
                    age_max,
                    reporter,
                    count,
                    r#type,
                    classification,
                    limit,
                    offset,
                }) => {
                    let drug = resolve_query_input(drug, positional_query, "--drug")?;
                    let query_type =
                        crate::entities::adverse_event::AdverseEventQueryType::from_flag(&r#type)?;

                    match query_type {
                        crate::entities::adverse_event::AdverseEventQueryType::Faers => {
                            if device.is_some() {
                                return Err(crate::error::BioMcpError::InvalidArgument(
                                    "--device can only be used with --type device".into(),
                                )
                                .into());
                            }
                            if manufacturer.is_some() {
                                return Err(crate::error::BioMcpError::InvalidArgument(
                                    "--manufacturer can only be used with --type device".into(),
                                )
                                .into());
                            }
                            if product_code.is_some() {
                                return Err(crate::error::BioMcpError::InvalidArgument(
                                    "--product-code can only be used with --type device".into(),
                                )
                                .into());
                            }
                            let filters = crate::entities::adverse_event::AdverseEventSearchFilters {
                                drug,
                                reaction,
                                outcome,
                                serious,
                                since: date_from,
                                date_to,
                                suspect_only,
                                sex,
                                age_min,
                                age_max,
                                reporter,
                            };
                            let mut query_summary =
                                crate::entities::adverse_event::search_query_summary(&filters);
                            if let Some(count_field) = count
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
                            if offset > 0 {
                                query_summary = format!("{query_summary}, offset={offset}");
                            }
                            if let Some(count_field) = count
                                .as_deref()
                                .map(str::trim)
                                .filter(|v| !v.is_empty())
                            {
                                let response = crate::entities::adverse_event::search_count(
                                    &filters,
                                    count_field,
                                    limit,
                                )
                                .await?;
                                if cli.json {
                                    #[derive(serde::Serialize)]
                                    struct CountResponse {
                                        query: String,
                                        count_field: String,
                                        buckets:
                                            Vec<crate::entities::adverse_event::AdverseEventCountBucket>,
                                    }

                                    return Ok(crate::render::json::to_pretty(&CountResponse {
                                        query: query_summary,
                                        count_field: response.count_field,
                                        buckets: response.buckets,
                                    })?);
                                }

                                return Ok(
                                    crate::render::markdown::adverse_event_count_markdown(
                                        &query_summary,
                                        &response.count_field,
                                        &response.buckets,
                                    )?,
                                );
                            }
                            let response =
                                crate::entities::adverse_event::search_with_summary(
                                    &filters,
                                    limit,
                                    offset,
                                )
                                .await?;
                            let summary = response.summary;
                            let results = response.results;
                            let pagination = PaginationMeta::offset(
                                offset,
                                limit,
                                results.len(),
                                Some(summary.total_reports),
                            );
                            if cli.json {
                                #[derive(serde::Serialize)]
                                struct SearchResponse {
                                    pagination: PaginationMeta,
                                    count: usize,
                                    summary:
                                        crate::entities::adverse_event::AdverseEventSearchSummary,
                                    results:
                                        Vec<crate::entities::adverse_event::AdverseEventSearchResult>,
                                }

                                Ok(crate::render::json::to_pretty(&SearchResponse {
                                    pagination,
                                    count: results.len(),
                                    summary,
                                    results,
                                })?)
                            } else {
                                let footer = pagination_footer_offset(&pagination);
                                Ok(crate::render::markdown::adverse_event_search_markdown_with_footer(
                                    &query_summary,
                                    &results,
                                    &summary,
                                    &footer,
                                )?)
                            }
                        }
                        crate::entities::adverse_event::AdverseEventQueryType::Recall => {
                            if date_from.is_some()
                                || date_to.is_some()
                                || suspect_only
                                || sex.is_some()
                                || age_min.is_some()
                                || age_max.is_some()
                                || reporter.is_some()
                                || count.is_some()
                            {
                                return Err(crate::error::BioMcpError::InvalidArgument(
                                    "--date-from/--date-to/--suspect-only/--sex/--age-min/--age-max/--reporter/--count are only valid for --type faers".into(),
                                )
                                .into());
                            }
                            if device.is_some() {
                                return Err(crate::error::BioMcpError::InvalidArgument(
                                    "--device can only be used with --type device".into(),
                                )
                                .into());
                            }
                            if manufacturer.is_some() {
                                return Err(crate::error::BioMcpError::InvalidArgument(
                                    "--manufacturer can only be used with --type device".into(),
                                )
                                .into());
                            }
                            if product_code.is_some() {
                                return Err(crate::error::BioMcpError::InvalidArgument(
                                    "--product-code can only be used with --type device".into(),
                                )
                                .into());
                            }
                            if outcome.is_some() {
                                return Err(crate::error::BioMcpError::InvalidArgument(
                                    "--outcome is only valid for --type faers".into(),
                                )
                                .into());
                            }
                            let filters = crate::entities::adverse_event::RecallSearchFilters {
                                drug,
                                classification,
                            };
                            let mut query_summary =
                                crate::entities::adverse_event::recall_query_summary(&filters);
                            if offset > 0 {
                                query_summary = format!("{query_summary}, offset={offset}");
                            }
                            let page = crate::entities::adverse_event::search_recalls_page(
                                &filters,
                                limit,
                                offset,
                            )
                            .await?;
                            let results = page.results;
                            let pagination =
                                PaginationMeta::offset(offset, limit, results.len(), page.total);
                            if cli.json {
                                search_json(results, pagination)
                            } else {
                                let footer = pagination_footer_offset(&pagination);
                                Ok(crate::render::markdown::recall_search_markdown_with_footer(
                                    &query_summary,
                                    &results,
                                    &footer,
                                )?)
                            }
                        }
                        crate::entities::adverse_event::AdverseEventQueryType::Device => {
                            if drug.is_some() {
                                return Err(crate::error::BioMcpError::InvalidArgument(
                                    "--drug cannot be used with --type device (use --device)".into(),
                                )
                                .into());
                            }
                            if reaction.is_some() {
                                return Err(crate::error::BioMcpError::InvalidArgument(
                                    "--reaction is not supported with --type device".into(),
                                )
                                .into());
                            }
                            if outcome.is_some() {
                                return Err(crate::error::BioMcpError::InvalidArgument(
                                    "--outcome is only valid for --type faers".into(),
                                )
                                .into());
                            }
                            if classification.is_some() {
                                return Err(crate::error::BioMcpError::InvalidArgument(
                                    "--classification is only valid for --type recall".into(),
                                )
                                .into());
                            }
                            if date_to.is_some()
                                || suspect_only
                                || sex.is_some()
                                || age_min.is_some()
                                || age_max.is_some()
                                || reporter.is_some()
                                || count.is_some()
                            {
                                return Err(crate::error::BioMcpError::InvalidArgument(
                                    "--date-to/--suspect-only/--sex/--age-min/--age-max/--reporter/--count are only valid for --type faers".into(),
                                )
                                .into());
                            }

                            let filters = crate::entities::adverse_event::DeviceEventSearchFilters {
                                device,
                                manufacturer,
                                product_code,
                                serious: serious.is_some(),
                                since: date_from,
                            };
                            let mut query_summary =
                                crate::entities::adverse_event::device_query_summary(&filters);
                            if offset > 0 {
                                query_summary = format!("{query_summary}, offset={offset}");
                            }
                            let page = crate::entities::adverse_event::search_device_page(
                                &filters,
                                limit,
                                offset,
                            )
                            .await?;
                            let results = page.results;
                            let pagination =
                                PaginationMeta::offset(offset, limit, results.len(), page.total);
                            if cli.json {
                                search_json(results, pagination)
                            } else {
                                let footer = pagination_footer_offset(&pagination);
                                Ok(crate::render::markdown::device_event_search_markdown_with_footer(
                                    &query_summary,
                                    &results,
                                    &footer,
                                )?)
                            }
                        }
                    }
                }
                }
            }
            Commands::Health(system::HealthArgs { apis_only }) => {
                let report = crate::cli::health::check(apis_only).await?;
                if cli.json {
                    Ok(crate::render::json::to_pretty(&report)?)
                } else {
                    Ok(report.to_markdown())
                }
            }
            Commands::Cache { cmd } => match cmd {
                cache::CacheCommand::Path => Ok(crate::cli::cache::render_path()?),
                cache::CacheCommand::Stats => {
                    let report = crate::cli::cache::collect_cache_stats_report()?;
                    if cli.json {
                        Ok(crate::render::json::to_pretty(&report)?)
                    } else {
                        Ok(report.to_markdown())
                    }
                }
                cache::CacheCommand::Clean {
                    max_age,
                    max_size,
                    dry_run,
                } => {
                    let report = crate::cli::cache::execute_clean(max_age, max_size, dry_run)?;
                    if cli.json {
                        Ok(crate::render::json::to_pretty(&report)?)
                    } else {
                        Ok(crate::cli::cache::render_clean_text(&report))
                    }
                }
                cache::CacheCommand::Clear { .. } => Err(
                    crate::error::BioMcpError::InvalidArgument(
                        "cache clear must be executed through run_outcome()".into(),
                    )
                    .into(),
                ),
            },
            Commands::Ema { cmd } => match cmd {
                EmaCommand::Sync => {
                    crate::sources::ema::EmaClient::sync(crate::sources::ema::EmaSyncMode::Force)
                        .await?;
                    Ok("EMA data synchronized successfully.\n".to_string())
                }
            },
            Commands::Who { cmd } => match cmd {
                WhoCommand::Sync => {
                    crate::sources::who_pq::WhoPqClient::sync(
                        crate::sources::who_pq::WhoPqSyncMode::Force,
                    )
                    .await?;
                    Ok("WHO Prequalification data synchronized successfully.\n".to_string())
                }
            },
            Commands::Skill { command } => match command {
                None => Ok(crate::cli::skill::show_overview()?),
                Some(crate::cli::skill::SkillCommand::List) => Ok(crate::cli::skill::list_use_cases()?),
                Some(crate::cli::skill::SkillCommand::Install { dir, force }) => {
                    Ok(crate::cli::skill::install_skills(dir.as_deref(), force)?)
                }
                Some(crate::cli::skill::SkillCommand::Show(args)) => {
                    let key = if args.is_empty() {
                        String::new()
                    } else if args.len() == 1 {
                        args[0].clone()
                    } else {
                        args.join("-")
                    };
                    Ok(crate::cli::skill::show_use_case(&key)?)
                }
            },
            Commands::Chart { command } => Ok(crate::cli::chart::show(command.as_ref())?),
            Commands::Update(system::UpdateArgs { check }) => {
                Ok(crate::cli::update::run(check).await?)
            }
            Commands::Uninstall => Ok(uninstall_self()?),
            Commands::Enrich(system::EnrichArgs { genes, limit }) => {
                const MAX_ENRICH_LIMIT: usize = 50;
                if limit == 0 || limit > MAX_ENRICH_LIMIT {
                    return Err(crate::error::BioMcpError::InvalidArgument(format!(
                        "--limit must be between 1 and {MAX_ENRICH_LIMIT}"
                    ))
                    .into());
                }
                let genes = genes
                    .split(',')
                    .map(str::trim)
                    .filter(|g| !g.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>();
                if genes.is_empty() {
                    return Err(crate::error::BioMcpError::InvalidArgument(
                        "At least one gene is required. Example: biomcp enrich BRAF,KRAS".into(),
                    )
                    .into());
                }
                let terms = crate::sources::gprofiler::GProfilerClient::new()?
                    .enrich_genes(&genes, limit)
                    .await?;
                if cli.json {
                    #[derive(serde::Serialize)]
                    struct EnrichResponse {
                        genes: Vec<String>,
                        count: usize,
                        results: Vec<crate::sources::gprofiler::GProfilerTerm>,
                    }
                    Ok(crate::render::json::to_pretty(&EnrichResponse {
                        genes,
                        count: terms.len(),
                        results: terms,
                    })?)
                } else {
                    Ok(enrich_markdown(&genes, &terms))
                }
            }
            Commands::Discover(system::DiscoverArgs { query }) => {
                crate::cli::discover::run(crate::cli::discover::DiscoverArgs { query }, cli.json)
                    .await
            }
            Commands::List(system::ListArgs { entity }) => {
                crate::cli::list::render(entity.as_deref()).map_err(Into::into)
            }
            Commands::Mcp
            | Commands::Serve
            | Commands::ServeHttp(_)
            | Commands::ServeSse => {
                anyhow::bail!("MCP/serve commands should not go through CLI run()")
            }
            Commands::Version(system::VersionArgs { verbose }) => Ok(version_output(verbose)),
        }
    })
    .await
}

async fn run_outcome_inner(
    cli: Cli,
    alias_suggestions_as_json: bool,
) -> anyhow::Result<CommandOutcome> {
    match cli.command {
        Commands::Cache {
            cmd: cache::CacheCommand::Clear { yes },
        } => {
            if !yes && !std::io::stdin().is_terminal() {
                return Ok(CommandOutcome::stderr_with_exit(
                    "Error: biomcp cache clear requires a TTY or --yes for non-interactive use."
                        .to_string(),
                    1,
                ));
            }

            let config = crate::cache::resolve_cache_config()?;
            let cache_path = config.cache_root.join("http");

            let report = if yes || crate::cli::cache::prompt_clear_confirmation(&cache_path)? {
                crate::cache::execute_cache_clear(&cache_path)?
            } else {
                crate::cache::ClearReport {
                    bytes_freed: None,
                    entries_removed: 0,
                }
            };

            let text = if cli.json {
                crate::render::json::to_pretty(&report)?
            } else {
                crate::cli::cache::render_clear_text(&report)
            };
            Ok(CommandOutcome::stdout(text))
        }
        Commands::Get {
            entity: GetEntity::Gene(gene::GeneGetArgs { symbol, sections }),
        } => {
            let json = cli.json;
            let no_cache = cli.no_cache;
            crate::sources::with_no_cache(no_cache, async move {
                let (sections, json_override) = extract_json_from_sections(&sections);
                let json_output = json || json_override;
                render_gene_card_outcome(&symbol, &sections, json_output, alias_suggestions_as_json)
                    .await
            })
            .await
        }
        Commands::Get {
            entity:
                GetEntity::Drug(drug::DrugGetArgs {
                    name,
                    sections,
                    region,
                    raw,
                }),
        } => {
            let json = cli.json;
            let no_cache = cli.no_cache;
            crate::sources::with_no_cache(no_cache, async move {
                let (sections, json_override) = extract_json_from_sections(&sections);
                let region = region.map(DrugRegion::from);
                let json_output = json || json_override;
                render_drug_card_outcome(
                    &name,
                    &sections,
                    region,
                    raw,
                    json_output,
                    alias_suggestions_as_json,
                )
                .await
            })
            .await
        }
        Commands::Get {
            entity: GetEntity::Variant(variant::VariantGetArgs { id, sections }),
        } => {
            let json = cli.json;
            let no_cache = cli.no_cache;
            crate::sources::with_no_cache(no_cache, async move {
                let (sections, json_override) = extract_json_from_sections(&sections);
                let json_output = json || json_override;
                render_variant_card_outcome(&id, &sections, json_output, alias_suggestions_as_json)
                    .await
            })
            .await
        }
        Commands::Search {
            entity:
                SearchEntity::Variant(variant::VariantSearchArgs {
                    gene,
                    positional_query,
                    hgvsp,
                    significance,
                    max_frequency,
                    min_cadd,
                    consequence,
                    review_status,
                    population,
                    revel_min,
                    gerp_min,
                    tumor_site,
                    condition,
                    impact,
                    lof,
                    has,
                    missing,
                    therapy,
                    limit,
                    offset,
                }),
        } => {
            let json = cli.json;
            let no_cache = cli.no_cache;
            crate::sources::with_no_cache(no_cache, async move {
                render_variant_search_outcome(
                    json,
                    alias_suggestions_as_json,
                    VariantSearchRequest {
                        gene,
                        positional_query,
                        hgvsp,
                        significance,
                        max_frequency,
                        min_cadd,
                        consequence,
                        review_status,
                        population,
                        revel_min,
                        gerp_min,
                        tumor_site,
                        condition,
                        impact,
                        lof,
                        has,
                        missing,
                        therapy,
                        limit,
                        offset,
                    },
                )
                .await
            })
            .await
        }
        Commands::Gene {
            cmd: GeneCommand::Definition { symbol },
        } => {
            let json = cli.json;
            let no_cache = cli.no_cache;
            crate::sources::with_no_cache(no_cache, async move {
                render_gene_card_outcome(&symbol, empty_sections(), json, alias_suggestions_as_json)
                    .await
            })
            .await
        }
        Commands::Gene {
            cmd: GeneCommand::External(args),
        } => {
            let symbol = args.join(" ");
            let json = cli.json;
            let no_cache = cli.no_cache;
            crate::sources::with_no_cache(no_cache, async move {
                render_gene_card_outcome(&symbol, empty_sections(), json, alias_suggestions_as_json)
                    .await
            })
            .await
        }
        Commands::Drug {
            cmd: DrugCommand::External(args),
        } => {
            let name = args.join(" ");
            let json = cli.json;
            let no_cache = cli.no_cache;
            crate::sources::with_no_cache(no_cache, async move {
                render_drug_card_outcome(
                    &name,
                    empty_sections(),
                    None,
                    false,
                    json,
                    alias_suggestions_as_json,
                )
                .await
            })
            .await
        }
        command => Ok(CommandOutcome::stdout(
            run(Cli {
                command,
                json: cli.json,
                no_cache: cli.no_cache,
            })
            .await?,
        )),
    }
}

pub async fn run_outcome(cli: Cli) -> anyhow::Result<CommandOutcome> {
    run_outcome_inner(cli, false).await
}

async fn run_outcome_with_worker_stack(cli: Cli) -> anyhow::Result<CommandOutcome> {
    const EXECUTE_STACK_BYTES: usize = 8 * 1024 * 1024;

    tokio::task::spawn_blocking(move || {
        let handle = std::thread::Builder::new()
            .name("biomcp-cli-execute".into())
            .stack_size(EXECUTE_STACK_BYTES)
            .spawn(move || -> anyhow::Result<CommandOutcome> {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                runtime.block_on(run_outcome(cli))
            })?;

        handle
            .join()
            .map_err(|_| anyhow::anyhow!("in-process CLI worker panicked"))?
    })
    .await
    .map_err(|err| anyhow::anyhow!("failed to join in-process CLI worker: {err}"))?
}

/// Main CLI execution - called by the MCP `biomcp` tool.
///
/// # Errors
///
/// Returns an error when CLI args cannot be parsed or when command execution fails.
pub async fn execute(mut args: Vec<String>) -> anyhow::Result<String> {
    if args.is_empty() {
        args.push("biomcp".to_string());
    }
    let cli = Cli::try_parse_from(args)?;
    // Run CLI dispatch on a dedicated worker with a normal thread stack so
    // in-process callers do not inherit the giant command future on a small
    // async worker stack.
    let outcome = run_outcome_with_worker_stack(cli).await?;
    if outcome.exit_code == 0 {
        Ok(outcome.text)
    } else {
        anyhow::bail!("{}", outcome.text)
    }
}

pub async fn execute_mcp(mut args: Vec<String>) -> anyhow::Result<CliOutput> {
    if args.is_empty() {
        args.push("biomcp".to_string());
    }

    let cli = Cli::try_parse_from(args.clone())?;
    if !is_charted_mcp_study_command(&cli)? {
        let outcome = Box::pin(run_outcome_inner(cli, true)).await?;
        return Ok(CliOutput {
            text: outcome.text,
            svg: None,
        });
    }

    let text = Box::pin(execute(rewrite_mcp_chart_args(&args, McpChartPass::Text)?)).await?;
    let svg = Box::pin(execute(rewrite_mcp_chart_args(&args, McpChartPass::Svg)?)).await?;
    Ok(CliOutput {
        text,
        svg: Some(svg),
    })
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::test_support::{
        Mock, MockServer, ResponseTemplate, TempDirGuard, lock_env, method, mount_drug_lookup_miss,
        mount_gene_lookup_hit, mount_gene_lookup_miss, mount_ols_alias, path, query_param,
        set_env_var,
    };
    use super::{
        ChartArgs, Cli, Commands, McpChartPass, OutputStream, PaginationMeta, StudyCommand,
        VariantSearchPlan, article_search_json, build_article_debug_plan, disease_search_json,
        drug_all_region_search_json, execute, execute_mcp, extract_json_from_sections,
        paginate_trial_locations, parse_simple_gene_change, parse_trial_location_paging,
        resolve_drug_search_region, resolve_query_input, resolve_variant_query,
        rewrite_mcp_chart_args, run_outcome, search_json,
        should_show_trial_zero_result_nickname_hint, should_try_pathway_trial_fallback,
        trial_locations_json, trial_search_query_summary, truncate_article_annotations,
    };
    use crate::entities::drug::{DrugRegion, DrugSearchFilters};
    use clap::{CommandFactory, FromArgMatches, Parser};

    #[test]
    fn extract_json_from_sections_detects_trailing_long_flag() {
        let sections = vec!["all".to_string(), "--json".to_string()];
        let (cleaned, json_override) = extract_json_from_sections(&sections);
        assert_eq!(cleaned, vec!["all".to_string()]);
        assert!(json_override);
    }

    #[test]
    fn extract_json_from_sections_detects_trailing_short_flag() {
        let sections = vec!["clinvar".to_string(), "-j".to_string()];
        let (cleaned, json_override) = extract_json_from_sections(&sections);
        assert_eq!(cleaned, vec!["clinvar".to_string()]);
        assert!(json_override);
    }

    #[test]
    fn extract_json_from_sections_keeps_regular_sections() {
        let sections = vec!["eligibility".to_string(), "locations".to_string()];
        let (cleaned, json_override) = extract_json_from_sections(&sections);
        assert_eq!(cleaned, sections);
        assert!(!json_override);
    }

    #[tokio::test]
    async fn get_drug_raw_rejects_non_label_sections() {
        let cli =
            Cli::try_parse_from(["biomcp", "get", "drug", "pembrolizumab", "targets", "--raw"])
                .expect("get drug --raw should parse");

        let err = run_outcome(cli)
            .await
            .expect_err("targets --raw should be rejected");
        assert!(
            err.to_string()
                .contains("--raw can only be used with label or all")
        );
    }

    #[test]
    fn skill_help_examples_match_installed_surface() {
        let mut command = Cli::command();
        let skill = command
            .find_subcommand_mut("skill")
            .expect("skill subcommand should exist");
        let mut help = Vec::new();
        skill
            .write_long_help(&mut help)
            .expect("skill help should render");
        let help = String::from_utf8(help).expect("help should be utf-8");

        assert!(help.contains("biomcp skill            # show skill overview"));
        assert!(help.contains("biomcp skill install    # install skill to your agent config"));
        assert!(help.contains("Commands:\n  list"));
        assert!(!help.contains("biomcp skill 03"));
        assert!(!help.contains("variant-to-treatment"));
    }

    #[test]
    fn runtime_help_hides_query_only_global_flags() {
        for subcommand_name in super::RUNTIME_HELP_SUBCOMMANDS {
            let mut command = super::build_cli();
            let runtime = command
                .find_subcommand_mut(subcommand_name)
                .expect("runtime subcommand should exist");
            let mut help = Vec::new();
            runtime
                .write_long_help(&mut help)
                .expect("runtime help should render");
            let help = String::from_utf8(help).expect("help should be utf-8");

            assert!(
                !help.contains("--json"),
                "{subcommand_name} help should not advertise --json"
            );
            assert!(
                !help.contains("--no-cache"),
                "{subcommand_name} help should not advertise --no-cache"
            );
        }
    }

    #[test]
    fn runtime_commands_still_parse_hidden_global_flags() {
        let cli = parse_built_cli([
            "biomcp",
            "serve-http",
            "--json",
            "--no-cache",
            "--host",
            "127.0.0.1",
            "--port",
            "8080",
        ]);
        assert!(cli.json);
        assert!(cli.no_cache);
        assert!(matches!(
            cli.command,
            Commands::ServeHttp(super::system::ServeHttpArgs { host, port })
                if host == "127.0.0.1" && port == 8080
        ));

        for args in [
            ["biomcp", "mcp", "--json", "--no-cache"].as_slice(),
            ["biomcp", "serve", "--json", "--no-cache"].as_slice(),
            ["biomcp", "serve-sse", "--json", "--no-cache"].as_slice(),
        ] {
            let cli = parse_built_cli(args);
            assert!(cli.json);
            assert!(cli.no_cache);
        }
    }

    #[test]
    fn serve_sse_help_stays_callable_and_deprecated() {
        let mut command = super::build_cli();
        let serve_sse = command
            .find_subcommand_mut("serve-sse")
            .expect("serve-sse subcommand should exist");
        let mut help = Vec::new();
        serve_sse
            .write_long_help(&mut help)
            .expect("serve-sse help should render");
        let help = String::from_utf8(help).expect("help should be utf-8");

        assert!(help.contains("serve-sse"));
        assert!(help.contains("removed"));
        assert!(help.contains("serve-http"));
        assert!(help.contains("/mcp"));
        assert!(!help.contains("--json"));
        assert!(!help.contains("--no-cache"));
    }

    #[test]
    fn top_level_help_hides_serve_sse_but_keeps_serve_http() {
        let mut command = super::build_cli();
        let mut help = Vec::new();
        command
            .write_long_help(&mut help)
            .expect("top-level help should render");
        let help = String::from_utf8(help).expect("help should be utf-8");

        assert!(help.contains("serve-http"));
        assert!(!help.contains("serve-sse"));
    }

    #[test]
    fn cache_path_command_parses() {
        Cli::try_parse_from(["biomcp", "cache", "path"]).expect("cache path should parse");
    }

    #[test]
    fn cache_stats_command_parses() {
        Cli::try_parse_from(["biomcp", "cache", "stats"]).expect("cache stats should parse");
    }

    #[test]
    fn cache_clean_command_parses_with_flags() {
        let cli = Cli::try_parse_from([
            "biomcp",
            "cache",
            "clean",
            "--max-age",
            "30d",
            "--max-size",
            "500M",
            "--dry-run",
        ])
        .expect("cache clean should parse");

        let Cli {
            command:
                Commands::Cache {
                    cmd:
                        crate::cli::cache::CacheCommand::Clean {
                            max_age,
                            max_size,
                            dry_run,
                        },
                },
            ..
        } = cli
        else {
            panic!("expected cache clean command");
        };

        assert_eq!(
            max_age,
            Some(std::time::Duration::from_secs(30 * 24 * 60 * 60))
        );
        assert_eq!(max_size, Some(500_000_000));
        assert!(dry_run);
    }

    #[test]
    fn cache_clear_command_parses() {
        Cli::try_parse_from(["biomcp", "cache", "clear"]).expect("cache clear should parse");
    }

    #[test]
    fn cache_clear_command_parses_with_yes_flag() {
        Cli::try_parse_from(["biomcp", "cache", "clear", "--yes"])
            .expect("cache clear --yes should parse");
    }

    #[test]
    fn top_level_help_lists_cache_command() {
        let mut command = super::build_cli();
        let mut help = Vec::new();
        command
            .write_long_help(&mut help)
            .expect("top-level help should render");
        let help = String::from_utf8(help).expect("help should be utf-8");

        assert!(
            help.lines()
                .any(|line| line.trim_start().starts_with("cache")),
            "top-level help should list the cache family: {help}"
        );
    }

    #[test]
    fn top_level_help_mentions_cache_path_json_exception() {
        let mut command = super::build_cli();
        let mut help = Vec::new();
        command
            .write_long_help(&mut help)
            .expect("top-level help should render");
        let help = String::from_utf8(help).expect("help should be utf-8");

        assert!(help.contains("except biomcp cache path"));
        assert!(help.contains("stays plain text"));
    }

    #[test]
    fn cache_path_help_mentions_plain_text_and_ignored_json() {
        let help = render_cache_path_long_help();

        assert!(help.contains("plain text"));
        assert!(help.contains("--json"));
        assert!(help.contains("ignored"));
    }

    #[test]
    fn cache_stats_help_mentions_json_and_cli_only() {
        let help = render_cache_stats_long_help();

        assert!(help.contains("cache statistics"));
        assert!(help.contains("--json"));
        assert!(help.contains("CLI-only"));
        assert!(help.contains("local filesystem paths"));
    }

    #[test]
    fn cache_clean_help_mentions_dry_run_json_and_limits() {
        let help = render_cache_clean_long_help();

        assert!(help.contains("--max-age"));
        assert!(help.contains("--max-size"));
        assert!(help.contains("--dry-run"));
        assert!(help.contains("--json"));
        assert!(help.contains("orphan"));
    }

    #[test]
    fn cache_clear_help_mentions_yes_tty_and_destructive_scope() {
        let help = render_cache_clear_long_help();

        assert!(help.contains("--yes"));
        assert!(help.contains("TTY"));
        assert!(help.contains("downloads"));
        assert!(help.contains("destructive"));
    }

    #[test]
    fn cache_help_lists_clear_subcommand() {
        let help = render_cache_long_help();

        assert!(help.contains("clear"));
    }

    #[test]
    fn top_level_help_describes_cache_family_not_path_only() {
        let mut command = super::build_cli();
        let mut help = Vec::new();
        command
            .write_long_help(&mut help)
            .expect("top-level help should render");
        let help = String::from_utf8(help).expect("help should be utf-8");

        assert!(help.contains(
            "Inspect the managed HTTP cache (CLI-only; cache commands reveal workstation-local filesystem paths)"
        ));
        assert!(!help.contains(
            "Print the managed HTTP cache path (CLI-only; plain text; ignores `--json`)"
        ));
    }

    fn render_cache_path_long_help() -> String {
        let mut command = Cli::command();
        let cache = command
            .find_subcommand_mut("cache")
            .expect("cache subcommand should exist");
        let path = cache
            .find_subcommand_mut("path")
            .expect("cache path subcommand should exist");
        let mut help = Vec::new();
        path.write_long_help(&mut help)
            .expect("cache path help should render");
        String::from_utf8(help).expect("help should be utf-8")
    }

    fn render_cache_long_help() -> String {
        let mut command = Cli::command();
        let cache = command
            .find_subcommand_mut("cache")
            .expect("cache subcommand should exist");
        let mut help = Vec::new();
        cache
            .write_long_help(&mut help)
            .expect("cache help should render");
        String::from_utf8(help).expect("help should be utf-8")
    }

    fn render_cache_stats_long_help() -> String {
        let mut command = Cli::command();
        let cache = command
            .find_subcommand_mut("cache")
            .expect("cache subcommand should exist");
        let stats = cache
            .find_subcommand_mut("stats")
            .expect("cache stats subcommand should exist");
        let mut help = Vec::new();
        stats
            .write_long_help(&mut help)
            .expect("cache stats help should render");
        String::from_utf8(help).expect("help should be utf-8")
    }

    fn render_cache_clean_long_help() -> String {
        let mut command = Cli::command();
        let cache = command
            .find_subcommand_mut("cache")
            .expect("cache subcommand should exist");
        let clean = cache
            .find_subcommand_mut("clean")
            .expect("cache clean subcommand should exist");
        let mut help = Vec::new();
        clean
            .write_long_help(&mut help)
            .expect("cache clean help should render");
        String::from_utf8(help).expect("help should be utf-8")
    }

    fn render_cache_clear_long_help() -> String {
        let mut command = Cli::command();
        let cache = command
            .find_subcommand_mut("cache")
            .expect("cache subcommand should exist");
        let clear = cache
            .find_subcommand_mut("clear")
            .expect("cache clear subcommand should exist");
        let mut help = Vec::new();
        clear
            .write_long_help(&mut help)
            .expect("cache clear help should render");
        String::from_utf8(help).expect("help should be utf-8")
    }

    fn parse_built_cli<I, T>(args: I) -> Cli
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        let matches = super::build_cli()
            .try_get_matches_from(args)
            .expect("args should parse with canonical CLI");
        Cli::from_arg_matches(&matches).expect("matches should decode into Cli")
    }

    #[test]
    fn parse_trial_location_paging_extracts_offset_limit_flags() {
        let sections = vec![
            "locations".to_string(),
            "--offset".to_string(),
            "20".to_string(),
            "--limit=10".to_string(),
        ];
        let (cleaned, offset, limit) =
            parse_trial_location_paging(&sections).expect("valid pagination flags");
        assert_eq!(cleaned, vec!["locations".to_string()]);
        assert_eq!(offset, Some(20));
        assert_eq!(limit, Some(10));
    }

    #[test]
    fn trial_locations_json_preserves_location_pagination_and_section_sources() {
        let trial = crate::entities::trial::Trial {
            nct_id: "NCT00000001".to_string(),
            source: Some("ctgov".to_string()),
            title: "Example trial".to_string(),
            status: "Recruiting".to_string(),
            phase: Some("Phase 2".to_string()),
            study_type: Some("Interventional".to_string()),
            age_range: Some("18 Years and older".to_string()),
            conditions: vec!["melanoma".to_string()],
            interventions: vec!["osimertinib".to_string()],
            sponsor: Some("Example Sponsor".to_string()),
            enrollment: Some(100),
            summary: Some("Example summary".to_string()),
            start_date: Some("2024-01-01".to_string()),
            completion_date: None,
            eligibility_text: None,
            locations: Some(vec![crate::entities::trial::TrialLocation {
                facility: "Example Hospital".to_string(),
                city: "Boston".to_string(),
                state: Some("MA".to_string()),
                country: "United States".to_string(),
                status: Some("Recruiting".to_string()),
                contact_name: None,
                contact_phone: None,
            }]),
            outcomes: None,
            arms: None,
            references: None,
        };

        let json = trial_locations_json(
            &trial,
            super::LocationPaginationMeta {
                total: 42,
                offset: 20,
                limit: 10,
                has_more: true,
            },
        )
        .expect("trial locations json");

        let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert_eq!(value["nct_id"], "NCT00000001");
        assert_eq!(value["location_pagination"]["total"], 42);
        assert_eq!(value["location_pagination"]["offset"], 20);
        assert_eq!(value["location_pagination"]["limit"], 10);
        assert_eq!(value["location_pagination"]["has_more"], true);
        assert!(value.get("_meta").is_some());
        assert_eq!(value["_meta"]["section_sources"][0]["key"], "overview");
        assert_eq!(
            value["_meta"]["section_sources"][0]["sources"][0],
            "ClinicalTrials.gov"
        );
        assert!(
            value["_meta"]["section_sources"]
                .as_array()
                .expect("section sources array")
                .iter()
                .any(|entry| entry["key"] == "locations")
        );
    }

    #[test]
    fn article_search_json_includes_query_and_ranking_context() {
        let pagination = PaginationMeta::offset(0, 3, 1, Some(1));
        let mut filters = super::related_article_filters();
        filters.gene = Some("BRAF".into());
        let query = super::article_query_summary(
            &filters,
            crate::entities::article::ArticleSourceFilter::All,
            false,
            3,
            0,
        );
        let json = article_search_json(
            &query,
            &filters,
            true,
            Some(
                "Note: --type restricts article search to Europe PMC and PubMed. PubTator3, LitSense2, and Semantic Scholar do not support publication-type filtering.".into(),
            ),
            None,
            vec![crate::entities::article::ArticleSearchResult {
                pmid: "22663011".into(),
                pmcid: Some("PMC9984800".into()),
                doi: Some("10.1056/NEJMoa1203421".into()),
                title: "BRAF melanoma review".into(),
                journal: Some("Journal".into()),
                date: Some("2025-01-01".into()),
                citation_count: Some(12),
                influential_citation_count: Some(4),
                source: crate::entities::article::ArticleSource::EuropePmc,
                matched_sources: vec![
                    crate::entities::article::ArticleSource::EuropePmc,
                    crate::entities::article::ArticleSource::SemanticScholar,
                ],
                score: None,
                is_retracted: Some(false),
                abstract_snippet: Some("Abstract".into()),
                ranking: Some(crate::entities::article::ArticleRankingMetadata {
                    directness_tier: 3,
                    anchor_count: 2,
                    title_anchor_hits: 2,
                    abstract_anchor_hits: 0,
                    combined_anchor_hits: 2,
                    all_anchors_in_title: true,
                    all_anchors_in_text: true,
                    study_or_review_cue: true,
                    pubmed_rescue: false,
                    pubmed_rescue_kind: None,
                    pubmed_source_position: None,
                    mode: Some(crate::entities::article::ArticleRankingMode::Lexical),
                    semantic_score: None,
                    lexical_score: None,
                    citation_score: None,
                    position_score: None,
                    composite_score: None,
                    avg_source_rank: None,
                }),
                normalized_title: "braf melanoma review".into(),
                normalized_abstract: "abstract".into(),
                publication_type: Some("Review".into()),
                source_local_position: 0,
            }],
            pagination,
        )
        .expect("article search json should render");

        let value: serde_json::Value =
            serde_json::from_str(&json).expect("json should parse successfully");
        assert_eq!(value["query"], query);
        assert_eq!(value["sort"], "relevance");
        assert_eq!(value["semantic_scholar_enabled"], true);
        assert_eq!(
            value["ranking_policy"],
            crate::entities::article::ARTICLE_RELEVANCE_RANKING_POLICY
        );
        assert_eq!(
            value["note"],
            "Note: --type restricts article search to Europe PMC and PubMed. PubTator3, LitSense2, and Semantic Scholar do not support publication-type filtering."
        );
        assert_eq!(value["results"][0]["ranking"]["directness_tier"], 3);
        assert_eq!(value["results"][0]["ranking"]["pubmed_rescue"], false);
        assert!(value["results"][0]["ranking"]["pubmed_rescue_kind"].is_null());
        assert!(value["results"][0]["ranking"]["pubmed_source_position"].is_null());
        assert_eq!(
            value["results"][0]["matched_sources"][1],
            serde_json::Value::String("semanticscholar".into())
        );
    }

    #[test]
    fn disease_search_json_includes_fallback_meta_and_provenance() {
        let pagination = PaginationMeta::offset(0, 10, 1, Some(1));
        let json = disease_search_json(
            vec![crate::entities::disease::DiseaseSearchResult {
                id: "MONDO:0000115".into(),
                name: "Arnold-Chiari malformation".into(),
                synonyms_preview: Some("Chiari malformation".into()),
                resolved_via: Some("MESH crosswalk".into()),
                source_id: Some("MESH:D001139".into()),
            }],
            pagination,
            true,
        )
        .expect("disease search json should render");

        let value: serde_json::Value =
            serde_json::from_str(&json).expect("json should parse successfully");
        assert_eq!(value["results"][0]["resolved_via"], "MESH crosswalk");
        assert_eq!(value["results"][0]["source_id"], "MESH:D001139");
        assert_eq!(value["_meta"]["fallback_used"], true);
    }

    #[test]
    fn disease_search_json_omits_meta_for_direct_hits() {
        let pagination = PaginationMeta::offset(0, 10, 1, Some(1));
        let json = disease_search_json(
            vec![crate::entities::disease::DiseaseSearchResult {
                id: "MONDO:0005105".into(),
                name: "melanoma".into(),
                synonyms_preview: Some("malignant melanoma".into()),
                resolved_via: None,
                source_id: None,
            }],
            pagination,
            false,
        )
        .expect("disease search json should render");

        let value: serde_json::Value =
            serde_json::from_str(&json).expect("json should parse successfully");
        assert!(value.get("_meta").is_none());
        assert!(value["results"][0].get("resolved_via").is_none());
        assert!(value["results"][0].get("source_id").is_none());
    }

    #[test]
    fn build_article_debug_plan_includes_article_type_limitation_note() {
        let filters = crate::entities::article::ArticleSearchFilters {
            gene: Some("BRAF".into()),
            gene_anchored: false,
            disease: None,
            drug: None,
            author: None,
            keyword: None,
            date_from: None,
            date_to: None,
            article_type: Some("review".into()),
            journal: None,
            open_access: false,
            no_preprints: false,
            exclude_retracted: false,
            max_per_source: None,
            sort: crate::entities::article::ArticleSort::Relevance,
            ranking: crate::entities::article::ArticleRankingOptions::default(),
        };
        let pagination = PaginationMeta::offset(0, 3, 0, Some(0));

        let plan = build_article_debug_plan(
            "gene=BRAF, type=review",
            &filters,
            crate::entities::article::ArticleSourceFilter::All,
            3,
            &[],
            &pagination,
        )
        .expect("debug plan should build");

        assert_eq!(plan.legs.len(), 1);
        assert!(
            plan.legs[0]
                .note
                .as_deref()
                .is_some_and(|value: &str| value.contains("Europe PMC and PubMed"))
        );
    }

    #[test]
    fn paginate_trial_locations_handles_missing_locations() {
        let mut trial = crate::entities::trial::Trial {
            nct_id: "NCT00000001".to_string(),
            source: Some("ctgov".to_string()),
            title: "Example trial".to_string(),
            status: "Recruiting".to_string(),
            phase: Some("Phase 2".to_string()),
            study_type: Some("Interventional".to_string()),
            age_range: Some("18 Years and older".to_string()),
            conditions: vec!["melanoma".to_string()],
            interventions: vec!["osimertinib".to_string()],
            sponsor: Some("Example Sponsor".to_string()),
            enrollment: Some(100),
            summary: Some("Example summary".to_string()),
            start_date: Some("2024-01-01".to_string()),
            completion_date: None,
            eligibility_text: None,
            locations: None,
            outcomes: None,
            arms: None,
            references: None,
        };

        let meta = paginate_trial_locations(&mut trial, 20, 10);
        assert_eq!(meta.total, 0);
        assert_eq!(meta.offset, 20);
        assert_eq!(meta.limit, 10);
        assert!(!meta.has_more);
        assert!(trial.locations.is_some());
        assert_eq!(trial.locations.as_ref().map_or(usize::MAX, Vec::len), 0);
    }

    #[test]
    fn pathway_trial_fallback_allows_no_match_on_first_page() {
        assert!(should_try_pathway_trial_fallback(0, 0, Some(0)));
        assert!(should_try_pathway_trial_fallback(0, 0, None));
    }

    #[test]
    fn pathway_trial_fallback_skips_offset_or_known_matches() {
        assert!(!should_try_pathway_trial_fallback(0, 5, Some(2)));
        assert!(!should_try_pathway_trial_fallback(0, 0, Some(7)));
        assert!(!should_try_pathway_trial_fallback(1, 0, Some(1)));
    }

    #[test]
    fn trial_search_query_summary_includes_geo_filters() {
        let summary = trial_search_query_summary(
            &crate::entities::trial::TrialSearchFilters {
                condition: Some("melanoma".into()),
                facility: Some("MD Anderson".into()),
                age: Some(67.0),
                sex: Some("female".into()),
                criteria: Some("mismatch repair deficient".into()),
                sponsor_type: Some("nih".into()),
                lat: Some(40.7128),
                lon: Some(-74.006),
                distance: Some(50),
                ..Default::default()
            },
            0,
            None,
        );
        assert!(summary.contains("condition=melanoma"));
        assert!(summary.contains("facility=MD Anderson"));
        assert!(summary.contains("age=67"));
        assert!(summary.contains("sex=female"));
        assert!(summary.contains("criteria=mismatch repair deficient"));
        assert!(summary.contains("sponsor_type=nih"));
        assert!(summary.contains("lat=40.7128"));
        assert!(summary.contains("lon=-74.006"));
        assert!(summary.contains("distance=50"));
    }

    #[test]
    fn trial_search_query_summary_includes_nci_source_marker() {
        let summary = trial_search_query_summary(
            &crate::entities::trial::TrialSearchFilters {
                condition: Some("melanoma".into()),
                source: crate::entities::trial::TrialSource::NciCts,
                ..Default::default()
            },
            0,
            None,
        );

        assert!(summary.contains("condition=melanoma"));
        assert!(summary.contains("source=nci"));
    }

    #[test]
    fn trial_zero_result_nickname_hint_requires_positional_ctgov_query_with_zero_results() {
        use crate::entities::trial::TrialSource;

        assert!(should_show_trial_zero_result_nickname_hint(
            Some("CodeBreaK 300"),
            TrialSource::ClinicalTrialsGov,
            0
        ));
        assert!(!should_show_trial_zero_result_nickname_hint(
            None,
            TrialSource::ClinicalTrialsGov,
            0
        ));
        assert!(!should_show_trial_zero_result_nickname_hint(
            Some("CodeBreaK 300"),
            TrialSource::NciCts,
            0
        ));
        assert!(!should_show_trial_zero_result_nickname_hint(
            Some("CodeBreaK 300"),
            TrialSource::ClinicalTrialsGov,
            1
        ));
    }

    #[test]
    fn resolve_query_input_accepts_flag_or_positional() {
        let from_flag = resolve_query_input(Some("BRAF".into()), None, "--query").unwrap();
        assert_eq!(from_flag.as_deref(), Some("BRAF"));

        let from_positional =
            resolve_query_input(None, Some("melanoma".into()), "--query").unwrap();
        assert_eq!(from_positional.as_deref(), Some("melanoma"));
    }

    #[test]
    fn resolve_query_input_rejects_dual_values() {
        let err =
            resolve_query_input(Some("BRAF".into()), Some("TP53".into()), "--query").unwrap_err();
        assert!(format!("{err}").contains("Use either positional QUERY or --query, not both"));

        let err_gene =
            resolve_query_input(Some("TP53".into()), Some("BRAF".into()), "--gene").unwrap_err();
        assert!(format!("{err_gene}").contains("Use either positional QUERY or --gene, not both"));
    }

    #[test]
    fn search_drug_region_defaults_to_all_for_name_only_queries() {
        let filters = DrugSearchFilters {
            query: Some("Keytruda".into()),
            ..Default::default()
        };

        let region = resolve_drug_search_region(None, &filters).expect("name-only default");
        assert_eq!(region, DrugRegion::All);
    }

    #[test]
    fn search_drug_region_defaults_to_us_for_structured_queries() {
        let filters = DrugSearchFilters {
            target: Some("EGFR".into()),
            ..Default::default()
        };

        let region = resolve_drug_search_region(None, &filters).expect("structured default");
        assert_eq!(region, DrugRegion::Us);
    }

    #[test]
    fn search_drug_region_rejects_explicit_non_us_for_structured_queries() {
        let filters = DrugSearchFilters {
            target: Some("EGFR".into()),
            ..Default::default()
        };

        let err = resolve_drug_search_region(Some(super::DrugRegionArg::Eu), &filters)
            .expect_err("explicit eu should be rejected");
        assert!(format!("{err}").contains(
            "EMA and all-region search currently support name/alias lookups only; use --region us for structured MyChem filters or --region who to filter structured U.S. hits through WHO prequalification."
        ));

        let err = resolve_drug_search_region(Some(super::DrugRegionArg::All), &filters)
            .expect_err("explicit all should be rejected");
        assert!(format!("{err}").contains(
            "EMA and all-region search currently support name/alias lookups only; use --region us for structured MyChem filters or --region who to filter structured U.S. hits through WHO prequalification."
        ));
    }

    #[test]
    fn search_drug_region_allows_explicit_who_for_structured_queries() {
        let filters = DrugSearchFilters {
            indication: Some("malaria".into()),
            ..Default::default()
        };

        let region =
            resolve_drug_search_region(Some(super::DrugRegionArg::Who), &filters).expect("who");
        assert_eq!(region, DrugRegion::Who);
    }

    #[test]
    fn search_json_preserves_who_search_fields() {
        let pagination = PaginationMeta::offset(0, 5, 1, Some(1));
        let json = search_json(
            vec![crate::entities::drug::WhoPrequalificationSearchResult {
                inn: "Trastuzumab".to_string(),
                therapeutic_area: "Oncology".to_string(),
                dosage_form: "Powder for concentrate for solution for infusion".to_string(),
                applicant: "Samsung Bioepis NL B.V.".to_string(),
                who_reference_number: "BT-ON001".to_string(),
                listing_basis: "Prequalification - Abridged".to_string(),
                prequalification_date: Some("2019-12-18".to_string()),
            }],
            pagination,
        )
        .expect("WHO search json");

        let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert_eq!(value["count"], 1);
        assert_eq!(value["results"][0]["who_reference_number"], "BT-ON001");
        assert_eq!(
            value["results"][0]["listing_basis"],
            "Prequalification - Abridged"
        );
        assert_eq!(value["results"][0]["prequalification_date"], "2019-12-18");
    }

    #[test]
    fn phenotype_search_json_contract_unchanged() {
        let pagination = PaginationMeta::offset(0, 1, 1, Some(1));
        let json = search_json(
            vec![crate::entities::disease::PhenotypeSearchResult {
                disease_id: "MONDO:0100135".to_string(),
                disease_name: "Dravet syndrome".to_string(),
                score: 15.036,
            }],
            pagination,
        )
        .expect("phenotype search json");

        let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert_eq!(value["count"], 1);
        assert_eq!(value["results"][0]["disease_id"], "MONDO:0100135");
        assert_eq!(value["results"][0]["disease_name"], "Dravet syndrome");
        assert!(
            value.get("_meta").is_none(),
            "generic search json should not grow entity-style _meta"
        );
    }

    #[test]
    fn drug_all_region_search_json_includes_who_bucket() {
        let json = drug_all_region_search_json(
            "trastuzumab",
            crate::entities::SearchPage::offset(
                vec![crate::entities::drug::DrugSearchResult {
                    name: "trastuzumab".to_string(),
                    drugbank_id: None,
                    drug_type: None,
                    mechanism: None,
                    target: Some("ERBB2".to_string()),
                }],
                Some(1),
            ),
            crate::entities::SearchPage::offset(
                vec![crate::entities::drug::EmaDrugSearchResult {
                    name: "Herzuma".to_string(),
                    active_substance: "trastuzumab".to_string(),
                    ema_product_number: "EMEA/H/C/004123".to_string(),
                    status: "Authorised".to_string(),
                }],
                Some(1),
            ),
            crate::entities::SearchPage::offset(
                vec![crate::entities::drug::WhoPrequalificationSearchResult {
                    inn: "Trastuzumab".to_string(),
                    therapeutic_area: "Oncology".to_string(),
                    dosage_form: "Powder for concentrate for solution for infusion".to_string(),
                    applicant: "Samsung Bioepis NL B.V.".to_string(),
                    who_reference_number: "BT-ON001".to_string(),
                    listing_basis: "Prequalification - Abridged".to_string(),
                    prequalification_date: Some("2019-12-18".to_string()),
                }],
                Some(1),
            ),
        )
        .expect("all-region drug search json");

        let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert_eq!(value["region"], "all");
        assert_eq!(value["who"]["count"], 1);
        assert_eq!(value["who"]["total"], 1);
        assert_eq!(
            value["who"]["results"][0]["who_reference_number"],
            "BT-ON001"
        );
        assert_eq!(
            value["eu"]["results"][0]["ema_product_number"],
            "EMEA/H/C/004123"
        );
    }

    #[test]
    fn parse_simple_gene_change_detects_supported_forms() {
        assert_eq!(
            parse_simple_gene_change("BRAF V600E"),
            Some(("BRAF".into(), "V600E".into()))
        );
        assert_eq!(
            parse_simple_gene_change("EGFR T790M"),
            Some(("EGFR".into(), "T790M".into()))
        );
        assert_eq!(
            parse_simple_gene_change("BRAF p.V600E"),
            Some(("BRAF".into(), "V600E".into()))
        );
        assert_eq!(
            parse_simple_gene_change("BRAF p.Val600Glu"),
            Some(("BRAF".into(), "V600E".into()))
        );
    }

    #[test]
    fn parse_simple_gene_change_rejects_non_simple_forms() {
        assert_eq!(parse_simple_gene_change("BRAF"), None);
        assert_eq!(parse_simple_gene_change("EGFR Exon 19 Deletion"), None);
        assert_eq!(parse_simple_gene_change("EGFR Exon19"), None);
        assert_eq!(parse_simple_gene_change("braf V600E"), None);
    }

    #[test]
    fn resolve_variant_query_maps_single_token_to_gene() {
        let resolved = resolve_variant_query(None, None, None, None, vec!["BRAF".into()]).unwrap();
        let VariantSearchPlan::Standard(resolved) = resolved else {
            panic!("expected standard search plan");
        };
        assert_eq!(resolved.gene.as_deref(), Some("BRAF"));
        assert!(resolved.hgvsp.is_none());
        assert!(resolved.hgvsc.is_none());
        assert!(resolved.rsid.is_none());
        assert!(resolved.condition.is_none());
    }

    #[test]
    fn resolve_variant_query_maps_simple_gene_change_to_gene_and_hgvsp() {
        let resolved =
            resolve_variant_query(None, None, None, None, vec!["BRAF".into(), "V600E".into()])
                .unwrap();
        let VariantSearchPlan::Standard(resolved) = resolved else {
            panic!("expected standard search plan");
        };
        assert_eq!(resolved.gene.as_deref(), Some("BRAF"));
        assert_eq!(resolved.hgvsp.as_deref(), Some("V600E"));
        assert!(resolved.hgvsc.is_none());
        assert!(resolved.rsid.is_none());
        assert!(resolved.condition.is_none());
    }

    #[test]
    fn resolve_variant_query_maps_long_form_positional_gene_change_to_gene_and_hgvsp() {
        let resolved = resolve_variant_query(
            None,
            None,
            None,
            None,
            vec!["BRAF".into(), "p.Val600Glu".into()],
        )
        .unwrap();
        let VariantSearchPlan::Standard(resolved) = resolved else {
            panic!("expected standard search plan");
        };
        assert_eq!(resolved.gene.as_deref(), Some("BRAF"));
        assert_eq!(resolved.hgvsp.as_deref(), Some("V600E"));
        assert!(resolved.hgvsc.is_none());
        assert!(resolved.rsid.is_none());
        assert!(resolved.condition.is_none());
    }

    #[test]
    fn resolve_variant_query_maps_rsid_to_rsid_filter() {
        let resolved =
            resolve_variant_query(None, None, None, None, vec!["rs113488022".into()]).unwrap();
        let VariantSearchPlan::Standard(resolved) = resolved else {
            panic!("expected standard search plan");
        };
        assert_eq!(resolved.rsid.as_deref(), Some("rs113488022"));
        assert!(resolved.gene.is_none());
        assert!(resolved.hgvsp.is_none());
        assert!(resolved.hgvsc.is_none());
        assert!(resolved.condition.is_none());
    }

    #[test]
    fn resolve_variant_query_maps_gene_hgvsc_text_to_gene_and_hgvsc() {
        let resolved = resolve_variant_query(
            None,
            None,
            None,
            None,
            vec!["BRAF".into(), "c.1799T>A".into()],
        )
        .unwrap();
        let VariantSearchPlan::Standard(resolved) = resolved else {
            panic!("expected standard search plan");
        };
        assert_eq!(resolved.gene.as_deref(), Some("BRAF"));
        assert_eq!(resolved.hgvsc.as_deref(), Some("c.1799T>A"));
        assert!(resolved.hgvsp.is_none());
        assert!(resolved.rsid.is_none());
        assert!(resolved.condition.is_none());
    }

    #[test]
    fn resolve_variant_query_maps_exon_deletion_phrase_to_gene_and_consequence() {
        let resolved = resolve_variant_query(
            None,
            None,
            None,
            None,
            vec!["EGFR".into(), "Exon".into(), "19".into(), "Deletion".into()],
        )
        .unwrap();
        let VariantSearchPlan::Standard(resolved) = resolved else {
            panic!("expected standard search plan");
        };
        assert_eq!(resolved.gene.as_deref(), Some("EGFR"));
        assert_eq!(resolved.consequence.as_deref(), Some("inframe_deletion"));
        assert!(resolved.hgvsp.is_none());
        assert!(resolved.hgvsc.is_none());
        assert!(resolved.rsid.is_none());
        assert!(resolved.condition.is_none());
    }

    #[test]
    fn resolve_variant_query_maps_gene_residue_alias_to_residue_alias_search() {
        let resolved =
            resolve_variant_query(None, None, None, None, vec!["PTPN22".into(), "620W".into()])
                .unwrap();
        let VariantSearchPlan::Standard(resolved) = resolved else {
            panic!("expected standard search plan");
        };
        assert_eq!(resolved.gene.as_deref(), Some("PTPN22"));
        assert_eq!(
            resolved.protein_alias,
            Some(crate::entities::variant::VariantProteinAlias {
                position: 620,
                residue: 'W',
            })
        );
        assert!(resolved.hgvsp.is_none());
        assert!(resolved.condition.is_none());
    }

    #[test]
    fn resolve_variant_query_maps_gene_flag_residue_alias_to_residue_alias_search() {
        let resolved =
            resolve_variant_query(Some("PTPN22".into()), None, None, None, vec!["620W".into()])
                .unwrap();
        let VariantSearchPlan::Standard(resolved) = resolved else {
            panic!("expected standard search plan");
        };
        assert_eq!(resolved.gene.as_deref(), Some("PTPN22"));
        assert_eq!(
            resolved.protein_alias,
            Some(crate::entities::variant::VariantProteinAlias {
                position: 620,
                residue: 'W',
            })
        );
        assert!(resolved.hgvsp.is_none());
        assert!(resolved.condition.is_none());
    }

    #[test]
    fn resolve_variant_query_uses_gene_context_for_standalone_protein_change() {
        let resolved = resolve_variant_query(
            Some("PTPN22".into()),
            None,
            None,
            None,
            vec!["R620W".into()],
        )
        .unwrap();
        let VariantSearchPlan::Standard(resolved) = resolved else {
            panic!("expected standard search plan");
        };
        assert_eq!(resolved.gene.as_deref(), Some("PTPN22"));
        assert_eq!(resolved.hgvsp.as_deref(), Some("R620W"));
        assert!(resolved.protein_alias.is_none());
    }

    #[test]
    fn resolve_variant_query_uses_gene_context_for_long_form_single_token_change() {
        let resolved = resolve_variant_query(
            Some("BRAF".into()),
            None,
            None,
            None,
            vec!["p.Val600Glu".into()],
        )
        .unwrap();
        let VariantSearchPlan::Standard(resolved) = resolved else {
            panic!("expected standard search plan");
        };
        assert_eq!(resolved.gene.as_deref(), Some("BRAF"));
        assert_eq!(resolved.hgvsp.as_deref(), Some("V600E"));
        assert!(resolved.protein_alias.is_none());
    }

    #[test]
    fn resolve_variant_query_returns_guidance_for_standalone_protein_change() {
        let resolved = resolve_variant_query(None, None, None, None, vec!["R620W".into()]).unwrap();
        let VariantSearchPlan::Guidance(guidance) = resolved else {
            panic!("expected guidance plan");
        };
        assert_eq!(guidance.query, "R620W");
        assert!(matches!(
            guidance.kind,
            crate::entities::variant::VariantGuidanceKind::ProteinChangeOnly { .. }
        ));
    }

    #[test]
    fn resolve_variant_query_returns_guidance_for_long_form_single_token_change() {
        let resolved =
            resolve_variant_query(None, None, None, None, vec!["p.Val600Glu".into()]).unwrap();
        let VariantSearchPlan::Guidance(guidance) = resolved else {
            panic!("expected guidance plan");
        };
        assert_eq!(guidance.query, "p.Val600Glu");
        assert!(matches!(
            guidance.kind,
            crate::entities::variant::VariantGuidanceKind::ProteinChangeOnly { .. }
        ));
        assert_eq!(
            guidance.next_commands.first().map(String::as_str),
            Some("biomcp search variant --hgvsp V600E --limit 10")
        );
    }

    #[test]
    fn resolve_variant_query_normalizes_long_form_hgvsp_flag() {
        let resolved = resolve_variant_query(
            Some("BRAF".into()),
            Some("p.Val600Glu".into()),
            None,
            None,
            Vec::new(),
        )
        .unwrap();
        let VariantSearchPlan::Standard(resolved) = resolved else {
            panic!("expected standard search plan");
        };
        assert_eq!(resolved.gene.as_deref(), Some("BRAF"));
        assert_eq!(resolved.hgvsp.as_deref(), Some("V600E"));
        assert!(resolved.hgvsc.is_none());
        assert!(resolved.rsid.is_none());
        assert!(resolved.condition.is_none());
    }

    #[test]
    fn resolve_variant_query_preserves_stop_x_for_hgvsp_flag() {
        let resolved = resolve_variant_query(
            Some("PLN".into()),
            Some("L39X".into()),
            None,
            None,
            Vec::new(),
        )
        .unwrap();
        let VariantSearchPlan::Standard(resolved) = resolved else {
            panic!("expected standard search plan");
        };
        assert_eq!(resolved.gene.as_deref(), Some("PLN"));
        assert_eq!(resolved.hgvsp.as_deref(), Some("L39X"));
    }

    #[test]
    fn resolve_variant_query_rejects_conflicts_with_positional_mapping() {
        let gene_conflict = resolve_variant_query(
            Some("TP53".into()),
            None,
            None,
            None,
            vec!["BRAF".into(), "V600E".into()],
        )
        .unwrap_err();
        assert!(format!("{gene_conflict}").contains("conflicts with --gene"));

        let hgvsp_conflict = resolve_variant_query(
            None,
            Some("G12D".into()),
            None,
            None,
            vec!["KRAS".into(), "G12C".into()],
        )
        .unwrap_err();
        assert!(format!("{hgvsp_conflict}").contains("conflicts with --hgvsp"));

        let consequence_conflict = resolve_variant_query(
            None,
            None,
            Some("missense_variant".into()),
            None,
            vec!["EGFR".into(), "Exon".into(), "19".into(), "Deletion".into()],
        )
        .unwrap_err();
        assert!(
            format!("{consequence_conflict}")
                .contains("Positional exon-deletion query conflicts with --consequence")
        );
    }

    #[test]
    fn related_article_filters_default_to_relevance_and_safety_flags() {
        let filters = super::related_article_filters();

        assert_eq!(
            filters.sort,
            crate::entities::article::ArticleSort::Relevance
        );
        assert!(!filters.open_access);
        assert!(filters.no_preprints);
        assert!(filters.exclude_retracted);
        assert_eq!(filters.max_per_source, None);
    }

    #[test]
    fn article_query_and_debug_filters_include_effective_ranking_context() {
        let mut filters = super::related_article_filters();
        filters.keyword = Some("melanoma".into());
        filters.max_per_source = Some(10);

        let summary = super::article_query_summary(
            &filters,
            crate::entities::article::ArticleSourceFilter::All,
            false,
            25,
            0,
        );
        assert!(summary.contains("ranking_mode=hybrid"));
        assert!(summary.contains("max_per_source=10"));
        assert!(summary.contains(
            "ranking_policy=hybrid relevance (score = 0.4*semantic + 0.3*lexical + 0.2*citations + 0.1*position)"
        ));

        let debug_filters = super::article_debug_filters(
            &filters,
            crate::entities::article::ArticleSourceFilter::All,
            25,
        );
        assert!(
            debug_filters
                .iter()
                .any(|entry| entry == "ranking_mode=hybrid")
        );
        assert!(
            debug_filters
                .iter()
                .any(|entry| entry == "max_per_source=10")
        );
        assert!(debug_filters.iter().any(|entry| {
            entry
                == "ranking_policy=hybrid relevance (score = 0.4*semantic + 0.3*lexical + 0.2*citations + 0.1*position)"
        }));
    }

    #[test]
    fn article_query_and_debug_filters_render_default_and_disabled_max_per_source_modes() {
        let mut filters = super::related_article_filters();
        filters.gene = Some("BRAF".into());
        filters.max_per_source = Some(0);

        let summary = super::article_query_summary(
            &filters,
            crate::entities::article::ArticleSourceFilter::All,
            false,
            25,
            0,
        );
        assert!(summary.contains("max_per_source=default"));

        let debug_filters = super::article_debug_filters(
            &filters,
            crate::entities::article::ArticleSourceFilter::All,
            25,
        );
        assert!(
            debug_filters
                .iter()
                .any(|entry| entry == "max_per_source=default")
        );

        filters.max_per_source = Some(25);
        let disabled_summary = super::article_query_summary(
            &filters,
            crate::entities::article::ArticleSourceFilter::All,
            false,
            25,
            0,
        );
        assert!(disabled_summary.contains("max_per_source=disabled"));

        let disabled_debug_filters = super::article_debug_filters(
            &filters,
            crate::entities::article::ArticleSourceFilter::All,
            25,
        );
        assert!(
            disabled_debug_filters
                .iter()
                .any(|entry| entry == "max_per_source=disabled")
        );
    }

    #[test]
    fn chart_args_default_to_no_chart() {
        let args = ChartArgs {
            chart: None,
            terminal: false,
            output: None,
            title: None,
            theme: None,
            palette: None,
            cols: None,
            rows: None,
            width: None,
            height: None,
            scale: None,
            mcp_inline: false,
        };
        assert_eq!(args.chart, None);
        assert!(!args.terminal);
        assert!(!args.mcp_inline);
        assert_eq!(args.cols, None);
        assert_eq!(args.rows, None);
        assert_eq!(args.width, None);
        assert_eq!(args.height, None);
        assert_eq!(args.scale, None);
    }

    #[test]
    fn chart_dimension_flags_validate_positive_values() {
        let cols_err = Cli::try_parse_from([
            "biomcp",
            "study",
            "query",
            "--study",
            "msk_impact_2017",
            "--gene",
            "TP53",
            "--type",
            "mutations",
            "--chart",
            "bar",
            "--cols",
            "0",
        ])
        .expect_err("zero columns should fail");
        assert!(cols_err.to_string().contains("--cols must be >= 1"));

        let scale_err = Cli::try_parse_from([
            "biomcp",
            "study",
            "query",
            "--study",
            "msk_impact_2017",
            "--gene",
            "TP53",
            "--type",
            "mutations",
            "--chart",
            "bar",
            "--scale",
            "0",
        ])
        .expect_err("zero scale should fail");
        assert!(scale_err.to_string().contains("--scale must be > 0"));

        let nan_err = Cli::try_parse_from([
            "biomcp",
            "study",
            "query",
            "--study",
            "msk_impact_2017",
            "--gene",
            "TP53",
            "--type",
            "mutations",
            "--chart",
            "bar",
            "--scale",
            "NaN",
            "-o",
            "chart.png",
        ])
        .expect_err("non-finite scale should fail");
        assert!(
            nan_err
                .to_string()
                .contains("--scale must be a finite number > 0")
        );
    }

    #[test]
    fn rewrite_mcp_chart_args_preserves_svg_sizing_flags() {
        let args = vec![
            "biomcp".to_string(),
            "study".to_string(),
            "query".to_string(),
            "--study".to_string(),
            "demo".to_string(),
            "--gene".to_string(),
            "TP53".to_string(),
            "--type".to_string(),
            "mutations".to_string(),
            "--chart".to_string(),
            "bar".to_string(),
            "--width".to_string(),
            "1200".to_string(),
            "--height".to_string(),
            "600".to_string(),
            "--title".to_string(),
            "Example".to_string(),
        ];

        let text = rewrite_mcp_chart_args(&args, McpChartPass::Text).expect("text rewrite");
        assert!(!text.iter().any(|value| value == "--chart"));
        assert!(!text.iter().any(|value| value == "--width"));
        assert!(!text.iter().any(|value| value == "--height"));

        let svg = rewrite_mcp_chart_args(&args, McpChartPass::Svg).expect("svg rewrite");
        assert!(svg.iter().any(|value| value == "--chart"));
        assert!(svg.iter().any(|value| value == "--width"));
        assert!(svg.iter().any(|value| value == "--height"));
        assert!(svg.iter().any(|value| value == "--mcp-inline"));
    }

    #[test]
    fn rewrite_mcp_chart_args_rejects_terminal_and_png_only_flags() {
        let cols_err = rewrite_mcp_chart_args(
            &[
                "biomcp".to_string(),
                "study".to_string(),
                "query".to_string(),
                "--study".to_string(),
                "demo".to_string(),
                "--gene".to_string(),
                "TP53".to_string(),
                "--type".to_string(),
                "mutations".to_string(),
                "--chart".to_string(),
                "bar".to_string(),
                "--cols".to_string(),
                "80".to_string(),
            ],
            McpChartPass::Svg,
        )
        .expect_err("mcp svg rewrite should reject terminal sizing");
        assert!(
            cols_err
                .to_string()
                .contains("--cols/--rows require terminal chart output"),
            "{cols_err}"
        );

        let scale_err = rewrite_mcp_chart_args(
            &[
                "biomcp".to_string(),
                "study".to_string(),
                "query".to_string(),
                "--study".to_string(),
                "demo".to_string(),
                "--gene".to_string(),
                "TP53".to_string(),
                "--type".to_string(),
                "mutations".to_string(),
                "--chart".to_string(),
                "bar".to_string(),
                "--scale".to_string(),
                "2.0".to_string(),
            ],
            McpChartPass::Svg,
        )
        .expect_err("mcp svg rewrite should reject png scale");
        assert!(
            scale_err
                .to_string()
                .contains("--scale requires PNG chart output"),
            "{scale_err}"
        );
    }

    #[test]
    fn study_survival_parses_endpoint_flag() {
        let cli = Cli::try_parse_from([
            "biomcp",
            "study",
            "survival",
            "--study",
            "brca_tcga_pan_can_atlas_2018",
            "--gene",
            "TP53",
            "--endpoint",
            "dfs",
        ])
        .expect("study survival should parse");
        match cli.command {
            Commands::Study {
                cmd:
                    StudyCommand::Survival {
                        study,
                        gene,
                        endpoint,
                        ..
                    },
            } => {
                assert_eq!(study, "brca_tcga_pan_can_atlas_2018");
                assert_eq!(gene, "TP53");
                assert_eq!(endpoint, "dfs");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn study_compare_parses_type_and_target() {
        let cli = Cli::try_parse_from([
            "biomcp",
            "study",
            "compare",
            "--study",
            "brca_tcga_pan_can_atlas_2018",
            "--gene",
            "TP53",
            "--type",
            "expression",
            "--target",
            "ERBB2",
        ])
        .expect("study compare should parse");
        match cli.command {
            Commands::Study {
                cmd:
                    StudyCommand::Compare {
                        study,
                        gene,
                        compare_type,
                        target,
                        ..
                    },
            } => {
                assert_eq!(study, "brca_tcga_pan_can_atlas_2018");
                assert_eq!(gene, "TP53");
                assert_eq!(compare_type, "expression");
                assert_eq!(target, "ERBB2");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn study_filter_parses_all_flags_and_repeated_values() {
        let cli = Cli::try_parse_from([
            "biomcp",
            "study",
            "filter",
            "--study",
            "brca_tcga_pan_can_atlas_2018",
            "--mutated",
            "TP53",
            "--mutated",
            "PIK3CA",
            "--amplified",
            "ERBB2",
            "--deleted",
            "PTEN",
            "--expression-above",
            "MYC:1.5",
            "--expression-above",
            "ERBB2:-0.5",
            "--expression-below",
            "ESR1:0.5",
            "--cancer-type",
            "Breast Cancer",
            "--cancer-type",
            "Lung Cancer",
        ])
        .expect("study filter should parse");
        match cli.command {
            Commands::Study {
                cmd:
                    StudyCommand::Filter {
                        study,
                        mutated,
                        amplified,
                        deleted,
                        expression_above,
                        expression_below,
                        cancer_type,
                    },
            } => {
                assert_eq!(study, "brca_tcga_pan_can_atlas_2018");
                assert_eq!(mutated, vec!["TP53", "PIK3CA"]);
                assert_eq!(amplified, vec!["ERBB2"]);
                assert_eq!(deleted, vec!["PTEN"]);
                assert_eq!(expression_above, vec!["MYC:1.5", "ERBB2:-0.5"]);
                assert_eq!(expression_below, vec!["ESR1:0.5"]);
                assert_eq!(cancer_type, vec!["Breast Cancer", "Lung Cancer"]);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn study_co_occurrence_parses_gene_list() {
        let cli = Cli::try_parse_from([
            "biomcp",
            "study",
            "co-occurrence",
            "--study",
            "brca_tcga_pan_can_atlas_2018",
            "--genes",
            "TP53,PIK3CA,GATA3",
        ])
        .expect("study co-occurrence should parse");
        match cli.command {
            Commands::Study {
                cmd: StudyCommand::CoOccurrence { study, genes, .. },
            } => {
                assert_eq!(study, "brca_tcga_pan_can_atlas_2018");
                assert_eq!(genes, "TP53,PIK3CA,GATA3");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn truncate_article_annotations_applies_limit_per_bucket() {
        let annotations = crate::entities::article::ArticleAnnotations {
            genes: vec![
                crate::entities::article::AnnotationCount {
                    text: "BRAF".into(),
                    count: 2,
                },
                crate::entities::article::AnnotationCount {
                    text: "TP53".into(),
                    count: 1,
                },
            ],
            diseases: vec![
                crate::entities::article::AnnotationCount {
                    text: "melanoma".into(),
                    count: 2,
                },
                crate::entities::article::AnnotationCount {
                    text: "glioma".into(),
                    count: 1,
                },
            ],
            chemicals: vec![
                crate::entities::article::AnnotationCount {
                    text: "vemurafenib".into(),
                    count: 1,
                },
                crate::entities::article::AnnotationCount {
                    text: "dabrafenib".into(),
                    count: 1,
                },
            ],
            mutations: vec![
                crate::entities::article::AnnotationCount {
                    text: "V600E".into(),
                    count: 1,
                },
                crate::entities::article::AnnotationCount {
                    text: "L858R".into(),
                    count: 1,
                },
            ],
        };
        let truncated = truncate_article_annotations(annotations, 1);
        assert_eq!(truncated.genes.len(), 1);
        assert_eq!(truncated.diseases.len(), 1);
        assert_eq!(truncated.chemicals.len(), 1);
        assert_eq!(truncated.mutations.len(), 1);
    }

    #[tokio::test]
    async fn enrich_rejects_zero_limit_before_api_call() {
        let err = execute(vec![
            "biomcp".to_string(),
            "enrich".to_string(),
            "BRCA1,TP53".to_string(),
            "--limit".to_string(),
            "0".to_string(),
        ])
        .await
        .expect_err("enrich should reject --limit 0");
        assert!(err.to_string().contains("--limit must be between 1 and 50"));
    }

    #[tokio::test]
    async fn enrich_rejects_limit_above_max_before_api_call() {
        let err = execute(vec![
            "biomcp".to_string(),
            "enrich".to_string(),
            "BRCA1,TP53".to_string(),
            "--limit".to_string(),
            "51".to_string(),
        ])
        .await
        .expect_err("enrich should reject --limit > 50");
        assert!(err.to_string().contains("--limit must be between 1 and 50"));
    }

    #[tokio::test]
    async fn search_adverse_event_device_rejects_positional_drug_alias() {
        let err = execute(vec![
            "biomcp".to_string(),
            "search".to_string(),
            "adverse-event".to_string(),
            "pembrolizumab".to_string(),
            "--type".to_string(),
            "device".to_string(),
        ])
        .await
        .expect_err("device query should reject positional drug alias");
        assert!(
            err.to_string()
                .contains("--drug cannot be used with --type device")
        );
    }

    #[tokio::test]
    async fn search_all_requires_at_least_one_typed_slot() {
        let err = execute(vec![
            "biomcp".to_string(),
            "search".to_string(),
            "all".to_string(),
        ])
        .await
        .expect_err("search all should require typed slots");
        assert!(err.to_string().contains("at least one typed slot"));
        assert!(err.to_string().contains("--gene"));
    }

    #[tokio::test]
    async fn search_pathway_requires_query_unless_top_level() {
        let err = execute(vec![
            "biomcp".to_string(),
            "search".to_string(),
            "pathway".to_string(),
        ])
        .await
        .expect_err("search pathway should require query unless --top-level");
        assert!(
            err.to_string().contains(
                "Query is required. Example: biomcp search pathway -q \"MAPK signaling\""
            )
        );
    }

    #[tokio::test]
    async fn study_co_occurrence_requires_2_to_10_genes() {
        let err = execute(vec![
            "biomcp".to_string(),
            "study".to_string(),
            "co-occurrence".to_string(),
            "--study".to_string(),
            "msk_impact_2017".to_string(),
            "--genes".to_string(),
            "TP53".to_string(),
        ])
        .await
        .expect_err("study co-occurrence should validate gene count");
        assert!(err.to_string().contains("--genes must contain 2 to 10"));
    }

    #[tokio::test]
    async fn study_filter_requires_at_least_one_criterion() {
        let err = execute(vec![
            "biomcp".to_string(),
            "study".to_string(),
            "filter".to_string(),
            "--study".to_string(),
            "brca_tcga_pan_can_atlas_2018".to_string(),
        ])
        .await
        .expect_err("study filter should require criteria");
        assert!(
            err.to_string()
                .contains("At least one filter criterion is required")
        );
    }

    #[tokio::test]
    async fn study_filter_rejects_malformed_expression_threshold() {
        let err = execute(vec![
            "biomcp".to_string(),
            "study".to_string(),
            "filter".to_string(),
            "--study".to_string(),
            "brca_tcga_pan_can_atlas_2018".to_string(),
            "--expression-above".to_string(),
            "MYC:not-a-number".to_string(),
        ])
        .await
        .expect_err("study filter should validate threshold format");
        assert!(err.to_string().contains("--expression-above"));
        assert!(err.to_string().contains("GENE:THRESHOLD"));
    }

    #[tokio::test]
    async fn study_survival_rejects_unknown_endpoint() {
        let err = execute(vec![
            "biomcp".to_string(),
            "study".to_string(),
            "survival".to_string(),
            "--study".to_string(),
            "msk_impact_2017".to_string(),
            "--gene".to_string(),
            "TP53".to_string(),
            "--endpoint".to_string(),
            "foo".to_string(),
        ])
        .await
        .expect_err("study survival should validate endpoint");
        assert!(err.to_string().contains("Unknown survival endpoint"));
    }

    #[tokio::test]
    async fn study_compare_rejects_unknown_type() {
        let err = execute(vec![
            "biomcp".to_string(),
            "study".to_string(),
            "compare".to_string(),
            "--study".to_string(),
            "msk_impact_2017".to_string(),
            "--gene".to_string(),
            "TP53".to_string(),
            "--type".to_string(),
            "foo".to_string(),
            "--target".to_string(),
            "ERBB2".to_string(),
        ])
        .await
        .expect_err("study compare should validate type");
        assert!(err.to_string().contains("Unknown comparison type"));
    }

    #[tokio::test]
    async fn study_co_occurrence_invalid_chart_lists_heatmap() {
        let err = execute(vec![
            "biomcp".to_string(),
            "study".to_string(),
            "co-occurrence".to_string(),
            "--study".to_string(),
            "msk_impact_2017".to_string(),
            "--genes".to_string(),
            "TP53,KRAS".to_string(),
            "--chart".to_string(),
            "violin".to_string(),
            "--terminal".to_string(),
        ])
        .await
        .expect_err("study co-occurrence should reject violin");
        let msg = err.to_string();
        assert!(msg.contains("study co-occurrence"));
        assert!(msg.contains("bar"));
        assert!(msg.contains("pie"));
        assert!(msg.contains("heatmap"));
    }

    #[tokio::test]
    async fn study_query_mutations_invalid_chart_lists_waterfall() {
        let err = execute(vec![
            "biomcp".to_string(),
            "study".to_string(),
            "query".to_string(),
            "--study".to_string(),
            "msk_impact_2017".to_string(),
            "--gene".to_string(),
            "TP53".to_string(),
            "--type".to_string(),
            "mutations".to_string(),
            "--chart".to_string(),
            "violin".to_string(),
            "--terminal".to_string(),
        ])
        .await
        .expect_err("study query mutations should reject violin");
        let msg = err.to_string();
        assert!(msg.contains("study query --type mutations"));
        assert!(msg.contains("bar"));
        assert!(msg.contains("pie"));
        assert!(msg.contains("waterfall"));
    }

    #[tokio::test]
    async fn study_compare_mutations_invalid_chart_lists_stacked_bar() {
        let err = execute(vec![
            "biomcp".to_string(),
            "study".to_string(),
            "compare".to_string(),
            "--study".to_string(),
            "msk_impact_2017".to_string(),
            "--gene".to_string(),
            "TP53".to_string(),
            "--type".to_string(),
            "mutations".to_string(),
            "--target".to_string(),
            "KRAS".to_string(),
            "--chart".to_string(),
            "violin".to_string(),
            "--terminal".to_string(),
        ])
        .await
        .expect_err("mutation compare should reject violin");
        let msg = err.to_string();
        assert!(msg.contains("study compare --type mutations"));
        assert!(msg.contains("bar"));
        assert!(msg.contains("stacked-bar"));
    }

    #[tokio::test]
    async fn study_compare_expression_invalid_chart_lists_scatter() {
        let err = execute(vec![
            "biomcp".to_string(),
            "study".to_string(),
            "compare".to_string(),
            "--study".to_string(),
            "msk_impact_2017".to_string(),
            "--gene".to_string(),
            "TP53".to_string(),
            "--type".to_string(),
            "expression".to_string(),
            "--target".to_string(),
            "ERBB2".to_string(),
            "--chart".to_string(),
            "pie".to_string(),
            "--terminal".to_string(),
        ])
        .await
        .expect_err("expression compare should reject pie");
        let msg = err.to_string();
        assert!(msg.contains("study compare --type expression"));
        assert!(msg.contains("box"));
        assert!(msg.contains("violin"));
        assert!(msg.contains("ridgeline"));
        assert!(msg.contains("scatter"));
    }

    #[tokio::test]
    async fn gene_alias_fallback_returns_exit_1_markdown_suggestion() {
        let _guard = lock_env().await;
        let mygene = MockServer::start().await;
        let ols = MockServer::start().await;
        let _mygene_base = set_env_var("BIOMCP_MYGENE_BASE", Some(&format!("{}/v3", mygene.uri())));
        let _ols_base = set_env_var("BIOMCP_OLS4_BASE", Some(&ols.uri()));
        let _umls_base = set_env_var("BIOMCP_UMLS_BASE", None);
        let _umls_key = set_env_var("UMLS_API_KEY", None);

        mount_gene_lookup_miss(&mygene, "ERBB1").await;
        mount_ols_alias(&ols, "ERBB1", "hgnc", "HGNC:3236", "EGFR", &["ERBB1"], 1).await;

        let cli = Cli::try_parse_from(["biomcp", "get", "gene", "ERBB1"]).expect("parse");
        let outcome = run_outcome(cli).await.expect("alias outcome");

        assert_eq!(outcome.stream, OutputStream::Stderr);
        assert_eq!(outcome.exit_code, 1);
        assert!(outcome.text.contains("Error: gene 'ERBB1' not found."));
        assert!(
            outcome
                .text
                .contains("Did you mean: `biomcp get gene EGFR`")
        );
    }

    #[tokio::test]
    async fn gene_alias_fallback_json_writes_stdout_and_exit_1() {
        let _guard = lock_env().await;
        let mygene = MockServer::start().await;
        let ols = MockServer::start().await;
        let _mygene_base = set_env_var("BIOMCP_MYGENE_BASE", Some(&format!("{}/v3", mygene.uri())));
        let _ols_base = set_env_var("BIOMCP_OLS4_BASE", Some(&ols.uri()));
        let _umls_base = set_env_var("BIOMCP_UMLS_BASE", None);
        let _umls_key = set_env_var("UMLS_API_KEY", None);

        mount_gene_lookup_miss(&mygene, "ERBB1").await;
        mount_ols_alias(&ols, "ERBB1", "hgnc", "HGNC:3236", "EGFR", &["ERBB1"], 1).await;

        let cli = Cli::try_parse_from(["biomcp", "--json", "get", "gene", "ERBB1"]).expect("parse");
        let outcome = run_outcome(cli).await.expect("alias json outcome");

        assert_eq!(outcome.stream, OutputStream::Stdout);
        assert_eq!(outcome.exit_code, 1);
        let value: serde_json::Value =
            serde_json::from_str(&outcome.text).expect("valid alias json");
        assert_eq!(
            value["_meta"]["alias_resolution"]["canonical"], "EGFR",
            "json={value}"
        );
        assert_eq!(value["_meta"]["next_commands"][0], "biomcp get gene EGFR");
    }

    #[tokio::test]
    async fn variant_get_shorthand_json_returns_variant_guidance_metadata() {
        let cli =
            Cli::try_parse_from(["biomcp", "--json", "get", "variant", "R620W"]).expect("parse");
        let outcome = run_outcome(cli).await.expect("variant guidance outcome");

        assert_eq!(outcome.stream, OutputStream::Stdout);
        assert_eq!(outcome.exit_code, 1);

        let value: serde_json::Value =
            serde_json::from_str(&outcome.text).expect("valid variant guidance json");
        assert_eq!(
            value["_meta"]["alias_resolution"]["requested_entity"],
            "variant"
        );
        assert_eq!(
            value["_meta"]["alias_resolution"]["kind"],
            "protein_change_only"
        );
        assert_eq!(value["_meta"]["alias_resolution"]["query"], "R620W");
        assert_eq!(value["_meta"]["alias_resolution"]["change"], "R620W");
        assert_eq!(
            value["_meta"]["next_commands"][0],
            "biomcp search variant --hgvsp R620W --limit 10"
        );
    }

    #[tokio::test]
    async fn variant_search_shorthand_json_returns_variant_guidance_metadata() {
        let cli =
            Cli::try_parse_from(["biomcp", "--json", "search", "variant", "R620W"]).expect("parse");
        let outcome = run_outcome(cli)
            .await
            .expect("variant search guidance outcome");

        assert_eq!(outcome.stream, OutputStream::Stdout);
        assert_eq!(outcome.exit_code, 1);

        let value: serde_json::Value =
            serde_json::from_str(&outcome.text).expect("valid variant guidance json");
        assert_eq!(
            value["_meta"]["alias_resolution"]["requested_entity"],
            "variant"
        );
        assert_eq!(
            value["_meta"]["alias_resolution"]["kind"],
            "protein_change_only"
        );
        assert_eq!(value["_meta"]["next_commands"][1], "biomcp discover R620W");
    }

    #[tokio::test]
    async fn canonical_gene_lookup_skips_discovery() {
        let _guard = lock_env().await;
        let mygene = MockServer::start().await;
        let ols = MockServer::start().await;
        let _mygene_base = set_env_var("BIOMCP_MYGENE_BASE", Some(&format!("{}/v3", mygene.uri())));
        let _ols_base = set_env_var("BIOMCP_OLS4_BASE", Some(&ols.uri()));
        let _umls_base = set_env_var("BIOMCP_UMLS_BASE", None);
        let _umls_key = set_env_var("UMLS_API_KEY", None);

        mount_gene_lookup_hit(&mygene, "TP53", "tumor protein p53", "7157").await;
        mount_ols_alias(&ols, "TP53", "hgnc", "HGNC:11998", "TP53", &["P53"], 0).await;

        let cli = Cli::try_parse_from(["biomcp", "get", "gene", "TP53"]).expect("parse");
        let outcome = run_outcome(cli).await.expect("success outcome");

        assert_eq!(outcome.stream, OutputStream::Stdout);
        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.text.contains("# TP53"));
    }

    #[test]
    fn batch_gene_json_includes_meta_per_item() {
        std::thread::Builder::new()
            .name("batch-gene-json-test".into())
            .stack_size(8 * 1024 * 1024)
            .spawn(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("runtime")
                    .block_on(async {
                        let _guard = lock_env().await;
                        let mygene = MockServer::start().await;
                        let _mygene_base = set_env_var(
                            "BIOMCP_MYGENE_BASE",
                            Some(&format!("{}/v3", mygene.uri())),
                        );

                        mount_gene_lookup_hit(&mygene, "BRAF", "B-Raf proto-oncogene", "673").await;
                        mount_gene_lookup_hit(&mygene, "TP53", "tumor protein p53", "7157").await;

                        let output = execute(vec![
                            "biomcp".to_string(),
                            "--json".to_string(),
                            "batch".to_string(),
                            "gene".to_string(),
                            "BRAF,TP53".to_string(),
                        ])
                        .await
                        .expect("batch outcome");
                        let value: serde_json::Value =
                            serde_json::from_str(&output).expect("valid batch json");
                        let items = value.as_array().expect("batch root should stay an array");
                        assert_eq!(items.len(), 2, "json={value}");
                        assert_eq!(items[0]["symbol"], "BRAF", "json={value}");
                        assert_eq!(items[1]["symbol"], "TP53", "json={value}");
                        assert!(
                            items.iter().all(|item| item["_meta"]["evidence_urls"]
                                .as_array()
                                .is_some_and(|urls| !urls.is_empty())),
                            "each batch item should include non-empty _meta.evidence_urls: {value}"
                        );
                        assert!(
                            items.iter().all(|item| item["_meta"]["next_commands"]
                                .as_array()
                                .is_some_and(|cmds| !cmds.is_empty())),
                            "each batch item should include non-empty _meta.next_commands: {value}"
                        );
                        assert!(
                            items.iter().any(|item| item["_meta"]["section_sources"]
                                .as_array()
                                .is_some_and(|sources| !sources.is_empty())),
                            "at least one batch item should include non-empty _meta.section_sources: {value}"
                        );
                    });
            })
            .expect("spawn")
            .join()
            .expect("thread should complete");
    }

    #[tokio::test]
    async fn ambiguous_gene_miss_points_to_discover() {
        let _guard = lock_env().await;
        let mygene = MockServer::start().await;
        let ols = MockServer::start().await;
        let _mygene_base = set_env_var("BIOMCP_MYGENE_BASE", Some(&format!("{}/v3", mygene.uri())));
        let _ols_base = set_env_var("BIOMCP_OLS4_BASE", Some(&ols.uri()));
        let _umls_base = set_env_var("BIOMCP_UMLS_BASE", None);
        let _umls_key = set_env_var("UMLS_API_KEY", None);

        mount_gene_lookup_miss(&mygene, "V600E").await;
        mount_ols_alias(&ols, "V600E", "so", "SO:0001583", "V600E", &["V600E"], 1).await;

        let cli = Cli::try_parse_from(["biomcp", "get", "gene", "V600E"]).expect("parse");
        let outcome = run_outcome(cli).await.expect("ambiguous outcome");

        assert_eq!(outcome.stream, OutputStream::Stderr);
        assert_eq!(outcome.exit_code, 1);
        assert!(
            outcome
                .text
                .contains("BioMCP could not map 'V600E' to a single gene.")
        );
        assert!(outcome.text.contains("1. biomcp discover V600E"));
        assert!(outcome.text.contains("2. biomcp search gene -q V600E"));
    }

    #[tokio::test]
    async fn alias_fallback_ols_failure_preserves_original_not_found() {
        let _guard = lock_env().await;
        let mygene = MockServer::start().await;
        let ols = MockServer::start().await;
        let _mygene_base = set_env_var("BIOMCP_MYGENE_BASE", Some(&format!("{}/v3", mygene.uri())));
        let _ols_base = set_env_var("BIOMCP_OLS4_BASE", Some(&ols.uri()));
        let _umls_base = set_env_var("BIOMCP_UMLS_BASE", None);
        let _umls_key = set_env_var("UMLS_API_KEY", None);

        mount_gene_lookup_miss(&mygene, "ERBB1").await;
        let ols_calls = Arc::new(AtomicUsize::new(0));
        let ols_calls_for_responder = Arc::clone(&ols_calls);
        Mock::given(method("GET"))
            .and(path("/api/search"))
            .and(query_param("q", "ERBB1"))
            .respond_with(move |_request: &wiremock::Request| {
                let call_index = ols_calls_for_responder.fetch_add(1, Ordering::SeqCst);
                if call_index == 0 {
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({
                        "response": {
                            "docs": [{
                                "iri": "http://example.org/hgnc/HGNC_3236",
                                "ontology_name": "hgnc",
                                "ontology_prefix": "hgnc",
                                "short_form": "hgnc:3236",
                                "obo_id": "HGNC:3236",
                                "label": "EGFR",
                                "description": [],
                                "exact_synonyms": ["ERBB1"],
                                "type": "class"
                            }]
                        }
                    }))
                } else {
                    ResponseTemplate::new(500).set_body_raw("upstream down", "text/plain")
                }
            })
            .expect(2u64..)
            .mount(&ols)
            .await;

        crate::entities::discover::resolve_query(
            "ERBB1",
            crate::entities::discover::DiscoverMode::Command,
        )
        .await
        .expect("warm cache with a successful discover lookup");

        let cli = Cli::try_parse_from(["biomcp", "get", "gene", "ERBB1"]).expect("parse");
        let err = run_outcome(cli)
            .await
            .expect_err("should preserve not found");
        let rendered = err.to_string();

        assert!(
            ols_calls.load(Ordering::SeqCst) >= 2,
            "alias fallback should re-query OLS after the cache warm-up"
        );
        assert!(rendered.contains("gene 'ERBB1' not found"));
        assert!(rendered.contains("Try searching: biomcp search gene -q ERBB1"));
    }

    #[tokio::test]
    async fn drug_alias_fallback_returns_exit_1_markdown_suggestion() {
        let _guard = lock_env().await;
        let mychem = MockServer::start().await;
        let ols = MockServer::start().await;
        let _mychem_base = set_env_var("BIOMCP_MYCHEM_BASE", Some(&format!("{}/v1", mychem.uri())));
        let _ols_base = set_env_var("BIOMCP_OLS4_BASE", Some(&ols.uri()));
        let _umls_base = set_env_var("BIOMCP_UMLS_BASE", None);
        let _umls_key = set_env_var("UMLS_API_KEY", None);

        mount_drug_lookup_miss(&mychem, "Keytruda").await;
        mount_ols_alias(
            &ols,
            "Keytruda",
            "mesh",
            "MESH:C582435",
            "pembrolizumab",
            &["Keytruda"],
            1,
        )
        .await;

        let cli = Cli::try_parse_from(["biomcp", "get", "drug", "Keytruda"]).expect("parse");
        let outcome = run_outcome(cli).await.expect("drug alias outcome");

        assert_eq!(outcome.stream, OutputStream::Stderr);
        assert_eq!(outcome.exit_code, 1);
        assert!(outcome.text.contains("Error: drug 'Keytruda' not found."));
        assert!(
            outcome
                .text
                .contains("Did you mean: `biomcp get drug pembrolizumab`")
        );
    }

    #[tokio::test]
    async fn drug_alias_fallback_json_writes_stdout_and_exit_1() {
        let _guard = lock_env().await;
        let mychem = MockServer::start().await;
        let ols = MockServer::start().await;
        let _mychem_base = set_env_var("BIOMCP_MYCHEM_BASE", Some(&format!("{}/v1", mychem.uri())));
        let _ols_base = set_env_var("BIOMCP_OLS4_BASE", Some(&ols.uri()));
        let _umls_base = set_env_var("BIOMCP_UMLS_BASE", None);
        let _umls_key = set_env_var("UMLS_API_KEY", None);

        mount_drug_lookup_miss(&mychem, "Keytruda").await;
        mount_ols_alias(
            &ols,
            "Keytruda",
            "mesh",
            "MESH:C582435",
            "pembrolizumab",
            &["Keytruda"],
            1,
        )
        .await;

        let cli =
            Cli::try_parse_from(["biomcp", "--json", "get", "drug", "Keytruda"]).expect("parse");
        let outcome = run_outcome(cli).await.expect("drug alias json outcome");

        assert_eq!(outcome.stream, OutputStream::Stdout);
        assert_eq!(outcome.exit_code, 1);
        let value: serde_json::Value =
            serde_json::from_str(&outcome.text).expect("valid alias json");
        assert_eq!(
            value["_meta"]["alias_resolution"]["canonical"],
            "pembrolizumab"
        );
        assert_eq!(
            value["_meta"]["next_commands"][0],
            "biomcp get drug pembrolizumab"
        );
    }

    #[tokio::test]
    async fn execute_mcp_alias_suggestion_returns_structured_json_text() {
        let _guard = lock_env().await;
        let mygene = MockServer::start().await;
        let ols = MockServer::start().await;
        let _mygene_base = set_env_var("BIOMCP_MYGENE_BASE", Some(&format!("{}/v3", mygene.uri())));
        let _ols_base = set_env_var("BIOMCP_OLS4_BASE", Some(&ols.uri()));
        let _umls_base = set_env_var("BIOMCP_UMLS_BASE", None);
        let _umls_key = set_env_var("UMLS_API_KEY", None);

        mount_gene_lookup_miss(&mygene, "ERBB1").await;
        mount_ols_alias(&ols, "ERBB1", "hgnc", "HGNC:3236", "EGFR", &["ERBB1"], 1).await;

        let output = execute_mcp(vec![
            "biomcp".to_string(),
            "get".to_string(),
            "gene".to_string(),
            "ERBB1".to_string(),
        ])
        .await
        .expect("mcp alias outcome");

        let value: serde_json::Value =
            serde_json::from_str(&output.text).expect("valid mcp alias json");
        assert_eq!(value["_meta"]["alias_resolution"]["kind"], "canonical");
        assert_eq!(value["_meta"]["alias_resolution"]["canonical"], "EGFR");
    }

    #[tokio::test]
    async fn json_cache_path_still_returns_plain_text() {
        let _guard = lock_env().await;
        let root = TempDirGuard::new("cache-path-json");
        let cache_home = root.path().join("cache-home");
        let config_home = root.path().join("config-home");
        std::fs::create_dir_all(&cache_home).expect("create cache home");
        std::fs::create_dir_all(&config_home).expect("create config home");
        let _cache_home = set_env_var("XDG_CACHE_HOME", Some(&cache_home.to_string_lossy()));
        let _config_home = set_env_var("XDG_CONFIG_HOME", Some(&config_home.to_string_lossy()));
        let _cache_dir = set_env_var("BIOMCP_CACHE_DIR", None);
        let _cache_size = set_env_var("BIOMCP_CACHE_MAX_SIZE", None);

        let output = execute(vec![
            "biomcp".to_string(),
            "--json".to_string(),
            "cache".to_string(),
            "path".to_string(),
        ])
        .await
        .expect("cache path should execute");

        assert_eq!(
            output.trim(),
            cache_home.join("biomcp").join("http").display().to_string()
        );
        assert!(!output.trim_start().starts_with('{'));
    }

    #[tokio::test]
    async fn cache_stats_execute_returns_markdown_table() {
        let _guard = lock_env().await;
        let root = TempDirGuard::new("cache-stats-text");
        let cache_home = root.path().join("cache-home");
        let config_home = root.path().join("config-home");
        std::fs::create_dir_all(&cache_home).expect("create cache home");
        std::fs::create_dir_all(&config_home).expect("create config home");
        let _cache_home = set_env_var("XDG_CACHE_HOME", Some(&cache_home.to_string_lossy()));
        let _config_home = set_env_var("XDG_CONFIG_HOME", Some(&config_home.to_string_lossy()));
        let _cache_dir = set_env_var("BIOMCP_CACHE_DIR", None);
        let _cache_size = set_env_var("BIOMCP_CACHE_MAX_SIZE", None);
        let _cache_age = set_env_var("BIOMCP_CACHE_MAX_AGE", None);

        let output = execute(vec![
            "biomcp".to_string(),
            "cache".to_string(),
            "stats".to_string(),
        ])
        .await
        .expect("cache stats should execute");

        for row in [
            "| Path |",
            "| Blob bytes |",
            "| Blob files |",
            "| Orphan blobs |",
            "| Age range |",
            "| Max size |",
            "| Max age |",
        ] {
            assert!(output.contains(row), "missing row {row}: {output}");
        }
        assert!(!output.trim_start().starts_with('{'));
    }

    #[tokio::test]
    async fn cache_stats_execute_json_returns_structured_report() {
        let _guard = lock_env().await;
        let root = TempDirGuard::new("cache-stats-json");
        let cache_home = root.path().join("cache-home");
        let config_home = root.path().join("config-home");
        std::fs::create_dir_all(&cache_home).expect("create cache home");
        std::fs::create_dir_all(&config_home).expect("create config home");
        let _cache_home = set_env_var("XDG_CACHE_HOME", Some(&cache_home.to_string_lossy()));
        let _config_home = set_env_var("XDG_CONFIG_HOME", Some(&config_home.to_string_lossy()));
        let _cache_dir = set_env_var("BIOMCP_CACHE_DIR", None);
        let _cache_size = set_env_var("BIOMCP_CACHE_MAX_SIZE", None);
        let _cache_age = set_env_var("BIOMCP_CACHE_MAX_AGE", None);

        let output = execute(vec![
            "biomcp".to_string(),
            "--json".to_string(),
            "cache".to_string(),
            "stats".to_string(),
        ])
        .await
        .expect("cache stats json should execute");

        let value: serde_json::Value =
            serde_json::from_str(&output).expect("cache stats json should be valid");
        for key in [
            "path",
            "blob_bytes",
            "blob_count",
            "orphan_count",
            "age_range",
            "max_size_bytes",
            "max_size_origin",
            "max_age_secs",
            "max_age_origin",
        ] {
            assert!(value.get(key).is_some(), "missing key {key}: {value}");
        }
        assert!(!output.contains("| Path |"));
        assert!(!output.contains("| Blob bytes |"));
    }

    #[tokio::test]
    async fn cache_clean_execute_returns_single_line_summary() {
        let _guard = lock_env().await;
        let root = TempDirGuard::new("cache-clean-text");
        let cache_home = root.path().join("cache-home");
        let config_home = root.path().join("config-home");
        std::fs::create_dir_all(&cache_home).expect("create cache home");
        std::fs::create_dir_all(&config_home).expect("create config home");
        let _cache_home = set_env_var("XDG_CACHE_HOME", Some(&cache_home.to_string_lossy()));
        let _config_home = set_env_var("XDG_CONFIG_HOME", Some(&config_home.to_string_lossy()));
        let _cache_dir = set_env_var("BIOMCP_CACHE_DIR", None);
        let _cache_size = set_env_var("BIOMCP_CACHE_MAX_SIZE", None);

        let output = execute(vec![
            "biomcp".to_string(),
            "cache".to_string(),
            "clean".to_string(),
        ])
        .await
        .expect("cache clean should execute");

        assert!(output.starts_with("Cache clean:"));
        assert!(output.contains("dry_run=false"));
        assert_eq!(output.lines().count(), 1);
    }

    #[tokio::test]
    async fn cache_clean_execute_json_returns_structured_report() {
        let _guard = lock_env().await;
        let root = TempDirGuard::new("cache-clean-json");
        let cache_home = root.path().join("cache-home");
        let config_home = root.path().join("config-home");
        std::fs::create_dir_all(&cache_home).expect("create cache home");
        std::fs::create_dir_all(&config_home).expect("create config home");
        let _cache_home = set_env_var("XDG_CACHE_HOME", Some(&cache_home.to_string_lossy()));
        let _config_home = set_env_var("XDG_CONFIG_HOME", Some(&config_home.to_string_lossy()));
        let _cache_dir = set_env_var("BIOMCP_CACHE_DIR", None);
        let _cache_size = set_env_var("BIOMCP_CACHE_MAX_SIZE", None);

        let output = execute(vec![
            "biomcp".to_string(),
            "--json".to_string(),
            "cache".to_string(),
            "clean".to_string(),
        ])
        .await
        .expect("cache clean json should execute");

        let value: serde_json::Value =
            serde_json::from_str(&output).expect("cache clean json should be valid");
        for key in [
            "dry_run",
            "orphans_removed",
            "entries_removed",
            "bytes_freed",
            "errors",
        ] {
            assert!(value.get(key).is_some(), "missing key {key}: {value}");
        }
    }
}

#[cfg(test)]
mod next_commands_validity {
    use super::Cli;
    use clap::Parser;

    fn parse_cmd(cmd: &str) -> Vec<String> {
        shlex::split(cmd).unwrap_or_else(|| panic!("shlex failed on: {cmd}"))
    }

    fn assert_parses(cmd: &str) {
        Cli::try_parse_from(parse_cmd(cmd))
            .unwrap_or_else(|e| panic!("failed to parse '{cmd}': {e}"));
    }

    #[test]
    fn gene_next_commands_parse() {
        assert_parses(r#"biomcp search trial -c "Dravet syndrome" -s recruiting"#);
        assert_parses("biomcp search pgx -g BRAF");
        assert_parses("biomcp search variant -g BRAF");
        assert_parses("biomcp search article -g BRAF");
        assert_parses("biomcp search drug --target BRAF");
        assert_parses("biomcp gene trials BRAF");
    }

    #[test]
    fn variant_next_commands_parse() {
        assert_parses("biomcp get gene BRAF");
        assert_parses(
            r#"biomcp search article -g SCN1A -d "Dravet syndrome" -k "T1174S" --limit 5"#,
        );
        assert_parses(r#"biomcp search article -g SCN1A -k "T1174S" --limit 5"#);
        assert_parses(r#"biomcp search article -d "Dravet syndrome" -k "T1174S" --limit 5"#);
        assert_parses(r#"biomcp search article -k "T1174S" --limit 5"#);
        assert_parses("biomcp search drug --target BRAF");
        assert_parses(r#"biomcp variant trials "rs113488022""#);
        assert_parses(r#"biomcp variant articles "rs113488022""#);
        assert_parses(r#"biomcp variant oncokb "rs113488022""#);
    }

    #[test]
    fn article_next_commands_parse() {
        assert_parses("biomcp search gene -q EGFR");
        assert_parses(r#"biomcp search gene -q "serine-threonine protein kinase""#);
        assert_parses("biomcp search disease --query melanoma");
        assert_parses("biomcp get drug osimertinib");
        assert_parses("biomcp article entities 12345");
        assert_parses("biomcp article citations 12345 --limit 3");
        assert_parses("biomcp article references 12345 --limit 3");
        assert_parses("biomcp article recommendations 12345 67890 --negative 11111 --limit 3");
    }

    #[test]
    fn trial_next_commands_parse() {
        assert_parses(
            r#"biomcp search article --drug dabrafenib -q "NCT01234567 Example trial" --limit 5"#,
        );
        assert_parses(r#"biomcp search article -q "NCT01234567 Example trial" --limit 5"#);
        assert_parses("biomcp search disease --query melanoma");
        assert_parses("biomcp search article -d melanoma");
        assert_parses("biomcp search trial -c melanoma");
        assert_parses("biomcp get drug dabrafenib");
        assert_parses("biomcp drug trials dabrafenib");
    }

    #[test]
    fn disease_next_commands_parse() {
        assert_parses("biomcp get gene SCN1A clingen constraint");
        assert_parses(r#"biomcp get disease "Dravet syndrome" genes phenotypes"#);
        assert_parses("biomcp search trial -c melanoma");
        assert_parses("biomcp search article -d melanoma");
        assert_parses(r#"biomcp search drug --indication "melanoma""#);
    }

    #[test]
    fn pgx_next_commands_parse() {
        assert_parses("biomcp search pgx -g CYP2D6");
        assert_parses("biomcp search pgx -d warfarin");
    }

    #[test]
    fn drug_next_commands_parse() {
        assert_parses("biomcp drug trials osimertinib");
        assert_parses("biomcp drug adverse-events osimertinib");
        assert_parses("biomcp get gene EGFR");
    }

    #[test]
    fn pathway_next_commands_parse() {
        assert_parses("biomcp pathway drugs R-HSA-5673001");
    }

    #[test]
    fn protein_next_commands_parse() {
        assert_parses("biomcp get protein P00533 structures");
        assert_parses("biomcp get protein P00533 complexes");
        assert_parses("biomcp get gene EGFR");
    }

    #[test]
    fn adverse_event_next_commands_parse() {
        assert_parses("biomcp get drug osimertinib");
        assert_parses("biomcp drug adverse-events osimertinib");
        assert_parses("biomcp drug trials osimertinib");
    }

    #[test]
    fn device_event_next_commands_parse() {
        assert_parses("biomcp search adverse-event --type device --device HeartValve");
        assert_parses(r#"biomcp search adverse-event --type recall --classification "Class I""#);
    }

    #[test]
    fn discover_next_commands_parse() {
        // gene — unambiguous and ambiguous
        assert_parses("biomcp get gene EGFR");
        assert_parses(r#"biomcp search gene -q "ERBB1" --limit 10"#);
        // drug
        assert_parses(r#"biomcp get drug "pembrolizumab""#);
        assert_parses(r#"biomcp drug adverse-events pembrolizumab"#);
        assert_parses(r#"biomcp get drug pembrolizumab safety"#);
        assert_parses(r#"biomcp search drug --indication "Myasthenia gravis" --limit 5"#);
        // disease — unambiguous helpers and ambiguous fallback
        assert_parses(r#"biomcp get disease "cystic fibrosis""#);
        assert_parses(r#"biomcp disease trials "cystic fibrosis""#);
        assert_parses(r#"biomcp search article -k "cystic fibrosis" --limit 5"#);
        assert_parses(r#"biomcp search disease -q "diabetes" --limit 10"#);
        assert_parses(r#"biomcp get disease MONDO:0007947 phenotypes"#);
        // symptom
        assert_parses(r#"biomcp search disease -q "chest pain" --limit 10"#);
        assert_parses(r#"biomcp search trial -c "chest pain" --limit 5"#);
        assert_parses(r#"biomcp search article -k "chest pain" --limit 5"#);
        // pathway
        assert_parses(r#"biomcp search pathway -q "MAPK signaling" --limit 5"#);
        // gene+disease orientation
        assert_parses(r#"biomcp search all --gene BRAF --disease "melanoma""#);
        // variant with and without gene inference
        assert_parses(r#"biomcp get variant "BRAF V600E""#);
        assert_parses(r#"biomcp search article -k "V600E" --limit 5"#);
        // trial intent
        assert_parses(r#"biomcp search trial -c "Breast Cancer" --limit 5"#);
        assert_parses(r#"biomcp search article -k "Breast Cancer" --limit 5"#);
    }
}

#[cfg(test)]
mod next_commands_json_property {
    use super::Cli;
    use clap::Parser;
    use serde::Serialize;

    use crate::entities::adverse_event::{AdverseEvent, AdverseEventReport, DeviceEvent};
    use crate::entities::article::{AnnotationCount, Article, ArticleAnnotations};
    use crate::entities::disease::Disease;
    use crate::entities::drug::Drug;
    use crate::entities::gene::Gene;
    use crate::entities::pathway::Pathway;
    use crate::entities::pgx::Pgx;
    use crate::entities::protein::Protein;
    use crate::entities::trial::Trial;
    use crate::entities::variant::Variant;

    fn collect_next_commands(json: &str) -> Vec<String> {
        let value: serde_json::Value = serde_json::from_str(json).expect("valid json");
        value["_meta"]["next_commands"]
            .as_array()
            .expect("next_commands array")
            .iter()
            .map(|cmd| cmd.as_str().expect("command string").to_string())
            .collect()
    }

    fn assert_json_next_commands_parse(label: &str, json: &str) {
        let value: serde_json::Value =
            serde_json::from_str(json).unwrap_or_else(|e| panic!("{label}: invalid json: {e}"));
        let cmds = value["_meta"]["next_commands"]
            .as_array()
            .unwrap_or_else(|| panic!("{label}: missing _meta.next_commands"));
        assert!(
            !cmds.is_empty(),
            "{label}: expected at least one next_command"
        );
        for cmd in cmds {
            let cmd = cmd
                .as_str()
                .unwrap_or_else(|| panic!("{label}: next_command was not a string"));
            let argv =
                shlex::split(cmd).unwrap_or_else(|| panic!("{label}: shlex failed on: {cmd}"));
            Cli::try_parse_from(argv)
                .unwrap_or_else(|e| panic!("{label}: failed to parse '{cmd}': {e}"));
        }
    }

    fn assert_entity_json_next_commands<T: Serialize>(
        label: &str,
        entity: &T,
        evidence_urls: Vec<(&'static str, String)>,
        next_commands: Vec<String>,
        section_sources: Vec<crate::render::provenance::SectionSource>,
    ) {
        let json = crate::render::json::to_entity_json(
            entity,
            evidence_urls,
            next_commands,
            section_sources,
        )
        .unwrap_or_else(|e| panic!("{label}: failed to render entity json: {e}"));
        assert_json_next_commands_parse(label, &json);
    }

    #[test]
    fn gene_json_next_commands_parse() {
        let gene = Gene {
            symbol: "BRAF".to_string(),
            name: "B-Raf proto-oncogene".to_string(),
            entrez_id: "673".to_string(),
            ensembl_id: Some("ENSG00000157764".to_string()),
            location: Some("7q34".to_string()),
            genomic_coordinates: None,
            omim_id: Some("164757".to_string()),
            uniprot_id: Some("P15056".to_string()),
            summary: None,
            gene_type: None,
            aliases: Vec::new(),
            clinical_diseases: Vec::new(),
            clinical_drugs: Vec::new(),
            pathways: None,
            ontology: None,
            diseases: None,
            protein: None,
            go: None,
            interactions: None,
            civic: None,
            expression: None,
            hpa: None,
            druggability: None,
            clingen: None,
            constraint: None,
            disgenet: None,
            funding: None,
            funding_note: None,
        };

        assert_entity_json_next_commands(
            "gene",
            &gene,
            crate::render::markdown::gene_evidence_urls(&gene),
            crate::render::markdown::related_gene(&gene),
            crate::render::provenance::gene_section_sources(&gene),
        );
    }

    #[test]
    fn gene_json_next_commands_include_clingen_trial_search() {
        let gene = Gene {
            symbol: "SCN1A".to_string(),
            name: "sodium voltage-gated channel alpha subunit 1".to_string(),
            entrez_id: "6323".to_string(),
            ensembl_id: Some("ENSG00000144285".to_string()),
            location: Some("2q24.3".to_string()),
            genomic_coordinates: None,
            omim_id: Some("182389".to_string()),
            uniprot_id: Some("P35498".to_string()),
            summary: None,
            gene_type: None,
            aliases: Vec::new(),
            clinical_diseases: Vec::new(),
            clinical_drugs: Vec::new(),
            pathways: None,
            ontology: None,
            diseases: None,
            protein: None,
            go: None,
            interactions: None,
            civic: None,
            expression: None,
            hpa: None,
            druggability: None,
            clingen: Some(crate::sources::clingen::GeneClinGen {
                validity: vec![crate::sources::clingen::ClinGenValidity {
                    disease: "genetic developmental and epileptic encephalopathy".to_string(),
                    classification: "Definitive".to_string(),
                    review_date: Some("2025-12-16".to_string()),
                    moi: Some("AD".to_string()),
                }],
                haploinsufficiency: None,
                triplosensitivity: None,
            }),
            constraint: None,
            disgenet: None,
            funding: None,
            funding_note: None,
        };

        let next_commands = crate::render::markdown::related_gene(&gene);
        let json = crate::render::json::to_entity_json(
            &gene,
            crate::render::markdown::gene_evidence_urls(&gene),
            next_commands,
            crate::render::provenance::gene_section_sources(&gene),
        )
        .expect("gene json");
        assert_json_next_commands_parse("gene-clingen", &json);
        assert!(collect_next_commands(&json).contains(
            &"biomcp search trial -c \"genetic developmental and epileptic encephalopathy\" -s recruiting"
                .to_string()
        ));
    }

    #[test]
    fn batch_protein_json_omits_requested_section_from_next_commands() {
        let protein = Protein {
            accession: "P00533".to_string(),
            entry_id: Some("EGFR_HUMAN".to_string()),
            name: "Epidermal growth factor receptor".to_string(),
            gene_symbol: Some("EGFR".to_string()),
            organism: None,
            length: None,
            function: None,
            structures: Vec::new(),
            structure_count: None,
            domains: Vec::new(),
            interactions: Vec::new(),
            complexes: Vec::new(),
        };
        let requested_sections = ["complexes".to_string()];
        let json = super::render_batch_json(std::slice::from_ref(&protein), |item| {
            crate::render::json::to_entity_json_value(
                item,
                crate::render::markdown::protein_evidence_urls(item),
                crate::render::markdown::related_protein(item, &requested_sections),
                crate::render::provenance::protein_section_sources(item),
            )
        })
        .expect("batch json");

        let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        let commands = value[0]["_meta"]["next_commands"]
            .as_array()
            .expect("next_commands array")
            .iter()
            .map(|cmd| cmd.as_str().expect("command string"))
            .collect::<Vec<_>>();

        assert!(
            !commands.contains(&"biomcp get protein P00533 complexes"),
            "requested section should not be suggested again: {value}"
        );
        assert!(
            commands.contains(&"biomcp get protein P00533 structures"),
            "expected structures follow-up: {value}"
        );
        assert!(
            commands.contains(&"biomcp get gene EGFR"),
            "expected linked gene follow-up: {value}"
        );
    }

    #[test]
    fn article_json_next_commands_parse() {
        let article = Article {
            pmid: Some("22663011".to_string()),
            pmcid: Some("PMC9984800".to_string()),
            doi: Some("10.1056/NEJMoa1203421".to_string()),
            title: "Example about melanoma".to_string(),
            authors: Vec::new(),
            journal: None,
            date: None,
            citation_count: None,
            publication_type: None,
            open_access: None,
            abstract_text: None,
            full_text_path: None,
            full_text_note: None,
            annotations: Some(ArticleAnnotations {
                genes: vec![AnnotationCount {
                    text: "serine-threonine protein kinase".to_string(),
                    count: 1,
                }],
                diseases: vec![AnnotationCount {
                    text: "melanoma".to_string(),
                    count: 1,
                }],
                chemicals: vec![AnnotationCount {
                    text: "osimertinib".to_string(),
                    count: 1,
                }],
                mutations: Vec::new(),
            }),
            semantic_scholar: None,
            pubtator_fallback: false,
        };
        let next_commands = crate::render::markdown::related_article(&article);
        assert!(
            next_commands
                .iter()
                .any(|cmd| { cmd == "biomcp search gene -q \"serine-threonine protein kinase\"" })
        );
        assert!(
            !next_commands
                .iter()
                .any(|cmd| cmd == "biomcp get gene serine-threonine protein kinase")
        );

        assert_entity_json_next_commands(
            "article",
            &article,
            crate::render::markdown::article_evidence_urls(&article),
            next_commands,
            crate::render::provenance::article_section_sources(&article),
        );
    }

    #[test]
    fn disease_json_next_commands_parse() {
        let disease = Disease {
            id: "MONDO:0004992".to_string(),
            name: "melanoma".to_string(),
            definition: None,
            synonyms: Vec::new(),
            parents: Vec::new(),
            associated_genes: Vec::new(),
            gene_associations: Vec::new(),
            top_genes: Vec::new(),
            top_gene_scores: Vec::new(),
            treatment_landscape: Vec::new(),
            recruiting_trial_count: None,
            pathways: Vec::new(),
            phenotypes: Vec::new(),
            key_features: Vec::new(),
            variants: Vec::new(),
            top_variant: None,
            models: Vec::new(),
            prevalence: Vec::new(),
            prevalence_note: None,
            survival: None,
            survival_note: None,
            civic: None,
            disgenet: None,
            funding: None,
            funding_note: None,
            xrefs: std::collections::HashMap::new(),
        };

        assert_entity_json_next_commands(
            "disease",
            &disease,
            crate::render::markdown::disease_evidence_urls(&disease),
            crate::render::markdown::related_disease(&disease),
            crate::render::provenance::disease_section_sources(&disease),
        );
    }

    #[test]
    fn disease_json_next_commands_include_top_gene_context() {
        let disease = Disease {
            id: "MONDO:0100135".to_string(),
            name: "Dravet syndrome".to_string(),
            definition: None,
            synonyms: Vec::new(),
            parents: Vec::new(),
            associated_genes: vec!["SCN1A".to_string()],
            gene_associations: Vec::new(),
            top_genes: vec!["SCN1A".to_string()],
            top_gene_scores: vec![crate::entities::disease::DiseaseTargetScore {
                symbol: "SCN1A".to_string(),
                summary: crate::entities::disease::DiseaseAssociationScoreSummary {
                    overall_score: 0.872,
                    gwas_score: None,
                    rare_variant_score: Some(0.997),
                    somatic_mutation_score: None,
                },
            }],
            treatment_landscape: Vec::new(),
            recruiting_trial_count: None,
            pathways: Vec::new(),
            phenotypes: Vec::new(),
            key_features: Vec::new(),
            variants: Vec::new(),
            top_variant: None,
            models: Vec::new(),
            prevalence: Vec::new(),
            prevalence_note: None,
            survival: None,
            survival_note: None,
            civic: None,
            disgenet: None,
            funding: None,
            funding_note: None,
            xrefs: std::collections::HashMap::new(),
        };

        let next_commands = crate::render::markdown::related_disease(&disease);
        let json = crate::render::json::to_entity_json(
            &disease,
            crate::render::markdown::disease_evidence_urls(&disease),
            next_commands,
            crate::render::provenance::disease_section_sources(&disease),
        )
        .expect("disease json");
        assert_json_next_commands_parse("disease-top-gene", &json);
        assert!(
            collect_next_commands(&json)
                .contains(&"biomcp get gene SCN1A clingen constraint".to_string())
        );
    }

    #[test]
    fn pgx_json_next_commands_parse() {
        let pgx = Pgx {
            query: "CYP2D6".to_string(),
            gene: Some("CYP2D6".to_string()),
            drug: Some("warfarin sodium".to_string()),
            interactions: Vec::new(),
            recommendations: Vec::new(),
            frequencies: Vec::new(),
            guidelines: Vec::new(),
            annotations: Vec::new(),
            annotations_note: None,
        };

        assert_entity_json_next_commands(
            "pgx",
            &pgx,
            crate::render::markdown::pgx_evidence_urls(&pgx),
            crate::render::markdown::related_pgx(&pgx),
            crate::render::provenance::pgx_section_sources(&pgx),
        );
    }

    #[test]
    fn trial_json_next_commands_parse() {
        let trial = Trial {
            nct_id: "NCT01234567".to_string(),
            source: None,
            title: "Example trial".to_string(),
            status: "Completed".to_string(),
            phase: None,
            study_type: None,
            age_range: None,
            conditions: vec!["melanoma".to_string()],
            interventions: vec!["dabrafenib".to_string()],
            sponsor: None,
            enrollment: None,
            summary: None,
            start_date: None,
            completion_date: None,
            eligibility_text: None,
            locations: None,
            outcomes: None,
            arms: None,
            references: None,
        };
        let next_commands = crate::render::markdown::related_trial(&trial);
        assert!(next_commands.iter().any(|cmd| {
            cmd == "biomcp search article --drug dabrafenib -q \"NCT01234567 Example trial\" --limit 5"
        }));

        assert_entity_json_next_commands(
            "trial",
            &trial,
            crate::render::markdown::trial_evidence_urls(&trial),
            next_commands,
            crate::render::provenance::trial_section_sources(&trial),
        );
    }

    #[test]
    fn variant_json_next_commands_parse() {
        let variant: Variant = serde_json::from_value(serde_json::json!({
            "id": "rs113488022",
            "gene": "BRAF",
            "hgvs_p": "p.V600E",
            "rsid": "rs113488022"
        }))
        .expect("variant should deserialize");

        assert_entity_json_next_commands(
            "variant",
            &variant,
            crate::render::markdown::variant_evidence_urls(&variant),
            crate::render::markdown::related_variant(&variant),
            crate::render::provenance::variant_section_sources(&variant),
        );
    }

    #[test]
    fn variant_json_next_commands_include_vus_literature_route() {
        let variant: Variant = serde_json::from_value(serde_json::json!({
            "id": "chr2:g.166848047C>G",
            "gene": "SCN1A",
            "hgvs_p": "p.T1174S",
            "legacy_name": "SCN1A T1174S",
            "significance": "Uncertain significance",
            "top_disease": {"condition": "Dravet syndrome", "reports": 7}
        }))
        .expect("variant should deserialize");

        let next_commands = crate::render::markdown::related_variant(&variant);
        let json = crate::render::json::to_entity_json(
            &variant,
            crate::render::markdown::variant_evidence_urls(&variant),
            next_commands,
            crate::render::provenance::variant_section_sources(&variant),
        )
        .expect("variant json");
        assert_json_next_commands_parse("variant-vus", &json);
        assert!(
            collect_next_commands(&json).contains(
                &"biomcp search article -g SCN1A -d \"Dravet syndrome\" -k \"T1174S\" --limit 5"
                    .to_string()
            )
        );
    }

    #[test]
    fn drug_json_next_commands_parse() {
        let drug = Drug {
            name: "osimertinib".to_string(),
            drugbank_id: Some("DB09330".to_string()),
            chembl_id: Some("CHEMBL3353410".to_string()),
            unii: None,
            drug_type: None,
            mechanism: None,
            mechanisms: Vec::new(),
            approval_date: None,
            approval_date_raw: None,
            approval_date_display: None,
            approval_summary: None,
            brand_names: Vec::new(),
            route: None,
            targets: vec!["EGFR".to_string()],
            variant_targets: Vec::new(),
            target_family: None,
            target_family_name: None,
            indications: Vec::new(),
            interactions: Vec::new(),
            interaction_text: None,
            pharm_classes: Vec::new(),
            top_adverse_events: Vec::new(),
            faers_query: None,
            label: None,
            label_set_id: None,
            shortage: None,
            approvals: None,
            us_safety_warnings: None,
            ema_regulatory: None,
            ema_safety: None,
            ema_shortage: None,
            who_prequalification: None,
            civic: None,
        };

        assert_entity_json_next_commands(
            "drug",
            &drug,
            crate::render::markdown::drug_evidence_urls(&drug),
            crate::render::markdown::related_drug(&drug),
            crate::render::provenance::drug_section_sources(&drug),
        );
    }

    #[test]
    fn pathway_json_next_commands_parse() {
        let pathway = Pathway {
            source: "KEGG".to_string(),
            id: "hsa05200".to_string(),
            name: "Pathways in cancer".to_string(),
            species: None,
            summary: None,
            genes: Vec::new(),
            events: Vec::new(),
            enrichment: Vec::new(),
        };

        let next_commands = crate::render::markdown::related_pathway(&pathway);
        assert_eq!(
            next_commands,
            vec!["biomcp pathway drugs hsa05200".to_string()]
        );
        assert!(
            next_commands
                .iter()
                .all(|cmd| !cmd.contains("get pathway hsa05200")),
            "pathway next_commands should not repeat the current flow"
        );
        assert!(
            next_commands
                .iter()
                .all(|cmd| !cmd.contains("events") && !cmd.contains("enrichment")),
            "pathway next_commands should not suggest unsupported sections"
        );

        assert_entity_json_next_commands(
            "pathway",
            &pathway,
            crate::render::markdown::pathway_evidence_urls(&pathway),
            next_commands,
            crate::render::provenance::pathway_section_sources(&pathway),
        );
    }

    #[test]
    fn protein_json_next_commands_parse() {
        let protein = Protein {
            accession: "P00533".to_string(),
            entry_id: Some("EGFR_HUMAN".to_string()),
            name: "Epidermal growth factor receptor".to_string(),
            gene_symbol: Some("EGFR".to_string()),
            organism: None,
            length: None,
            function: None,
            structures: Vec::new(),
            structure_count: None,
            domains: Vec::new(),
            interactions: Vec::new(),
            complexes: Vec::new(),
        };

        let base_next_commands = crate::render::markdown::related_protein(&protein, &[]);
        assert!(base_next_commands.contains(&"biomcp get protein P00533 structures".to_string()));
        assert!(base_next_commands.contains(&"biomcp get protein P00533 complexes".to_string()));

        let section_next_commands =
            crate::render::markdown::related_protein(&protein, &["complexes".to_string()]);
        assert!(
            !section_next_commands.contains(&"biomcp get protein P00533 complexes".to_string())
        );
        assert!(
            section_next_commands.contains(&"biomcp get protein P00533 structures".to_string())
        );
        assert!(section_next_commands.contains(&"biomcp get gene EGFR".to_string()));

        assert_entity_json_next_commands(
            "protein",
            &protein,
            crate::render::markdown::protein_evidence_urls(&protein),
            section_next_commands,
            crate::render::provenance::protein_section_sources(&protein),
        );
    }

    #[test]
    fn batch_adverse_event_json_uses_variant_specific_meta() {
        let faers = AdverseEvent {
            report_id: "1001".to_string(),
            drug: "osimertinib".to_string(),
            reactions: Vec::new(),
            outcomes: Vec::new(),
            patient: None,
            concomitant_medications: Vec::new(),
            reporter_type: None,
            reporter_country: None,
            indication: None,
            serious: true,
            date: None,
        };
        let device = DeviceEvent {
            report_id: "MDR-123".to_string(),
            report_number: None,
            device: "HeartValve".to_string(),
            manufacturer: None,
            event_type: None,
            date: None,
            description: None,
        };
        let reports = vec![
            AdverseEventReport::Faers(faers),
            AdverseEventReport::Device(device),
        ];

        let json = super::render_batch_json(&reports, |item| match item {
            AdverseEventReport::Faers(report) => crate::render::json::to_entity_json_value(
                item,
                crate::render::markdown::adverse_event_evidence_urls(report),
                crate::render::markdown::related_adverse_event(report),
                crate::render::provenance::adverse_event_report_section_sources(item),
            ),
            AdverseEventReport::Device(report) => crate::render::json::to_entity_json_value(
                item,
                crate::render::markdown::device_event_evidence_urls(report),
                crate::render::markdown::related_device_event(report),
                crate::render::provenance::adverse_event_report_section_sources(item),
            ),
        })
        .expect("batch json");

        let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        let items = value.as_array().expect("batch array");
        assert_eq!(items.len(), 2, "json={value}");
        assert_eq!(items[0]["_meta"]["evidence_urls"][0]["label"], "OpenFDA");
        assert_eq!(items[1]["_meta"]["evidence_urls"][0]["label"], "OpenFDA");
        assert!(
            items[0]["_meta"]["evidence_urls"][0]["url"]
                .as_str()
                .is_some_and(|url| url.contains("/drug/event.json")),
            "faers report should use drug event evidence url: {value}"
        );
        assert!(
            items[1]["_meta"]["evidence_urls"][0]["url"]
                .as_str()
                .is_some_and(|url| url.contains("/device/event.json")),
            "device report should use device event evidence url: {value}"
        );
        assert!(
            items.iter().all(|item| item["_meta"]["next_commands"]
                .as_array()
                .is_some_and(|cmds| !cmds.is_empty())),
            "each report should retain next commands: {value}"
        );
    }

    #[test]
    fn faers_json_next_commands_parse() {
        let faers = AdverseEvent {
            report_id: "1001".to_string(),
            drug: "osimertinib".to_string(),
            reactions: Vec::new(),
            outcomes: Vec::new(),
            patient: None,
            concomitant_medications: Vec::new(),
            reporter_type: None,
            reporter_country: None,
            indication: None,
            serious: true,
            date: None,
        };
        let report = AdverseEventReport::Faers(faers.clone());

        assert_entity_json_next_commands(
            "adverse-event-faers",
            &report,
            crate::render::markdown::adverse_event_evidence_urls(&faers),
            crate::render::markdown::related_adverse_event(&faers),
            crate::render::provenance::adverse_event_report_section_sources(&report),
        );
    }

    #[test]
    fn device_event_json_next_commands_parse() {
        let device = DeviceEvent {
            report_id: "MDR-123".to_string(),
            report_number: None,
            device: "HeartValve".to_string(),
            manufacturer: None,
            event_type: None,
            date: None,
            description: None,
        };
        let report = AdverseEventReport::Device(device.clone());

        assert_entity_json_next_commands(
            "adverse-event-device",
            &report,
            crate::render::markdown::device_event_evidence_urls(&device),
            crate::render::markdown::related_device_event(&device),
            crate::render::provenance::adverse_event_report_section_sources(&report),
        );
    }
}
