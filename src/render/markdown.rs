use std::collections::HashSet;
use std::fmt::Write as _;
use std::sync::OnceLock;

use minijinja::{Environment, context};

use crate::cli::debug_plan::DebugPlan;
use crate::cli::search_all::SearchAllResults;
use crate::entities::adverse_event::{
    AdverseEvent, AdverseEventCountBucket, AdverseEventSearchResult, AdverseEventSearchSummary,
    DeviceEvent, DeviceEventSearchResult, RecallSearchResult,
};
use crate::entities::article::{
    AnnotationCount, Article, ArticleAnnotations, ArticleBatchEntitySummary, ArticleBatchItem,
    ArticleGraphResult, ArticleRankingMetadata, ArticleRankingMode, ArticleRecommendationsResult,
    ArticleRelatedPaper, ArticleSearchFilters, ArticleSearchResult, ArticleSort, ArticleSource,
};
use crate::entities::discover::{DiscoverResult, DiscoverType};
use crate::entities::disease::{
    Disease, DiseaseAssociationScoreSummary, DiseaseSearchResult, PhenotypeSearchResult,
};
use crate::entities::drug::{
    Drug, DrugApproval, DrugRegion, DrugSearchResult, EmaDrugSearchResult, EmaRegulatoryRow,
    EmaSafetyInfo, EmaShortageEntry, WhoPrequalificationEntry, WhoPrequalificationSearchResult,
};
use crate::entities::gene::{Gene, GeneSearchResult};
use crate::entities::pathway::{Pathway, PathwaySearchResult};
use crate::entities::pgx::{Pgx, PgxSearchResult};
use crate::entities::protein::{
    Protein, ProteinComplex, ProteinComplexComponent, ProteinComplexCuration, ProteinSearchResult,
};
use crate::entities::study::{
    CoOccurrenceResult as StudyCoOccurrenceResult, CohortResult as StudyCohortResult,
    ExpressionComparisonResult as StudyExpressionComparisonResult,
    FilterResult as StudyFilterResult, MutationComparisonResult as StudyMutationComparisonResult,
    SampleUniverseBasis as StudySampleUniverseBasis, StudyDownloadCatalog, StudyDownloadResult,
    StudyInfo, StudyQueryResult, SurvivalResult as StudySurvivalResult,
    TopMutatedGenesResult as StudyTopMutatedGenesResult,
};
use crate::entities::trial::{Trial, TrialSearchResult};
use crate::entities::variant::{
    Variant, VariantGwasAssociation, VariantOncoKbResult, VariantPrediction, VariantSearchResult,
    gnomad_variant_slug,
};
use crate::error::BioMcpError;
use crate::sources::nih_reporter::{NihReporterFundingSection, NihReporterGrant};

static ENV: OnceLock<Environment<'static>> = OnceLock::new();

#[derive(serde::Serialize)]
struct XrefRow {
    source: String,
    id: String,
}

#[derive(serde::Serialize)]
struct ArticleSearchRenderRow {
    pmid: String,
    title: String,
    sources: String,
    date: Option<String>,
    why: String,
    citation_count: Option<u64>,
    is_retracted: Option<bool>,
}

#[derive(serde::Serialize)]
struct ProteinComplexSummaryRow {
    accession: String,
    name: String,
    component_count: usize,
    curation: String,
}

#[derive(serde::Serialize)]
struct ProteinComplexDetailRow {
    accession: String,
    component_count: usize,
    component_preview: String,
    remaining_count: usize,
    description: Option<String>,
}

#[derive(serde::Serialize)]
struct DiseaseGeneAssociationRenderRow {
    gene: String,
    relationship: Option<String>,
    source: Option<String>,
    source_url: Option<String>,
    opentargets: Option<String>,
}

#[derive(serde::Serialize)]
struct DiseasePhenotypeRenderRow {
    hpo_id: String,
    name: Option<String>,
    evidence: Option<String>,
    frequency: Option<String>,
    frequency_qualifier: Option<String>,
    onset_qualifier: Option<String>,
    sex_qualifier: Option<String>,
    stage_qualifier: Option<String>,
    qualifiers: Vec<String>,
    source: Option<String>,
    source_url: Option<String>,
}

#[derive(serde::Serialize)]
struct DiseaseModelAssociationRenderRow {
    model: String,
    organism: Option<String>,
    relationship: Option<String>,
    source: Option<String>,
    source_url: Option<String>,
    evidence_count: Option<u32>,
}

#[derive(serde::Serialize)]
struct DiseaseSurvivalSummaryRenderRow {
    sex: String,
    latest_observed_year: Option<u16>,
    relative_survival: Option<String>,
    ci_95: Option<String>,
    cases: Option<u32>,
    latest_modeled: Option<String>,
}

#[derive(serde::Serialize)]
struct DiseaseSurvivalHistoryRenderRow {
    sex: String,
    year: u16,
    relative_survival: String,
    ci_95: Option<String>,
    cases: Option<u32>,
}

#[derive(serde::Serialize)]
struct FundingGrantRenderRow {
    project_title: String,
    pi_name: String,
    organization: String,
    fiscal_year: String,
    award_amount: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaginationFooterMode {
    Offset,
    Cursor,
}

fn offset_pagination_footer(
    offset: usize,
    limit: usize,
    returned: usize,
    total: Option<usize>,
) -> String {
    let next_offset = offset.saturating_add(returned.max(limit.max(1)));
    if let Some(total) = total {
        if returned == 0 {
            return format!("Showing 0 of {total} results.");
        }
        let start = offset.saturating_add(1);
        let end = offset.saturating_add(returned);
        if end < total {
            format!(
                "Showing {start}-{end} of {total} results. Use --offset {next_offset} for more."
            )
        } else if start == end {
            format!("Showing {end} of {total} results.")
        } else {
            format!("Showing {start}-{end} of {total} results.")
        }
    } else {
        format!("Showing {returned} results (total unknown). Use --offset {next_offset} for more.")
    }
}

pub fn pagination_footer(
    mode: PaginationFooterMode,
    offset: usize,
    limit: usize,
    returned: usize,
    total: Option<usize>,
    next_page_token: Option<&str>,
) -> String {
    match mode {
        PaginationFooterMode::Offset => offset_pagination_footer(offset, limit, returned, total),
        PaginationFooterMode::Cursor => {
            let mut footer = offset_pagination_footer(offset, limit, returned, total);
            let has_token = next_page_token
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_some();
            if has_token && footer.contains("Use --offset") {
                footer.push_str(" (--next-page is also supported.)");
            }
            footer
        }
    }
}

fn with_pagination_footer(mut body: String, pagination_footer: &str) -> String {
    let footer = pagination_footer.trim();
    if footer.is_empty() || body.contains(footer) {
        return body;
    }
    if !body.ends_with('\n') {
        body.push('\n');
    }
    body.push('\n');
    body.push_str(footer);
    body.push('\n');
    body
}

fn env() -> Result<&'static Environment<'static>, BioMcpError> {
    if let Some(env) = ENV.get() {
        return Ok(env);
    }

    let mut env = Environment::new();
    env.add_filter("truncate", |s: String, max_bytes: usize| -> String {
        if s.len() <= max_bytes {
            return s;
        }
        if max_bytes == 0 {
            return "…".to_string();
        }
        let mut boundary = max_bytes;
        while boundary > 0 && !s.is_char_boundary(boundary) {
            boundary -= 1;
        }
        let mut out = s[..boundary].trim_end().to_string();
        out.push('…');
        out
    });
    env.add_filter("phase_short", |phase: String| -> String {
        let p = phase.trim();
        if p.is_empty() || p == "-" {
            return "-".to_string();
        }

        let up = p.to_ascii_uppercase();
        let mut parts: Vec<String> = Vec::new();
        for raw in up.split('/') {
            let seg = raw.trim();
            if seg.is_empty() {
                continue;
            }
            let seg = seg.strip_prefix("PHASE").unwrap_or(seg);
            let seg = seg.trim_matches(|c: char| c == '_' || c.is_whitespace());
            if !seg.is_empty() {
                parts.push(seg.to_string());
            }
        }

        if parts.is_empty() {
            "-".to_string()
        } else {
            parts.join("/")
        }
    });
    env.add_filter("conditions_short", |conditions: Vec<String>| -> String {
        crate::transform::trial::format_conditions(&conditions)
    });
    env.add_filter("pval", |v: f64| -> String {
        if v == 0.0 {
            return "0".to_string();
        }
        if v < 0.001 {
            format!("{v:.2e}")
        } else if v < 0.01 {
            format!("{v:.4}")
        } else {
            format!("{v:.3}")
        }
    });
    env.add_filter("score", |v: f64| -> String { format!("{v:.3}") });
    env.add_filter("af", |v: f64| -> String {
        let mut out = format!("{v:.6}");
        while out.contains('.') && out.ends_with('0') {
            out.pop();
        }
        if out.ends_with('.') {
            out.pop();
        }
        if out.is_empty() { "0".to_string() } else { out }
    });
    env.add_template("gene.md.j2", include_str!("../../templates/gene.md.j2"))?;
    env.add_template(
        "gene_search.md.j2",
        include_str!("../../templates/gene_search.md.j2"),
    )?;
    env.add_template(
        "article.md.j2",
        include_str!("../../templates/article.md.j2"),
    )?;
    env.add_template(
        "article_entities.md.j2",
        include_str!("../../templates/article_entities.md.j2"),
    )?;
    env.add_template(
        "article_search.md.j2",
        include_str!("../../templates/article_search.md.j2"),
    )?;
    env.add_template(
        "disease.md.j2",
        include_str!("../../templates/disease.md.j2"),
    )?;
    env.add_template(
        "disease_search.md.j2",
        include_str!("../../templates/disease_search.md.j2"),
    )?;
    env.add_template("pgx.md.j2", include_str!("../../templates/pgx.md.j2"))?;
    env.add_template(
        "pgx_search.md.j2",
        include_str!("../../templates/pgx_search.md.j2"),
    )?;
    env.add_template("trial.md.j2", include_str!("../../templates/trial.md.j2"))?;
    env.add_template(
        "trial_search.md.j2",
        include_str!("../../templates/trial_search.md.j2"),
    )?;
    env.add_template(
        "variant.md.j2",
        include_str!("../../templates/variant.md.j2"),
    )?;
    env.add_template(
        "variant_search.md.j2",
        include_str!("../../templates/variant_search.md.j2"),
    )?;
    env.add_template(
        "phenotype_search.md.j2",
        include_str!("../../templates/phenotype_search.md.j2"),
    )?;
    env.add_template(
        "gwas_search.md.j2",
        include_str!("../../templates/gwas_search.md.j2"),
    )?;
    env.add_template("drug.md.j2", include_str!("../../templates/drug.md.j2"))?;
    env.add_template(
        "drug_search.md.j2",
        include_str!("../../templates/drug_search.md.j2"),
    )?;
    env.add_template(
        "pathway.md.j2",
        include_str!("../../templates/pathway.md.j2"),
    )?;
    env.add_template(
        "pathway_search.md.j2",
        include_str!("../../templates/pathway_search.md.j2"),
    )?;
    env.add_template(
        "protein.md.j2",
        include_str!("../../templates/protein.md.j2"),
    )?;
    env.add_template(
        "protein_search.md.j2",
        include_str!("../../templates/protein_search.md.j2"),
    )?;
    env.add_template(
        "adverse_event.md.j2",
        include_str!("../../templates/adverse_event.md.j2"),
    )?;
    env.add_template(
        "adverse_event_search.md.j2",
        include_str!("../../templates/adverse_event_search.md.j2"),
    )?;
    env.add_template(
        "device_event.md.j2",
        include_str!("../../templates/device_event.md.j2"),
    )?;
    env.add_template(
        "device_event_search.md.j2",
        include_str!("../../templates/device_event_search.md.j2"),
    )?;
    env.add_template(
        "recall_search.md.j2",
        include_str!("../../templates/recall_search.md.j2"),
    )?;
    env.add_template(
        "search_all.md.j2",
        include_str!("../../templates/search_all.md.j2"),
    )?;
    env.add_template(
        "discover.md.j2",
        include_str!("../../templates/discover.md.j2"),
    )?;

    let _ = ENV.set(env);
    Ok(ENV
        .get()
        .expect("ENV should be initialized by the time this is reached"))
}

fn format_disease_association_score(summary: &DiseaseAssociationScoreSummary) -> String {
    let mut parts = vec![format!("overall {:.3}", summary.overall_score)];
    if let Some(score) = summary.gwas_score {
        parts.push(format!("GWAS {:.3}", score));
    }
    if let Some(score) = summary.rare_variant_score {
        parts.push(format!("rare {:.3}", score));
    }
    if let Some(score) = summary.somatic_mutation_score {
        parts.push(format!("somatic {:.3}", score));
    }
    parts.join("; ")
}

fn disease_top_gene_score_labels(disease: &Disease) -> Vec<String> {
    disease
        .top_gene_scores
        .iter()
        .take(5)
        .map(|row| format!("{} (OT {:.3})", row.symbol, row.summary.overall_score))
        .collect()
}

fn disease_gene_association_rows(disease: &Disease) -> Vec<DiseaseGeneAssociationRenderRow> {
    disease
        .gene_associations
        .iter()
        .map(|row| DiseaseGeneAssociationRenderRow {
            gene: row.gene.clone(),
            relationship: row.relationship.clone(),
            source: row.source.clone(),
            source_url: disease_source_url(disease, row.source.as_deref(), None),
            opentargets: row
                .opentargets_score
                .as_ref()
                .map(format_disease_association_score),
        })
        .collect()
}

fn disease_phenotype_rows(disease: &Disease) -> Vec<DiseasePhenotypeRenderRow> {
    disease
        .phenotypes
        .iter()
        .map(|row| DiseasePhenotypeRenderRow {
            hpo_id: row.hpo_id.clone(),
            name: row.name.clone(),
            evidence: row.evidence.clone(),
            frequency: row.frequency.clone(),
            frequency_qualifier: row.frequency_qualifier.clone(),
            onset_qualifier: row.onset_qualifier.clone(),
            sex_qualifier: row.sex_qualifier.clone(),
            stage_qualifier: row.stage_qualifier.clone(),
            qualifiers: row.qualifiers.clone(),
            source: row.source.clone(),
            source_url: disease_source_url(disease, row.source.as_deref(), None),
        })
        .collect()
}

fn disease_model_rows(disease: &Disease) -> Vec<DiseaseModelAssociationRenderRow> {
    disease
        .models
        .iter()
        .map(|row| DiseaseModelAssociationRenderRow {
            model: row.model.clone(),
            organism: row.organism.clone(),
            relationship: row.relationship.clone(),
            source: row.source.clone(),
            source_url: disease_source_url(disease, row.source.as_deref(), row.model_id.as_deref())
                .or_else(|| disease_source_url(disease, row.source.as_deref(), Some(&row.model))),
            evidence_count: row.evidence_count,
        })
        .collect()
}

fn format_survival_percent(value: Option<f64>) -> Option<String> {
    value.map(|value| format!("{value:.1}%"))
}

fn format_survival_ci(lower_ci: Option<f64>, upper_ci: Option<f64>) -> Option<String> {
    match (lower_ci, upper_ci) {
        (Some(lower), Some(upper)) => Some(format!("{lower:.1}%-{upper:.1}%")),
        _ => None,
    }
}

fn disease_survival_source_line(disease: &Disease) -> Option<String> {
    disease.survival.as_ref().map(|survival| {
        format!(
            "{} (site code {}) · All Ages · All Races / Ethnicities",
            survival.site_label, survival.site_code
        )
    })
}

fn disease_survival_summary_rows(disease: &Disease) -> Vec<DiseaseSurvivalSummaryRenderRow> {
    let Some(survival) = disease.survival.as_ref() else {
        return Vec::new();
    };

    survival
        .series
        .iter()
        .map(|series| {
            let latest_observed = series.latest_observed.as_ref();
            let latest_modeled = series.latest_modeled.as_ref().and_then(|point| {
                point
                    .modeled_relative_survival_rate
                    .map(|value| format!("{}: {value:.1}%", point.year))
            });

            DiseaseSurvivalSummaryRenderRow {
                sex: series.sex.clone(),
                latest_observed_year: latest_observed.map(|point| point.year),
                relative_survival: format_survival_percent(
                    latest_observed.and_then(|point| point.relative_survival_rate),
                ),
                ci_95: latest_observed
                    .and_then(|point| format_survival_ci(point.lower_ci, point.upper_ci)),
                cases: latest_observed.and_then(|point| point.case_count),
                latest_modeled,
            }
        })
        .collect()
}

fn disease_survival_history_rows(disease: &Disease) -> Vec<DiseaseSurvivalHistoryRenderRow> {
    let Some(survival) = disease.survival.as_ref() else {
        return Vec::new();
    };

    let mut rows = Vec::new();
    for series in &survival.series {
        for point in series
            .points
            .iter()
            .rev()
            .filter(|point| point.relative_survival_rate.is_some())
            .take(10)
        {
            rows.push(DiseaseSurvivalHistoryRenderRow {
                sex: series.sex.clone(),
                year: point.year,
                relative_survival: format!("{:.1}%", point.relative_survival_rate.unwrap_or(0.0)),
                ci_95: format_survival_ci(point.lower_ci, point.upper_ci),
                cases: point.case_count,
            });
        }
    }

    rows
}

fn format_funding_amount(amount: u64) -> String {
    let digits = amount.to_string();
    let mut out = String::new();
    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    let grouped = out.chars().rev().collect::<String>();
    format!("${grouped}")
}

fn funding_year_window(section: &NihReporterFundingSection) -> String {
    match (section.fiscal_years.first(), section.fiscal_years.last()) {
        (Some(start), Some(end)) if start == end => format!("FY{start}"),
        (Some(start), Some(end)) => format!("FY{start}-FY{end}"),
        _ => "recent NIH fiscal years".to_string(),
    }
}

fn funding_summary_line(section: Option<&NihReporterFundingSection>) -> Option<String> {
    let section = section.filter(|section| !section.grants.is_empty())?;
    Some(format!(
        "Showing top {} unique grants from {} matching NIH project-year records across {}.",
        section.grants.len(),
        section.matching_project_years,
        funding_year_window(section)
    ))
}

fn funding_project_cell(grant: &NihReporterGrant) -> String {
    let title = markdown_cell(&grant.project_title);
    if let Some(url) = grant
        .project_detail_url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
    {
        format!("[{title}]({url})")
    } else {
        title
    }
}

fn funding_rows(section: Option<&NihReporterFundingSection>) -> Vec<FundingGrantRenderRow> {
    let Some(section) = section else {
        return Vec::new();
    };

    section
        .grants
        .iter()
        .map(|grant| FundingGrantRenderRow {
            project_title: funding_project_cell(grant),
            pi_name: grant
                .pi_name
                .as_deref()
                .map(markdown_cell)
                .unwrap_or_else(|| "-".to_string()),
            organization: grant
                .organization
                .as_deref()
                .map(markdown_cell)
                .unwrap_or_else(|| "-".to_string()),
            fiscal_year: grant.fiscal_year.to_string(),
            award_amount: format_funding_amount(grant.award_amount),
        })
        .collect()
}

fn source_matches(source: Option<&str>, needle: &str) -> bool {
    source
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some_and(|value| value.to_ascii_lowercase().contains(needle))
}

fn orphanet_disease_url(disease: &Disease) -> Option<String> {
    disease
        .xrefs
        .get("Orphanet")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("https://www.orpha.net/en/disease/detail/{value}"))
}

fn omim_disease_url(disease: &Disease) -> Option<String> {
    disease
        .xrefs
        .get("OMIM")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("https://www.omim.org/entry/{value}"))
}

fn mgi_model_url(model_id: Option<&str>) -> Option<String> {
    let model_id = model_id
        .map(str::trim)
        .filter(|value| value.starts_with("MGI:"))?;
    Some(format!(
        "https://www.informatics.jax.org/accession/{model_id}"
    ))
}

fn disease_source_url(
    disease: &Disease,
    source: Option<&str>,
    model_id: Option<&str>,
) -> Option<String> {
    if source_matches(source, "orphanet") {
        return orphanet_disease_url(disease);
    }
    if source_matches(source, "omim") {
        return omim_disease_url(disease);
    }
    if source_matches(source, "mgi") {
        return mgi_model_url(model_id);
    }
    None
}

fn openfda_count_query_url(query: &str, count_field: &str, limit: usize) -> Option<String> {
    let mut url = reqwest::Url::parse("https://api.fda.gov/drug/event.json").ok()?;
    url.query_pairs_mut()
        .append_pair("search", query)
        .append_pair("count", count_field)
        .append_pair("limit", &limit.to_string());
    Some(url.into())
}

fn dailymed_setid_url(set_id: &str) -> Option<String> {
    let set_id = set_id.trim();
    if set_id.is_empty() {
        return None;
    }
    let mut url = reqwest::Url::parse("https://dailymed.nlm.nih.gov/dailymed/drugInfo.cfm").ok()?;
    url.query_pairs_mut().append_pair("setid", set_id);
    Some(url.into())
}

fn dailymed_search_url(name: &str) -> Option<String> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let mut url = reqwest::Url::parse("https://dailymed.nlm.nih.gov/dailymed/search.cfm").ok()?;
    url.query_pairs_mut().append_pair("query", name);
    Some(url.into())
}

fn append_evidence_urls(mut body: String, urls: Vec<(&str, String)>) -> String {
    let links = urls
        .into_iter()
        .filter_map(|(label, url)| {
            let label = label.trim();
            let url = url.trim();
            if label.is_empty() || url.is_empty() {
                return None;
            }
            Some(format!("[{label}]({url})"))
        })
        .collect::<Vec<_>>();
    if links.is_empty() {
        return body;
    }
    if !body.ends_with('\n') {
        body.push('\n');
    }
    body.push('\n');
    body.push_str(&links.join(" | "));
    body.push('\n');
    body
}

pub(crate) fn gene_evidence_urls(gene: &Gene) -> Vec<(&'static str, String)> {
    let mut urls = Vec::new();
    if !gene.entrez_id.trim().is_empty() {
        urls.push((
            "NCBI Gene",
            format!(
                "https://www.ncbi.nlm.nih.gov/gene/{}",
                gene.entrez_id.trim()
            ),
        ));
    }
    if let Some(uniprot) = gene
        .uniprot_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        urls.push((
            "UniProt",
            format!("https://www.uniprot.org/uniprot/{uniprot}"),
        ));
    }
    if let Some(ensembl) = gene
        .ensembl_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        urls.push((
            "Ensembl",
            format!("https://www.ensembl.org/Homo_sapiens/Gene/Summary?g={ensembl}"),
        ));
    }
    if let Some(omim) = gene
        .omim_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        urls.push(("OMIM", format!("https://www.omim.org/entry/{omim}")));
    }
    urls
}

pub(crate) fn variant_evidence_urls(variant: &Variant) -> Vec<(&'static str, String)> {
    let mut urls = Vec::new();
    if let Some(clinvar_id) = variant
        .clinvar_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        urls.push((
            "ClinVar",
            format!("https://www.ncbi.nlm.nih.gov/clinvar/variation/{clinvar_id}/"),
        ));
    }
    if let Some(rsid) = variant
        .rsid
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        urls.push(("dbSNP", format!("https://www.ncbi.nlm.nih.gov/snp/{rsid}")));
    }
    if let Some(cosmic_id) = variant
        .cosmic_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        urls.push((
            "COSMIC",
            format!("https://cancer.sanger.ac.uk/cosmic/mutation/overview?id={cosmic_id}"),
        ));
    }
    if (variant.gnomad_af.is_some() || variant.population_breakdown.is_some())
        && let Some(variant_id) = variant
            .rsid
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| gnomad_variant_slug(&variant.id))
    {
        urls.push((
            "gnomAD",
            format!("https://gnomad.broadinstitute.org/variant/{variant_id}"),
        ));
    }
    urls
}

pub(crate) fn discover_evidence_urls(result: &DiscoverResult) -> Vec<(&'static str, String)> {
    let mut urls = Vec::new();
    if let Ok(mut url) = reqwest::Url::parse("https://www.ebi.ac.uk/ols4/api/search") {
        url.query_pairs_mut()
            .append_pair("q", result.query.trim())
            .append_pair("rows", "10")
            .append_pair("groupField", "iri");
        urls.push(("OLS4", url.into()));
    }
    if let Some(topic) = result.plain_language.as_ref() {
        urls.push(("MedlinePlus", topic.url.clone()));
    }
    urls
}

pub(crate) fn article_evidence_urls(article: &Article) -> Vec<(&'static str, String)> {
    let mut urls = Vec::new();
    if let Some(pmid) = article
        .pmid
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        urls.push(("PubMed", format!("https://pubmed.ncbi.nlm.nih.gov/{pmid}/")));
    }
    if let Some(pmcid) = article
        .pmcid
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        urls.push((
            "PMC",
            format!("https://pmc.ncbi.nlm.nih.gov/articles/{pmcid}/"),
        ));
    }
    urls
}

pub(crate) fn trial_evidence_urls(trial: &Trial) -> Vec<(&'static str, String)> {
    if trial.nct_id.trim().is_empty() {
        return Vec::new();
    }
    vec![(
        "ClinicalTrials.gov",
        format!("https://clinicaltrials.gov/study/{}", trial.nct_id.trim()),
    )]
}

pub(crate) fn disease_evidence_urls(disease: &Disease) -> Vec<(&'static str, String)> {
    let mut urls = Vec::new();
    if !disease.id.trim().is_empty() {
        urls.push((
            "Monarch",
            format!("https://monarchinitiative.org/{}", disease.id.trim()),
        ));
    }
    if disease
        .gene_associations
        .iter()
        .any(|row| source_matches(row.source.as_deref(), "orphanet"))
        && let Some(url) = orphanet_disease_url(disease)
    {
        urls.push(("Orphanet", url));
    }
    let has_omim_source = disease
        .gene_associations
        .iter()
        .any(|row| source_matches(row.source.as_deref(), "omim"))
        || disease
            .phenotypes
            .iter()
            .any(|row| source_matches(row.source.as_deref(), "omim"));
    if has_omim_source && let Some(url) = omim_disease_url(disease) {
        urls.push(("OMIM", url));
    }
    if let Some(url) = disease
        .models
        .iter()
        .find(|row| source_matches(row.source.as_deref(), "mgi"))
        .and_then(|row| {
            mgi_model_url(row.model_id.as_deref()).or_else(|| mgi_model_url(Some(&row.model)))
        })
    {
        urls.push(("MGI", url));
    }
    urls
}

pub(crate) fn drug_evidence_urls(drug: &Drug) -> Vec<(&'static str, String)> {
    let mut urls = Vec::new();
    if let Some(drugbank_id) = drug
        .drugbank_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        urls.push((
            "DrugBank",
            format!("https://go.drugbank.com/drugs/{drugbank_id}"),
        ));
    }
    if let Some(chembl_id) = drug
        .chembl_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        urls.push((
            "ChEMBL",
            format!("https://www.ebi.ac.uk/chembl/compound_report_card/{chembl_id}"),
        ));
    }
    if !drug.top_adverse_events.is_empty()
        && let Some(query) = drug
            .faers_query
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        && let Some(url) =
            openfda_count_query_url(query, "patient.reaction.reactionmeddrapt.exact", 50)
    {
        urls.push(("OpenFDA FAERS", url));
    }
    if drug.label.is_some()
        && let Some(url) = drug
            .label_set_id
            .as_deref()
            .and_then(dailymed_setid_url)
            .or_else(|| dailymed_search_url(&drug.name))
    {
        urls.push(("DailyMed", url));
    }
    urls
}

pub(crate) fn pathway_evidence_urls(pathway: &Pathway) -> Vec<(&'static str, String)> {
    let id = pathway.id.trim();
    if id.is_empty() {
        return Vec::new();
    }
    if pathway.source.eq_ignore_ascii_case("KEGG") {
        return vec![("KEGG", format!("https://www.kegg.jp/entry/{id}"))];
    }
    if pathway.source.eq_ignore_ascii_case("WikiPathways") {
        return vec![(
            "WikiPathways",
            format!("https://www.wikipathways.org/pathways/{id}.html"),
        )];
    }
    vec![(
        "Reactome",
        format!("https://reactome.org/content/detail/{id}"),
    )]
}

pub(crate) fn protein_evidence_urls(protein: &Protein) -> Vec<(&'static str, String)> {
    if protein.accession.trim().is_empty() {
        return Vec::new();
    }
    vec![(
        "UniProt",
        format!(
            "https://www.uniprot.org/uniprot/{}",
            protein.accession.trim()
        ),
    )]
}

pub(crate) fn adverse_event_evidence_urls(event: &AdverseEvent) -> Vec<(&'static str, String)> {
    if event.report_id.trim().is_empty() {
        return Vec::new();
    }
    vec![(
        "OpenFDA",
        format!(
            "https://api.fda.gov/drug/event.json?search=safetyreportid:{}",
            event.report_id.trim()
        ),
    )]
}

pub(crate) fn device_event_evidence_urls(event: &DeviceEvent) -> Vec<(&'static str, String)> {
    if event.report_id.trim().is_empty() {
        return Vec::new();
    }
    vec![(
        "OpenFDA",
        format!(
            "https://api.fda.gov/device/event.json?search=mdr_report_key:{}",
            event.report_id.trim()
        ),
    )]
}

pub(crate) fn pgx_evidence_urls(pgx: &Pgx) -> Vec<(&'static str, String)> {
    let mut urls = Vec::new();
    if let Some(gene) = pgx.gene.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        urls.push((
            "CPIC",
            format!("https://cpicpgx.org/genes/{}/", gene.to_ascii_lowercase()),
        ));
        urls.push(("PharmGKB", format!("https://www.pharmgkb.org/gene/{gene}")));
    }
    if let Some(drug) = pgx.drug.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        urls.push((
            "PharmGKB",
            format!("https://www.pharmgkb.org/chemical/{drug}"),
        ));
    }
    urls
}

pub(crate) fn quote_arg(value: &str) -> String {
    let v = value.trim();
    if v.is_empty() {
        return String::new();
    }
    if v.chars().any(|c| c.is_whitespace()) {
        return format!("\"{}\"", v.replace('\"', "\\\""));
    }
    v.to_string()
}

fn force_quote_arg(value: &str) -> String {
    let v = value.trim();
    if v.is_empty() {
        return String::new();
    }
    format!("\"{}\"", v.replace('\"', "\\\""))
}

pub(crate) fn alias_fallback_suggestion(
    decision: &crate::entities::discover::AliasFallbackDecision,
) -> String {
    match decision {
        crate::entities::discover::AliasFallbackDecision::Canonical(alias) => {
            let command = alias.next_commands.first().cloned().unwrap_or_else(|| {
                format!(
                    "biomcp get {} {}",
                    alias.requested_entity.cli_name(),
                    quote_arg(&alias.canonical)
                )
            });
            format!("Did you mean: `{command}`")
        }
        crate::entities::discover::AliasFallbackDecision::Ambiguous(alias) => {
            let mut out = format!(
                "BioMCP could not map '{}' to a single {}.\n\nTry:",
                alias.query,
                alias.requested_entity.cli_name()
            );
            for (idx, command) in alias.next_commands.iter().enumerate() {
                out.push_str(&format!("\n{}. {command}", idx + 1));
            }
            if !alias.candidates.is_empty() {
                out.push_str("\n\nPossible matches:");
                for candidate in &alias.candidates {
                    match candidate.primary_id.as_deref() {
                        Some(primary_id) => out.push_str(&format!(
                            "\n- {} ({}, {})",
                            candidate.label,
                            candidate.primary_type.label(),
                            primary_id
                        )),
                        None => out.push_str(&format!(
                            "\n- {} ({})",
                            candidate.label,
                            candidate.primary_type.label()
                        )),
                    }
                }
            }
            out
        }
        crate::entities::discover::AliasFallbackDecision::None => String::new(),
    }
}

pub(crate) fn variant_guidance_suggestion(
    guidance: &crate::entities::variant::VariantGuidance,
) -> String {
    match &guidance.kind {
        crate::entities::variant::VariantGuidanceKind::GeneResidueAlias { .. } => {
            let mut out = format!(
                "BioMCP could not map '{}' to an exact variant.\n\nTry:",
                guidance.query
            );
            for (idx, command) in guidance.next_commands.iter().enumerate() {
                out.push_str(&format!("\n{}. {command}", idx + 1));
            }
            out
        }
        crate::entities::variant::VariantGuidanceKind::ProteinChangeOnly { .. } => {
            let mut out = format!(
                "BioMCP could not map '{}' to an exact variant without gene context.\n\nTry:",
                guidance.query
            );
            for (idx, command) in guidance.next_commands.iter().enumerate() {
                out.push_str(&format!("\n{}. {command}", idx + 1));
            }
            out
        }
    }
}

fn has_all_section(requested: &[String]) -> bool {
    requested
        .iter()
        .any(|s| s.trim().eq_ignore_ascii_case("all"))
}

fn is_section_only_requested(requested: &[String]) -> bool {
    !has_all_section(requested) && requested.iter().any(|s| !s.trim().is_empty())
}

fn requested_section_names(requested: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for section in requested {
        let section = section.trim();
        if section.is_empty() || section.eq_ignore_ascii_case("all") {
            continue;
        }
        let normalized = section.to_ascii_lowercase();
        if out.iter().any(|v| v == &normalized) {
            continue;
        }
        out.push(normalized);
    }
    out
}

fn section_header(entity_label: &str, requested: &[String]) -> String {
    let names = requested_section_names(requested);
    if names.is_empty() {
        entity_label.to_string()
    } else {
        format!("{entity_label} - {}", names.join(", "))
    }
}

fn format_sections_block(entity: &str, id: &str, sections: Vec<String>) -> String {
    if sections.is_empty() {
        return String::new();
    }
    let id_q = quote_arg(id);
    if id_q.is_empty() {
        return String::new();
    }
    let mut out = String::from("More:");
    for section in sections.iter().take(3) {
        let _ = write!(
            out,
            "\n  biomcp get {entity} {id_q} {section}   - {}",
            section_description(entity, section)
        );
    }
    let _ = write!(out, "\nAll:\n  biomcp get {entity} {id_q} all");
    out
}

fn format_related_block(commands: Vec<String>) -> String {
    let commands: Vec<String> = commands
        .into_iter()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect();
    if commands.is_empty() {
        return String::new();
    }
    let mut out = String::from("See also:");
    for cmd in &commands {
        if let Some(description) = related_command_description(cmd) {
            out.push_str(&format!("\n  {cmd}   - {description}"));
        } else {
            out.push_str(&format!("\n  {cmd}"));
        }
    }
    out
}

fn section_description(entity: &str, section: &str) -> &'static str {
    match (entity, section) {
        ("gene", "pathways") => "Reactome/KEGG pathway context",
        ("gene", "ontology") => "GO-style functional enrichment",
        ("gene", "diseases") => "disease associations",
        ("gene", "protein") => "UniProt function and localization detail",
        ("gene", "expression") => "GTEx tissue expression",
        ("gene", "hpa") => "Human Protein Atlas tissue expression and localization",
        ("gene", "go") => "QuickGO term annotations",
        ("gene", "interactions") => "STRING interaction partners",
        ("gene", "civic") => "CIViC clinical evidence",
        ("gene", "druggability") => "DGIdb interactions and tractability",
        ("gene", "clingen") => "ClinGen validity and dosage sensitivity",
        ("gene", "constraint") => "gnomAD gene constraint metrics",
        ("gene", "disgenet") => "DisGeNET scored disease links",
        ("gene", "funding") => "NIH Reporter grant support",
        ("article", "annotations") => "PubTator normalized entity mentions",
        ("article", "fulltext") => "cached full text when available",
        ("article", "tldr") => "Semantic Scholar summary and influence",
        ("disease", "genes") => "associated genes",
        ("disease", "pathways") => "pathways from associated genes",
        ("disease", "phenotypes") => "HPO phenotype annotations",
        ("disease", "variants") => "disease-associated variants",
        ("disease", "models") => "model-organism evidence",
        ("disease", "prevalence") => "prevalence and epidemiology context",
        ("disease", "funding") => "NIH Reporter grant support",
        ("disease", "civic") => "CIViC disease-context evidence",
        ("disease", "disgenet") => "DisGeNET scored disease-gene links",
        ("drug", "label") => "approved-indication and FDA label detail beyond the base card",
        ("drug", "regulatory") => {
            "approval and supplement history; use only if the base card lacks approval context"
        }
        ("drug", "safety") => {
            "regulatory safety detail; use `biomcp drug adverse-events <name>` first when you want post-marketing signal"
        }
        ("drug", "targets") => "ChEMBL and OpenTargets targets",
        ("drug", "indications") => "OpenTargets indication evidence",
        ("drug", "interactions") => "label interactions and public-data fallback",
        ("drug", "civic") => "CIViC therapy evidence",
        ("drug", "approvals") => "Drugs@FDA approval history",
        ("trial", "eligibility") => "inclusion and exclusion criteria",
        ("trial", "locations") => "site list and contact details",
        ("trial", "outcomes") => "endpoint measures and time frames",
        ("trial", "arms") => "study arms and assigned interventions",
        ("trial", "references") => "linked publications and PMID citations",
        _ => "additional detail",
    }
}

fn is_trial_results_search_command(command: &str) -> bool {
    (command.starts_with("biomcp search article -q \"NCT") && command.ends_with(" --limit 5"))
        || (command.starts_with("biomcp search article --drug ")
            && command.contains(" -q \"NCT")
            && command.ends_with(" --limit 5"))
}

fn is_variant_literature_follow_up_command(command: &str) -> bool {
    command.starts_with("biomcp search article ")
        && command.contains(" -k ")
        && command.ends_with(" --limit 5")
        && !command.contains(" -q ")
        && !command.contains(" --type ")
}

fn related_command_description(command: &str) -> Option<&'static str> {
    if command.starts_with("biomcp article entities ") {
        Some("standardized entity extraction from this article")
    } else if is_trial_results_search_command(command) {
        Some("find publications or conference reports from this completed/terminated trial")
    } else if command.starts_with("biomcp article citations ") {
        Some("later papers that cite this article; use only if the primary paper lacks your answer")
    } else if command.starts_with("biomcp article references ") {
        Some("background evidence this paper builds on; use if the primary paper lacks context")
    } else if command.starts_with("biomcp article recommendations ") {
        Some("related papers to broaden coverage; use only if the primary paper lacks your answer")
    } else if command.contains(" --type review --limit 5") {
        Some("supplement sparse structured data with review literature for indication context")
    } else if command.starts_with("biomcp get gene ") && command.ends_with(" clingen constraint") {
        Some("review ClinGen validity and constraint evidence for the top disease gene")
    } else if command.starts_with("biomcp get gene ") && command.ends_with(" protein") {
        Some("deepen into protein function and localization")
    } else if command.starts_with("biomcp get gene ") && command.ends_with(" hpa") {
        Some("deepen into tissue expression and localization")
    } else if command.starts_with("biomcp search trial -c ") && command.ends_with(" -s recruiting")
    {
        Some("recruiting trials for the top ClinGen disease on this gene card")
    } else if command.starts_with("biomcp search pgx -d ")
        || command.starts_with("biomcp search pgx -g ")
    {
        Some("pharmacogenomics interactions")
    } else if command.starts_with("biomcp get disease ") && command.ends_with(" genes phenotypes") {
        Some("open the top phenotype-match disease with genes and phenotypes")
    } else if is_variant_literature_follow_up_command(command) {
        Some("literature follow-up for an uncertain-significance variant")
    } else if command.starts_with("biomcp search drug --indication ") {
        Some("treatment options for this condition")
    } else if command.starts_with("biomcp get pgx ") {
        Some("pharmacogenomics card")
    } else if command.starts_with("biomcp study top-mutated --study ") {
        Some("mutation frequency ranking")
    } else if command == "biomcp study download --list" {
        Some("browse downloadable cancer genomics studies")
    } else if command.starts_with("biomcp drug adverse-events ") {
        Some("inspect safety reports and adverse-event signal")
    } else {
        None
    }
}

fn shell_quote_arg(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let quoted = quote_arg(trimmed);
    if quoted != trimmed {
        return quoted;
    }

    if trimmed.chars().any(|ch| {
        matches!(
            ch,
            '"' | '\''
                | '\\'
                | '$'
                | '`'
                | '|'
                | '&'
                | ';'
                | '<'
                | '>'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '*'
                | '?'
                | '!'
                | '#'
        )
    }) {
        return format!("\"{}\"", trimmed.replace('\\', "\\\\").replace('"', "\\\""));
    }

    trimmed.to_string()
}

fn discover_try_line(query: &str, description: &str) -> String {
    let query = shell_quote_arg(query);
    if query.is_empty() {
        return String::new();
    }
    format!("Try: biomcp discover {query}   - {description}")
}

fn markdown_cell(value: &str) -> String {
    let value = value.replace(['\n', '\r'], " ").replace('|', "\\|");
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if value.is_empty() {
        "-".to_string()
    } else {
        value
    }
}

fn dedupe_markdown_commands(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        let key = trimmed.to_ascii_lowercase();
        if seen.insert(key) {
            out.push(trimmed.to_string());
        }
    }
    out
}

fn disease_top_gene_symbol(disease: &Disease) -> Option<String> {
    disease
        .top_gene_scores
        .iter()
        .map(|row| row.symbol.trim())
        .chain(disease.top_genes.iter().map(String::as_str).map(str::trim))
        .chain(
            disease
                .associated_genes
                .iter()
                .map(String::as_str)
                .map(str::trim),
        )
        .find(|symbol| !symbol.is_empty())
        .map(str::to_string)
}

fn gene_trial_disease_label(gene: &Gene) -> Option<String> {
    gene.clingen
        .as_ref()?
        .validity
        .iter()
        .map(|row| row.disease.trim())
        .find(|label| !label.is_empty())
        .map(str::to_string)
}

fn variant_keyword_from_legacy_name(legacy_name: &str, gene: &str) -> String {
    let legacy_name = legacy_name.trim();
    let gene = gene.trim();
    if legacy_name.is_empty() {
        return String::new();
    }
    if gene.is_empty() {
        return legacy_name.to_string();
    }

    let mut tokens = legacy_name.split_whitespace();
    if tokens
        .next()
        .is_some_and(|token| token.eq_ignore_ascii_case(gene))
    {
        let remainder = tokens.collect::<Vec<_>>().join(" ");
        if !remainder.is_empty() {
            return remainder;
        }
    }

    legacy_name.to_string()
}

fn variant_literature_keyword_seed(variant: &Variant) -> Option<String> {
    if let Some(seed) = variant
        .legacy_name
        .as_deref()
        .map(|legacy| variant_keyword_from_legacy_name(legacy, &variant.gene))
        .filter(|value| !value.trim().is_empty())
    {
        return Some(seed.trim().to_string());
    }

    if let Some(seed) = variant
        .hgvs_p
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let seed = seed.strip_prefix("p.").unwrap_or(seed).trim();
        if !seed.is_empty() {
            return Some(seed.to_string());
        }
    }

    let id = variant.id.trim();
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

fn variant_literature_follow_up(variant: &Variant) -> Option<String> {
    let keyword = force_quote_arg(&variant_literature_keyword_seed(variant)?);
    if keyword.is_empty() {
        return None;
    }

    let gene = variant.gene.trim();
    let disease = variant
        .top_disease
        .as_ref()
        .map(|row| row.condition.trim())
        .filter(|condition| !condition.is_empty())
        .map(force_quote_arg);

    if !gene.is_empty() {
        if let Some(disease) = disease {
            return Some(format!(
                "biomcp search article -g {gene} -d {disease} -k {keyword} --limit 5"
            ));
        }
        return Some(format!(
            "biomcp search article -g {gene} -k {keyword} --limit 5"
        ));
    }

    if let Some(disease) = disease {
        return Some(format!(
            "biomcp search article -d {disease} -k {keyword} --limit 5"
        ));
    }

    Some(format!("biomcp search article -k {keyword} --limit 5"))
}

fn article_related_id(paper: &ArticleRelatedPaper) -> String {
    paper
        .pmid
        .as_deref()
        .or(paper.doi.as_deref())
        .or(paper.arxiv_id.as_deref())
        .or(paper.paper_id.as_deref())
        .map(markdown_cell)
        .unwrap_or_else(|| "-".to_string())
}

fn article_related_label(paper: &ArticleRelatedPaper) -> String {
    paper
        .pmid
        .as_deref()
        .map(|pmid| format!("PMID {pmid}"))
        .or_else(|| paper.doi.as_deref().map(|doi| format!("DOI {doi}")))
        .or_else(|| {
            paper
                .arxiv_id
                .as_deref()
                .map(|arxiv| format!("arXiv {arxiv}"))
        })
        .or_else(|| {
            paper
                .paper_id
                .as_deref()
                .map(|paper_id| format!("paper {paper_id}"))
        })
        .unwrap_or_else(|| markdown_cell(&paper.title))
}

fn sections_for(requested: &[String], available: &[&str]) -> Vec<String> {
    if has_all_section(requested) {
        return Vec::new();
    }

    let requested_set: HashSet<String> = requested
        .iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    available
        .iter()
        .copied()
        .filter(|s| *s != "all")
        .filter(|s| !requested_set.contains(&s.to_ascii_lowercase()))
        .map(|section| section.to_string())
        .collect()
}

fn sections_gene(gene: &Gene, requested: &[String]) -> Vec<String> {
    let symbol = gene.symbol.trim();
    if symbol.is_empty() {
        return Vec::new();
    }

    sections_for(requested, crate::entities::gene::GENE_SECTION_NAMES)
}

fn sections_variant(variant: &Variant, requested: &[String]) -> Vec<String> {
    let id = quote_arg(&variant.id);
    if id.is_empty() {
        return Vec::new();
    }
    sections_for(requested, crate::entities::variant::VARIANT_SECTION_NAMES)
}

fn sections_article(article: &Article, requested: &[String]) -> Vec<String> {
    let key = article
        .pmid
        .as_deref()
        .or(article.pmcid.as_deref())
        .or(article.doi.as_deref())
        .unwrap_or("");
    let key = quote_arg(key);
    if key.is_empty() {
        return Vec::new();
    }
    sections_for(requested, crate::entities::article::ARTICLE_SECTION_NAMES)
}

const COMPLETED_TRIAL_SECTION_NAMES: &[&str] = &[
    "outcomes",
    "references",
    "arms",
    "eligibility",
    "locations",
    "all",
];

fn is_completed_or_terminated_trial_status(status: &str) -> bool {
    let status = status.trim();
    status.eq_ignore_ascii_case("COMPLETED") || status.eq_ignore_ascii_case("TERMINATED")
}

fn sections_trial(trial: &Trial, requested: &[String]) -> Vec<String> {
    let nct_id = trial.nct_id.trim();
    if nct_id.is_empty() {
        return Vec::new();
    }
    let available = if is_completed_or_terminated_trial_status(&trial.status) {
        COMPLETED_TRIAL_SECTION_NAMES
    } else {
        crate::entities::trial::TRIAL_SECTION_NAMES
    };
    sections_for(requested, available)
}

fn sections_drug(drug: &Drug, requested: &[String]) -> Vec<String> {
    let name = quote_arg(&drug.name);
    if name.is_empty() {
        return Vec::new();
    }
    sections_for(requested, crate::entities::drug::DRUG_SECTION_NAMES)
}

fn sections_disease(disease: &Disease, requested: &[String]) -> Vec<String> {
    let key = quote_arg(&disease.id);
    if key.is_empty() {
        return Vec::new();
    }
    sections_for(requested, crate::entities::disease::DISEASE_SECTION_NAMES)
}

fn sections_pgx(pgx: &Pgx, requested: &[String]) -> Vec<String> {
    if pgx.query.trim().is_empty() {
        return Vec::new();
    }
    sections_for(requested, crate::entities::pgx::PGX_SECTION_NAMES)
}

fn sections_pathway(pathway: &Pathway, requested: &[String]) -> Vec<String> {
    let id = quote_arg(&pathway.id);
    if id.is_empty() {
        return Vec::new();
    }
    sections_for(
        requested,
        crate::entities::pathway::supported_pathway_sections_for_source(&pathway.source),
    )
}

fn sections_protein(protein: &Protein, requested: &[String]) -> Vec<String> {
    let accession = quote_arg(&protein.accession);
    if accession.is_empty() {
        return Vec::new();
    }
    sections_for(requested, crate::entities::protein::PROTEIN_SECTION_NAMES)
}

fn sections_adverse_event(event: &AdverseEvent, requested: &[String]) -> Vec<String> {
    let report_id = quote_arg(&event.report_id);
    if report_id.is_empty() {
        return Vec::new();
    }
    sections_for(
        requested,
        crate::entities::adverse_event::ADVERSE_EVENT_SECTION_NAMES,
    )
}

pub(crate) fn related_gene(gene: &Gene) -> Vec<String> {
    let symbol = gene.symbol.trim();
    if symbol.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let summary = gene.summary.as_deref().unwrap_or("").to_ascii_lowercase();

    if gene.protein.is_some() || summary.contains("mitochond") || summary.contains("membrane") {
        out.push(format!("biomcp get gene {symbol} protein"));
    }
    if gene.hpa.is_some()
        || summary.contains("mitochond")
        || summary.contains("localiz")
        || summary.contains("tissue")
    {
        out.push(format!("biomcp get gene {symbol} hpa"));
    }
    if let Some(disease) = gene_trial_disease_label(gene) {
        out.push(format!(
            "biomcp search trial -c {} -s recruiting",
            force_quote_arg(&disease)
        ));
    }
    out.push(format!("biomcp search pgx -g {symbol}"));
    out.push(format!("biomcp search variant -g {symbol}"));
    out.push(format!("biomcp search article -g {symbol}"));
    out.push(format!("biomcp search drug --target {symbol}"));
    out.push(format!("biomcp gene trials {symbol}"));
    dedupe_markdown_commands(out)
}

pub(crate) fn related_variant(variant: &Variant) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let gene = variant.gene.trim();
    let significance = variant
        .significance
        .as_deref()
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_default();
    let pathogenic_like = significance.contains("pathogenic");
    let vus_like = significance.contains("uncertain significance") || significance.contains("vus");

    if !gene.is_empty() {
        out.push(format!("biomcp get gene {gene}"));
    }

    if !pathogenic_like
        && vus_like
        && let Some(command) = variant_literature_follow_up(variant)
    {
        out.push(command);
    }

    if !gene.is_empty() {
        out.push(format!("biomcp search drug --target {gene}"));
    }

    if !variant.id.trim().is_empty() {
        let id = quote_arg(&variant.id);
        out.push(format!("biomcp variant trials {id}"));
        out.push(format!("biomcp variant articles {id}"));
        let has_oncokb_token = std::env::var("ONCOKB_TOKEN")
            .ok()
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false);
        if has_oncokb_token {
            out.push(format!("biomcp variant oncokb {id}"));
        }
    }
    dedupe_markdown_commands(out)
}

pub(crate) fn related_variant_search_results(
    results: &[VariantSearchResult],
    gene_filter: Option<&str>,
    condition_filter: Option<&str>,
) -> Vec<String> {
    let mut out = Vec::new();

    if let Some(id) = results
        .first()
        .map(|result| quote_arg(&result.id))
        .filter(|id| !id.is_empty())
    {
        out.push(format!("biomcp get variant {id}"));
    }

    if let Some(gene) = gene_filter.map(str::trim).filter(|gene| !gene.is_empty()) {
        out.push(format!("biomcp get gene {gene}"));
    }

    if let Some(condition) = condition_filter
        .map(quote_arg)
        .filter(|value| !value.is_empty())
    {
        out.push(format!("biomcp search disease --query {condition}"));
    }

    dedupe_markdown_commands(out)
}

pub(crate) fn related_phenotype_search_results(results: &[PhenotypeSearchResult]) -> Vec<String> {
    let Some(label) = results.first().and_then(|row| {
        let name = row.disease_name.trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
        let id = row.disease_id.trim();
        if id.is_empty() {
            None
        } else {
            Some(id.to_string())
        }
    }) else {
        return Vec::new();
    };

    dedupe_markdown_commands(vec![format!(
        "biomcp get disease {} genes phenotypes",
        force_quote_arg(&label)
    )])
}

#[derive(Clone, Copy)]
enum ArticleAnnotationBucket {
    Gene,
    Disease,
    Chemical,
    Mutation,
}

fn article_annotation_command(bucket: ArticleAnnotationBucket, text: &str) -> Option<String> {
    let text = text.trim();
    let quoted = quote_arg(text);
    if quoted.is_empty() {
        return None;
    }

    Some(match bucket {
        ArticleAnnotationBucket::Gene => format!("biomcp search gene -q {quoted}"),
        ArticleAnnotationBucket::Disease => format!("biomcp search disease --query {quoted}"),
        ArticleAnnotationBucket::Chemical => format!("biomcp get drug {quoted}"),
        ArticleAnnotationBucket::Mutation => format!("biomcp get variant {quoted}"),
    })
}

fn ranked_article_annotation_commands(
    title: &str,
    rows: &[AnnotationCount],
    bucket: ArticleAnnotationBucket,
    limit: usize,
) -> Vec<String> {
    if limit == 0 {
        return Vec::new();
    }

    let normalized_title = normalize_match_text(title);
    let mut ranked = rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            let command = article_annotation_command(bucket, &row.text)?;
            let normalized_text = normalize_match_text(&row.text);
            let title_hit =
                !normalized_text.is_empty() && normalized_title.contains(normalized_text.as_str());
            Some((title_hit, row.count, index, command))
        })
        .collect::<Vec<_>>();

    ranked.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| a.2.cmp(&b.2))
    });

    ranked
        .into_iter()
        .take(limit)
        .map(|(_, _, _, command)| command)
        .collect()
}

fn trial_results_search_command(trial: &Trial) -> Option<String> {
    let nct_id = trial.nct_id.trim();
    if nct_id.is_empty() {
        return None;
    }

    let title_seed = trial
        .title
        .split_whitespace()
        .take(6)
        .collect::<Vec<_>>()
        .join(" ");
    let seed = if title_seed.is_empty() {
        nct_id.to_string()
    } else {
        format!("{nct_id} {title_seed}")
    };
    let seed_q = format!("\"{}\"", seed.replace('\"', "\\\""));
    if seed_q.is_empty() {
        return None;
    }

    if let Some(intervention) = trial.interventions.first().map(String::as_str) {
        let intervention_q = quote_arg(intervention);
        if !intervention_q.is_empty() {
            return Some(format!(
                "biomcp search article --drug {intervention_q} -q {seed_q} --limit 5"
            ));
        }
    }

    Some(format!("biomcp search article -q {seed_q} --limit 5"))
}

pub(crate) fn related_article(article: &Article) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(pmid) = article
        .pmid
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        && article.annotations.is_some()
    {
        out.push(format!("biomcp article entities {pmid}"));
    }
    if let Some(ann) = article.annotations.as_ref() {
        out.extend(ranked_article_annotation_commands(
            &article.title,
            &ann.genes,
            ArticleAnnotationBucket::Gene,
            2,
        ));
        out.extend(ranked_article_annotation_commands(
            &article.title,
            &ann.diseases,
            ArticleAnnotationBucket::Disease,
            2,
        ));
        out.extend(ranked_article_annotation_commands(
            &article.title,
            &ann.chemicals,
            ArticleAnnotationBucket::Chemical,
            2,
        ));
    }
    if let Some(pmid) = article
        .pmid
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        out.push(format!("biomcp article references {pmid} --limit 3"));
        out.push(format!("biomcp article citations {pmid} --limit 3"));
        out.push(format!("biomcp article recommendations {pmid} --limit 3"));
    }
    dedupe_markdown_commands(out)
}

pub(crate) fn related_trial(trial: &Trial) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    if is_completed_or_terminated_trial_status(&trial.status)
        && let Some(command) = trial_results_search_command(trial)
    {
        out.push(command);
    }

    if let Some(condition) = trial.conditions.first().map(String::as_str) {
        let cond = quote_arg(condition);
        if !cond.is_empty() {
            out.push(format!("biomcp search disease --query {cond}"));
            out.push(format!("biomcp search article -d {cond}"));
            out.push(format!("biomcp search trial -c {cond}"));
        }
    }

    if let Some(intervention) = trial.interventions.first().map(String::as_str) {
        let name = quote_arg(intervention);
        if !name.is_empty() {
            out.push(format!("biomcp get drug {name}"));
            out.push(format!("biomcp drug trials {name}"));
        }
    }

    dedupe_markdown_commands(out)
}

pub(crate) fn related_disease(disease: &Disease) -> Vec<String> {
    let name = force_quote_arg(&disease_literature_query(disease));
    let mut out = Vec::new();
    if let Some(symbol) = disease_top_gene_symbol(disease) {
        out.push(format!("biomcp get gene {symbol} clingen constraint"));
    }
    if !name.is_empty() && !disease.phenotypes.is_empty() && disease.phenotypes.len() <= 3 {
        out.push(format!(
            "biomcp search article -d {name} --type review --limit 5"
        ));
    }
    if !name.is_empty() {
        out.push(format!("biomcp search trial -c {name}"));
        out.push(format!("biomcp search article -d {name}"));
        out.push(format!("biomcp search drug --indication {name}"));
    }
    if is_oncology_disease(disease) {
        if let Some(study_id) = best_local_oncology_study_id(disease) {
            out.push(format!("biomcp study top-mutated --study {study_id}"));
        } else {
            out.push("biomcp study download --list".to_string());
        }
    }
    dedupe_markdown_commands(out)
}

fn is_oncology_disease(disease: &Disease) -> bool {
    disease.civic.is_some()
        || disease.top_gene_scores.iter().any(|row| {
            row.summary
                .somatic_mutation_score
                .map(|score| score > 0.0)
                .unwrap_or(false)
        })
}

fn normalize_match_text(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn token_subset_match(left: &str, right: &str) -> bool {
    let left_tokens = left.split_whitespace().collect::<Vec<_>>();
    let right_tokens = right.split_whitespace().collect::<Vec<_>>();
    if left_tokens.is_empty() || right_tokens.is_empty() {
        return false;
    }

    left_tokens.iter().all(|token| right_tokens.contains(token))
        || right_tokens.iter().all(|token| left_tokens.contains(token))
}

fn best_oncology_study_id(
    disease: &Disease,
    studies: &[crate::sources::cbioportal_study::StudyLookupRow],
) -> Option<String> {
    let candidate_labels = std::iter::once(disease.name.as_str())
        .chain(disease.synonyms.iter().map(String::as_str))
        .map(normalize_match_text)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    let mut best: Option<(u8, usize, &str)> = None;
    for study in studies.iter().filter(|study| study.has_mutations) {
        for term in &study.terms {
            let normalized_term = normalize_match_text(term);
            if normalized_term.is_empty() {
                continue;
            }

            let match_kind = if candidate_labels
                .iter()
                .any(|candidate| candidate == &normalized_term)
            {
                Some(0)
            } else if candidate_labels.iter().any(|candidate| {
                candidate.contains(&normalized_term) || normalized_term.contains(candidate)
            }) {
                Some(1)
            } else if candidate_labels
                .iter()
                .any(|candidate| token_subset_match(candidate, &normalized_term))
            {
                Some(2)
            } else {
                None
            };

            let Some(match_kind) = match_kind else {
                continue;
            };

            let candidate = (match_kind, normalized_term.len(), study.study_id.as_str());
            if best
                .as_ref()
                .map(|current| candidate < *current)
                .unwrap_or(true)
            {
                best = Some(candidate);
            }
        }
    }

    best.map(|(_, _, study_id)| study_id.to_string())
}

fn best_local_oncology_study_id(disease: &Disease) -> Option<String> {
    let root = crate::sources::cbioportal_study::resolve_study_root();
    let rows = crate::sources::cbioportal_study::list_study_lookup_rows(&root).ok()?;
    best_oncology_study_id(disease, &rows)
}

fn disease_literature_query(disease: &Disease) -> String {
    let name = disease.name.trim();
    if !name.is_empty() && !name.eq_ignore_ascii_case(disease.id.trim()) {
        return name.to_string();
    }

    disease
        .synonyms
        .iter()
        .map(String::as_str)
        .map(str::trim)
        .find(|synonym| !synonym.is_empty())
        .unwrap_or(disease.id.trim())
        .to_string()
}

pub(crate) fn related_pgx(pgx: &Pgx) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(gene) = pgx.gene.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        out.push(format!("biomcp search pgx -g {gene}"));
    }
    if let Some(drug) = pgx.drug.as_deref().map(quote_arg).filter(|v| !v.is_empty()) {
        out.push(format!("biomcp search pgx -d {drug}"));
    }
    out
}

pub(crate) fn related_pathway(pathway: &Pathway) -> Vec<String> {
    let id = quote_arg(&pathway.id);
    if id.is_empty() {
        return Vec::new();
    }

    vec![format!("biomcp pathway drugs {id}")]
}

pub(crate) fn related_protein(protein: &Protein, requested_sections: &[String]) -> Vec<String> {
    let accession = quote_arg(&protein.accession);
    let requested = requested_section_names(requested_sections);
    let requested_section = |name: &str| requested.iter().any(|value| value == name);
    let mut out = Vec::new();
    if !accession.is_empty() {
        if !requested_section("structures") {
            out.push(format!("biomcp get protein {accession} structures"));
        }
        if !requested_section("complexes") {
            out.push(format!("biomcp get protein {accession} complexes"));
        }
    }
    if let Some(symbol) = protein
        .gene_symbol
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        out.push(format!("biomcp get gene {symbol}"));
    }
    out
}

fn format_protein_complex_component(component: &ProteinComplexComponent) -> String {
    let accession = component.accession.trim();
    let name = component.name.trim();
    let label = if name.is_empty() { accession } else { name };
    let stoichiometry = component
        .stoichiometry
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    match stoichiometry {
        Some(stoichiometry) => format!("{label} ({stoichiometry})"),
        None => label.to_string(),
    }
}

fn protein_complex_summary_rows(complexes: &[ProteinComplex]) -> Vec<ProteinComplexSummaryRow> {
    complexes
        .iter()
        .map(|complex| ProteinComplexSummaryRow {
            accession: markdown_cell(&complex.accession),
            name: markdown_cell(&complex.name),
            component_count: complex.components.len(),
            curation: match &complex.curation {
                ProteinComplexCuration::Curated => "curated".to_string(),
                ProteinComplexCuration::Predicted => "predicted".to_string(),
            },
        })
        .collect()
}

fn protein_complex_detail_rows(complexes: &[ProteinComplex]) -> Vec<ProteinComplexDetailRow> {
    complexes
        .iter()
        .map(|complex| {
            let component_count = complex.components.len();
            let preview_components = complex
                .components
                .iter()
                .take(5)
                .map(format_protein_complex_component)
                .map(|component| markdown_cell(&component))
                .collect::<Vec<_>>();
            ProteinComplexDetailRow {
                accession: markdown_cell(&complex.accession),
                component_count,
                component_preview: if preview_components.is_empty() {
                    "none listed".to_string()
                } else {
                    preview_components.join(", ")
                },
                remaining_count: component_count.saturating_sub(preview_components.len()),
                description: complex
                    .description
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(markdown_cell),
            }
        })
        .collect()
}

pub(crate) fn related_drug(drug: &Drug) -> Vec<String> {
    let name = quote_arg(&drug.name);
    if name.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();

    let sparse_regulatory = drug.label.is_none()
        && drug
            .approvals
            .as_ref()
            .map(|rows| rows.is_empty())
            .unwrap_or(true)
        && drug.ema_regulatory.is_none();
    let sparse_indications = drug.indications.is_empty();

    if sparse_regulatory || sparse_indications {
        out.push(format!(
            "biomcp search article --drug {name} --type review --limit 5"
        ));
    }

    out.push(format!("biomcp drug trials {name}"));
    out.push(format!("biomcp drug adverse-events {name}"));
    out.push(format!("biomcp search pgx -d {name}"));

    if let Some(target) = drug.targets.first().map(String::as_str) {
        let sym = target.trim();
        if !sym.is_empty() {
            out.push(format!("biomcp get gene {sym}"));
        }
    }

    dedupe_markdown_commands(out)
}

pub(crate) fn related_adverse_event(event: &AdverseEvent) -> Vec<String> {
    let drug = quote_arg(&event.drug);
    if drug.is_empty() {
        return Vec::new();
    }
    vec![
        format!("biomcp get drug {drug}"),
        format!("biomcp drug adverse-events {drug}"),
        format!("biomcp drug trials {drug}"),
    ]
}

pub(crate) fn related_device_event(event: &DeviceEvent) -> Vec<String> {
    let device = quote_arg(&event.device);
    if device.is_empty() {
        return Vec::new();
    }
    vec![
        format!("biomcp search adverse-event --type device --device {device}"),
        "biomcp search adverse-event --type recall --classification \"Class I\"".to_string(),
    ]
}

pub fn gene_markdown(gene: &Gene, requested_sections: &[String]) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("gene.md.j2")?;
    let section_only = is_section_only_requested(requested_sections);
    let include_all = has_all_section(requested_sections);
    let requested = requested_section_names(requested_sections);
    let has_requested = |name: &str| requested.iter().any(|s| s.eq_ignore_ascii_case(name));
    let show_civic_section = include_all || has_requested("civic");
    let show_expression_section = include_all || has_requested("expression");
    let show_hpa_section = include_all || has_requested("hpa");
    let show_druggability_section =
        include_all || has_requested("druggability") || has_requested("drugs");
    let show_clingen_section = include_all || has_requested("clingen");
    let show_constraint_section = include_all || has_requested("constraint");
    let show_disgenet_section = has_requested("disgenet");
    let show_funding_section = has_requested("funding");
    let funding_rows = funding_rows(gene.funding.as_ref());
    let funding_summary = funding_summary_line(gene.funding.as_ref());
    let body = tmpl.render(context! {
        section_only => section_only,
        section_header => section_header(&gene.symbol, requested_sections),
        symbol => &gene.symbol,
        name => &gene.name,
        entrez_id => &gene.entrez_id,
        ensembl_id => &gene.ensembl_id,
        location => &gene.location,
        genomic_coordinates => &gene.genomic_coordinates,
        omim_id => &gene.omim_id,
        uniprot_id => &gene.uniprot_id,
        summary => &gene.summary,
        gene_type => &gene.gene_type,
        aliases => &gene.aliases,
        clinical_diseases => &gene.clinical_diseases,
        clinical_drugs => &gene.clinical_drugs,
        pathways => &gene.pathways,
        ontology => &gene.ontology,
        diseases => &gene.diseases,
        protein => &gene.protein,
        go_terms => &gene.go,
        interactions => &gene.interactions,
        civic => &gene.civic,
        expression => &gene.expression,
        hpa => &gene.hpa,
        druggability => &gene.druggability,
        clingen => &gene.clingen,
        constraint => &gene.constraint,
        disgenet => &gene.disgenet,
        funding => &gene.funding,
        funding_note => &gene.funding_note,
        funding_rows => funding_rows,
        funding_summary => funding_summary,
        show_civic_section => show_civic_section,
        show_expression_section => show_expression_section,
        show_hpa_section => show_hpa_section,
        show_druggability_section => show_druggability_section,
        show_clingen_section => show_clingen_section,
        show_constraint_section => show_constraint_section,
        show_disgenet_section => show_disgenet_section,
        show_funding_section => show_funding_section,
        sections_block => format_sections_block("gene", &gene.symbol, sections_gene(gene, requested_sections)),
        related_block => format_related_block(related_gene(gene)),
    })?;
    Ok(append_evidence_urls(body, gene_evidence_urls(gene)))
}

#[allow(dead_code)]
pub fn gene_search_markdown(
    query: &str,
    results: &[GeneSearchResult],
) -> Result<String, BioMcpError> {
    gene_search_markdown_with_footer(query, results, "")
}

pub fn gene_search_markdown_with_footer(
    query: &str,
    results: &[GeneSearchResult],
    pagination_footer: &str,
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("gene_search.md.j2")?;
    let body = tmpl.render(context! {
        query => query,
        count => results.len(),
        results => results,
        pagination_footer => pagination_footer,
    })?;
    Ok(with_pagination_footer(body, pagination_footer))
}

pub fn article_markdown(
    article: &Article,
    requested_sections: &[String],
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("article.md.j2")?;
    let section_only = is_section_only_requested(requested_sections);
    let include_all = has_all_section(requested_sections);
    let requested = requested_section_names(requested_sections);
    let has_requested = |name: &str| requested.iter().any(|s| s.eq_ignore_ascii_case(name));
    let show_annotations_section = include_all || has_requested("annotations");
    let show_fulltext_section = include_all || has_requested("fulltext");
    let show_semantic_scholar_section = !section_only || include_all || has_requested("tldr");
    let article_label = if article.title.trim().is_empty() {
        "Article"
    } else {
        article.title.trim()
    };
    let body = tmpl.render(context! {
        section_only => section_only,
        section_header => section_header(article_label, requested_sections),
        pmid => &article.pmid,
        pmcid => &article.pmcid,
        doi => &article.doi,
        title => &article.title,
        authors => &article.authors,
        journal => &article.journal,
        date => &article.date,
        citation_count => &article.citation_count,
        publication_type => &article.publication_type,
        open_access => &article.open_access,
        abstract_text => &article.abstract_text,
        full_text_path => &article.full_text_path,
        full_text_note => &article.full_text_note,
        annotations => &article.annotations,
        semantic_scholar => &article.semantic_scholar,
        pubtator_fallback => article.pubtator_fallback,
        show_annotations_section => show_annotations_section,
        show_fulltext_section => show_fulltext_section,
        show_semantic_scholar_section => show_semantic_scholar_section,
        sections_block => format_sections_block("article", article.pmid.as_deref().or(article.pmcid.as_deref()).or(article.doi.as_deref()).unwrap_or(""), sections_article(article, requested_sections)),
        related_block => format_related_block(related_article(article)),
    })?;
    Ok(append_evidence_urls(body, article_evidence_urls(article)))
}

pub fn article_entities_markdown(
    pmid: &str,
    annotations: Option<&ArticleAnnotations>,
    limit: Option<usize>,
) -> Result<String, BioMcpError> {
    #[derive(serde::Serialize)]
    struct EntityRow {
        text: String,
        count: u32,
        command: String,
    }

    fn row(text: &str, count: u32, command: String) -> EntityRow {
        EntityRow {
            text: text.to_string(),
            count,
            command,
        }
    }

    let (mut genes, mut diseases, mut chemicals, mut mutations) = if let Some(ann) = annotations {
        (
            ann.genes
                .iter()
                .filter_map(|g| {
                    let text = g.text.trim();
                    let command = article_annotation_command(ArticleAnnotationBucket::Gene, text)?;
                    Some(row(text, g.count, command))
                })
                .collect::<Vec<_>>(),
            ann.diseases
                .iter()
                .filter_map(|d| {
                    let text = d.text.trim();
                    let command =
                        article_annotation_command(ArticleAnnotationBucket::Disease, text)?;
                    Some(row(text, d.count, command))
                })
                .collect::<Vec<_>>(),
            ann.chemicals
                .iter()
                .filter_map(|c| {
                    let text = c.text.trim();
                    let command =
                        article_annotation_command(ArticleAnnotationBucket::Chemical, text)?;
                    Some(row(text, c.count, command))
                })
                .collect::<Vec<_>>(),
            ann.mutations
                .iter()
                .filter_map(|m| {
                    let text = m.text.trim();
                    let command =
                        article_annotation_command(ArticleAnnotationBucket::Mutation, text)?;
                    Some(row(text, m.count, command))
                })
                .collect::<Vec<_>>(),
        )
    } else {
        (Vec::new(), Vec::new(), Vec::new(), Vec::new())
    };

    if let Some(limit) = limit {
        genes.truncate(limit);
        diseases.truncate(limit);
        chemicals.truncate(limit);
        mutations.truncate(limit);
    }

    let tmpl = env()?.get_template("article_entities.md.j2")?;
    Ok(tmpl.render(context! {
        pmid => pmid,
        genes => genes,
        diseases => diseases,
        chemicals => chemicals,
        mutations => mutations,
    })?)
}

fn article_batch_counts(label: &str, rows: &[AnnotationCount]) -> Option<String> {
    if rows.is_empty() {
        return None;
    }
    Some(format!(
        "{label}: {}",
        rows.iter()
            .map(|row| format!("{} ({})", row.text.trim(), row.count))
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

fn article_batch_entities(summary: Option<&ArticleBatchEntitySummary>) -> Option<String> {
    let summary = summary?;
    let parts = [
        article_batch_counts("Genes", &summary.genes),
        article_batch_counts("Diseases", &summary.diseases),
        article_batch_counts("Chemicals", &summary.chemicals),
        article_batch_counts("Mutations", &summary.mutations),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("; "))
    }
}

pub fn article_batch_markdown(items: &[ArticleBatchItem]) -> Result<String, BioMcpError> {
    let mut out = format!("# Article Batch ({})\n\n", items.len());
    for (idx, item) in items.iter().enumerate() {
        out.push_str(&format!("## {}. {}\n", idx + 1, item.title.trim()));
        if let Some(pmid) = &item.pmid {
            out.push_str(&format!("PMID: {}\n", pmid.trim()));
        } else if let Some(pmcid) = &item.pmcid {
            out.push_str(&format!("PMCID: {}\n", pmcid.trim()));
        } else if let Some(doi) = &item.doi {
            out.push_str(&format!("DOI: {}\n", doi.trim()));
        }
        if let Some(journal) = &item.journal {
            out.push_str(&format!("Journal: {}\n", journal.trim()));
        }
        if let Some(year) = item.year {
            out.push_str(&format!("Year: {}\n", year));
        }
        if let Some(entities) = article_batch_entities(item.entity_summary.as_ref()) {
            out.push_str(&format!("Entities: {}\n", entities));
        }
        if let Some(tldr) = &item.tldr {
            out.push_str(&format!("TLDR: {}\n", tldr.trim()));
        }
        match (item.citation_count, item.influential_citation_count) {
            (Some(c), Some(ic)) => out.push_str(&format!("Citations: {c} (influential: {ic})\n")),
            (Some(c), None) => out.push_str(&format!("Citations: {c}\n")),
            (None, Some(ic)) => out.push_str(&format!("Citations: influential {ic}\n")),
            (None, None) => {}
        }
        out.push('\n');
    }
    Ok(out)
}

pub fn article_graph_markdown(
    kind: &str,
    result: &ArticleGraphResult,
) -> Result<String, BioMcpError> {
    let mut out = format!(
        "# {} for {}\n\n",
        markdown_cell(kind),
        markdown_cell(&article_related_label(&result.article))
    );
    out.push_str("| PMID | Title | Intents | Influential | Context |\n");
    out.push_str("| --- | --- | --- | --- | --- |\n");
    if result.edges.is_empty() {
        out.push_str("| - | - | - | - | No related papers returned |\n");
        return Ok(out);
    }
    for edge in &result.edges {
        let intents = if edge.intents.is_empty() {
            "-".to_string()
        } else {
            markdown_cell(&edge.intents.join(", "))
        };
        let context = edge
            .contexts
            .first()
            .map(|value| markdown_cell(value))
            .unwrap_or_else(|| "-".to_string());
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            article_related_id(&edge.paper),
            markdown_cell(&edge.paper.title),
            intents,
            if edge.is_influential { "yes" } else { "no" },
            context,
        ));
    }
    Ok(out)
}

pub fn article_recommendations_markdown(
    result: &ArticleRecommendationsResult,
) -> Result<String, BioMcpError> {
    let positives = if result.positive_seeds.is_empty() {
        "article".to_string()
    } else {
        result
            .positive_seeds
            .iter()
            .map(article_related_label)
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut out = format!("# Recommendations for {}\n\n", markdown_cell(&positives));
    if !result.negative_seeds.is_empty() {
        let negatives = result
            .negative_seeds
            .iter()
            .map(article_related_label)
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "Negative seeds: {}\n\n",
            markdown_cell(&negatives)
        ));
    }
    out.push_str("| PMID | Title | Journal | Year |\n");
    out.push_str("| --- | --- | --- | --- |\n");
    if result.recommendations.is_empty() {
        out.push_str("| - | No recommendations returned | - | - |\n");
        return Ok(out);
    }
    for paper in &result.recommendations {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            article_related_id(paper),
            markdown_cell(&paper.title),
            paper
                .journal
                .as_deref()
                .map(markdown_cell)
                .unwrap_or_else(|| "-".to_string()),
            paper
                .year
                .map(|year| year.to_string())
                .unwrap_or_else(|| "-".to_string()),
        ));
    }
    Ok(out)
}

fn article_sources_label(row: &ArticleSearchResult) -> String {
    let mut sources = if row.matched_sources.is_empty() {
        vec![row.source]
    } else {
        row.matched_sources.clone()
    };
    sources.dedup();
    sources
        .into_iter()
        .map(ArticleSource::display_name)
        .collect::<Vec<_>>()
        .join(", ")
}

fn article_lexical_ranking_label(ranking: &ArticleRankingMetadata) -> Option<String> {
    if ranking.anchor_count == 0 {
        return None;
    }
    if ranking.all_anchors_in_title {
        return Some(format!(
            "title {}/{}",
            ranking.title_anchor_hits, ranking.anchor_count
        ));
    }
    if ranking.all_anchors_in_text {
        return Some(format!(
            "title+abstract {}/{}",
            ranking.combined_anchor_hits, ranking.anchor_count
        ));
    }
    if ranking.abstract_anchor_hits > 0 && ranking.title_anchor_hits > 0 {
        return Some(format!(
            "title+abstract {}/{}",
            ranking.combined_anchor_hits, ranking.anchor_count
        ));
    }
    if ranking.abstract_anchor_hits > 0 {
        return Some(format!(
            "abstract {}/{}",
            ranking.abstract_anchor_hits, ranking.anchor_count
        ));
    }
    if ranking.title_anchor_hits > 0 {
        return Some(format!(
            "title {}/{}",
            ranking.title_anchor_hits, ranking.anchor_count
        ));
    }
    None
}

fn article_lexical_reason(ranking: &ArticleRankingMetadata) -> Option<String> {
    let lexical_label = article_lexical_ranking_label(ranking);
    if ranking.pubmed_rescue {
        return Some(lexical_label.map_or_else(
            || "pubmed-rescue".to_string(),
            |label| format!("pubmed-rescue + {label}"),
        ));
    }
    lexical_label
}

fn format_article_score(value: f64) -> String {
    let mut out = format!("{value:.3}");
    while out.contains('.') && out.ends_with('0') {
        out.pop();
    }
    if out.ends_with('.') {
        out.pop();
    }
    if out == "-0" { "0".to_string() } else { out }
}

fn article_ranking_why(row: &ArticleSearchResult, filters: &ArticleSearchFilters) -> String {
    if filters.sort != ArticleSort::Relevance {
        return "-".to_string();
    }
    let Some(ranking) = row.ranking.as_ref() else {
        return "-".to_string();
    };
    let lexical_label = article_lexical_ranking_label(ranking);
    match ranking
        .mode
        .or_else(|| crate::entities::article::article_effective_ranking_mode(filters))
        .unwrap_or(ArticleRankingMode::Lexical)
    {
        ArticleRankingMode::Lexical => {
            article_lexical_reason(ranking).unwrap_or_else(|| "-".to_string())
        }
        ArticleRankingMode::Semantic => {
            let mut why = format!(
                "semantic {}",
                format_article_score(ranking.semantic_score.unwrap_or(0.0))
            );
            if let Some(label) = lexical_label {
                why.push_str(" + ");
                why.push_str(&label);
            }
            why
        }
        ArticleRankingMode::Hybrid => {
            let mut why = format!(
                "hybrid {}",
                format_article_score(ranking.composite_score.unwrap_or(0.0))
            );
            if let Some(label) = lexical_label {
                why.push_str(" + ");
                why.push_str(&label);
            }
            why
        }
    }
}

pub fn article_search_markdown_with_footer_and_context(
    query: &str,
    results: &[ArticleSearchResult],
    pagination_footer: &str,
    filters: &ArticleSearchFilters,
    semantic_scholar_enabled: bool,
    note: Option<&str>,
    debug_plan: Option<&DebugPlan>,
) -> Result<String, BioMcpError> {
    let rows = results
        .iter()
        .map(|row| ArticleSearchRenderRow {
            pmid: row.pmid.clone(),
            title: row.title.clone(),
            sources: article_sources_label(row),
            date: row.date.clone(),
            why: article_ranking_why(row, filters),
            citation_count: row.citation_count,
            is_retracted: row.is_retracted,
        })
        .collect::<Vec<_>>();

    let tmpl = env()?.get_template("article_search.md.j2")?;
    let body = tmpl.render(context! {
        query => query,
        count => results.len(),
        rows => rows,
        semantic_scholar_enabled => semantic_scholar_enabled,
        note => note,
        sort => filters.sort.as_str(),
        ranking_policy => crate::entities::article::article_relevance_ranking_policy(filters),
        pagination_footer => pagination_footer,
    })?;
    let body = with_pagination_footer(body, pagination_footer);
    if let Some(debug_plan) = debug_plan {
        Ok(format!("{}{}", render_debug_plan_block(debug_plan)?, body))
    } else {
        Ok(body)
    }
}

fn render_debug_plan_block(debug_plan: &DebugPlan) -> Result<String, BioMcpError> {
    Ok(format!(
        "## Debug plan\n\n```json\n{}\n```\n\n",
        serde_json::to_string_pretty(debug_plan)?
    ))
}

pub fn disease_markdown(
    disease: &Disease,
    requested_sections: &[String],
) -> Result<String, BioMcpError> {
    let mut xrefs: Vec<XrefRow> = disease
        .xrefs
        .iter()
        .map(|(k, v)| XrefRow {
            source: k.clone(),
            id: v.clone(),
        })
        .collect();
    xrefs.sort_by(|a, b| a.source.cmp(&b.source));

    let section_only = is_section_only_requested(requested_sections);
    let include_all = has_all_section(requested_sections);
    let requested = requested_section_names(requested_sections);
    let has_requested = |name: &str| requested.iter().any(|s| s.eq_ignore_ascii_case(name));
    let show_genes_section = include_all || has_requested("genes");
    let show_pathways_section = include_all || has_requested("pathways");
    let show_phenotypes_section = include_all || has_requested("phenotypes");
    let show_variants_section = include_all || has_requested("variants");
    let show_models_section = include_all || has_requested("models");
    let show_prevalence_section = include_all || has_requested("prevalence");
    let show_survival_section = include_all || has_requested("survival");
    let show_funding_section = has_requested("funding");
    let show_civic_section = include_all || has_requested("civic");
    let show_disgenet_section = has_requested("disgenet");
    let disease_label = if disease.name.trim().is_empty() {
        disease.id.as_str()
    } else {
        disease.name.as_str()
    };

    let tmpl = env()?.get_template("disease.md.j2")?;
    let top_gene_score_labels = disease_top_gene_score_labels(disease);
    let gene_association_rows = disease_gene_association_rows(disease);
    let phenotype_rows = disease_phenotype_rows(disease);
    let model_rows = disease_model_rows(disease);
    let survival_source_line = disease_survival_source_line(disease);
    let survival_summary_rows = disease_survival_summary_rows(disease);
    let survival_history_rows = disease_survival_history_rows(disease);
    let funding_rows = funding_rows(disease.funding.as_ref());
    let funding_summary = funding_summary_line(disease.funding.as_ref());
    let body = tmpl.render(context! {
        section_only => section_only,
        section_header => section_header(disease_label, requested_sections),
        id => &disease.id,
        name => &disease.name,
        definition => &disease.definition,
        synonyms => &disease.synonyms,
        parents => &disease.parents,
        associated_genes => &disease.associated_genes,
        gene_associations => &disease.gene_associations,
        gene_association_rows => gene_association_rows,
        top_genes => &disease.top_genes,
        top_gene_scores => &disease.top_gene_scores,
        top_gene_score_labels => top_gene_score_labels,
        treatment_landscape => &disease.treatment_landscape,
        recruiting_trial_count => &disease.recruiting_trial_count,
        pathways => &disease.pathways,
        phenotypes => phenotype_rows,
        key_features => &disease.key_features,
        has_definition => disease.definition.is_some(),
        literature_query => disease_literature_query(disease),
        variants => &disease.variants,
        top_variant => &disease.top_variant,
        models => model_rows,
        prevalence => &disease.prevalence,
        prevalence_note => &disease.prevalence_note,
        survival => &disease.survival,
        survival_note => &disease.survival_note,
        funding => &disease.funding,
        funding_note => &disease.funding_note,
        funding_rows => funding_rows,
        funding_summary => funding_summary,
        survival_source_line => survival_source_line,
        survival_summary_rows => survival_summary_rows,
        survival_history_rows => survival_history_rows,
        civic => &disease.civic,
        disgenet => &disease.disgenet,
        show_genes_section => show_genes_section,
        show_pathways_section => show_pathways_section,
        show_phenotypes_section => show_phenotypes_section,
        show_variants_section => show_variants_section,
        show_models_section => show_models_section,
        show_prevalence_section => show_prevalence_section,
        show_survival_section => show_survival_section,
        show_funding_section => show_funding_section,
        show_civic_section => show_civic_section,
        show_disgenet_section => show_disgenet_section,
        xrefs => xrefs,
        sections_block => format_sections_block("disease", &disease.id, sections_disease(disease, requested_sections)),
        related_block => format_related_block(related_disease(disease)),
    })?;
    Ok(append_evidence_urls(body, disease_evidence_urls(disease)))
}

#[allow(dead_code)]
pub fn disease_search_markdown(
    query: &str,
    results: &[DiseaseSearchResult],
) -> Result<String, BioMcpError> {
    disease_search_markdown_with_footer(query, query, results, false, "")
}

pub fn disease_search_markdown_with_footer(
    raw_query: &str,
    query_summary: &str,
    results: &[DiseaseSearchResult],
    fallback_used: bool,
    pagination_footer: &str,
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("disease_search.md.j2")?;
    let discover_hint = discover_try_line(raw_query, "resolve abbreviations and synonyms");
    let body = tmpl.render(context! {
        query => query_summary,
        count => results.len(),
        results => results,
        fallback_used => fallback_used,
        discover_hint => discover_hint,
        pagination_footer => pagination_footer,
    })?;
    Ok(with_pagination_footer(body, pagination_footer))
}

pub fn pgx_markdown(pgx: &Pgx, requested_sections: &[String]) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("pgx.md.j2")?;
    let section_only = is_section_only_requested(requested_sections);
    let include_all = has_all_section(requested_sections);
    let requested = requested_section_names(requested_sections);
    let has_requested = |name: &str| requested.iter().any(|s| s.eq_ignore_ascii_case(name));
    let show_recommendations_section = include_all || has_requested("recommendations");
    let show_frequencies_section = include_all || has_requested("frequencies");
    let show_guidelines_section = include_all || has_requested("guidelines");
    let show_annotations_section = include_all || has_requested("annotations");
    let label = pgx
        .gene
        .as_deref()
        .or(pgx.drug.as_deref())
        .unwrap_or(pgx.query.as_str());

    let body = tmpl.render(context! {
        section_only => section_only,
        section_header => section_header(label, requested_sections),
        query => &pgx.query,
        gene => &pgx.gene,
        drug => &pgx.drug,
        interactions => &pgx.interactions,
        recommendations => &pgx.recommendations,
        frequencies => &pgx.frequencies,
        guidelines => &pgx.guidelines,
        annotations => &pgx.annotations,
        annotations_note => &pgx.annotations_note,
        show_recommendations_section => show_recommendations_section,
        show_frequencies_section => show_frequencies_section,
        show_guidelines_section => show_guidelines_section,
        show_annotations_section => show_annotations_section,
        sections_block => format_sections_block("pgx", &pgx.query, sections_pgx(pgx, requested_sections)),
        related_block => format_related_block(related_pgx(pgx)),
    })?;
    Ok(append_evidence_urls(body, pgx_evidence_urls(pgx)))
}

#[allow(dead_code)]
pub fn pgx_search_markdown(
    query: &str,
    results: &[PgxSearchResult],
) -> Result<String, BioMcpError> {
    pgx_search_markdown_with_footer(query, results, "")
}

pub fn pgx_search_markdown_with_footer(
    query: &str,
    results: &[PgxSearchResult],
    pagination_footer: &str,
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("pgx_search.md.j2")?;
    let body = tmpl.render(context! {
        query => query,
        count => results.len(),
        results => results,
        pagination_footer => pagination_footer,
    })?;
    Ok(with_pagination_footer(body, pagination_footer))
}

pub fn trial_markdown(trial: &Trial, requested_sections: &[String]) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("trial.md.j2")?;
    let section_only = is_section_only_requested(requested_sections);
    let include_all = has_all_section(requested_sections);
    let requested = requested_section_names(requested_sections);
    let show_eligibility_section = include_all
        || requested
            .iter()
            .any(|s| s.eq_ignore_ascii_case("eligibility"));
    let show_locations_section = include_all
        || requested
            .iter()
            .any(|s| s.eq_ignore_ascii_case("locations"));
    let show_outcomes_section =
        include_all || requested.iter().any(|s| s.eq_ignore_ascii_case("outcomes"));
    let show_arms_section = include_all || requested.iter().any(|s| s.eq_ignore_ascii_case("arms"));
    let show_references_section = include_all
        || requested
            .iter()
            .any(|s| s.eq_ignore_ascii_case("references"));
    let body = tmpl.render(context! {
        section_only => section_only,
        section_header => section_header(&trial.nct_id, requested_sections),
        trial_source_label => crate::render::provenance::trial_source_label(trial.source.as_deref()),
        nct_id => &trial.nct_id,
        title => &trial.title,
        status => &trial.status,
        phase => &trial.phase,
        study_type => &trial.study_type,
        age_range => &trial.age_range,
        conditions => &trial.conditions,
        interventions => &trial.interventions,
        sponsor => &trial.sponsor,
        enrollment => &trial.enrollment,
        summary => &trial.summary,
        start_date => &trial.start_date,
        completion_date => &trial.completion_date,
        eligibility_text => &trial.eligibility_text,
        locations => &trial.locations,
        outcomes => &trial.outcomes,
        arms => &trial.arms,
        references => &trial.references,
        show_eligibility_section => show_eligibility_section,
        show_locations_section => show_locations_section,
        show_outcomes_section => show_outcomes_section,
        show_arms_section => show_arms_section,
        show_references_section => show_references_section,
        sections_block => format_sections_block("trial", &trial.nct_id, sections_trial(trial, requested_sections)),
        related_block => format_related_block(related_trial(trial)),
    })?;
    Ok(append_evidence_urls(body, trial_evidence_urls(trial)))
}

pub fn trial_search_markdown(
    query: &str,
    results: &[TrialSearchResult],
    total: Option<u32>,
) -> Result<String, BioMcpError> {
    trial_search_markdown_with_footer(query, results, total, "", false, None)
}

pub fn trial_search_markdown_with_footer(
    query: &str,
    results: &[TrialSearchResult],
    total: Option<u32>,
    pagination_footer: &str,
    show_zero_result_nickname_hint: bool,
    nickname_query: Option<&str>,
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("trial_search.md.j2")?;
    let body = tmpl.render(context! {
        query => query,
        count => results.len(),
        total => total,
        results => results,
        pagination_footer => pagination_footer,
        show_zero_result_nickname_hint => show_zero_result_nickname_hint,
        nickname_query => nickname_query,
    })?;
    Ok(with_pagination_footer(body, pagination_footer))
}

pub fn variant_markdown(
    variant: &Variant,
    requested_sections: &[String],
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("variant.md.j2")?;
    let section_only = is_section_only_requested(requested_sections);
    let include_all = has_all_section(requested_sections);
    let requested = requested_section_names(requested_sections);
    let has_requested = |name: &str| requested.iter().any(|s| s.eq_ignore_ascii_case(name));
    let show_prediction_section = !section_only || include_all || has_requested("predict");
    let show_predictions_section = include_all || has_requested("predictions");
    let show_clinvar_section = !section_only || include_all || has_requested("clinvar");
    let show_population_section = !section_only || include_all || has_requested("population");
    let show_conservation_section = include_all || has_requested("conservation");
    let show_cosmic_section = include_all || has_requested("cosmic");
    let show_cgi_section = include_all || has_requested("cgi");
    let show_civic_section = include_all || has_requested("civic");
    let show_cbioportal_section = include_all || has_requested("cbioportal");
    let show_gwas_section = include_all || has_requested("gwas");
    let variant_label = if !variant.gene.trim().is_empty() && variant.hgvs_p.is_some() {
        format!(
            "{} {}",
            variant.gene.trim(),
            variant.hgvs_p.as_deref().unwrap_or_default().trim()
        )
    } else if !variant.gene.trim().is_empty() {
        variant.gene.trim().to_string()
    } else {
        variant.id.trim().to_string()
    };
    let prediction = variant.prediction.as_ref();
    let (expr_i, splice_i, chrom_i) = prediction
        .map(prediction_interpretations)
        .unwrap_or((None, None, None));
    let body = tmpl.render(context! {
        section_only => section_only,
        section_header => section_header(&variant_label, requested_sections),
        id => &variant.id,
        gene => &variant.gene,
        hgvs_p => &variant.hgvs_p,
        legacy_name => &variant.legacy_name,
        hgvs_c => &variant.hgvs_c,
        consequence => &variant.consequence,
        rsid => &variant.rsid,
        cosmic_id => &variant.cosmic_id,
        significance => &variant.significance,
        clinvar_id => &variant.clinvar_id,
        clinvar_review_status => &variant.clinvar_review_status,
        clinvar_review_stars => &variant.clinvar_review_stars,
        conditions => &variant.conditions,
        clinvar_conditions => &variant.clinvar_conditions,
        clinvar_condition_reports => &variant.clinvar_condition_reports,
        top_disease => &variant.top_disease,
        gnomad_af => &variant.gnomad_af,
        allele_frequency_percent => &variant.allele_frequency_percent,
        population_breakdown => &variant.population_breakdown,
        cadd_score => &variant.cadd_score,
        sift_pred => &variant.sift_pred,
        polyphen_pred => &variant.polyphen_pred,
        conservation => &variant.conservation,
        expanded_predictions => &variant.expanded_predictions,
        cosmic_context => &variant.cosmic_context,
        cgi_associations => &variant.cgi_associations,
        civic => &variant.civic,
        cancer_frequencies => &variant.cancer_frequencies,
        cancer_frequency_source => &variant.cancer_frequency_source,
        gwas => &variant.gwas,
        gwas_unavailable_reason => &variant.gwas_unavailable_reason,
        prediction => prediction,
        expression_interpretation => expr_i,
        splice_interpretation => splice_i,
        chromatin_interpretation => chrom_i,
        show_prediction_section => show_prediction_section,
        show_predictions_section => show_predictions_section,
        show_clinvar_section => show_clinvar_section,
        show_population_section => show_population_section,
        show_conservation_section => show_conservation_section,
        show_cosmic_section => show_cosmic_section,
        show_cgi_section => show_cgi_section,
        show_civic_section => show_civic_section,
        show_cbioportal_section => show_cbioportal_section,
        show_gwas_section => show_gwas_section,
        sections_block => format_sections_block("variant", &variant.id, sections_variant(variant, requested_sections)),
        related_block => format_related_block(related_variant(variant)),
    })?;
    Ok(append_evidence_urls(body, variant_evidence_urls(variant)))
}

fn prediction_interpretations(
    pred: &VariantPrediction,
) -> (
    Option<&'static str>,
    Option<&'static str>,
    Option<&'static str>,
) {
    let expr = pred.expression_lfc.map(|v| {
        if v > 0.2 {
            "Increased expression"
        } else if v < -0.2 {
            "Decreased expression"
        } else {
            "Minimal change"
        }
    });

    let splice = pred.splice_score.map(|v| {
        if v.abs() > 0.5 {
            "Higher splice impact"
        } else {
            "Low splice impact"
        }
    });

    let chrom = pred.chromatin_score.map(|v| {
        if v.abs() > 0.5 {
            "Altered accessibility"
        } else {
            "Low chromatin impact"
        }
    });

    (expr, splice, chrom)
}

#[allow(dead_code)]
pub fn variant_search_markdown(
    query: &str,
    results: &[VariantSearchResult],
) -> Result<String, BioMcpError> {
    variant_search_markdown_with_footer(query, results, "")
}

pub fn variant_search_markdown_with_footer(
    query: &str,
    results: &[VariantSearchResult],
    pagination_footer: &str,
) -> Result<String, BioMcpError> {
    variant_search_markdown_with_context(query, results, pagination_footer, None, None)
}

pub fn variant_search_markdown_with_context(
    query: &str,
    results: &[VariantSearchResult],
    pagination_footer: &str,
    gene_filter: Option<&str>,
    condition_filter: Option<&str>,
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("variant_search.md.j2")?;
    let body = tmpl.render(context! {
        query => query,
        count => results.len(),
        results => results,
        related_block => format_related_block(related_variant_search_results(
            results,
            gene_filter,
            condition_filter,
        )),
        pagination_footer => pagination_footer,
    })?;
    Ok(with_pagination_footer(body, pagination_footer))
}

#[allow(dead_code)]
pub fn phenotype_search_markdown(
    query: &str,
    results: &[PhenotypeSearchResult],
) -> Result<String, BioMcpError> {
    phenotype_search_markdown_with_footer(query, results, "")
}

pub fn phenotype_search_markdown_with_footer(
    query: &str,
    results: &[PhenotypeSearchResult],
    pagination_footer: &str,
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("phenotype_search.md.j2")?;
    let body = tmpl.render(context! {
        query => query,
        count => results.len(),
        results => results,
        related_block => format_related_block(related_phenotype_search_results(results)),
        pagination_footer => pagination_footer,
    })?;
    Ok(with_pagination_footer(body, pagination_footer))
}

#[allow(dead_code)]
pub fn gwas_search_markdown(
    query: &str,
    results: &[VariantGwasAssociation],
) -> Result<String, BioMcpError> {
    gwas_search_markdown_with_footer(query, results, "")
}

pub fn gwas_search_markdown_with_footer(
    query: &str,
    results: &[VariantGwasAssociation],
    pagination_footer: &str,
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("gwas_search.md.j2")?;
    let body = tmpl.render(context! {
        query => query,
        count => results.len(),
        results => results,
        pagination_footer => pagination_footer,
    })?;
    Ok(with_pagination_footer(body, pagination_footer))
}

pub fn variant_oncokb_markdown(result: &VariantOncoKbResult) -> String {
    let mut out = String::new();
    out.push_str("# OncoKB\n\n");
    out.push_str(&format!("Gene: {}\n", result.gene.trim()));
    out.push_str(&format!("Alteration: {}\n", result.alteration.trim()));
    if let Some(level) = result
        .level
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        out.push_str(&format!("Level: {level}\n"));
    }
    if let Some(oncogenic) = result
        .oncogenic
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        out.push_str(&format!("Oncogenic: {oncogenic}\n"));
    }
    if let Some(effect) = result
        .effect
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        out.push_str(&format!("Effect: {effect}\n"));
    }
    out.push('\n');

    if result.therapies.is_empty() {
        out.push_str("No therapy implications returned by OncoKB.\n");
    } else {
        out.push_str("## Therapies\n\n");
        out.push_str("| Drug | Level | Cancer Type | Note |\n");
        out.push_str("|------|-------|-------------|------|\n");
        for row in &result.therapies {
            let drugs = if row.drugs.is_empty() {
                "unspecified".to_string()
            } else {
                row.drugs.join(" + ")
            };
            let cancer = row.cancer_type.as_deref().unwrap_or("-");
            let note = row.note.as_deref().unwrap_or("-");
            out.push_str(&format!(
                "| {drugs} | {} | {cancer} | {note} |\n",
                row.level
            ));
        }
    }

    if !result.gene.trim().is_empty() && !result.alteration.trim().is_empty() {
        out.push_str(&format!(
            "\n[OncoKB](https://www.oncokb.org/gene/{}/{})\n",
            result.gene.trim(),
            result.alteration.trim()
        ));
    }

    out
}

fn render_us_approvals_block(heading: &str, approvals: Option<&[DrugApproval]>) -> String {
    let Some(approvals) = approvals else {
        return String::new();
    };

    let mut out = String::new();
    let _ = writeln!(out, "{heading}\n");
    if approvals.is_empty() {
        out.push_str("No approvals found in Drugs@FDA for this query.\n");
        return out;
    }

    for app in approvals {
        let _ = writeln!(out, "### {}\n", markdown_cell(&app.application_number));
        if let Some(sponsor_name) = app.sponsor_name.as_deref() {
            let _ = writeln!(out, "- Sponsor: {}", markdown_cell(sponsor_name));
        }
        if !app.openfda_brand_names.is_empty() {
            let brands = app
                .openfda_brand_names
                .iter()
                .map(|value| markdown_cell(value))
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(out, "- Brands: {brands}");
        }
        if !app.openfda_generic_names.is_empty() {
            let generics = app
                .openfda_generic_names
                .iter()
                .map(|value| markdown_cell(value))
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(out, "- Generic Names: {generics}");
        }
        if !app.products.is_empty() {
            out.push_str("| Product | Dosage Form | Route | Marketing Status |\n");
            out.push_str("|---|---|---|---|\n");
            for product in &app.products {
                let _ = writeln!(
                    out,
                    "| {} | {} | {} | {} |",
                    product
                        .brand_name
                        .as_deref()
                        .map(markdown_cell)
                        .unwrap_or_else(|| "-".to_string()),
                    product
                        .dosage_form
                        .as_deref()
                        .map(markdown_cell)
                        .unwrap_or_else(|| "-".to_string()),
                    product
                        .route
                        .as_deref()
                        .map(markdown_cell)
                        .unwrap_or_else(|| "-".to_string()),
                    product
                        .marketing_status
                        .as_deref()
                        .map(markdown_cell)
                        .unwrap_or_else(|| "-".to_string()),
                );
            }
        }
        if !app.submissions.is_empty() {
            out.push_str("| Submission Type | Number | Status | Date |\n");
            out.push_str("|---|---|---|---|\n");
            for submission in &app.submissions {
                let _ = writeln!(
                    out,
                    "| {} | {} | {} | {} |",
                    submission
                        .submission_type
                        .as_deref()
                        .map(markdown_cell)
                        .unwrap_or_else(|| "-".to_string()),
                    submission
                        .submission_number
                        .as_deref()
                        .map(markdown_cell)
                        .unwrap_or_else(|| "-".to_string()),
                    submission
                        .status
                        .as_deref()
                        .map(markdown_cell)
                        .unwrap_or_else(|| "-".to_string()),
                    submission
                        .status_date
                        .as_deref()
                        .map(markdown_cell)
                        .unwrap_or_else(|| "-".to_string()),
                );
            }
        }
        out.push('\n');
    }

    out
}

fn render_eu_regulatory_block(heading: &str, rows: Option<&[EmaRegulatoryRow]>) -> String {
    let Some(rows) = rows else {
        return String::new();
    };

    let mut out = String::new();
    let _ = writeln!(out, "{heading}\n");
    if rows.is_empty() {
        out.push_str("No data found (EMA)\n");
        return out;
    }

    out.push_str("| Medicine | Active Substance | EMA Number | Status | Holder |\n");
    out.push_str("|---|---|---|---|---|\n");
    for row in rows {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} |",
            markdown_cell(&row.medicine_name),
            markdown_cell(&row.active_substance),
            markdown_cell(&row.ema_product_number),
            markdown_cell(&row.status),
            row.holder
                .as_deref()
                .map(markdown_cell)
                .unwrap_or_else(|| "-".to_string()),
        );
    }

    out.push_str("\n### Recent post-authorisation activity\n");
    let activity_rows = rows
        .iter()
        .flat_map(|row| {
            row.recent_activity.iter().map(move |activity| {
                (
                    row.medicine_name.as_str(),
                    activity.first_published_date.as_str(),
                    activity.last_updated_date.as_deref(),
                )
            })
        })
        .collect::<Vec<_>>();
    if activity_rows.is_empty() {
        out.push_str("No recent post-authorisation activity found.\n");
        return out;
    }

    out.push_str("| Medicine | First Published | Last Updated |\n");
    out.push_str("|---|---|---|\n");
    for (medicine_name, first_published_date, last_updated_date) in activity_rows {
        let _ = writeln!(
            out,
            "| {} | {} | {} |",
            markdown_cell(medicine_name),
            markdown_cell(first_published_date),
            last_updated_date
                .map(markdown_cell)
                .unwrap_or_else(|| "-".to_string()),
        );
    }
    out
}

fn render_who_regulatory_block(heading: &str, rows: Option<&[WhoPrequalificationEntry]>) -> String {
    let Some(rows) = rows else {
        return String::new();
    };

    let mut out = String::new();
    let _ = writeln!(out, "{heading}\n");
    if rows.is_empty() {
        out.push_str("Not WHO-prequalified\n");
        return out;
    }

    out.push_str("| WHO Ref | Presentation | Dosage Form | Therapeutic Area | Applicant | Listing Basis | Alternative Basis | Prequalification Date |\n");
    out.push_str("|---|---|---|---|---|---|---|---|\n");
    for row in rows {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} |",
            markdown_cell(&row.who_reference_number),
            markdown_cell(&row.presentation),
            markdown_cell(&row.dosage_form),
            markdown_cell(&row.therapeutic_area),
            markdown_cell(&row.applicant),
            markdown_cell(&row.listing_basis),
            row.alternative_listing_basis
                .as_deref()
                .map(markdown_cell)
                .unwrap_or_else(|| "-".to_string()),
            row.prequalification_date
                .as_deref()
                .map(markdown_cell)
                .unwrap_or_else(|| "-".to_string()),
        );
    }

    out
}

fn render_us_safety_block(drug: &Drug, heading: &str) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{heading}\n");

    out.push_str("### Top adverse events (FAERS)\n");
    if drug.top_adverse_events.is_empty() {
        out.push_str("No data found (OpenFDA FAERS)\n");
    } else {
        let _ = writeln!(out, "{}", drug.top_adverse_events.join(", "));
    }

    out.push_str("\n### FDA label warnings\n");
    if let Some(warnings) = drug.us_safety_warnings.as_deref() {
        out.push_str(warnings);
        out.push('\n');
    } else {
        out.push_str("No data found (OpenFDA label)\n");
    }

    out
}

fn render_eu_safety_block(heading: &str, safety: Option<&EmaSafetyInfo>) -> String {
    let Some(safety) = safety else {
        return String::new();
    };

    let mut out = String::new();
    let _ = writeln!(out, "{heading}\n");

    out.push_str("### DHPCs\n");
    if safety.dhpcs.is_empty() {
        out.push_str("No data found (EMA)\n");
    } else {
        out.push_str("| Medicine | Type | Outcome | First Published | Last Updated |\n");
        out.push_str("|---|---|---|---|---|\n");
        for row in &safety.dhpcs {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} |",
                markdown_cell(&row.medicine_name),
                row.dhpc_type
                    .as_deref()
                    .map(markdown_cell)
                    .unwrap_or_else(|| "-".to_string()),
                row.regulatory_outcome
                    .as_deref()
                    .map(markdown_cell)
                    .unwrap_or_else(|| "-".to_string()),
                row.first_published_date
                    .as_deref()
                    .map(markdown_cell)
                    .unwrap_or_else(|| "-".to_string()),
                row.last_updated_date
                    .as_deref()
                    .map(markdown_cell)
                    .unwrap_or_else(|| "-".to_string()),
            );
        }
    }

    out.push_str("\n### Referrals\n");
    if safety.referrals.is_empty() {
        out.push_str("No data found (EMA)\n");
    } else {
        out.push_str("| Referral | Active Substance | Medicines | Status | Type | Start |\n");
        out.push_str("|---|---|---|---|---|---|\n");
        for row in &safety.referrals {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} |",
                markdown_cell(&row.referral_name),
                row.active_substance
                    .as_deref()
                    .map(markdown_cell)
                    .unwrap_or_else(|| "-".to_string()),
                row.associated_medicines
                    .as_deref()
                    .map(markdown_cell)
                    .unwrap_or_else(|| "-".to_string()),
                row.current_status
                    .as_deref()
                    .map(markdown_cell)
                    .unwrap_or_else(|| "-".to_string()),
                row.referral_type
                    .as_deref()
                    .map(markdown_cell)
                    .unwrap_or_else(|| "-".to_string()),
                row.procedure_start_date
                    .as_deref()
                    .map(markdown_cell)
                    .unwrap_or_else(|| "-".to_string()),
            );
        }
    }

    out.push_str("\n### PSUSAs\n");
    if safety.psusas.is_empty() {
        out.push_str("No data found (EMA)\n");
    } else {
        out.push_str("| Related Medicines | Active Substance | Procedure | Outcome | First Published | Last Updated |\n");
        out.push_str("|---|---|---|---|---|---|\n");
        for row in &safety.psusas {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} |",
                row.related_medicines
                    .as_deref()
                    .map(markdown_cell)
                    .unwrap_or_else(|| "-".to_string()),
                row.active_substance
                    .as_deref()
                    .map(markdown_cell)
                    .unwrap_or_else(|| "-".to_string()),
                row.procedure_number
                    .as_deref()
                    .map(markdown_cell)
                    .unwrap_or_else(|| "-".to_string()),
                row.regulatory_outcome
                    .as_deref()
                    .map(markdown_cell)
                    .unwrap_or_else(|| "-".to_string()),
                row.first_published_date
                    .as_deref()
                    .map(markdown_cell)
                    .unwrap_or_else(|| "-".to_string()),
                row.last_updated_date
                    .as_deref()
                    .map(markdown_cell)
                    .unwrap_or_else(|| "-".to_string()),
            );
        }
    }

    out
}

fn render_us_shortage_block(
    heading: &str,
    shortage: Option<&[crate::entities::drug::DrugShortageEntry]>,
) -> String {
    let Some(shortage) = shortage else {
        return String::new();
    };

    let mut out = String::new();
    let _ = writeln!(out, "{heading}\n");
    if shortage.is_empty() {
        out.push_str("No shortage entries found\n");
        return out;
    }

    out.push_str("| Status | Availability | Company | Updated | Info |\n");
    out.push_str("|---|---|---|---|---|\n");
    for row in shortage {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} |",
            row.status
                .as_deref()
                .map(markdown_cell)
                .unwrap_or_else(|| "-".to_string()),
            row.availability
                .as_deref()
                .map(markdown_cell)
                .unwrap_or_else(|| "-".to_string()),
            row.company_name
                .as_deref()
                .map(markdown_cell)
                .unwrap_or_else(|| "-".to_string()),
            row.update_date
                .as_deref()
                .map(markdown_cell)
                .unwrap_or_else(|| "-".to_string()),
            row.related_info
                .as_deref()
                .map(markdown_cell)
                .unwrap_or_else(|| "-".to_string()),
        );
    }
    out
}

fn render_eu_shortage_block(heading: &str, shortage: Option<&[EmaShortageEntry]>) -> String {
    let Some(shortage) = shortage else {
        return String::new();
    };

    let mut out = String::new();
    let _ = writeln!(out, "{heading}\n");
    if shortage.is_empty() {
        out.push_str("No data found (EMA)\n");
        return out;
    }

    out.push_str("| Medicine | Status | Alternatives | First Published | Last Updated |\n");
    out.push_str("|---|---|---|---|---|\n");
    for row in shortage {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} |",
            markdown_cell(&row.medicine_affected),
            row.status
                .as_deref()
                .map(markdown_cell)
                .unwrap_or_else(|| "-".to_string()),
            row.availability_of_alternatives
                .as_deref()
                .map(markdown_cell)
                .unwrap_or_else(|| "-".to_string()),
            row.first_published_date
                .as_deref()
                .map(markdown_cell)
                .unwrap_or_else(|| "-".to_string()),
            row.last_updated_date
                .as_deref()
                .map(markdown_cell)
                .unwrap_or_else(|| "-".to_string()),
        );
    }
    out
}

fn render_regulatory_block(drug: &Drug, region: DrugRegion) -> String {
    match region {
        DrugRegion::Us => {
            render_us_approvals_block("## Regulatory (US - Drugs@FDA)", drug.approvals.as_deref())
        }
        DrugRegion::Eu => {
            render_eu_regulatory_block("## Regulatory (EU - EMA)", drug.ema_regulatory.as_deref())
        }
        DrugRegion::Who => render_who_regulatory_block(
            "## Regulatory (WHO Prequalification)",
            drug.who_prequalification.as_deref(),
        ),
        DrugRegion::All => {
            let us = render_us_approvals_block(
                "## Regulatory (US - Drugs@FDA)",
                drug.approvals.as_deref(),
            );
            let eu = render_eu_regulatory_block(
                "## Regulatory (EU - EMA)",
                drug.ema_regulatory.as_deref(),
            );
            let who = render_who_regulatory_block(
                "## Regulatory (WHO Prequalification)",
                drug.who_prequalification.as_deref(),
            );
            [us, eu, who]
                .into_iter()
                .filter(|block| !block.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
}

fn render_safety_block(drug: &Drug, region: DrugRegion) -> String {
    match region {
        DrugRegion::Us => render_us_safety_block(drug, "## Safety (US - OpenFDA)"),
        DrugRegion::Eu => render_eu_safety_block("## Safety (EU - EMA)", drug.ema_safety.as_ref()),
        DrugRegion::Who => String::new(),
        DrugRegion::All => {
            let us = render_us_safety_block(drug, "## Safety (US - OpenFDA)");
            let eu = render_eu_safety_block("## Safety (EU - EMA)", drug.ema_safety.as_ref());
            [us, eu]
                .into_iter()
                .filter(|block| !block.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
}

fn render_shortage_block(drug: &Drug, region: DrugRegion) -> String {
    match region {
        DrugRegion::Us => render_us_shortage_block(
            "## Shortage (US - OpenFDA Drug Shortages)",
            drug.shortage.as_deref(),
        ),
        DrugRegion::Eu => {
            render_eu_shortage_block("## Shortage (EU - EMA)", drug.ema_shortage.as_deref())
        }
        DrugRegion::Who => String::new(),
        DrugRegion::All => {
            let us = render_us_shortage_block(
                "## Shortage (US - OpenFDA Drug Shortages)",
                drug.shortage.as_deref(),
            );
            let eu =
                render_eu_shortage_block("## Shortage (EU - EMA)", drug.ema_shortage.as_deref());
            [us, eu]
                .into_iter()
                .filter(|block| !block.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
}

pub fn drug_markdown_with_region(
    drug: &Drug,
    requested_sections: &[String],
    region: DrugRegion,
    raw_label: bool,
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("drug.md.j2")?;
    let section_only = is_section_only_requested(requested_sections);
    let include_all = has_all_section(requested_sections);
    let requested = requested_section_names(requested_sections);
    let has_requested = |name: &str| requested.iter().any(|s| s.eq_ignore_ascii_case(name));
    let show_label_section = !section_only || include_all || has_requested("label");
    let show_targets_section = !section_only || include_all || has_requested("targets");
    let show_indications_section = !section_only || include_all || has_requested("indications");
    let show_interactions_section = include_all || has_requested("interactions");
    let show_civic_section = include_all || has_requested("civic");
    let show_regulatory_section = include_all || has_requested("regulatory");
    let show_safety_section =
        !matches!(region, DrugRegion::Who) && (include_all || has_requested("safety"));
    let show_shortage_section = !matches!(region, DrugRegion::Who)
        && (!section_only || include_all || has_requested("shortage"));
    let show_approvals_section = has_requested("approvals");
    // Suppress US-only header facts when rendering a full card (not section_only) for EU region.
    let show_us_header = section_only || region.includes_us();
    let approval_date_display: Option<&str> = if show_us_header {
        drug.approval_date_display.as_deref()
    } else {
        None
    };
    let body = tmpl.render(context! {
        section_only => section_only,
        section_header => section_header(&drug.name, requested_sections),
        drug_interactions_heading => crate::render::provenance::drug_interaction_heading_label(drug),
        name => &drug.name,
        drugbank_id => &drug.drugbank_id,
        chembl_id => &drug.chembl_id,
        unii => &drug.unii,
        drug_type => &drug.drug_type,
        mechanism => &drug.mechanism,
        mechanisms => &drug.mechanisms,
        approval_date => &drug.approval_date,
        approval_date_display => approval_date_display,
        brand_names => &drug.brand_names,
        route => &drug.route,
        show_us_header => show_us_header,
        top_adverse_events => &drug.top_adverse_events,
        targets => &drug.targets,
        variant_targets => &drug.variant_targets,
        target_family => &drug.target_family,
        target_family_name => &drug.target_family_name,
        indications => &drug.indications,
        interactions => &drug.interactions,
        interaction_text => &drug.interaction_text,
        pharm_classes => &drug.pharm_classes,
        label => &drug.label,
        raw_label => raw_label,
        civic => &drug.civic,
        show_label_section => show_label_section,
        show_targets_section => show_targets_section,
        show_indications_section => show_indications_section,
        show_interactions_section => show_interactions_section,
        show_civic_section => show_civic_section,
        regulatory_block => if show_regulatory_section { render_regulatory_block(drug, region) } else { String::new() },
        safety_block => if show_safety_section { render_safety_block(drug, region) } else { String::new() },
        shortage_block => if show_shortage_section { render_shortage_block(drug, region) } else { String::new() },
        approvals_block => if show_approvals_section {
            render_us_approvals_block("## Drugs@FDA Approvals", drug.approvals.as_deref())
        } else {
            String::new()
        },
        sections_block => format_sections_block("drug", &drug.name, sections_drug(drug, requested_sections)),
        related_block => format_related_block(related_drug(drug)),
    })?;
    Ok(append_evidence_urls(body, drug_evidence_urls(drug)))
}

pub fn drug_markdown(drug: &Drug, requested_sections: &[String]) -> Result<String, BioMcpError> {
    drug_markdown_with_region(drug, requested_sections, DrugRegion::Us, false)
}

pub fn drug_search_markdown(
    query: &str,
    results: &[DrugSearchResult],
) -> Result<String, BioMcpError> {
    drug_search_markdown_with_footer(query, results, None, "")
}

pub fn drug_search_markdown_with_footer(
    query: &str,
    results: &[DrugSearchResult],
    total_count: Option<usize>,
    pagination_footer: &str,
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("drug_search.md.j2")?;
    let count = total_count.unwrap_or(results.len());
    let discover_hint = discover_try_line(query, "resolve drug trial codes and aliases");
    let body = tmpl.render(context! {
        query => query,
        count => count,
        results => results,
        discover_hint => discover_hint,
        pagination_footer => pagination_footer,
    })?;
    Ok(with_pagination_footer(body, pagination_footer))
}

#[allow(clippy::too_many_arguments)]
pub fn drug_search_markdown_with_region(
    query: &str,
    region: DrugRegion,
    us_results: &[DrugSearchResult],
    us_total: Option<usize>,
    eu_results: &[EmaDrugSearchResult],
    eu_total: Option<usize>,
    who_results: &[WhoPrequalificationSearchResult],
    who_total: Option<usize>,
    pagination_footer: &str,
) -> Result<String, BioMcpError> {
    match region {
        DrugRegion::Us => {
            let count = us_total.unwrap_or(us_results.len());
            if count == 0 && is_structured_indication_query(query) {
                return Ok(empty_drug_indication_search_message(query, region));
            }
            drug_search_markdown_with_footer(query, us_results, us_total, pagination_footer)
        }
        DrugRegion::Eu => {
            let count = eu_total.unwrap_or(eu_results.len());
            if count == 0 && is_structured_indication_query(query) {
                return Ok(empty_drug_indication_search_message(query, region));
            }
            let mut out = String::new();
            let _ = writeln!(out, "# Drugs: {query}\n");
            if count == 0 {
                out.push_str("No drugs found\n");
                let discover_hint =
                    discover_try_line(query, "resolve drug trial codes and aliases");
                if !discover_hint.is_empty() {
                    let _ = writeln!(out, "\n{discover_hint}");
                }
                return Ok(out);
            }

            let _ = writeln!(
                out,
                "Found {count} drug{}\n",
                if count == 1 { "" } else { "s" }
            );
            out.push_str("|Name|Active Substance|EMA Number|Status|\n");
            out.push_str("|---|---|---|---|\n");
            for row in eu_results {
                let _ = writeln!(
                    out,
                    "|{}|{}|{}|{}|",
                    markdown_cell(&row.name),
                    markdown_cell(&row.active_substance),
                    markdown_cell(&row.ema_product_number),
                    markdown_cell(&row.status),
                );
            }
            out.push_str("\nUse `get drug <name>` for full details.\n");
            if !pagination_footer.trim().is_empty() {
                let _ = writeln!(out, "\n{pagination_footer}");
            }
            Ok(out)
        }
        DrugRegion::Who => {
            let count = who_total.unwrap_or(who_results.len());
            if count == 0 && is_structured_indication_query(query) {
                return Ok(empty_drug_indication_search_message(query, region));
            }

            let mut out = String::new();
            let _ = writeln!(out, "# Drugs: {query}\n");
            if count == 0 {
                out.push_str("No WHO-prequalified drugs found\n");
                let discover_hint =
                    discover_try_line(query, "resolve drug trial codes and aliases");
                if !discover_hint.is_empty() {
                    let _ = writeln!(out, "\n{discover_hint}");
                }
                return Ok(out);
            }

            let _ = writeln!(
                out,
                "Found {count} drug{}\n",
                if count == 1 { "" } else { "s" }
            );
            out.push_str(
                "|INN|Therapeutic Area|Dosage Form|Applicant|WHO Ref|Listing Basis|Date|\n",
            );
            out.push_str("|---|---|---|---|---|---|---|\n");
            for row in who_results {
                let _ = writeln!(
                    out,
                    "|{}|{}|{}|{}|{}|{}|{}|",
                    markdown_cell(&row.inn),
                    markdown_cell(&row.therapeutic_area),
                    markdown_cell(&row.dosage_form),
                    markdown_cell(&row.applicant),
                    markdown_cell(&row.who_reference_number),
                    markdown_cell(&row.listing_basis),
                    row.prequalification_date
                        .as_deref()
                        .map(markdown_cell)
                        .unwrap_or_else(|| "-".to_string()),
                );
            }
            out.push_str("\nUse `get drug <name>` for full details.\n");
            if !pagination_footer.trim().is_empty() {
                let _ = writeln!(out, "\n{pagination_footer}");
            }
            Ok(out)
        }
        DrugRegion::All => {
            let mut out = String::new();
            let _ = writeln!(out, "# Drugs: {query}\n");

            out.push_str("## US (MyChem.info / OpenFDA)\n\n");
            let us_count = us_total.unwrap_or(us_results.len());
            let eu_count = eu_total.unwrap_or(eu_results.len());
            let who_count = who_total.unwrap_or(who_results.len());
            if us_results.is_empty() {
                if us_count == 0 && is_structured_indication_query(query) {
                    out.push_str(&empty_drug_indication_search_body(query, DrugRegion::All));
                    out.push('\n');
                } else {
                    out.push_str("No drugs found\n");
                }
            } else {
                let _ = writeln!(
                    out,
                    "Found {us_count} drug{}\n",
                    if us_count == 1 { "" } else { "s" }
                );
                out.push_str("|Name|Mechanism|Target|\n");
                out.push_str("|---|---|---|\n");
                for row in us_results {
                    let mechanism = row
                        .mechanism
                        .as_deref()
                        .or(row.drug_type.as_deref())
                        .unwrap_or("-");
                    let _ = writeln!(
                        out,
                        "|{}|{}|{}|",
                        markdown_cell(&row.name),
                        markdown_cell(mechanism),
                        row.target
                            .as_deref()
                            .map(markdown_cell)
                            .unwrap_or_else(|| "-".to_string()),
                    );
                }
            }

            out.push_str("\n## EU (EMA)\n\n");
            if eu_results.is_empty() {
                out.push_str("No drugs found\n");
            } else {
                let count = eu_total.unwrap_or(eu_results.len());
                let _ = writeln!(
                    out,
                    "Found {count} drug{}\n",
                    if count == 1 { "" } else { "s" }
                );
                out.push_str("|Name|Active Substance|EMA Number|Status|\n");
                out.push_str("|---|---|---|---|\n");
                for row in eu_results {
                    let _ = writeln!(
                        out,
                        "|{}|{}|{}|{}|",
                        markdown_cell(&row.name),
                        markdown_cell(&row.active_substance),
                        markdown_cell(&row.ema_product_number),
                        markdown_cell(&row.status),
                    );
                }
            }

            out.push_str("\n## WHO (WHO Prequalification)\n\n");
            if who_results.is_empty() {
                out.push_str("No WHO-prequalified drugs found\n");
            } else {
                let _ = writeln!(
                    out,
                    "Found {who_count} drug{}\n",
                    if who_count == 1 { "" } else { "s" }
                );
                out.push_str(
                    "|INN|Therapeutic Area|Dosage Form|Applicant|WHO Ref|Listing Basis|Date|\n",
                );
                out.push_str("|---|---|---|---|---|---|---|\n");
                for row in who_results {
                    let _ = writeln!(
                        out,
                        "|{}|{}|{}|{}|{}|{}|{}|",
                        markdown_cell(&row.inn),
                        markdown_cell(&row.therapeutic_area),
                        markdown_cell(&row.dosage_form),
                        markdown_cell(&row.applicant),
                        markdown_cell(&row.who_reference_number),
                        markdown_cell(&row.listing_basis),
                        row.prequalification_date
                            .as_deref()
                            .map(markdown_cell)
                            .unwrap_or_else(|| "-".to_string()),
                    );
                }
            }

            if us_count == 0
                && eu_count == 0
                && who_count == 0
                && !is_structured_indication_query(query)
            {
                let discover_hint =
                    discover_try_line(query, "resolve drug trial codes and aliases");
                if !discover_hint.is_empty() {
                    let _ = writeln!(out, "\n{discover_hint}");
                }
            }

            out.push_str("\nUse `get drug <name>` for full details.\n");
            if !pagination_footer.trim().is_empty() {
                let _ = writeln!(out, "\n{pagination_footer}");
            }
            Ok(out)
        }
    }
}

fn is_structured_indication_query(query: &str) -> bool {
    query
        .trim_start()
        .to_ascii_lowercase()
        .starts_with("indication=")
}

fn indication_query_value(query: &str) -> &str {
    query
        .split_once('=')
        .map(|(_, value)| value.trim())
        .unwrap_or(query.trim())
}

fn empty_drug_indication_search_body(query: &str, region: DrugRegion) -> String {
    let disease = indication_query_value(query);
    let review_query = quote_arg(&format!("{disease} treatment"));
    let discover_hint = discover_try_line(disease, "resolve drug trial codes and aliases");
    match region {
        DrugRegion::Us => format!(
            "No drugs found in U.S. regulatory data for this indication.\nThis absence is informative for approved-drug questions, but it does not rule out investigational or off-label evidence.\nTry `biomcp search article -k {review_query} --type review --limit 5` for broader treatment literature.\n{discover_hint}"
        ),
        DrugRegion::All => format!(
            "No drugs found in U.S. regulatory data for this indication.\nThis absence is informative for approved-drug questions and is specific to the structured regulatory portion of the combined search.\nTry `biomcp search article -k {review_query} --type review --limit 5` for broader treatment literature.\n{discover_hint}"
        ),
        DrugRegion::Eu => format!("No drugs found\n{discover_hint}"),
        DrugRegion::Who => format!(
            "No WHO-prequalified drugs found for this structured search.\nThis absence is informative for WHO-prequalified regulatory coverage, but it does not rule out U.S. approvals or broader investigational evidence.\nTry `biomcp search article -k {review_query} --type review --limit 5` for broader treatment literature.\n{discover_hint}"
        ),
    }
}

fn empty_drug_indication_search_message(query: &str, region: DrugRegion) -> String {
    format!(
        "# Drugs: {query}\n\n{}\n",
        empty_drug_indication_search_body(query, region)
    )
}

pub fn pathway_markdown(
    pathway: &Pathway,
    requested_sections: &[String],
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("pathway.md.j2")?;
    let section_only = is_section_only_requested(requested_sections);
    let include_all = has_all_section(requested_sections);
    let requested = requested_section_names(requested_sections);
    let has_requested = |name: &str| requested.iter().any(|s| s.eq_ignore_ascii_case(name));
    let show_genes_section = !section_only || include_all || has_requested("genes");
    let show_events_section = !section_only || include_all || has_requested("events");
    let show_enrichment_section = !section_only || include_all || has_requested("enrichment");
    let pathway_label = if pathway.name.trim().is_empty() {
        pathway.id.as_str()
    } else {
        pathway.name.as_str()
    };
    let body = tmpl.render(context! {
        section_only => section_only,
        section_header => section_header(pathway_label, requested_sections),
        pathway_source_label => crate::render::provenance::pathway_source_label(&pathway.source),
        source => &pathway.source,
        id => &pathway.id,
        name => &pathway.name,
        species => &pathway.species,
        summary => &pathway.summary,
        genes => &pathway.genes,
        events => &pathway.events,
        enrichment => &pathway.enrichment,
        show_genes_section => show_genes_section,
        show_events_section => show_events_section,
        show_enrichment_section => show_enrichment_section,
        sections_block => format_sections_block("pathway", &pathway.id, sections_pathway(pathway, requested_sections)),
        related_block => format_related_block(related_pathway(pathway)),
    })?;
    Ok(append_evidence_urls(body, pathway_evidence_urls(pathway)))
}

#[allow(dead_code)]
pub fn pathway_search_markdown(
    query: &str,
    results: &[PathwaySearchResult],
    total: Option<usize>,
) -> Result<String, BioMcpError> {
    pathway_search_markdown_with_footer(query, results, total, "")
}

pub fn pathway_search_markdown_with_footer(
    query: &str,
    results: &[PathwaySearchResult],
    total: Option<usize>,
    pagination_footer: &str,
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("pathway_search.md.j2")?;
    let body = tmpl.render(context! {
        query => query,
        count => results.len(),
        total => total,
        results => results,
        pagination_footer => pagination_footer,
    })?;
    Ok(with_pagination_footer(body, pagination_footer))
}

pub fn protein_markdown(
    protein: &Protein,
    requested_sections: &[String],
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("protein.md.j2")?;
    let section_only = is_section_only_requested(requested_sections);
    let include_all = has_all_section(requested_sections);
    let requested = requested_section_names(requested_sections);
    let has_requested = |name: &str| requested.iter().any(|s| s.eq_ignore_ascii_case(name));
    let show_domains_section = !section_only || include_all || has_requested("domains");
    let show_interactions_section = !section_only || include_all || has_requested("interactions");
    let show_complexes_section = !section_only || include_all || has_requested("complexes");
    let show_structures_section = !section_only || include_all || has_requested("structures");
    let protein_label = if protein.name.trim().is_empty() {
        protein.accession.as_str()
    } else {
        protein.name.as_str()
    };
    let complex_summaries = protein_complex_summary_rows(&protein.complexes);
    let complex_details = protein_complex_detail_rows(&protein.complexes);
    let body = tmpl.render(context! {
        section_only => section_only,
        section_header => section_header(protein_label, requested_sections),
        accession => &protein.accession,
        entry_id => &protein.entry_id,
        name => &protein.name,
        gene_symbol => &protein.gene_symbol,
        organism => &protein.organism,
        length => &protein.length,
        function => &protein.function,
        structures => &protein.structures,
        structure_count => &protein.structure_count,
        domains => &protein.domains,
        interactions => &protein.interactions,
        complexes => complex_summaries,
        complex_details => complex_details,
        show_domains_section => show_domains_section,
        show_interactions_section => show_interactions_section,
        show_complexes_section => show_complexes_section,
        show_structures_section => show_structures_section,
        sections_block => format_sections_block("protein", &protein.accession, sections_protein(protein, requested_sections)),
        related_block => format_related_block(related_protein(protein, requested_sections)),
    })?;
    Ok(append_evidence_urls(body, protein_evidence_urls(protein)))
}

#[allow(dead_code)]
pub fn protein_search_markdown(
    query: &str,
    results: &[ProteinSearchResult],
) -> Result<String, BioMcpError> {
    protein_search_markdown_with_footer(query, results, "")
}

pub fn protein_search_markdown_with_footer(
    query: &str,
    results: &[ProteinSearchResult],
    pagination_footer: &str,
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("protein_search.md.j2")?;
    let body = tmpl.render(context! {
        query => query,
        count => results.len(),
        results => results,
        pagination_footer => pagination_footer,
    })?;
    Ok(with_pagination_footer(body, pagination_footer))
}

pub fn adverse_event_markdown(
    event: &AdverseEvent,
    requested_sections: &[String],
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("adverse_event.md.j2")?;
    let section_only = is_section_only_requested(requested_sections);
    let parsed = crate::entities::adverse_event::parse_sections(requested_sections)?;
    let show_reactions_section = !section_only || parsed.include_reactions;
    let show_outcomes_section = !section_only || parsed.include_outcomes;
    let show_concomitant_section = !section_only || parsed.include_concomitant;
    let show_guidance_section = !section_only || parsed.include_guidance;
    let drug = quote_arg(&event.drug);
    let indication = event
        .indication
        .as_deref()
        .map(quote_arg)
        .unwrap_or_default();
    let body = tmpl.render(context! {
        section_only => section_only,
        section_header => section_header("Adverse Event", requested_sections),
        report_id => &event.report_id,
        drug => &event.drug,
        reactions => &event.reactions,
        outcomes => &event.outcomes,
        patient => &event.patient,
        concomitant_medications => &event.concomitant_medications,
        reporter_type => &event.reporter_type,
        reporter_country => &event.reporter_country,
        indication => &event.indication,
        guidance_indication => indication,
        guidance_drug => drug,
        show_reactions_section => show_reactions_section,
        show_outcomes_section => show_outcomes_section,
        show_concomitant_section => show_concomitant_section,
        show_guidance_section => show_guidance_section,
        serious => &event.serious,
        date => &event.date,
        sections_block => format_sections_block("adverse-event", &event.report_id, sections_adverse_event(event, requested_sections)),
        related_block => format_related_block(related_adverse_event(event)),
    })?;
    Ok(append_evidence_urls(
        body,
        adverse_event_evidence_urls(event),
    ))
}

pub fn adverse_event_search_markdown(
    query: &str,
    results: &[AdverseEventSearchResult],
    summary: &AdverseEventSearchSummary,
) -> Result<String, BioMcpError> {
    adverse_event_search_markdown_with_footer(query, results, summary, "")
}

pub fn adverse_event_search_markdown_with_footer(
    query: &str,
    results: &[AdverseEventSearchResult],
    summary: &AdverseEventSearchSummary,
    pagination_footer: &str,
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("adverse_event_search.md.j2")?;
    let body = tmpl.render(context! {
        query => query,
        count => results.len(),
        summary => summary,
        results => results,
        pagination_footer => pagination_footer,
    })?;
    Ok(with_pagination_footer(body, pagination_footer))
}

pub fn adverse_event_count_markdown(
    query: &str,
    count_field: &str,
    buckets: &[AdverseEventCountBucket],
) -> Result<String, BioMcpError> {
    let mut out = String::new();
    out.push_str("# Adverse Event Counts\n");
    out.push_str(&format!("\nQuery: {query}\n"));
    out.push_str(&format!("Count field: {count_field}\n\n"));
    out.push_str("| Value | Count |\n");
    out.push_str("|---|---|\n");
    if buckets.is_empty() {
        out.push_str("| - | 0 |\n");
    } else {
        for bucket in buckets {
            out.push_str(&format!("| {} | {} |\n", bucket.value, bucket.count));
        }
    }
    Ok(out)
}

pub fn device_event_markdown(event: &DeviceEvent) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("device_event.md.j2")?;
    let body = tmpl.render(context! {
        report_id => &event.report_id,
        report_number => &event.report_number,
        device => &event.device,
        manufacturer => &event.manufacturer,
        event_type => &event.event_type,
        date => &event.date,
        description => &event.description,
        related_block => format_related_block(related_device_event(event)),
    })?;
    Ok(append_evidence_urls(
        body,
        device_event_evidence_urls(event),
    ))
}

#[allow(dead_code)]
pub fn device_event_search_markdown(
    query: &str,
    results: &[DeviceEventSearchResult],
) -> Result<String, BioMcpError> {
    device_event_search_markdown_with_footer(query, results, "")
}

pub fn device_event_search_markdown_with_footer(
    query: &str,
    results: &[DeviceEventSearchResult],
    pagination_footer: &str,
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("device_event_search.md.j2")?;
    let body = tmpl.render(context! {
        query => query,
        count => results.len(),
        results => results,
        pagination_footer => pagination_footer,
    })?;
    Ok(with_pagination_footer(body, pagination_footer))
}

#[allow(dead_code)]
pub fn recall_search_markdown(
    query: &str,
    results: &[RecallSearchResult],
) -> Result<String, BioMcpError> {
    recall_search_markdown_with_footer(query, results, "")
}

pub fn recall_search_markdown_with_footer(
    query: &str,
    results: &[RecallSearchResult],
    pagination_footer: &str,
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("recall_search.md.j2")?;
    let body = tmpl.render(context! {
        query => query,
        count => results.len(),
        results => results,
        pagination_footer => pagination_footer,
    })?;
    Ok(with_pagination_footer(body, pagination_footer))
}

pub fn study_list_markdown(studies: &[StudyInfo]) -> String {
    let mut out = String::new();
    out.push_str("# Study Datasets\n\n");
    if studies.is_empty() {
        out.push_str("No local studies found.\n");
        return out;
    }

    out.push_str("| Study ID | Name | Cancer Type | Samples | Available Data |\n");
    out.push_str("|---|---|---|---|---|\n");
    for study in studies {
        let cancer_type = study.cancer_type.as_deref().unwrap_or("-");
        let sample_count = study
            .sample_count
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string());
        let available = if study.available_data.is_empty() {
            "-".to_string()
        } else {
            study.available_data.join(", ")
        };
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            study.study_id, study.name, cancer_type, sample_count, available
        ));
    }
    out
}

fn format_optional_stat(value: Option<f64>, decimals: usize) -> String {
    value
        .map(|value| format!("{value:.prec$}", prec = decimals))
        .unwrap_or_else(|| "-".to_string())
}

fn format_optional_p_value(value: Option<f64>) -> String {
    value
        .map(|value| {
            if value == 0.0 {
                "0".to_string()
            } else if value < 0.001 {
                format!("{value:.2e}")
            } else if value < 0.01 {
                format!("{value:.4}")
            } else {
                format!("{value:.3}")
            }
        })
        .unwrap_or_else(|| "not available".to_string())
}

pub fn study_download_catalog_markdown(result: &StudyDownloadCatalog) -> String {
    let mut out = String::new();
    out.push_str("# Downloadable cBioPortal Studies\n\n");
    if result.study_ids.is_empty() {
        out.push_str("No remote study IDs found.\n");
        return out;
    }

    out.push_str("| Study ID |\n");
    out.push_str("|---|\n");
    for study_id in &result.study_ids {
        out.push_str(&format!("| {study_id} |\n"));
    }
    out
}

pub fn study_download_markdown(result: &StudyDownloadResult) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Study Download: {}\n\n", result.study_id));
    out.push_str("| Metric | Value |\n");
    out.push_str("|---|---|\n");
    out.push_str(&format!("| Study ID | {} |\n", result.study_id));
    out.push_str(&format!("| Path | {} |\n", result.path));
    out.push_str(&format!(
        "| Downloaded | {} |\n",
        if result.downloaded {
            "yes"
        } else {
            "already present"
        }
    ));
    out
}

pub fn study_query_markdown(result: &StudyQueryResult) -> String {
    match result {
        StudyQueryResult::MutationFrequency(result) => {
            let mut out = String::new();
            out.push_str(&format!(
                "# Study Mutation Frequency: {} ({})\n\n",
                result.gene, result.study_id
            ));
            out.push_str("| Metric | Value |\n");
            out.push_str("|---|---|\n");
            out.push_str(&format!(
                "| Mutation records | {} |\n",
                result.mutation_count
            ));
            out.push_str(&format!("| Unique samples | {} |\n", result.unique_samples));
            out.push_str(&format!("| Total samples | {} |\n", result.total_samples));
            out.push_str(&format!("| Frequency | {:.6} |\n", result.frequency));
            out.push_str("\n## Top Variant Classes\n\n");
            out.push_str("| Class | Count |\n");
            out.push_str("|---|---|\n");
            if result.top_variant_classes.is_empty() {
                out.push_str("| - | 0 |\n");
            } else {
                for (class_name, count) in &result.top_variant_classes {
                    out.push_str(&format!("| {} | {} |\n", class_name, count));
                }
            }
            out.push_str("\n## Top Protein Changes\n\n");
            out.push_str("| Change | Count |\n");
            out.push_str("|---|---|\n");
            if result.top_protein_changes.is_empty() {
                out.push_str("| - | 0 |\n");
            } else {
                for (change, count) in &result.top_protein_changes {
                    out.push_str(&format!("| {} | {} |\n", change, count));
                }
            }
            out
        }
        StudyQueryResult::CnaDistribution(result) => {
            let mut out = String::new();
            out.push_str(&format!(
                "# Study CNA Distribution: {} ({})\n\n",
                result.gene, result.study_id
            ));
            out.push_str("| Bucket | Count |\n");
            out.push_str("|---|---|\n");
            out.push_str(&format!(
                "| Deep deletion (-2) | {} |\n",
                result.deep_deletion
            ));
            out.push_str(&format!(
                "| Shallow deletion (-1) | {} |\n",
                result.shallow_deletion
            ));
            out.push_str(&format!("| Diploid (0) | {} |\n", result.diploid));
            out.push_str(&format!("| Gain (1) | {} |\n", result.gain));
            out.push_str(&format!(
                "| Amplification (2) | {} |\n",
                result.amplification
            ));
            out.push_str(&format!("| Total samples | {} |\n", result.total_samples));
            out
        }
        StudyQueryResult::ExpressionDistribution(result) => {
            let mut out = String::new();
            out.push_str(&format!(
                "# Study Expression Distribution: {} ({})\n\n",
                result.gene, result.study_id
            ));
            out.push_str("| Metric | Value |\n");
            out.push_str("|---|---|\n");
            out.push_str(&format!("| File | {} |\n", result.file));
            out.push_str(&format!("| Sample count | {} |\n", result.sample_count));
            out.push_str(&format!("| Mean | {:.6} |\n", result.mean));
            out.push_str(&format!("| Median | {:.6} |\n", result.median));
            out.push_str(&format!("| Min | {:.6} |\n", result.min));
            out.push_str(&format!("| Max | {:.6} |\n", result.max));
            out.push_str(&format!("| Q1 | {:.6} |\n", result.q1));
            out.push_str(&format!("| Q3 | {:.6} |\n", result.q3));
            out
        }
    }
}

pub fn study_top_mutated_markdown(result: &StudyTopMutatedGenesResult) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# Study Top Mutated Genes: {}\n\n",
        result.study_id
    ));
    out.push_str("| Gene | Mutated Samples | Mutation Events | Total Samples | Mutation Rate |\n");
    out.push_str("|---|---|---|---|---|\n");
    if result.rows.is_empty() {
        out.push_str("| - | 0 | 0 | 0 | 0.000000 |\n");
        return out;
    }

    for row in &result.rows {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {:.6} |\n",
            row.gene,
            row.mutated_samples,
            row.mutation_events,
            result.total_samples,
            row.mutation_rate
        ));
    }
    out
}

pub fn study_filter_markdown(result: &StudyFilterResult) -> String {
    const SAMPLE_DISPLAY_LIMIT: usize = 50;

    let mut out = String::new();
    out.push_str(&format!("# Study Filter: {}\n\n", result.study_id));
    out.push_str("## Criteria\n\n");
    out.push_str("| Filter | Matching Samples |\n");
    out.push_str("|---|---|\n");
    if result.criteria.is_empty() {
        out.push_str("| - | 0 |\n");
    } else {
        for criterion in &result.criteria {
            out.push_str(&format!(
                "| {} | {} |\n",
                criterion.description, criterion.matched_count
            ));
        }
    }

    out.push_str("\n## Result\n\n");
    out.push_str("| Metric | Value |\n");
    out.push_str("|---|---|\n");
    let total = result
        .total_study_samples
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string());
    out.push_str(&format!("| Study Total Samples | {total} |\n"));
    out.push_str(&format!("| Intersection | {} |\n", result.matched_count));

    out.push_str("\n## Matched Samples\n\n");
    if result.matched_sample_ids.is_empty() {
        out.push_str("None\n");
        return out;
    }

    for sample_id in result.matched_sample_ids.iter().take(SAMPLE_DISPLAY_LIMIT) {
        out.push_str(sample_id);
        out.push('\n');
    }
    let remaining = result
        .matched_sample_ids
        .len()
        .saturating_sub(SAMPLE_DISPLAY_LIMIT);
    if remaining > 0 {
        out.push_str(&format!(
            "... and {remaining} more (use --json for full list)\n"
        ));
    }
    out
}

pub fn study_cohort_markdown(result: &StudyCohortResult) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# Study Cohort: {} ({})\n\n",
        result.gene, result.study_id
    ));
    let stratification = match result.stratification.as_str() {
        "mutation" => "mutation status",
        other => other,
    };
    out.push_str(&format!("Stratification: {stratification}\n\n"));
    out.push_str("| Group | Samples | Patients |\n");
    out.push_str("|---|---|---|\n");
    out.push_str(&format!(
        "| {}-mutant | {} | {} |\n",
        result.gene, result.mutant_samples, result.mutant_patients
    ));
    out.push_str(&format!(
        "| {}-wildtype | {} | {} |\n",
        result.gene, result.wildtype_samples, result.wildtype_patients
    ));
    out.push_str(&format!(
        "| Total | {} | {} |\n",
        result.total_samples, result.total_patients
    ));
    out
}

pub fn study_survival_markdown(result: &StudySurvivalResult) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# Study Survival: {} ({})\n\n",
        result.gene, result.study_id
    ));
    out.push_str(&format!(
        "Endpoint: {} ({})\n\n",
        result.endpoint.label(),
        result.endpoint.code()
    ));
    out.push_str("| Group | N | Events | Censored | Event Rate | KM Median | 1yr | 3yr | 5yr |\n");
    out.push_str("|---|---|---|---|---|---|---|---|---|\n");
    for group in &result.groups {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {:.6} | {} | {} | {} | {} |\n",
            group.group_name,
            group.n_patients,
            group.n_events,
            group.n_censored,
            group.event_rate,
            format_optional_stat(group.km_median_months, 1),
            format_optional_stat(group.survival_1yr, 3),
            format_optional_stat(group.survival_3yr, 3),
            format_optional_stat(group.survival_5yr, 3)
        ));
    }
    out.push('\n');
    out.push_str(&format!(
        "Log-rank p-value: {}\n",
        format_optional_p_value(result.log_rank_p)
    ));
    out
}

pub fn study_compare_expression_markdown(result: &StudyExpressionComparisonResult) -> String {
    let mut out = String::new();
    out.push_str("# Study Group Comparison: Expression\n\n");
    out.push_str(&format!(
        "Stratify gene: {} | Target gene: {} | Study: {}\n\n",
        result.stratify_gene, result.target_gene, result.study_id
    ));
    out.push_str("| Group | N | Mean | Median | Q1 | Q3 | Min | Max |\n");
    out.push_str("|---|---|---|---|---|---|---|---|\n");
    for group in &result.groups {
        out.push_str(&format!(
            "| {} | {} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} |\n",
            group.group_name,
            group.sample_count,
            group.mean,
            group.median,
            group.q1,
            group.q3,
            group.min,
            group.max
        ));
    }
    out.push('\n');
    out.push_str(&format!(
        "Mann-Whitney U: {}\n",
        format_optional_stat(result.mann_whitney_u, 3)
    ));
    out.push_str(&format!(
        "Mann-Whitney p-value: {}\n",
        format_optional_p_value(result.mann_whitney_p)
    ));
    out
}

pub fn study_compare_mutations_markdown(result: &StudyMutationComparisonResult) -> String {
    let mut out = String::new();
    out.push_str("# Study Group Comparison: Mutation Rate\n\n");
    out.push_str(&format!(
        "Stratify gene: {} | Target gene: {} | Study: {}\n\n",
        result.stratify_gene, result.target_gene, result.study_id
    ));
    out.push_str("| Group | N | Mutated | Mutation Rate |\n");
    out.push_str("|---|---|---|---|\n");
    for group in &result.groups {
        out.push_str(&format!(
            "| {} | {} | {} | {:.6} |\n",
            group.group_name, group.sample_count, group.mutated_count, group.mutation_rate
        ));
    }
    out
}

pub fn study_co_occurrence_markdown(result: &StudyCoOccurrenceResult) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Study Co-occurrence: {}\n\n", result.study_id));
    out.push_str(&format!("Genes: {}\n\n", result.genes.join(", ")));
    out.push_str(&format!("Total samples: {}\n\n", result.total_samples));
    out.push_str(&format!(
        "Sample universe: {}\n\n",
        match result.sample_universe_basis {
            StudySampleUniverseBasis::ClinicalSampleFile => "clinical sample file",
            StudySampleUniverseBasis::MutationObserved => {
                "mutation-observed samples only (clinical sample file unavailable)"
            }
        }
    ));
    out.push_str(
        "| Gene A | Gene B | Both | A only | B only | Neither | Log Odds Ratio | p-value |\n",
    );
    out.push_str("|---|---|---|---|---|---|---|---|\n");
    if result.pairs.is_empty() {
        out.push_str("| - | - | 0 | 0 | 0 | 0 | - | - |\n");
        return out;
    }
    for pair in &result.pairs {
        let lor = pair
            .log_odds_ratio
            .map(|v| format!("{v:.6}"))
            .unwrap_or_else(|| "-".to_string());
        let p_value = pair
            .p_value
            .map(|v| format!("{v:.3e}"))
            .unwrap_or_else(|| "-".to_string());
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
            pair.gene_a,
            pair.gene_b,
            pair.both_mutated,
            pair.a_only,
            pair.b_only,
            pair.neither,
            lor,
            p_value
        ));
    }
    out
}

pub fn search_all_markdown(
    results: &SearchAllResults,
    counts_only: bool,
) -> Result<String, BioMcpError> {
    #[derive(serde::Serialize)]
    struct SearchAllSectionView {
        entity: String,
        label: String,
        heading_count: usize,
        error: Option<String>,
        note: Option<String>,
        links: Vec<crate::cli::search_all::SearchAllLink>,
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
    }

    let tmpl = env()?.get_template("search_all.md.j2")?;
    let sections = results
        .sections
        .iter()
        .map(|section| {
            let rows = section.markdown_rows();
            let heading_count = if counts_only {
                section.total.unwrap_or(section.count)
            } else {
                rows.len()
            };
            SearchAllSectionView {
                entity: section.entity.clone(),
                label: section.label.clone(),
                heading_count,
                error: section.error.clone(),
                note: section.note.clone(),
                links: section.links.clone(),
                columns: section
                    .markdown_columns()
                    .iter()
                    .map(|column| (*column).to_string())
                    .collect(),
                rows,
            }
        })
        .collect::<Vec<_>>();

    let body = tmpl.render(context! {
        query => &results.query,
        sections => sections,
        counts_only => counts_only,
        searches_dispatched => results.searches_dispatched,
        searches_with_results => results.searches_with_results,
        wall_time_ms => results.wall_time_ms,
    })?;

    if let Some(debug_plan) = results.debug_plan.as_ref() {
        Ok(format!("{}{}", render_debug_plan_block(debug_plan)?, body))
    } else {
        Ok(body)
    }
}

pub fn render_discover(result: &DiscoverResult) -> Result<String, BioMcpError> {
    #[derive(serde::Serialize)]
    struct DiscoverConceptView {
        label: String,
        primary_id: Option<String>,
        synonyms: Vec<String>,
        xrefs: Vec<String>,
        sources: Vec<String>,
    }

    #[derive(serde::Serialize)]
    struct DiscoverGroupView {
        label: String,
        concepts: Vec<DiscoverConceptView>,
    }

    let tmpl = env()?.get_template("discover.md.j2")?;
    let groups = [
        DiscoverType::Gene,
        DiscoverType::Drug,
        DiscoverType::Disease,
        DiscoverType::Symptom,
        DiscoverType::Pathway,
        DiscoverType::Variant,
        DiscoverType::Unknown,
    ]
    .into_iter()
    .filter_map(|kind| {
        let concepts = result
            .concepts
            .iter()
            .filter(|concept| concept.primary_type == kind)
            .map(|concept| DiscoverConceptView {
                label: concept.label.clone(),
                primary_id: concept.primary_id.clone(),
                synonyms: concept.synonyms.clone(),
                xrefs: concept
                    .xrefs
                    .iter()
                    .map(|xref| format!("{}:{}", xref.source, xref.id))
                    .collect(),
                sources: concept
                    .sources
                    .iter()
                    .map(|source| format!("{} ({})", source.source, source.source_type))
                    .collect(),
            })
            .collect::<Vec<_>>();
        if concepts.is_empty() {
            None
        } else {
            Some(DiscoverGroupView {
                label: kind.label().to_string(),
                concepts,
            })
        }
    })
    .collect::<Vec<_>>();

    let body = tmpl.render(context! {
        query => &result.query,
        notes => &result.notes,
        ambiguous => result.ambiguous,
        groups => groups,
        plain_language => &result.plain_language,
        next_commands => &result.next_commands,
    })?;
    Ok(append_evidence_urls(body, discover_evidence_urls(result)))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::entities::adverse_event::DeviceEvent;
    use crate::entities::article::{
        AnnotationCount, Article, ArticleAnnotations, ArticleRankingOptions, ArticleSearchFilters,
        ArticleSearchResult, ArticleSource,
    };
    use crate::entities::disease::{Disease, DiseaseVariantAssociation};
    use crate::entities::drug::Drug;
    use crate::entities::gene::Gene;
    use crate::entities::pathway::Pathway;
    use crate::entities::pgx::Pgx;
    use crate::entities::study::{
        CnaDistributionResult as StudyCnaDistributionResult,
        CoOccurrencePair as StudyCoOccurrencePair, CoOccurrenceResult as StudyCoOccurrenceResult,
        CohortResult as StudyCohortResult,
        ExpressionComparisonResult as StudyExpressionComparisonResult,
        ExpressionDistributionResult as StudyExpressionDistributionResult,
        ExpressionGroupStats as StudyExpressionGroupStats,
        MutationComparisonResult as StudyMutationComparisonResult,
        MutationFrequencyResult as StudyMutationFrequencyResult,
        MutationGroupStats as StudyMutationGroupStats,
        SampleUniverseBasis as StudySampleUniverseBasis, StudyDownloadCatalog, StudyDownloadResult,
        StudyInfo, StudyQueryResult, SurvivalEndpoint as StudySurvivalEndpoint,
        SurvivalGroupResult as StudySurvivalGroupResult, SurvivalResult as StudySurvivalResult,
        TopMutatedGeneRow as StudyTopMutatedGeneRow,
        TopMutatedGenesResult as StudyTopMutatedGenesResult,
    };
    use crate::entities::variant::{TreatmentImplication, Variant, VariantOncoKbResult};

    fn article_filters_for_test(sort: ArticleSort) -> ArticleSearchFilters {
        ArticleSearchFilters {
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
            sort,
            ranking: ArticleRankingOptions::default(),
        }
    }

    #[test]
    fn quote_arg_wraps_whitespace_and_escapes_quotes() {
        assert_eq!(quote_arg("BRAF"), "BRAF");
        assert_eq!(quote_arg("BRAF V600E"), "\"BRAF V600E\"");
        assert_eq!(quote_arg("BRAF \"V600E\""), "\"BRAF \\\"V600E\\\"\"");
    }

    #[test]
    fn trial_search_markdown_with_footer_shows_scoped_zero_result_nickname_hint() {
        let markdown = trial_search_markdown_with_footer(
            "condition=CodeBreaK 300",
            &[],
            Some(0),
            "",
            true,
            Some("CodeBreaK 300"),
        )
        .expect("markdown");

        assert!(markdown.contains("ClinicalTrials.gov does not index trial nicknames."));
        assert!(markdown.contains("biomcp search trial -i \"<drug>\" -c \"<condition>\""));
        assert!(markdown.contains("biomcp search article \"CodeBreaK 300\" to find the NCT ID"));
    }

    #[test]
    fn trial_search_markdown_with_footer_omits_zero_result_nickname_hint_without_flag() {
        let markdown =
            trial_search_markdown_with_footer("condition=melanoma", &[], Some(0), "", false, None)
                .expect("markdown");

        assert!(!markdown.contains("ClinicalTrials.gov does not index trial nicknames."));
    }

    #[test]
    fn sections_pathway_for_kegg_excludes_unsupported_sections() {
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

        let sections = sections_pathway(&pathway, &[]);
        assert_eq!(sections, vec!["genes".to_string()]);
    }

    #[test]
    fn sections_pathway_for_reactome_keeps_full_supported_set() {
        let pathway = Pathway {
            source: "Reactome".to_string(),
            id: "R-HSA-5673001".to_string(),
            name: "RAF/MAP kinase cascade".to_string(),
            species: None,
            summary: None,
            genes: Vec::new(),
            events: Vec::new(),
            enrichment: Vec::new(),
        };

        let sections = sections_pathway(&pathway, &[]);
        assert_eq!(
            sections,
            vec![
                "genes".to_string(),
                "events".to_string(),
                "enrichment".to_string()
            ]
        );
    }

    #[test]
    fn gene_markdown_includes_evidence_links() {
        let gene = Gene {
            symbol: "BRAF".to_string(),
            name: "B-Raf proto-oncogene".to_string(),
            entrez_id: "673".to_string(),
            ensembl_id: Some("ENSG00000157764".to_string()),
            location: Some("7q34".to_string()),
            genomic_coordinates: None,
            omim_id: None,
            uniprot_id: Some("P15056".to_string()),
            summary: Some("Kinase involved in MAPK signaling.".to_string()),
            gene_type: Some("protein-coding".to_string()),
            aliases: vec!["BRAF1".to_string()],
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

        let markdown = gene_markdown(&gene, &[]).expect("rendered markdown");
        assert!(markdown.contains("BRAF"));
        assert!(markdown.contains("[NCBI Gene](https://www.ncbi.nlm.nih.gov/gene/673)"));
        assert!(markdown.contains("[UniProt](https://www.uniprot.org/uniprot/P15056)"));
    }

    #[test]
    fn markdown_detail_outputs_label_gene_drug_and_disease_sources() {
        let gene = Gene {
            symbol: "CFTR".to_string(),
            name: "CF transmembrane conductance regulator".to_string(),
            entrez_id: "1080".to_string(),
            ensembl_id: None,
            location: Some("7q31.2".to_string()),
            genomic_coordinates: None,
            omim_id: None,
            uniprot_id: Some("P13569".to_string()),
            summary: Some("Chloride channel.".to_string()),
            gene_type: Some("protein-coding".to_string()),
            aliases: vec!["ABCC7".to_string()],
            clinical_diseases: Vec::new(),
            clinical_drugs: Vec::new(),
            pathways: None,
            ontology: Some(vec![crate::entities::gene::EnrichmentResult {
                library: "GO_Biological_Process_2025".to_string(),
                terms: vec![crate::entities::gene::EnrichmentTerm {
                    name: "ion transport".to_string(),
                    p_value: 0.001,
                    genes: "CFTR".to_string(),
                }],
            }]),
            diseases: Some(vec![crate::entities::gene::EnrichmentResult {
                library: "DisGeNET".to_string(),
                terms: vec![crate::entities::gene::EnrichmentTerm {
                    name: "cystic fibrosis".to_string(),
                    p_value: 0.002,
                    genes: "CFTR".to_string(),
                }],
            }]),
            protein: Some(crate::entities::gene::GeneProtein {
                accession: "P13569".to_string(),
                name: "CFTR".to_string(),
                function: Some("ATP-binding cassette transporter.".to_string()),
                length: Some(1480),
                isoforms: Vec::new(),
                alternative_names: Vec::new(),
            }),
            go: Some(vec![crate::entities::gene::GeneGoTerm {
                id: "GO:0006811".to_string(),
                name: "ion transport".to_string(),
                aspect: Some("BP".to_string()),
                evidence: Some("EXP".to_string()),
            }]),
            interactions: Some(vec![crate::entities::gene::GeneInteraction {
                partner: "SLC26A9".to_string(),
                score: Some(0.83),
            }]),
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
        let gene_markdown = gene_markdown(&gene, &[]).expect("gene markdown");
        assert!(gene_markdown.contains("Source: NCBI Gene / MyGene.info"));
        assert!(gene_markdown.contains("## Summary (NCBI Gene)"));
        assert!(gene_markdown.contains("## Aliases (NCBI Gene / MyGene.info)"));
        assert!(gene_markdown.contains("## Ontology (Enrichr)"));
        assert!(gene_markdown.contains("## Diseases (Enrichr)"));
        assert!(gene_markdown.contains("## Protein (UniProt)"));
        assert!(gene_markdown.contains("## GO Terms (QuickGO)"));
        assert!(gene_markdown.contains("## Interactions (STRING)"));
        assert!(gene_markdown.contains("See also:"));
        assert!(gene_markdown.contains("biomcp search variant -g CFTR"));

        let drug = Drug {
            name: "ivacaftor".to_string(),
            drugbank_id: Some("DB08820".to_string()),
            chembl_id: Some("CHEMBL1200749".to_string()),
            unii: None,
            drug_type: Some("small molecule".to_string()),
            mechanism: Some("CFTR potentiator".to_string()),
            mechanisms: vec!["Potentiates CFTR chloride transport.".to_string()],
            approval_date: Some("2012-01-31".to_string()),
            approval_date_raw: Some("2012-01-31".to_string()),
            approval_date_display: Some("January 31, 2012".to_string()),
            approval_summary: Some("FDA approved on January 31, 2012".to_string()),
            brand_names: vec!["Kalydeco".to_string()],
            route: Some("Oral".to_string()),
            targets: vec!["CFTR".to_string()],
            variant_targets: Vec::new(),
            target_family: None,
            target_family_name: None,
            indications: vec!["Cystic fibrosis".to_string()],
            interactions: vec![crate::entities::drug::DrugInteraction {
                drug: "rifampin".to_string(),
                description: Some("May reduce ivacaftor exposure.".to_string()),
            }],
            interaction_text: None,
            pharm_classes: Vec::new(),
            top_adverse_events: vec!["Cough".to_string()],
            faers_query: None,
            label: None,
            label_set_id: None,
            shortage: Some(Vec::new()),
            approvals: Some(Vec::new()),
            us_safety_warnings: None,
            ema_regulatory: None,
            ema_safety: None,
            ema_shortage: None,
            who_prequalification: None,
            civic: None,
        };
        let drug_markdown = drug_markdown(&drug, &["all".to_string()]).expect("drug markdown");
        assert!(drug_markdown.contains("Type (MyChem.info): small molecule"));
        assert!(drug_markdown.contains("FDA Approved (DrugCentral): January 31, 2012"));
        assert!(drug_markdown.contains("Brand Names (DrugBank): Kalydeco"));
        assert!(drug_markdown.contains("Safety (OpenFDA FAERS): Cough"));
        assert!(drug_markdown.contains("## Mechanisms (MyChem.info / ChEMBL)"));
        assert!(drug_markdown.contains("## Targets (ChEMBL / Open Targets)"));
        assert!(drug_markdown.contains("## Indications (Open Targets)"));
        assert!(drug_markdown.contains("## Interactions (DrugBank)"));
        assert!(drug_markdown.contains("## Shortage (US - OpenFDA Drug Shortages)"));
        assert!(drug_markdown.contains("## Regulatory (US - Drugs@FDA)"));

        let disease = crate::entities::disease::Disease {
            id: "MONDO:0009061".to_string(),
            name: "cystic fibrosis".to_string(),
            definition: Some("Inherited disease affecting chloride transport.".to_string()),
            synonyms: vec!["CF".to_string()],
            parents: vec!["autosomal recessive disease".to_string()],
            associated_genes: vec!["CFTR".to_string()],
            gene_associations: Vec::new(),
            top_genes: vec!["CFTR".to_string()],
            top_gene_scores: Vec::new(),
            treatment_landscape: vec!["ivacaftor".to_string()],
            recruiting_trial_count: Some(4),
            pathways: vec![crate::entities::disease::DiseasePathway {
                id: "R-HSA-5673001".to_string(),
                name: "Ion channel transport".to_string(),
            }],
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
        let disease_markdown =
            disease_markdown(&disease, &["all".to_string()]).expect("disease markdown");
        assert!(disease_markdown.contains("## Definition (MyDisease.info)"));
        assert!(disease_markdown.contains("Inherited disease affecting chloride transport."));
        assert!(disease_markdown.contains("Genes (Open Targets): CFTR"));
        assert!(disease_markdown.contains("Treatments (MyChem.info indication search): ivacaftor"));
        assert!(disease_markdown.contains("Recruiting Trials (ClinicalTrials.gov): 4"));
        assert!(
            disease_markdown.contains("## Synonyms (MONDO / Disease Ontology via MyDisease.info)")
        );
        assert!(
            disease_markdown.contains("## Parents (MONDO / Disease Ontology via MyDisease.info)")
        );
        assert!(disease_markdown.contains("## Pathways (Reactome)"));
    }

    #[test]
    fn markdown_detail_outputs_label_article_trial_and_pathway_sources() {
        let article = Article {
            pmid: Some("22663011".to_string()),
            pmcid: Some("PMC9984800".to_string()),
            doi: Some("10.1000/example".to_string()),
            title: "Example article".to_string(),
            authors: vec!["A. Author".to_string()],
            journal: Some("Example Journal".to_string()),
            date: Some("2012-05-31".to_string()),
            citation_count: Some(12),
            publication_type: Some("Journal Article".to_string()),
            open_access: Some(true),
            abstract_text: Some("Abstract text.".to_string()),
            full_text_path: None,
            full_text_note: Some("Saved full text unavailable.".to_string()),
            annotations: Some(ArticleAnnotations {
                genes: vec![AnnotationCount {
                    text: "CFTR".to_string(),
                    count: 1,
                }],
                diseases: Vec::new(),
                chemicals: Vec::new(),
                mutations: Vec::new(),
            }),
            semantic_scholar: Some(crate::entities::article::ArticleSemanticScholar {
                paper_id: Some("paper-1".to_string()),
                tldr: Some("TLDR".to_string()),
                citation_count: Some(10),
                influential_citation_count: Some(2),
                reference_count: Some(5),
                is_open_access: Some(true),
                open_access_pdf: None,
            }),
            pubtator_fallback: false,
        };
        let article_markdown = article_markdown(&article, &["all".to_string()]).expect("article");
        assert!(article_markdown.contains("Source: PubMed / Europe PMC"));
        assert!(article_markdown.contains("## Authors (PubMed / Europe PMC)"));
        assert!(article_markdown.contains("## Abstract (PubMed / Europe PMC)"));
        assert!(article_markdown.contains("## PubTator Annotations"));
        assert!(article_markdown.contains("## Full Text (PMC OA)"));
        assert!(article_markdown.contains("## Semantic Scholar"));

        let trial = crate::entities::trial::Trial {
            nct_id: "NCT06668103".to_string(),
            source: Some("ClinicalTrials.gov".to_string()),
            title: "Example trial".to_string(),
            status: "Recruiting".to_string(),
            phase: Some("Phase 2".to_string()),
            study_type: Some("Interventional".to_string()),
            age_range: Some("18 Years and older".to_string()),
            conditions: vec!["cystic fibrosis".to_string()],
            interventions: vec!["ivacaftor".to_string()],
            sponsor: Some("Example Sponsor".to_string()),
            enrollment: Some(42),
            summary: Some("Trial summary.".to_string()),
            start_date: Some("2025-01-01".to_string()),
            completion_date: None,
            eligibility_text: Some("Eligibility text.".to_string()),
            locations: Some(vec![crate::entities::trial::TrialLocation {
                facility: "Example Hospital".to_string(),
                city: "Boston".to_string(),
                state: Some("MA".to_string()),
                country: "United States".to_string(),
                status: Some("Recruiting".to_string()),
                contact_name: None,
                contact_phone: None,
            }]),
            outcomes: Some(crate::entities::trial::TrialOutcomes {
                primary: vec![crate::entities::trial::TrialOutcome {
                    measure: "FEV1".to_string(),
                    description: None,
                    time_frame: None,
                }],
                secondary: Vec::new(),
            }),
            arms: Some(vec![crate::entities::trial::TrialArm {
                label: "Arm A".to_string(),
                arm_type: Some("Experimental".to_string()),
                description: Some("Description".to_string()),
                interventions: vec!["ivacaftor".to_string()],
            }]),
            references: Some(vec![crate::entities::trial::TrialReference {
                pmid: Some("22663011".to_string()),
                citation: "Example citation".to_string(),
                reference_type: Some("background".to_string()),
            }]),
        };
        let trial_markdown = trial_markdown(&trial, &["all".to_string()]).expect("trial");
        assert!(trial_markdown.contains("Source: ClinicalTrials.gov"));
        assert!(trial_markdown.contains("## Conditions (ClinicalTrials.gov)"));
        assert!(trial_markdown.contains("## Interventions (ClinicalTrials.gov)"));
        assert!(trial_markdown.contains("## Summary (ClinicalTrials.gov)"));
        assert!(trial_markdown.contains("## Eligibility (ClinicalTrials.gov)"));
        assert!(trial_markdown.contains("## Locations (ClinicalTrials.gov)"));
        assert!(trial_markdown.contains("## Outcomes (ClinicalTrials.gov)"));
        assert!(trial_markdown.contains("## Arms (ClinicalTrials.gov)"));
        assert!(trial_markdown.contains("## References (ClinicalTrials.gov)"));

        let pathway = Pathway {
            source: "Reactome".to_string(),
            id: "R-HSA-5358351".to_string(),
            name: "Signal transduction".to_string(),
            species: Some("Homo sapiens".to_string()),
            summary: Some("Reactome summary.".to_string()),
            genes: vec!["CFTR".to_string()],
            events: vec!["Channel gating".to_string()],
            enrichment: vec![crate::entities::pathway::PathwayEnrichment {
                source: "Reactome".to_string(),
                id: "R-HSA-1234".to_string(),
                name: "Transport".to_string(),
                p_value: Some(0.001),
            }],
        };
        let pathway_markdown = pathway_markdown(&pathway, &["all".to_string()]).expect("pathway");
        assert!(pathway_markdown.contains("## Summary (Reactome)"));
        assert!(pathway_markdown.contains("## Genes (Reactome)"));
        assert!(pathway_markdown.contains("## Events (Reactome)"));
        assert!(pathway_markdown.contains("## Enrichment (g:Profiler)"));
    }

    #[test]
    fn markdown_detail_outputs_label_variant_protein_pgx_and_openfda_sources() {
        let variant: Variant = serde_json::from_value(serde_json::json!({
            "id": "rs334",
            "gene": "HBB",
            "hgvs_p": "p.Glu7Val",
            "rsid": "rs334",
            "prediction": {
                "expression_lfc": 0.3,
                "splice_score": 0.1,
                "chromatin_score": 0.2,
                "top_gene": "HBB"
            },
            "gnomad_af": 0.01,
            "conservation": {
                "phylop_100way_vertebrate": 0.8
            },
            "expanded_predictions": [
                {"tool": "REVEL", "score": 0.91, "prediction": "Damaging"}
            ],
            "cgi_associations": [
                {"drug": "hydroxyurea", "association": "responsive"}
            ]
        }))
        .expect("variant");
        let variant_markdown = variant_markdown(&variant, &["all".to_string()]).expect("variant");
        assert!(variant_markdown.contains("Source: MyVariant.info / ClinVar"));
        assert!(variant_markdown.contains("## AlphaGenome Prediction"));
        assert!(variant_markdown.contains("## Population (gnomAD via MyVariant.info)"));
        assert!(variant_markdown.contains("## Conservation (MyVariant.info)"));
        assert!(variant_markdown.contains("## Expanded Predictions (MyVariant.info)"));
        assert!(variant_markdown.contains("## CGI Drug Associations (Cancer Genome Interpreter)"));

        let protein = crate::entities::protein::Protein {
            accession: "P15056".to_string(),
            entry_id: Some("BRAF_HUMAN".to_string()),
            name: "Serine/threonine-protein kinase B-raf".to_string(),
            gene_symbol: Some("BRAF".to_string()),
            organism: Some("Homo sapiens".to_string()),
            length: Some(766),
            function: Some("Kinase function.".to_string()),
            structures: vec!["6V34".to_string()],
            structure_count: Some(1),
            domains: vec![crate::entities::protein::ProteinDomain {
                accession: "IPR000719".to_string(),
                name: Some("Protein kinase domain".to_string()),
                domain_type: Some("domain".to_string()),
            }],
            interactions: vec![crate::entities::protein::ProteinInteraction {
                partner: "MEK1".to_string(),
                score: Some(0.92),
            }],
            complexes: vec![crate::entities::protein::ProteinComplex {
                accession: "CPX-1".to_string(),
                name: "BRAF complex".to_string(),
                description: None,
                curation: crate::entities::protein::ProteinComplexCuration::Curated,
                components: vec![crate::entities::protein::ProteinComplexComponent {
                    accession: "P15056".to_string(),
                    name: "BRAF".to_string(),
                    stoichiometry: None,
                }],
            }],
        };
        let protein_markdown = protein_markdown(&protein, &["all".to_string()]).expect("protein");
        assert!(protein_markdown.contains("Source: UniProt"));
        assert!(protein_markdown.contains("## Function (UniProt)"));
        assert!(protein_markdown.contains("## Structures (PDB / AlphaFold via UniProt)"));
        assert!(protein_markdown.contains("## Domains (InterPro)"));
        assert!(protein_markdown.contains("## Interactions (STRING)"));
        assert!(protein_markdown.contains("## Complexes (ComplexPortal)"));

        let pgx = Pgx {
            query: "CYP2D6".to_string(),
            gene: Some("CYP2D6".to_string()),
            drug: Some("codeine".to_string()),
            interactions: vec![crate::entities::pgx::PgxInteraction {
                genesymbol: "CYP2D6".to_string(),
                drugname: "codeine".to_string(),
                cpiclevel: Some("A".to_string()),
                pgxtesting: Some("Recommended".to_string()),
                guidelinename: None,
                guidelineurl: None,
            }],
            recommendations: vec![crate::entities::pgx::PgxRecommendation {
                drugname: "codeine".to_string(),
                phenotype: Some("Poor metabolizer".to_string()),
                activity_score: None,
                implication: None,
                recommendation: Some("Avoid codeine".to_string()),
                classification: Some("Strong".to_string()),
                population: None,
                guidelinename: None,
                guidelineurl: None,
            }],
            frequencies: vec![crate::entities::pgx::PgxFrequency {
                genesymbol: "CYP2D6".to_string(),
                allele: "*4".to_string(),
                population_group: Some("European".to_string()),
                subject_count: None,
                frequency: None,
                min_frequency: None,
                max_frequency: None,
            }],
            guidelines: vec![crate::entities::pgx::PgxGuideline {
                name: "CPIC Guideline".to_string(),
                url: Some("https://example.org/guideline".to_string()),
                genes: vec!["CYP2D6".to_string()],
                drugs: vec!["codeine".to_string()],
            }],
            annotations: Vec::new(),
            annotations_note: Some("PharmGKB note.".to_string()),
        };
        let pgx_markdown = pgx_markdown(&pgx, &["all".to_string()]).expect("pgx");
        assert!(pgx_markdown.contains("Source: CPIC"));
        assert!(pgx_markdown.contains("## Interactions (CPIC)"));
        assert!(pgx_markdown.contains("## Recommendations (CPIC)"));
        assert!(pgx_markdown.contains("## Population Frequencies (CPIC)"));
        assert!(pgx_markdown.contains("| CYP2D6 | *4 | European | - | - |"));
        assert!(pgx_markdown.contains("## Guidelines (CPIC)"));

        let faers = crate::entities::adverse_event::AdverseEvent {
            report_id: "10329882".to_string(),
            drug: "ivacaftor".to_string(),
            reactions: vec!["Cough".to_string()],
            outcomes: vec!["Hospitalization".to_string()],
            patient: None,
            concomitant_medications: vec!["azithromycin".to_string()],
            reporter_type: None,
            reporter_country: None,
            indication: None,
            serious: true,
            date: Some("2024-01-01".to_string()),
        };
        let faers_markdown = adverse_event_markdown(&faers, &["all".to_string()]).expect("faers");
        assert!(faers_markdown.contains("Source: OpenFDA"));
        assert!(faers_markdown.contains("## Reactions (OpenFDA)"));
        assert!(faers_markdown.contains("## Outcomes (OpenFDA)"));
        assert!(faers_markdown.contains("## Concomitant Drugs (OpenFDA)"));

        let device = DeviceEvent {
            report_id: "MDR-123".to_string(),
            report_number: None,
            device: "Infusion Pump".to_string(),
            manufacturer: Some("Example".to_string()),
            event_type: Some("Malfunction".to_string()),
            date: Some("2024-02-01".to_string()),
            description: Some("Description text.".to_string()),
        };
        let device_markdown = device_event_markdown(&device).expect("device");
        assert!(device_markdown.contains("Source: OpenFDA"));
        assert!(device_markdown.contains("## Description (OpenFDA)"));
    }

    #[test]
    fn gene_markdown_section_only_shows_new_gene_enrichment_sections() {
        let gene = Gene {
            symbol: "BRAF".to_string(),
            name: "B-Raf proto-oncogene".to_string(),
            entrez_id: "673".to_string(),
            ensembl_id: Some("ENSG00000157764".to_string()),
            location: Some("7q34".to_string()),
            genomic_coordinates: None,
            omim_id: None,
            uniprot_id: Some("P15056".to_string()),
            summary: Some("Kinase involved in MAPK signaling.".to_string()),
            gene_type: Some("protein-coding".to_string()),
            aliases: vec!["BRAF1".to_string()],
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

        let markdown = gene_markdown(
            &gene,
            &[
                "expression".to_string(),
                "hpa".to_string(),
                "druggability".to_string(),
                "clingen".to_string(),
            ],
        )
        .expect("rendered markdown");

        assert!(markdown.contains("# BRAF - expression, hpa, druggability, clingen"));
        assert!(markdown.contains("## Expression (GTEx)"));
        assert!(markdown.contains("## Human Protein Atlas"));
        assert!(markdown.contains("## Druggability"));
        assert!(markdown.contains("## ClinGen"));
        assert!(markdown.contains("No GTEx expression records returned"));
        assert!(markdown.contains("No Human Protein Atlas records returned"));
        assert!(markdown.contains("No DGIdb interactions returned"));
        assert!(markdown.contains("No ClinGen records returned"));
    }

    #[test]
    fn gene_markdown_renders_combined_dgidb_and_opentargets_druggability() {
        let gene = Gene {
            symbol: "EGFR".to_string(),
            name: "epidermal growth factor receptor".to_string(),
            entrez_id: "1956".to_string(),
            ensembl_id: Some("ENSG00000146648".to_string()),
            location: Some("7p11.2".to_string()),
            genomic_coordinates: None,
            omim_id: None,
            uniprot_id: Some("P00533".to_string()),
            summary: None,
            gene_type: Some("protein-coding".to_string()),
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
            druggability: Some(crate::sources::dgidb::GeneDruggability {
                categories: vec!["Kinase".to_string()],
                interactions: Vec::new(),
                tractability: vec![
                    crate::sources::dgidb::GeneTractabilityModality {
                        modality: "small molecule".to_string(),
                        tractable: true,
                        evidence_labels: vec!["Approved Drug".to_string()],
                    },
                    crate::sources::dgidb::GeneTractabilityModality {
                        modality: "antibody".to_string(),
                        tractable: true,
                        evidence_labels: vec!["Clinical Precedence".to_string()],
                    },
                ],
                safety_liabilities: vec![crate::sources::dgidb::GeneSafetyLiability {
                    event: "Skin rash".to_string(),
                    datasource: Some("ForceGenetics".to_string()),
                    effect_direction: Some("activation".to_string()),
                    biosample: Some("Skin".to_string()),
                }],
            }),
            clingen: None,
            constraint: None,
            disgenet: None,
            funding: None,
            funding_note: None,
        };

        let markdown =
            gene_markdown(&gene, &["druggability".to_string()]).expect("rendered markdown");

        assert!(markdown.contains("## Druggability"));
        assert!(!markdown.contains("## Druggability (DGIdb)"));
        assert!(markdown.contains("OpenTargets tractability"));
        assert!(markdown.contains("| Modality | Tractable | Evidence |"));
        assert!(markdown.contains("| small molecule | yes | Approved Drug |"));
        assert!(markdown.contains("| antibody | yes | Clinical Precedence |"));
        assert!(markdown.contains("OpenTargets safety liabilities"));
        assert!(markdown.contains("Skin rash"));
        assert!(markdown.contains("No DGIdb interactions returned for this gene query."));
    }

    #[test]
    fn gene_markdown_renders_dgidb_interaction_table_alongside_opentargets_data() {
        let gene = Gene {
            symbol: "BRAF".to_string(),
            name: "B-Raf proto-oncogene".to_string(),
            entrez_id: "673".to_string(),
            ensembl_id: None,
            location: None,
            genomic_coordinates: None,
            omim_id: None,
            uniprot_id: None,
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
            druggability: Some(crate::sources::dgidb::GeneDruggability {
                categories: vec!["Kinase".to_string()],
                interactions: vec![crate::sources::dgidb::DrugInteraction {
                    drug: "Dabrafenib".to_string(),
                    interaction_types: vec!["inhibitor".to_string()],
                    score: Some(1.2),
                    approved: Some(true),
                    source_count: 2,
                }],
                tractability: vec![crate::sources::dgidb::GeneTractabilityModality {
                    modality: "small molecule".to_string(),
                    tractable: true,
                    evidence_labels: vec!["Approved Drug".to_string()],
                }],
                safety_liabilities: Vec::new(),
            }),
            clingen: None,
            constraint: None,
            disgenet: None,
            funding: None,
            funding_note: None,
        };

        let markdown =
            gene_markdown(&gene, &["druggability".to_string()]).expect("rendered markdown");

        assert!(markdown.contains("## Druggability"));
        assert!(markdown.contains("OpenTargets tractability"));
        assert!(markdown.contains("| small molecule | yes | Approved Drug |"));
        // DGIdb interaction table renders (not replaced by empty-state message)
        assert!(markdown.contains("| Drug | Interaction Types | Score | Approved | Sources |"));
        assert!(markdown.contains("| Dabrafenib | inhibitor | 1.200 | yes | 2 |"));
        assert!(!markdown.contains("No DGIdb interactions returned for this gene query."));
    }

    #[test]
    fn disease_markdown_renders_opentargets_scores_in_summary_and_genes_table() {
        let disease = Disease {
            id: "MONDO:0005105".to_string(),
            name: "melanoma".to_string(),
            definition: None,
            synonyms: Vec::new(),
            parents: Vec::new(),
            associated_genes: vec!["BRAF".to_string(), "NRAS".to_string()],
            gene_associations: vec![
                crate::entities::disease::DiseaseGeneAssociation {
                    gene: "BRAF".to_string(),
                    relationship: Some("causal".to_string()),
                    source: Some("Monarch".to_string()),
                    opentargets_score: Some(
                        crate::entities::disease::DiseaseAssociationScoreSummary {
                            overall_score: 0.912,
                            gwas_score: Some(0.321),
                            rare_variant_score: Some(0.654),
                            somatic_mutation_score: Some(0.876),
                        },
                    ),
                },
                crate::entities::disease::DiseaseGeneAssociation {
                    gene: "NRAS".to_string(),
                    relationship: Some("associated".to_string()),
                    source: Some("CIViC".to_string()),
                    opentargets_score: None,
                },
            ],
            top_genes: vec!["BRAF".to_string(), "NRAS".to_string()],
            top_gene_scores: vec![
                crate::entities::disease::DiseaseTargetScore {
                    symbol: "BRAF".to_string(),
                    summary: crate::entities::disease::DiseaseAssociationScoreSummary {
                        overall_score: 0.912,
                        gwas_score: Some(0.321),
                        rare_variant_score: Some(0.654),
                        somatic_mutation_score: Some(0.876),
                    },
                },
                crate::entities::disease::DiseaseTargetScore {
                    symbol: "NRAS".to_string(),
                    summary: crate::entities::disease::DiseaseAssociationScoreSummary {
                        overall_score: 0.701,
                        gwas_score: None,
                        rare_variant_score: None,
                        somatic_mutation_score: Some(0.443),
                    },
                },
            ],
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

        let summary = disease_markdown(&disease, &[]).expect("rendered markdown");
        assert!(summary.contains("Genes (Open Targets): BRAF (OT 0.912), NRAS (OT 0.701)"));

        let genes = disease_markdown(&disease, &["genes".to_string()]).expect("rendered markdown");
        assert!(genes.contains("| Gene | Relationship | Source | OpenTargets |"));
        assert!(genes.contains("overall 0.912; GWAS 0.321; rare 0.654; somatic 0.876"));
        assert!(genes.contains("| NRAS | associated | CIViC | - |"));
    }

    pub(crate) fn proof_disease_markdown_renders_ot_only_gene_association_table() {
        let disease = Disease {
            id: "MONDO:0003864".to_string(),
            name: "chronic lymphocytic leukemia".to_string(),
            definition: None,
            synonyms: Vec::new(),
            parents: Vec::new(),
            associated_genes: vec!["TP53".to_string(), "ATM".to_string()],
            gene_associations: vec![
                crate::entities::disease::DiseaseGeneAssociation {
                    gene: "TP53".to_string(),
                    relationship: Some("associated with disease".to_string()),
                    source: Some("OpenTargets".to_string()),
                    opentargets_score: Some(
                        crate::entities::disease::DiseaseAssociationScoreSummary {
                            overall_score: 0.991,
                            gwas_score: None,
                            rare_variant_score: None,
                            somatic_mutation_score: Some(0.881),
                        },
                    ),
                },
                crate::entities::disease::DiseaseGeneAssociation {
                    gene: "ATM".to_string(),
                    relationship: Some("associated with disease".to_string()),
                    source: Some("OpenTargets".to_string()),
                    opentargets_score: Some(
                        crate::entities::disease::DiseaseAssociationScoreSummary {
                            overall_score: 0.942,
                            gwas_score: None,
                            rare_variant_score: None,
                            somatic_mutation_score: Some(0.731),
                        },
                    ),
                },
            ],
            top_genes: vec!["TP53".to_string(), "ATM".to_string()],
            top_gene_scores: vec![
                crate::entities::disease::DiseaseTargetScore {
                    symbol: "TP53".to_string(),
                    summary: crate::entities::disease::DiseaseAssociationScoreSummary {
                        overall_score: 0.991,
                        gwas_score: None,
                        rare_variant_score: None,
                        somatic_mutation_score: Some(0.881),
                    },
                },
                crate::entities::disease::DiseaseTargetScore {
                    symbol: "ATM".to_string(),
                    summary: crate::entities::disease::DiseaseAssociationScoreSummary {
                        overall_score: 0.942,
                        gwas_score: None,
                        rare_variant_score: None,
                        somatic_mutation_score: Some(0.731),
                    },
                },
            ],
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

        let genes = disease_markdown(&disease, &["genes".to_string()]).expect("rendered markdown");
        assert!(genes.contains("| Gene | Relationship | Source | OpenTargets |"));
        assert!(genes.contains(
            "| TP53 | associated with disease | OpenTargets | overall 0.991; somatic 0.881 |"
        ));
        assert!(!genes.contains("TP53, ATM"));
    }

    #[test]
    fn disease_markdown_renders_ot_only_gene_association_table() {
        proof_disease_markdown_renders_ot_only_gene_association_table();
    }

    #[test]
    fn disease_markdown_preserves_full_definition_text() {
        let full_definition = concat!(
            "A rare hypomyelinating leukodystrophy disorder in which the cause of the disease is ",
            "a variation in any of the POLR genes, including POLR1C, POLR3A or POLR3B. ",
            "It is characterized by the association of hypomyelination, hypodontia, ",
            "hypogonadotropic hypogonadism, and neurodevelopmental delay or regression."
        );
        let disease = Disease {
            id: "MONDO:0100605".to_string(),
            name: "4H leukodystrophy".to_string(),
            definition: Some(full_definition.to_string()),
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

        let markdown = disease_markdown(&disease, &[]).expect("rendered markdown");
        assert!(markdown.contains(full_definition));
        assert!(markdown.contains("hypogonadotropic hypogonadism"));
        assert!(markdown.contains("neurodevelopmental delay or regression"));
        assert!(!markdown.contains("It is characterized by the association of…"));
    }

    #[test]
    fn disease_markdown_phenotypes_section_renders_key_features() {
        let disease = Disease {
            id: "MONDO:0008222".to_string(),
            name: "Andersen-Tawil syndrome".to_string(),
            definition: Some(
                "A potassium channel disorder characterized by periodic paralysis, prolonged QT interval, and ventricular arrhythmias."
                    .to_string(),
            ),
            synonyms: Vec::new(),
            parents: Vec::new(),
            associated_genes: Vec::new(),
            gene_associations: Vec::new(),
            top_genes: Vec::new(),
            top_gene_scores: Vec::new(),
            treatment_landscape: Vec::new(),
            recruiting_trial_count: None,
            pathways: Vec::new(),
            phenotypes: vec![crate::entities::disease::DiseasePhenotype {
                hpo_id: "HP:0000001".to_string(),
                name: Some("Periodic paralysis".to_string()),
                evidence: None,
                frequency: None,
                frequency_qualifier: Some("Very frequent (80-99%)".to_string()),
                onset_qualifier: None,
                sex_qualifier: None,
                stage_qualifier: None,
                qualifiers: Vec::new(),
                source: None,
            }],
            key_features: vec![
                "periodic paralysis".to_string(),
                "prolonged QT interval".to_string(),
                "ventricular arrhythmias".to_string(),
            ],
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

        let markdown = disease_markdown(&disease, &["phenotypes".to_string()]).expect("markdown");
        assert!(markdown.contains("### Key Features"));
        assert!(markdown.contains("- periodic paralysis"));
        assert!(markdown.contains("These summarize the classic presentation; the table below is the comprehensive HPO annotation list."));
        assert!(markdown.contains("source-backed"));
    }

    #[test]
    fn disease_markdown_phenotypes_section_renders_definition_hint_when_key_features_missing() {
        let disease = Disease {
            id: "MONDO:0001111".to_string(),
            name: "Example syndrome".to_string(),
            definition: Some("Example syndrome is a rare inherited condition.".to_string()),
            synonyms: Vec::new(),
            parents: Vec::new(),
            associated_genes: Vec::new(),
            gene_associations: Vec::new(),
            top_genes: Vec::new(),
            top_gene_scores: Vec::new(),
            treatment_landscape: Vec::new(),
            recruiting_trial_count: None,
            pathways: Vec::new(),
            phenotypes: vec![crate::entities::disease::DiseasePhenotype {
                hpo_id: "HP:0001250".to_string(),
                name: Some("Seizure".to_string()),
                evidence: None,
                frequency: None,
                frequency_qualifier: None,
                onset_qualifier: None,
                sex_qualifier: None,
                stage_qualifier: None,
                qualifiers: Vec::new(),
                source: None,
            }],
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

        let markdown = disease_markdown(&disease, &["phenotypes".to_string()]).expect("markdown");
        assert!(markdown.contains(
            "Classic features are best summarized in the disease definition. Run `biomcp get disease MONDO:0001111` to review the definition; the table below is the comprehensive HPO annotation list."
        ));
        assert!(markdown.contains("source-backed"));
        assert!(!markdown.contains("### Key Features"));
    }

    #[test]
    fn disease_markdown_phenotypes_section_without_definition_only_shows_completeness_note() {
        let disease = Disease {
            id: "MONDO:0002222".to_string(),
            name: "Undocumented syndrome".to_string(),
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
            phenotypes: vec![crate::entities::disease::DiseasePhenotype {
                hpo_id: "HP:0001250".to_string(),
                name: Some("Seizure".to_string()),
                evidence: None,
                frequency: None,
                frequency_qualifier: None,
                onset_qualifier: None,
                sex_qualifier: None,
                stage_qualifier: None,
                qualifiers: Vec::new(),
                source: None,
            }],
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

        let markdown = disease_markdown(&disease, &["phenotypes".to_string()]).expect("markdown");
        assert!(markdown.contains("source-backed"));
        assert!(!markdown.contains("Classic features are best summarized"));
        assert!(!markdown.contains("### Key Features"));
    }

    #[test]
    fn gene_markdown_renders_hpa_section_details() {
        let gene = Gene {
            symbol: "BRAF".to_string(),
            name: "B-Raf proto-oncogene".to_string(),
            entrez_id: "673".to_string(),
            ensembl_id: Some("ENSG00000157764".to_string()),
            location: Some("7q34".to_string()),
            genomic_coordinates: None,
            omim_id: None,
            uniprot_id: Some("P15056".to_string()),
            summary: Some("Kinase involved in MAPK signaling.".to_string()),
            gene_type: Some("protein-coding".to_string()),
            aliases: vec!["BRAF1".to_string()],
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
            hpa: Some(crate::sources::hpa::GeneHpa {
                tissues: vec![
                    crate::sources::hpa::HpaTissueExpression {
                        tissue: "Liver".to_string(),
                        level: "High".to_string(),
                    },
                    crate::sources::hpa::HpaTissueExpression {
                        tissue: "Kidney".to_string(),
                        level: "Medium".to_string(),
                    },
                ],
                subcellular_main_location: vec!["cytosol".to_string(), "vesicles".to_string()],
                subcellular_additional_location: vec!["plasma membrane".to_string()],
                reliability: Some("Supported".to_string()),
                protein_summary: Some("Ubiquitous cytoplasmic expression.".to_string()),
                rna_summary: Some("Low tissue specificity; Detected in all".to_string()),
            }),
            druggability: None,
            clingen: None,
            constraint: None,
            disgenet: None,
            funding: None,
            funding_note: None,
        };

        let summary = gene_markdown(&gene, &[]).expect("rendered markdown");
        assert!(summary.contains(
            "Aliases are alternate names used in literature and databases when a paper does not use the HGNC symbol."
        ));

        let markdown = gene_markdown(&gene, &["hpa".to_string()]).expect("rendered markdown");

        assert!(markdown.contains("# BRAF - hpa"));
        assert!(markdown.contains("## Human Protein Atlas"));
        assert!(markdown.contains("Reliability: Supported"));
        assert!(markdown.contains("Protein summary: Ubiquitous cytoplasmic expression."));
        assert!(markdown.contains("RNA summary: Low tissue specificity; Detected in all"));
        assert!(markdown.contains("Subcellular main locations: cytosol, vesicles"));
        assert!(markdown.contains("Subcellular additional locations: plasma membrane"));
        assert!(markdown.contains("| Tissue | Level |"));
        assert!(markdown.contains("| Liver | High |"));
        assert!(markdown.contains("| Kidney | Medium |"));
        assert!(
            markdown
                .find("| Tissue | Level |")
                .expect("tissue table should render")
                < markdown
                    .find("RNA summary:")
                    .expect("rna summary should render")
        );
    }

    #[test]
    fn format_sections_block_renders_described_executable_commands() {
        let block = format_sections_block(
            "gene",
            "TP53",
            vec![
                "pathways".to_string(),
                "hpa".to_string(),
                "diseases".to_string(),
                "protein".to_string(),
            ],
        );

        assert!(block.contains("More:"));
        assert!(block.contains("biomcp get gene TP53 pathways"));
        assert!(block.contains("Reactome/KEGG pathway context"));
        assert!(block.contains("biomcp get gene TP53 hpa"));
        assert!(block.contains("Human Protein Atlas tissue expression and localization"));
        assert!(block.contains("biomcp get gene TP53 diseases"));
        assert!(block.contains("disease associations"));
        assert!(block.contains("All:"));
        assert!(block.contains("biomcp get gene TP53 all"));
    }

    #[test]
    fn format_sections_block_keeps_gene_ontology_in_top_more_entries() {
        let block = format_sections_block(
            "gene",
            "NANOG",
            vec![
                "pathways".to_string(),
                "ontology".to_string(),
                "diseases".to_string(),
                "protein".to_string(),
            ],
        );

        let pathways = block
            .find("biomcp get gene NANOG pathways")
            .expect("pathways command");
        let ontology = block
            .find("biomcp get gene NANOG ontology")
            .expect("ontology command");
        let diseases = block
            .find("biomcp get gene NANOG diseases")
            .expect("diseases command");
        assert!(pathways < ontology);
        assert!(ontology < diseases);
    }

    #[test]
    fn format_sections_block_describes_guardrailed_drug_and_trial_sections() {
        let drug_block = format_sections_block(
            "drug",
            "pembrolizumab",
            vec![
                "label".to_string(),
                "regulatory".to_string(),
                "safety".to_string(),
            ],
        );

        assert!(drug_block.contains(
            "biomcp get drug pembrolizumab label   - approved-indication and FDA label detail beyond the base card"
        ));
        assert!(drug_block.contains(
            "biomcp get drug pembrolizumab regulatory   - approval and supplement history; use only if the base card lacks approval context"
        ));
        assert!(drug_block.contains(
            "biomcp get drug pembrolizumab safety   - regulatory safety detail; use `biomcp drug adverse-events <name>` first when you want post-marketing signal"
        ));

        let terminated = crate::entities::trial::Trial {
            nct_id: "NCT02576665".to_string(),
            source: None,
            title: "Completed trial".to_string(),
            status: "TERMINATED".to_string(),
            phase: None,
            study_type: None,
            age_range: None,
            conditions: vec!["melanoma".to_string()],
            interventions: vec!["trametinib".to_string()],
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
        let terminated_sections = sections_trial(&terminated, &[]);
        assert_eq!(terminated_sections[0], "outcomes");
        assert_eq!(terminated_sections[1], "references");
        assert_eq!(terminated_sections[2], "arms");

        let trial_block =
            format_sections_block("trial", &terminated.nct_id, terminated_sections.clone());
        assert!(trial_block.contains(
            "biomcp get trial NCT02576665 outcomes   - endpoint measures and time frames"
        ));
        assert!(trial_block.contains(
            "biomcp get trial NCT02576665 references   - linked publications and PMID citations"
        ));
        assert!(trial_block.contains(
            "biomcp get trial NCT02576665 arms   - study arms and assigned interventions"
        ));

        let recruiting = crate::entities::trial::Trial {
            status: "Recruiting".to_string(),
            ..terminated
        };
        let recruiting_sections = sections_trial(&recruiting, &[]);
        assert_eq!(recruiting_sections[0], "eligibility");
        assert_eq!(recruiting_sections[1], "locations");
        assert_eq!(recruiting_sections[2], "outcomes");
    }

    #[test]
    fn related_gene_prioritizes_localization_deepening_when_supported() {
        let gene = Gene {
            symbol: "OPA1".to_string(),
            name: "OPA1 mitochondrial dynamin like GTPase".to_string(),
            entrez_id: "4976".to_string(),
            ensembl_id: Some("ENSG00000198836".to_string()),
            location: Some("3q29".to_string()),
            genomic_coordinates: None,
            omim_id: None,
            uniprot_id: Some("O60313".to_string()),
            summary: Some(
                "Mitochondrial inner membrane fusion GTPase required for cristae organization."
                    .to_string(),
            ),
            gene_type: Some("protein-coding".to_string()),
            aliases: vec!["large GTPase 1".to_string()],
            clinical_diseases: Vec::new(),
            clinical_drugs: Vec::new(),
            pathways: None,
            ontology: None,
            diseases: None,
            protein: Some(crate::entities::gene::GeneProtein {
                accession: "O60313".to_string(),
                name: "Dynamin-like 120 kDa protein, mitochondrial".to_string(),
                function: None,
                length: None,
                isoforms: Vec::new(),
                alternative_names: Vec::new(),
            }),
            go: None,
            interactions: None,
            civic: None,
            expression: None,
            hpa: Some(crate::sources::hpa::GeneHpa {
                tissues: vec![crate::sources::hpa::HpaTissueExpression {
                    tissue: "Retina".to_string(),
                    level: "High".to_string(),
                }],
                subcellular_main_location: vec!["Mitochondria".to_string()],
                subcellular_additional_location: Vec::new(),
                reliability: Some("Approved".to_string()),
                protein_summary: None,
                rna_summary: None,
            }),
            druggability: None,
            clingen: None,
            constraint: None,
            disgenet: None,
            funding: None,
            funding_note: None,
        };

        let related = related_gene(&gene);
        assert_eq!(related[0], "biomcp get gene OPA1 protein");
        assert_eq!(related[1], "biomcp get gene OPA1 hpa");
        assert!(related.contains(&"biomcp search pgx -g OPA1".to_string()));
        assert!(related.contains(&"biomcp search variant -g OPA1".to_string()));
    }

    #[test]
    fn related_gene_promotes_clingen_trial_search_before_generic_pivots() {
        let gene = Gene {
            symbol: "OPA1".to_string(),
            name: "OPA1 mitochondrial dynamin like GTPase".to_string(),
            entrez_id: "4976".to_string(),
            ensembl_id: Some("ENSG00000198836".to_string()),
            location: Some("3q29".to_string()),
            genomic_coordinates: None,
            omim_id: None,
            uniprot_id: Some("O60313".to_string()),
            summary: Some(
                "Mitochondrial inner membrane fusion GTPase required for cristae organization."
                    .to_string(),
            ),
            gene_type: Some("protein-coding".to_string()),
            aliases: vec!["large GTPase 1".to_string()],
            clinical_diseases: Vec::new(),
            clinical_drugs: Vec::new(),
            pathways: None,
            ontology: None,
            diseases: None,
            protein: Some(crate::entities::gene::GeneProtein {
                accession: "O60313".to_string(),
                name: "Dynamin-like 120 kDa protein, mitochondrial".to_string(),
                function: None,
                length: None,
                isoforms: Vec::new(),
                alternative_names: Vec::new(),
            }),
            go: None,
            interactions: None,
            civic: None,
            expression: None,
            hpa: Some(crate::sources::hpa::GeneHpa {
                tissues: vec![crate::sources::hpa::HpaTissueExpression {
                    tissue: "Retina".to_string(),
                    level: "High".to_string(),
                }],
                subcellular_main_location: vec!["Mitochondria".to_string()],
                subcellular_additional_location: Vec::new(),
                reliability: Some("Approved".to_string()),
                protein_summary: None,
                rna_summary: None,
            }),
            druggability: None,
            clingen: Some(crate::sources::clingen::GeneClinGen {
                validity: vec![crate::sources::clingen::ClinGenValidity {
                    disease: "dominant optic atrophy".to_string(),
                    classification: "Definitive".to_string(),
                    review_date: Some("2024-01-01".to_string()),
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

        let related = related_gene(&gene);
        assert_eq!(related[0], "biomcp get gene OPA1 protein");
        assert_eq!(related[1], "biomcp get gene OPA1 hpa");
        assert_eq!(
            related[2],
            "biomcp search trial -c \"dominant optic atrophy\" -s recruiting"
        );
        assert_eq!(
            related_command_description(&related[2]),
            Some("recruiting trials for the top ClinGen disease on this gene card")
        );
        assert!(related.contains(&"biomcp search pgx -g OPA1".to_string()));
    }

    #[test]
    fn gene_markdown_renders_protein_isoforms_with_count_and_displayed_length() {
        let gene = Gene {
            symbol: "KRAS".to_string(),
            name: "KRAS proto-oncogene, GTPase".to_string(),
            entrez_id: "3845".to_string(),
            ensembl_id: Some("ENSG00000133703".to_string()),
            location: Some("12p12.1".to_string()),
            genomic_coordinates: None,
            omim_id: None,
            uniprot_id: Some("P01116".to_string()),
            summary: None,
            gene_type: Some("protein-coding".to_string()),
            aliases: vec!["K-RAS2A".to_string(), "K-RAS2B".to_string()],
            clinical_diseases: Vec::new(),
            clinical_drugs: Vec::new(),
            pathways: None,
            ontology: None,
            diseases: None,
            protein: Some(crate::entities::gene::GeneProtein {
                accession: "P01116".to_string(),
                name: "GTPase KRas".to_string(),
                function: Some("Small GTPase involved in signal transduction.".to_string()),
                length: Some(189),
                isoforms: vec![
                    crate::entities::gene::GeneProteinIsoform {
                        name: "K-Ras4A".to_string(),
                        length: Some(189),
                    },
                    crate::entities::gene::GeneProteinIsoform {
                        name: "K-Ras4B".to_string(),
                        length: None,
                    },
                ],
                alternative_names: Vec::new(),
            }),
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

        let markdown = gene_markdown(&gene, &["protein".to_string()]).expect("gene markdown");
        assert!(markdown.contains("## Protein (UniProt)"));
        assert!(markdown.contains("- Length: 189 aa"));
        assert!(markdown.contains("- Isoforms (2): K-Ras4A (189 aa), K-Ras4B"));
        assert!(markdown.contains("- Function: Small GTPase involved in signal transduction."));
    }

    #[test]
    fn gene_markdown_without_isoforms_keeps_protein_lines_contiguous() {
        let gene = Gene {
            symbol: "BRAF".to_string(),
            name: "B-Raf proto-oncogene, serine/threonine kinase".to_string(),
            entrez_id: "673".to_string(),
            ensembl_id: Some("ENSG00000157764".to_string()),
            location: Some("7q34".to_string()),
            genomic_coordinates: None,
            omim_id: None,
            uniprot_id: Some("P15056".to_string()),
            summary: None,
            gene_type: Some("protein-coding".to_string()),
            aliases: Vec::new(),
            clinical_diseases: Vec::new(),
            clinical_drugs: Vec::new(),
            pathways: None,
            ontology: None,
            diseases: None,
            protein: Some(crate::entities::gene::GeneProtein {
                accession: "P15056".to_string(),
                name: "Serine/threonine-protein kinase B-raf".to_string(),
                function: Some("Kinase function.".to_string()),
                length: Some(766),
                isoforms: Vec::new(),
                alternative_names: Vec::new(),
            }),
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

        let markdown = gene_markdown(&gene, &["protein".to_string()]).expect("gene markdown");
        assert!(markdown.contains("- Length: 766 aa\n- Function: Kinase function."));
        assert!(!markdown.contains("- Isoforms ("));
        assert!(!markdown.contains("- Length: 766 aa\n\n- Function:"));
    }

    #[test]
    fn gene_markdown_renders_protein_alternative_names() {
        let gene = Gene {
            symbol: "PLIN2".to_string(),
            name: "perilipin 2".to_string(),
            entrez_id: "123".to_string(),
            ensembl_id: Some("ENSG00000147889".to_string()),
            location: Some("9p22.1".to_string()),
            genomic_coordinates: None,
            omim_id: None,
            uniprot_id: Some("Q99541".to_string()),
            summary: None,
            gene_type: Some("protein-coding".to_string()),
            aliases: Vec::new(),
            clinical_diseases: Vec::new(),
            clinical_drugs: Vec::new(),
            pathways: None,
            ontology: None,
            diseases: None,
            protein: Some(crate::entities::gene::GeneProtein {
                accession: "Q99541".to_string(),
                name: "Perilipin-2".to_string(),
                function: None,
                length: Some(437),
                isoforms: Vec::new(),
                alternative_names: vec![
                    "Adipophilin".to_string(),
                    "ADRP".to_string(),
                    "Adipose differentiation-related protein".to_string(),
                ],
            }),
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

        let markdown = gene_markdown(&gene, &["protein".to_string()]).expect("gene markdown");
        assert!(markdown.contains(
            "- Also known as: Adipophilin, ADRP, Adipose differentiation-related protein"
        ));
    }

    #[test]
    fn gene_markdown_preserves_full_protein_function_text() {
        let long_function = "Mitochondrial dynamin-like GTPase required for fusion. Localizes to the intermembrane space where it helps organize cristae architecture and mitochondrial DNA maintenance.".to_string();
        let gene = Gene {
            symbol: "OPA1".to_string(),
            name: "OPA1 mitochondrial dynamin like GTPase".to_string(),
            entrez_id: "4976".to_string(),
            ensembl_id: Some("ENSG00000198836".to_string()),
            location: Some("3q29".to_string()),
            genomic_coordinates: None,
            omim_id: None,
            uniprot_id: Some("O60313".to_string()),
            summary: None,
            gene_type: Some("protein-coding".to_string()),
            aliases: Vec::new(),
            clinical_diseases: Vec::new(),
            clinical_drugs: Vec::new(),
            pathways: None,
            ontology: None,
            diseases: None,
            protein: Some(crate::entities::gene::GeneProtein {
                accession: "O60313".to_string(),
                name: "Dynamin-like 120 kDa protein, mitochondrial".to_string(),
                function: Some(long_function.clone()),
                length: Some(960),
                isoforms: Vec::new(),
                alternative_names: Vec::new(),
            }),
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

        let markdown = gene_markdown(&gene, &["protein".to_string()]).expect("gene markdown");
        assert!(markdown.contains(&long_function));
        assert!(markdown.contains("intermembrane space"));
        assert!(!markdown.contains("architecture and mitochondrial DNA…"));
    }

    #[test]
    fn gene_markdown_omits_protein_alternative_names_when_absent() {
        let gene = Gene {
            symbol: "BRAF".to_string(),
            name: "B-Raf proto-oncogene, serine/threonine kinase".to_string(),
            entrez_id: "673".to_string(),
            ensembl_id: Some("ENSG00000157764".to_string()),
            location: Some("7q34".to_string()),
            genomic_coordinates: None,
            omim_id: None,
            uniprot_id: Some("P15056".to_string()),
            summary: None,
            gene_type: Some("protein-coding".to_string()),
            aliases: Vec::new(),
            clinical_diseases: Vec::new(),
            clinical_drugs: Vec::new(),
            pathways: None,
            ontology: None,
            diseases: None,
            protein: Some(crate::entities::gene::GeneProtein {
                accession: "P15056".to_string(),
                name: "Serine/threonine-protein kinase B-raf".to_string(),
                function: Some("Kinase function.".to_string()),
                length: Some(766),
                isoforms: Vec::new(),
                alternative_names: Vec::new(),
            }),
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

        let markdown = gene_markdown(&gene, &["protein".to_string()]).expect("gene markdown");
        assert!(!markdown.contains("- Also known as:"));
    }

    #[test]
    fn related_drug_includes_pgx_search() {
        let drug = Drug {
            name: "warfarin".to_string(),
            drugbank_id: None,
            chembl_id: None,
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
            targets: Vec::new(),
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

        let related = related_drug(&drug);
        assert!(related.contains(&"biomcp search pgx -d warfarin".to_string()));
    }

    #[test]
    fn related_disease_suggests_review_when_phenotypes_are_sparse() {
        let disease = Disease {
            id: "MONDO:0007947".to_string(),
            name: "Marfan syndrome".to_string(),
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
            phenotypes: vec![
                crate::entities::disease::DiseasePhenotype {
                    hpo_id: "HP:0001166".to_string(),
                    name: Some("Arachnodactyly".to_string()),
                    evidence: None,
                    frequency: None,
                    frequency_qualifier: None,
                    onset_qualifier: None,
                    sex_qualifier: None,
                    stage_qualifier: None,
                    qualifiers: Vec::new(),
                    source: None,
                },
                crate::entities::disease::DiseasePhenotype {
                    hpo_id: "HP:0002616".to_string(),
                    name: Some("Aortic root dilatation".to_string()),
                    evidence: None,
                    frequency: None,
                    frequency_qualifier: None,
                    onset_qualifier: None,
                    sex_qualifier: None,
                    stage_qualifier: None,
                    qualifiers: Vec::new(),
                    source: None,
                },
            ],
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

        let related = related_disease(&disease);
        assert_eq!(
            related[0],
            "biomcp search article -d \"Marfan syndrome\" --type review --limit 5"
        );
        assert!(related.contains(&"biomcp search trial -c \"Marfan syndrome\"".to_string()));
        assert!(
            related.contains(&"biomcp search drug --indication \"Marfan syndrome\"".to_string())
        );
    }

    #[test]
    fn related_disease_promotes_top_gene_context_before_generic_pivots() {
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

        let related = related_disease(&disease);
        assert_eq!(related[0], "biomcp get gene SCN1A clingen constraint");
        assert_eq!(related[1], "biomcp search trial -c \"Dravet syndrome\"");
        assert_eq!(
            related_command_description(&related[0]),
            Some("review ClinGen validity and constraint evidence for the top disease gene")
        );
    }

    #[test]
    fn related_disease_falls_back_to_unscored_top_gene_context() {
        let disease = Disease {
            id: "MONDO:0100135".to_string(),
            name: "Dravet syndrome".to_string(),
            definition: None,
            synonyms: Vec::new(),
            parents: Vec::new(),
            associated_genes: vec!["SCN1A".to_string()],
            gene_associations: Vec::new(),
            top_genes: vec!["SCN1A".to_string()],
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

        let related = related_disease(&disease);
        assert_eq!(related[0], "biomcp get gene SCN1A clingen constraint");
    }

    #[test]
    fn related_disease_uses_synonym_when_name_is_raw_id() {
        let disease = Disease {
            id: "MONDO:0100605".to_string(),
            name: "MONDO:0100605".to_string(),
            definition: None,
            synonyms: vec!["4H leukodystrophy".to_string()],
            parents: Vec::new(),
            associated_genes: Vec::new(),
            gene_associations: Vec::new(),
            top_genes: Vec::new(),
            top_gene_scores: Vec::new(),
            treatment_landscape: Vec::new(),
            recruiting_trial_count: None,
            pathways: Vec::new(),
            phenotypes: vec![crate::entities::disease::DiseasePhenotype {
                hpo_id: "HP:0001252".to_string(),
                name: Some("Hypomyelination".to_string()),
                evidence: None,
                frequency: None,
                frequency_qualifier: None,
                onset_qualifier: None,
                sex_qualifier: None,
                stage_qualifier: None,
                qualifiers: Vec::new(),
                source: None,
            }],
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

        let related = related_disease(&disease);
        assert_eq!(
            related[0],
            "biomcp search article -d \"4H leukodystrophy\" --type review --limit 5"
        );
    }

    #[test]
    fn related_disease_non_oncology_skips_study_hints() {
        let disease = Disease {
            id: "MONDO:0007947".to_string(),
            name: "Marfan syndrome".to_string(),
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

        let related = related_disease(&disease);
        assert!(!related.iter().any(|cmd| cmd.contains("biomcp study ")));
    }

    #[test]
    fn related_disease_quotes_single_word_indication_search() {
        let disease = Disease {
            id: "MONDO:0005105".to_string(),
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

        let related = related_disease(&disease);
        assert!(related.contains(&"biomcp search drug --indication \"melanoma\"".to_string()));
    }

    #[test]
    fn related_disease_oncology_without_local_match_falls_back_to_download_list() {
        let _guard = crate::test_support::env_lock().blocking_lock();
        let disease = Disease {
            id: "MONDO:0005105".to_string(),
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
            civic: Some(crate::sources::civic::CivicContext::default()),
            disgenet: None,
            funding: None,
            funding_note: None,
            xrefs: std::collections::HashMap::new(),
        };

        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "biomcp-render-study-empty-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create empty study root");
        let original = std::env::var_os("BIOMCP_STUDY_DIR");
        unsafe { std::env::set_var("BIOMCP_STUDY_DIR", &root) };
        let related = related_disease(&disease);
        match original {
            Some(value) => unsafe { std::env::set_var("BIOMCP_STUDY_DIR", value) },
            None => unsafe { std::env::remove_var("BIOMCP_STUDY_DIR") },
        }
        let _ = std::fs::remove_dir_all(&root);

        assert!(related.contains(&"biomcp study download --list".to_string()));
    }

    #[test]
    fn related_disease_oncology_with_local_match_prefers_top_mutated() {
        let _guard = crate::test_support::env_lock().blocking_lock();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "biomcp-render-study-match-{}-{unique}",
            std::process::id()
        ));
        let study_dir = root.join("brca_tcga_pan_can_atlas_2018");
        std::fs::create_dir_all(&study_dir).expect("create study dir");
        std::fs::write(
            study_dir.join("meta_study.txt"),
            "cancer_study_identifier: brca_tcga_pan_can_atlas_2018\nname: BRCA TCGA PanCan Atlas 2018\ntype_of_cancer: brca\n",
        )
        .expect("write meta");
        std::fs::write(
            study_dir.join("data_mutations.txt"),
            "Hugo_Symbol\tTumor_Sample_Barcode\tVariant_Classification\tHGVSp_Short\nTP53\tS1\tMissense_Mutation\tp.R175H\n",
        )
        .expect("write mutations");
        std::fs::write(
            study_dir.join("data_clinical_sample.txt"),
            "# comment\nPATIENT_ID\tSAMPLE_ID\tCANCER_TYPE\tCANCER_TYPE_DETAILED\tONCOTREE_CODE\nP1\tS1\tBreast Cancer\tBreast Invasive Carcinoma\tBRCA\n",
        )
        .expect("write clinical sample");

        let disease = Disease {
            id: "MONDO:0007254".to_string(),
            name: "breast cancer".to_string(),
            definition: None,
            synonyms: vec!["mammary carcinoma".to_string()],
            parents: Vec::new(),
            associated_genes: Vec::new(),
            gene_associations: Vec::new(),
            top_genes: Vec::new(),
            top_gene_scores: vec![crate::entities::disease::DiseaseTargetScore {
                symbol: "TP53".to_string(),
                summary: crate::entities::disease::DiseaseAssociationScoreSummary {
                    overall_score: 0.8,
                    gwas_score: None,
                    rare_variant_score: None,
                    somatic_mutation_score: Some(0.4),
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

        let original = std::env::var_os("BIOMCP_STUDY_DIR");
        unsafe { std::env::set_var("BIOMCP_STUDY_DIR", &root) };
        let related = related_disease(&disease);
        match original {
            Some(value) => unsafe { std::env::set_var("BIOMCP_STUDY_DIR", value) },
            None => unsafe { std::env::remove_var("BIOMCP_STUDY_DIR") },
        }
        let _ = std::fs::remove_dir_all(&root);

        assert!(related.contains(
            &"biomcp study top-mutated --study brca_tcga_pan_can_atlas_2018".to_string()
        ));
        assert!(!related.contains(&"biomcp study download --list".to_string()));
    }

    #[test]
    fn related_disease_oncology_matches_noncontiguous_carcinoma_study_labels() {
        let _guard = crate::test_support::env_lock().blocking_lock();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "biomcp-render-study-carcinoma-{}-{unique}",
            std::process::id()
        ));
        let study_dir = root.join("brca_tcga_pan_can_atlas_2018");
        std::fs::create_dir_all(&study_dir).expect("create study dir");
        std::fs::write(
            study_dir.join("meta_study.txt"),
            "cancer_study_identifier: brca_tcga_pan_can_atlas_2018\nname: BRCA TCGA PanCan Atlas 2018\ntype_of_cancer: brca\n",
        )
        .expect("write meta");
        std::fs::write(
            study_dir.join("data_mutations.txt"),
            "Hugo_Symbol\tTumor_Sample_Barcode\tVariant_Classification\tHGVSp_Short\nTP53\tS1\tMissense_Mutation\tp.R175H\n",
        )
        .expect("write mutations");
        std::fs::write(
            study_dir.join("data_clinical_sample.txt"),
            "# comment\nPATIENT_ID\tSAMPLE_ID\tCANCER_TYPE\tCANCER_TYPE_DETAILED\tONCOTREE_CODE\nP1\tS1\tBreast Cancer\tBreast Invasive Carcinoma\tBRCA\n",
        )
        .expect("write clinical sample");

        let disease = Disease {
            id: "MONDO:0004989".to_string(),
            name: "breast carcinoma".to_string(),
            definition: None,
            synonyms: Vec::new(),
            parents: Vec::new(),
            associated_genes: Vec::new(),
            gene_associations: Vec::new(),
            top_genes: Vec::new(),
            top_gene_scores: vec![crate::entities::disease::DiseaseTargetScore {
                symbol: "TP53".to_string(),
                summary: crate::entities::disease::DiseaseAssociationScoreSummary {
                    overall_score: 0.8,
                    gwas_score: None,
                    rare_variant_score: None,
                    somatic_mutation_score: Some(0.4),
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

        let original = std::env::var_os("BIOMCP_STUDY_DIR");
        unsafe { std::env::set_var("BIOMCP_STUDY_DIR", &root) };
        let related = related_disease(&disease);
        match original {
            Some(value) => unsafe { std::env::set_var("BIOMCP_STUDY_DIR", value) },
            None => unsafe { std::env::remove_var("BIOMCP_STUDY_DIR") },
        }
        let _ = std::fs::remove_dir_all(&root);

        assert!(related.contains(
            &"biomcp study top-mutated --study brca_tcga_pan_can_atlas_2018".to_string()
        ));
        assert!(!related.contains(&"biomcp study download --list".to_string()));
    }

    #[test]
    fn study_top_mutated_markdown_renders_ranked_table() {
        let markdown = study_top_mutated_markdown(&StudyTopMutatedGenesResult {
            study_id: "msk_impact_2017".to_string(),
            total_samples: 3,
            rows: vec![
                StudyTopMutatedGeneRow {
                    gene: "TP53".to_string(),
                    mutated_samples: 2,
                    mutation_events: 2,
                    mutation_rate: 2.0 / 3.0,
                },
                StudyTopMutatedGeneRow {
                    gene: "KRAS".to_string(),
                    mutated_samples: 2,
                    mutation_events: 2,
                    mutation_rate: 2.0 / 3.0,
                },
            ],
        });

        assert!(markdown.contains("# Study Top Mutated Genes: msk_impact_2017"));
        assert!(markdown.contains(
            "| Gene | Mutated Samples | Mutation Events | Total Samples | Mutation Rate |"
        ));
        assert!(markdown.contains("| TP53 | 2 | 2 | 3 |"));
        assert!(markdown.contains("| KRAS | 2 | 2 | 3 |"));
    }

    #[test]
    fn related_drug_suggests_review_when_label_and_indications_are_sparse() {
        let drug = Drug {
            name: "orteronel".to_string(),
            drugbank_id: None,
            chembl_id: None,
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
            targets: Vec::new(),
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

        let related = related_drug(&drug);
        assert_eq!(
            related[0],
            "biomcp search article --drug orteronel --type review --limit 5"
        );
        assert!(related.contains(&"biomcp search pgx -d orteronel".to_string()));
        assert!(related.contains(&"biomcp drug adverse-events orteronel".to_string()));
    }

    #[test]
    fn discover_try_line_quotes_shell_sensitive_queries() {
        assert_eq!(
            discover_try_line("ERBB1\"alias", "resolve abbreviations and synonyms"),
            "Try: biomcp discover \"ERBB1\\\"alias\"   - resolve abbreviations and synonyms"
        );
        assert_eq!(
            discover_try_line("BRAF V600E", "resolve abbreviations and synonyms"),
            "Try: biomcp discover \"BRAF V600E\"   - resolve abbreviations and synonyms"
        );
    }

    #[test]
    fn gene_markdown_section_only_shows_constraint_section() {
        let gene = Gene {
            symbol: "TP53".to_string(),
            name: "tumor protein p53".to_string(),
            entrez_id: "7157".to_string(),
            ensembl_id: Some("ENSG00000141510".to_string()),
            location: Some("17p13.1".to_string()),
            genomic_coordinates: None,
            omim_id: None,
            uniprot_id: Some("P04637".to_string()),
            summary: Some("Tumor suppressor.".to_string()),
            gene_type: Some("protein-coding".to_string()),
            aliases: vec!["P53".to_string()],
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
            constraint: Some(crate::entities::gene::GeneConstraint {
                pli: None,
                loeuf: None,
                mis_z: None,
                syn_z: None,
                transcript: Some("ENST00000269305".to_string()),
                source: "gnomAD".to_string(),
                source_version: "v4".to_string(),
                reference_genome: "GRCh38".to_string(),
            }),
            disgenet: None,
            funding: None,
            funding_note: None,
        };

        let markdown =
            gene_markdown(&gene, &["constraint".to_string()]).expect("rendered markdown");

        assert!(markdown.contains("# TP53 - constraint"));
        assert!(markdown.contains("## Constraint (gnomAD)"));
        assert!(markdown.contains("Source: gnomAD"));
        assert!(markdown.contains("Version: v4"));
        assert!(markdown.contains("Reference genome: GRCh38"));
        assert!(markdown.contains("Transcript: ENST00000269305"));
        assert!(markdown.contains("No gnomAD constraint metrics returned for this gene query."));
    }

    #[test]
    fn gene_markdown_section_only_shows_disgenet_section() {
        let gene = Gene {
            symbol: "TP53".to_string(),
            name: "tumor protein p53".to_string(),
            entrez_id: "7157".to_string(),
            ensembl_id: None,
            location: None,
            genomic_coordinates: None,
            omim_id: None,
            uniprot_id: None,
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
            disgenet: Some(crate::entities::gene::GeneDisgenet {
                associations: vec![crate::entities::gene::GeneDisgenetAssociation {
                    disease_name: "Breast Carcinoma".to_string(),
                    disease_cui: "C0678222".to_string(),
                    score: 0.91,
                    publication_count: Some(1234),
                    clinical_trial_count: Some(4),
                    evidence_index: Some(0.72),
                    evidence_level: Some("Definitive".to_string()),
                }],
            }),
            funding: None,
            funding_note: None,
        };

        let markdown = gene_markdown(&gene, &["disgenet".to_string()]).expect("rendered markdown");

        assert!(markdown.contains("# TP53 - disgenet"));
        assert!(markdown.contains("## DisGeNET"));
        assert!(markdown.contains("| Disease | UMLS CUI | Score | PMIDs | Trials | EL | EI |"));
        assert!(
            markdown.contains(
                "| Breast Carcinoma | C0678222 | 0.910 | 1234 | 4 | Definitive | 0.720 |"
            )
        );
    }

    #[test]
    fn gene_markdown_disgenet_renders_sparse_optional_fields() {
        let gene = Gene {
            symbol: "KYNU".to_string(),
            name: "kynureninase".to_string(),
            entrez_id: "8942".to_string(),
            ensembl_id: None,
            location: None,
            genomic_coordinates: None,
            omim_id: None,
            uniprot_id: None,
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
            disgenet: Some(crate::entities::gene::GeneDisgenet {
                associations: vec![crate::entities::gene::GeneDisgenetAssociation {
                    disease_name: "Sparse Disease".to_string(),
                    disease_cui: "C1234567".to_string(),
                    score: 0.23,
                    publication_count: None,
                    clinical_trial_count: None,
                    evidence_index: None,
                    evidence_level: None,
                }],
            }),
            funding: None,
            funding_note: None,
        };

        let markdown = gene_markdown(&gene, &["disgenet".to_string()]).expect("rendered markdown");

        assert!(markdown.contains("| Disease | UMLS CUI | Score | PMIDs | Trials | EL | EI |"));
        assert!(markdown.contains("| Sparse Disease | C1234567 | 0.230 | - | - | - | - |"));
    }

    #[test]
    fn gene_markdown_funding_renders_linked_rows_and_currency() {
        let gene = Gene {
            symbol: "ERBB2".to_string(),
            name: "erb-b2 receptor tyrosine kinase 2".to_string(),
            entrez_id: "2064".to_string(),
            ensembl_id: None,
            location: None,
            genomic_coordinates: None,
            omim_id: None,
            uniprot_id: None,
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
            funding: Some(crate::sources::nih_reporter::NihReporterFundingSection {
                query: "ERBB2".to_string(),
                fiscal_years: vec![2022, 2023, 2024, 2025, 2026],
                matching_project_years: 176,
                grants: vec![crate::sources::nih_reporter::NihReporterGrant {
                    project_title: "Regulation Of Epidermal Differentiation".to_string(),
                    project_num: "1ZIAAR041124-23".to_string(),
                    core_project_num: Some("ZIAAR041124".to_string()),
                    project_detail_url: Some(
                        "https://reporter.nih.gov/project-details/10697688".to_string(),
                    ),
                    pi_name: Some("MORASSO, MARIA".to_string()),
                    organization: Some(
                        "NATIONAL INSTITUTE OF ARTHRITIS AND MUSCULOSKELETAL AND SKIN DISEASES"
                            .to_string(),
                    ),
                    fiscal_year: 2022,
                    award_amount: 2_219_287,
                }],
            }),
            funding_note: None,
        };

        let markdown = gene_markdown(&gene, &["funding".to_string()]).expect("funding markdown");
        let summary = "Showing top 1 unique grants from 176 matching NIH project-year records across FY2022-FY2026.";
        let row = "| [Regulation Of Epidermal Differentiation](https://reporter.nih.gov/project-details/10697688) | MORASSO, MARIA | NATIONAL INSTITUTE OF ARTHRITIS AND MUSCULOSKELETAL AND SKIN DISEASES | 2022 | $2,219,287 |";

        assert!(markdown.contains("# ERBB2 - funding"));
        assert!(markdown.contains("## Funding (NIH Reporter)"));
        assert!(markdown.contains(summary));
        assert!(markdown.contains("| Project | PI | Organization | FY | Amount |"));
        assert!(markdown.contains("[Regulation Of Epidermal Differentiation](https://reporter.nih.gov/project-details/10697688)"));
        assert!(markdown.contains("| MORASSO, MARIA |"));
        assert!(markdown.contains("| 2022 | $2,219,287 |"));
        assert!(
            markdown
                .find(summary)
                .expect("funding summary should render")
                > markdown.find(row).expect("funding row should render")
        );
    }

    #[test]
    fn gene_markdown_all_keeps_opt_in_sections_hidden() {
        let gene = Gene {
            symbol: "ERBB2".to_string(),
            name: "erb-b2 receptor tyrosine kinase 2".to_string(),
            entrez_id: "2064".to_string(),
            ensembl_id: None,
            location: None,
            genomic_coordinates: None,
            omim_id: None,
            uniprot_id: None,
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

        let markdown = gene_markdown(&gene, &["all".to_string()]).expect("all markdown");

        assert!(!markdown.contains("## Funding (NIH Reporter)"));
        assert!(!markdown.contains("## DisGeNET"));
    }

    #[test]
    fn gene_markdown_pathways_show_source_labels() {
        let gene = Gene {
            symbol: "BRAF".to_string(),
            name: "B-Raf proto-oncogene".to_string(),
            entrez_id: "673".to_string(),
            ensembl_id: None,
            location: Some("7q34".to_string()),
            genomic_coordinates: None,
            omim_id: None,
            uniprot_id: None,
            summary: None,
            gene_type: None,
            aliases: Vec::new(),
            clinical_diseases: Vec::new(),
            clinical_drugs: Vec::new(),
            pathways: Some(vec![
                crate::entities::gene::GenePathway {
                    source: "KEGG".to_string(),
                    id: "hsa05200".to_string(),
                    name: "Pathways in cancer".to_string(),
                },
                crate::entities::gene::GenePathway {
                    source: "Reactome".to_string(),
                    id: "R-HSA-5673001".to_string(),
                    name: "RAF/MAP kinase cascade".to_string(),
                },
            ]),
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

        let markdown = gene_markdown(&gene, &[]).expect("rendered markdown");
        assert!(markdown.contains("| Source | ID | Name |"));
        assert!(markdown.contains("| KEGG | hsa05200 | Pathways in cancer |"));
        assert!(markdown.contains("| Reactome | R-HSA-5673001 | RAF/MAP kinase cascade |"));
        assert!(!markdown.contains("Showing pathway rows from Reactome search results."));
    }

    #[test]
    fn pathway_markdown_uses_source_and_source_specific_evidence_url() {
        let pathway = Pathway {
            source: "KEGG".to_string(),
            id: "hsa05200".to_string(),
            name: "Pathways in cancer".to_string(),
            species: Some("Homo sapiens".to_string()),
            summary: Some("Cancer pathway overview.".to_string()),
            genes: vec!["BRAF".to_string(), "EGFR".to_string()],
            events: Vec::new(),
            enrichment: Vec::new(),
        };

        let markdown = pathway_markdown(&pathway, &[]).expect("rendered markdown");
        assert!(markdown.contains("Source: KEGG"));
        assert!(markdown.contains("[KEGG](https://www.kegg.jp/entry/hsa05200)"));

        let wikipathways = Pathway {
            source: "WikiPathways".to_string(),
            id: "WP254".to_string(),
            name: "Apoptosis".to_string(),
            species: Some("Homo sapiens".to_string()),
            summary: None,
            genes: vec!["TP53".to_string()],
            events: Vec::new(),
            enrichment: Vec::new(),
        };

        let markdown = pathway_markdown(&wikipathways, &[]).expect("rendered markdown");
        assert!(markdown.contains("Source: WikiPathways"));
        assert!(
            markdown.contains("[WikiPathways](https://www.wikipathways.org/pathways/WP254.html)")
        );
    }

    #[test]
    fn pathway_markdown_hides_genes_section_when_genes_are_empty() {
        let pathway = Pathway {
            source: "KEGG".to_string(),
            id: "hsa05200".to_string(),
            name: "Pathways in cancer".to_string(),
            species: Some("Homo sapiens".to_string()),
            summary: Some("Cancer pathway overview.".to_string()),
            genes: Vec::new(),
            events: Vec::new(),
            enrichment: Vec::new(),
        };

        let markdown = pathway_markdown(&pathway, &[]).expect("rendered markdown");
        assert!(!markdown.contains("## Genes"));
        assert!(!markdown.contains("BRAF"));
    }

    #[test]
    fn pathway_search_markdown_shows_source_column() {
        let results = vec![
            PathwaySearchResult {
                source: "Reactome".to_string(),
                id: "R-HSA-5673001".to_string(),
                name: "RAF/MAP kinase cascade".to_string(),
            },
            PathwaySearchResult {
                source: "KEGG".to_string(),
                id: "hsa04010".to_string(),
                name: "MAPK signaling pathway".to_string(),
            },
        ];

        let markdown =
            pathway_search_markdown("MAPK", &results, Some(results.len())).expect("markdown");
        assert!(markdown.contains("| Source | ID | Name |"));
        assert!(markdown.contains("| Reactome | R-HSA-5673001 | RAF/MAP kinase cascade |"));
        assert!(markdown.contains("| KEGG | hsa04010 | MAPK signaling pathway |"));
    }

    #[test]
    fn markdown_render_variant_entity() {
        let variant: Variant = serde_json::from_value(serde_json::json!({
            "id": "chr7:g.55259515T>G",
            "gene": "EGFR",
            "hgvs_p": "p.L858R",
            "legacy_name": "EGFR L858R",
            "significance": "Pathogenic"
        }))
        .expect("variant should deserialize");

        let markdown = variant_markdown(&variant, &[]).expect("rendered markdown");
        assert!(markdown.contains("EGFR"));
        assert!(markdown.contains("p.L858R"));
        assert!(markdown.contains("Legacy Name: EGFR L858R"));
    }

    #[test]
    fn variant_markdown_renders_compact_clinvar_and_population_fields() {
        let variant: Variant = serde_json::from_value(serde_json::json!({
            "id": "chr7:g.140453136A>T",
            "gene": "BRAF",
            "gnomad_af": 0.0001,
            "allele_frequency_percent": "0.0100%",
            "top_disease": {"condition": "Melanoma", "reports": 2},
            "clinvar_conditions": [{"condition": "Melanoma", "reports": 2}]
        }))
        .expect("variant should deserialize");

        let markdown = variant_markdown(&variant, &["all".to_string()]).expect("rendered markdown");
        assert!(markdown.contains("Top disease (ClinVar): Melanoma (2 reports)"));
        assert!(markdown.contains("gnomAD AF:"));
        assert!(markdown.contains("(0.0100%)"));
    }

    #[test]
    fn variant_markdown_renders_gwas_unavailable_message() {
        let variant: Variant = serde_json::from_value(serde_json::json!({
            "id": "rs7903146",
            "gene": "TCF7L2",
            "rsid": "rs7903146",
            "gwas": [],
            "gwas_unavailable_reason": "GWAS association data temporarily unavailable."
        }))
        .expect("variant should deserialize");

        let markdown =
            variant_markdown(&variant, &["gwas".to_string()]).expect("rendered markdown");
        assert!(markdown.contains("GWAS association data temporarily unavailable."));
        assert!(!markdown.contains("No GWAS associations found for this variant."));
    }

    #[test]
    fn variant_search_markdown_renders_legacy_name_column_and_fallback() {
        let results = vec![
            VariantSearchResult {
                id: "chr6:g.118880200T>G".to_string(),
                gene: "PLN".to_string(),
                hgvs_p: Some("p.L39X".to_string()),
                legacy_name: Some("PLN L39stop".to_string()),
                significance: Some("Pathogenic".to_string()),
                clinvar_stars: Some(2),
                gnomad_af: None,
                revel: Some(0.935),
                gerp: Some(5.12),
            },
            VariantSearchResult {
                id: "chr6:g.118880100A>G".to_string(),
                gene: "PLN".to_string(),
                hgvs_p: Some("p.K3R".to_string()),
                legacy_name: None,
                significance: None,
                clinvar_stars: None,
                gnomad_af: None,
                revel: None,
                gerp: None,
            },
        ];

        let markdown =
            variant_search_markdown("gene=PLN, hgvsp=L39X", &results).expect("rendered markdown");
        assert!(markdown.contains("| ID | Gene | Protein | Legacy Name | Significance |"));
        assert!(markdown.contains("| chr6:g.118880200T>G | PLN | p.L39X | PLN L39stop |"));
        assert!(markdown.contains("| chr6:g.118880100A>G | PLN | p.K3R | - |"));
    }

    #[test]
    fn variant_search_markdown_renders_related_commands_from_context() {
        let results = vec![
            VariantSearchResult {
                id: "rs199473688".to_string(),
                gene: "SCN5A".to_string(),
                hgvs_p: Some("p.Arg282His".to_string()),
                legacy_name: None,
                significance: Some("Pathogenic".to_string()),
                clinvar_stars: Some(2),
                gnomad_af: None,
                revel: Some(0.91),
                gerp: Some(5.7),
            },
            VariantSearchResult {
                id: "rs7626962".to_string(),
                gene: "SCN5A".to_string(),
                hgvs_p: Some("p.Gly514Cys".to_string()),
                legacy_name: None,
                significance: Some("Likely pathogenic".to_string()),
                clinvar_stars: Some(1),
                gnomad_af: None,
                revel: Some(0.88),
                gerp: Some(5.1),
            },
        ];

        let markdown = variant_search_markdown_with_context(
            "gene=SCN5A, condition=Brugada",
            &results,
            "",
            Some("SCN5A"),
            Some("Brugada"),
        )
        .expect("rendered markdown");

        assert!(markdown.contains("See also:"));
        assert!(markdown.contains("biomcp get variant rs199473688"));
        assert!(markdown.contains("biomcp get gene SCN5A"));
        assert!(markdown.contains("biomcp search disease --query Brugada"));
    }

    #[test]
    fn phenotype_search_markdown_renders_top_disease_follow_up() {
        let results = vec![
            crate::entities::disease::PhenotypeSearchResult {
                disease_id: "MONDO:0100135".to_string(),
                disease_name: "Dravet syndrome".to_string(),
                score: 15.036,
            },
            crate::entities::disease::PhenotypeSearchResult {
                disease_id: "MONDO:0000032".to_string(),
                disease_name: "febrile seizures, familial".to_string(),
                score: 15.036,
            },
        ];

        let markdown = phenotype_search_markdown_with_footer(
            "HP:0002373 HP:0001250",
            &results,
            "Showing 1-2 of 2 results.",
        )
        .expect("rendered markdown");

        assert!(markdown.contains("See also:"));
        assert!(markdown.contains("biomcp get disease \"Dravet syndrome\" genes phenotypes"));
        assert_eq!(
            related_command_description("biomcp get disease \"Dravet syndrome\" genes phenotypes"),
            Some("open the top phenotype-match disease with genes and phenotypes")
        );
    }

    #[test]
    fn related_variant_vus_promotes_literature_before_drug_target() {
        let variant: Variant = serde_json::from_value(serde_json::json!({
            "id": "chr2:g.166848047C>G",
            "gene": "SCN1A",
            "hgvs_p": "p.T1174S",
            "legacy_name": "SCN1A T1174S",
            "significance": "Uncertain significance",
            "top_disease": {"condition": "Dravet syndrome", "reports": 7}
        }))
        .expect("variant should deserialize");

        let related = related_variant(&variant);
        assert_eq!(related[0], "biomcp get gene SCN1A");
        assert_eq!(
            related[1],
            "biomcp search article -g SCN1A -d \"Dravet syndrome\" -k \"T1174S\" --limit 5"
        );
        assert_eq!(related[2], "biomcp search drug --target SCN1A");
        assert_eq!(
            related_command_description(&related[1]),
            Some("literature follow-up for an uncertain-significance variant")
        );
    }

    #[test]
    fn related_variant_vus_keyword_only_follow_up_keeps_description() {
        let variant: Variant = serde_json::from_value(serde_json::json!({
            "id": "chr2:g.166848047C>G",
            "gene": "",
            "hgvs_p": "p.T1174S",
            "significance": "VUS"
        }))
        .expect("variant should deserialize");

        let related = related_variant(&variant);
        assert_eq!(related[0], "biomcp search article -k \"T1174S\" --limit 5");
        assert_eq!(
            related_command_description(&related[0]),
            Some("literature follow-up for an uncertain-significance variant")
        );

        let rendered = format_related_block(related);
        assert!(rendered.contains("literature follow-up for an uncertain-significance variant"));
    }

    #[test]
    fn related_variant_pathogenic_keeps_drug_target_without_vus_literature_pivot() {
        let variant: Variant = serde_json::from_value(serde_json::json!({
            "id": "chr7:g.140453136A>T",
            "gene": "BRAF",
            "hgvs_p": "p.V600E",
            "legacy_name": "BRAF V600E",
            "significance": "Likely pathogenic",
            "top_disease": {"condition": "Melanoma", "reports": 5}
        }))
        .expect("variant should deserialize");

        let related = related_variant(&variant);
        assert_eq!(related[0], "biomcp get gene BRAF");
        assert_eq!(related[1], "biomcp search drug --target BRAF");
        assert!(
            !related
                .iter()
                .any(|cmd| cmd.starts_with("biomcp search article -g BRAF"))
        );
    }

    #[test]
    fn disease_markdown_renders_top_variant_summary() {
        let disease = Disease {
            id: "MONDO:0005105".to_string(),
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
            variants: vec![DiseaseVariantAssociation {
                variant: "BRAF V600E".to_string(),
                relationship: Some("associated with disease".to_string()),
                source: Some("CIViC".to_string()),
                evidence_count: Some(3),
            }],
            top_variant: Some(DiseaseVariantAssociation {
                variant: "BRAF V600E".to_string(),
                relationship: Some("associated with disease".to_string()),
                source: Some("CIViC".to_string()),
                evidence_count: Some(3),
            }),
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

        let markdown = disease_markdown(&disease, &["variants".to_string()]).expect("markdown");
        assert!(markdown.contains(
            "Top Variant: BRAF V600E - associated with disease (CIViC, 3 evidence items)"
        ));
    }

    #[test]
    fn disease_markdown_renders_survival_summary_and_note() {
        let disease = Disease {
            id: "MONDO:0001234".to_string(),
            name: "chronic myeloid leukemia".to_string(),
            definition: None,
            synonyms: vec!["CML".to_string()],
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
            survival: Some(crate::entities::disease::DiseaseSurvival {
                site_code: 97,
                site_label: "Chronic Myeloid Leukemia (CML)".to_string(),
                series: vec![
                    crate::entities::disease::DiseaseSurvivalSeries {
                        sex: "Both Sexes".to_string(),
                        latest_observed: Some(crate::entities::disease::DiseaseSurvivalPoint {
                            year: 2017,
                            relative_survival_rate: Some(69.4),
                            standard_error: Some(1.1),
                            lower_ci: Some(67.2),
                            upper_ci: Some(71.3),
                            modeled_relative_survival_rate: Some(70.4),
                            case_count: Some(471),
                        }),
                        latest_modeled: Some(crate::entities::disease::DiseaseSurvivalPoint {
                            year: 2018,
                            relative_survival_rate: None,
                            standard_error: None,
                            lower_ci: None,
                            upper_ci: None,
                            modeled_relative_survival_rate: Some(70.0),
                            case_count: None,
                        }),
                        points: vec![
                            crate::entities::disease::DiseaseSurvivalPoint {
                                year: 2016,
                                relative_survival_rate: Some(67.1),
                                standard_error: Some(1.2),
                                lower_ci: Some(64.8),
                                upper_ci: Some(69.5),
                                modeled_relative_survival_rate: Some(67.1),
                                case_count: Some(450),
                            },
                            crate::entities::disease::DiseaseSurvivalPoint {
                                year: 2017,
                                relative_survival_rate: Some(69.4),
                                standard_error: Some(1.1),
                                lower_ci: Some(67.2),
                                upper_ci: Some(71.3),
                                modeled_relative_survival_rate: Some(70.4),
                                case_count: Some(471),
                            },
                            crate::entities::disease::DiseaseSurvivalPoint {
                                year: 2018,
                                relative_survival_rate: None,
                                standard_error: None,
                                lower_ci: None,
                                upper_ci: None,
                                modeled_relative_survival_rate: Some(70.0),
                                case_count: None,
                            },
                        ],
                    },
                    crate::entities::disease::DiseaseSurvivalSeries {
                        sex: "Male".to_string(),
                        latest_observed: Some(crate::entities::disease::DiseaseSurvivalPoint {
                            year: 2017,
                            relative_survival_rate: Some(63.9),
                            standard_error: Some(1.6),
                            lower_ci: Some(60.8),
                            upper_ci: Some(66.9),
                            modeled_relative_survival_rate: Some(64.8),
                            case_count: Some(284),
                        }),
                        latest_modeled: Some(crate::entities::disease::DiseaseSurvivalPoint {
                            year: 2018,
                            relative_survival_rate: None,
                            standard_error: None,
                            lower_ci: None,
                            upper_ci: None,
                            modeled_relative_survival_rate: Some(64.2),
                            case_count: None,
                        }),
                        points: vec![
                            crate::entities::disease::DiseaseSurvivalPoint {
                                year: 2016,
                                relative_survival_rate: Some(61.3),
                                standard_error: Some(1.7),
                                lower_ci: Some(58.1),
                                upper_ci: Some(64.4),
                                modeled_relative_survival_rate: Some(61.3),
                                case_count: Some(273),
                            },
                            crate::entities::disease::DiseaseSurvivalPoint {
                                year: 2017,
                                relative_survival_rate: Some(63.9),
                                standard_error: Some(1.6),
                                lower_ci: Some(60.8),
                                upper_ci: Some(66.9),
                                modeled_relative_survival_rate: Some(64.8),
                                case_count: Some(284),
                            },
                            crate::entities::disease::DiseaseSurvivalPoint {
                                year: 2018,
                                relative_survival_rate: None,
                                standard_error: None,
                                lower_ci: None,
                                upper_ci: None,
                                modeled_relative_survival_rate: Some(64.2),
                                case_count: None,
                            },
                        ],
                    },
                ],
            }),
            survival_note: None,
            civic: None,
            disgenet: None,
            funding: None,
            funding_note: None,
            xrefs: std::collections::HashMap::new(),
        };

        let markdown = disease_markdown(&disease, &["survival".to_string()]).expect("markdown");
        assert!(markdown.contains("## Survival (SEER Explorer)"));
        assert!(markdown.contains("site code 97"));
        assert!(markdown.contains("All Ages"));
        assert!(markdown.contains("All Races / Ethnicities"));
        assert!(markdown.contains(
            "| Sex | Latest observed year | 5-year relative survival | 95% CI | Cases | Latest modeled |",
        ));
        assert!(
            markdown.contains("| Both Sexes | 2017 | 69.4% | 67.2%-71.3% | 471 | 2018: 70.0% |",)
        );
        assert!(markdown.contains("### Recent History"));
        assert!(markdown.contains("| Male | 2017 | 63.9% | 60.8%-66.9% | 284 |"));

        let mut note_disease = disease.clone();
        note_disease.survival = None;
        note_disease.survival_note =
            Some("SEER survival data not available for this condition.".to_string());

        let note_markdown =
            disease_markdown(&note_disease, &["survival".to_string()]).expect("note markdown");
        assert!(note_markdown.contains("## Survival (SEER Explorer)"));
        assert!(note_markdown.contains("SEER survival data not available for this condition."));
        assert!(!note_markdown.contains("| Sex | Latest observed year |"));
    }

    #[test]
    fn variant_oncokb_markdown_shows_truncation_note() {
        let result = VariantOncoKbResult {
            gene: "EGFR".to_string(),
            alteration: "L858R".to_string(),
            oncogenic: Some("Oncogenic".to_string()),
            level: Some("Level 1".to_string()),
            effect: Some("Gain-of-function".to_string()),
            therapies: vec![
                TreatmentImplication {
                    level: "Level 1".to_string(),
                    drugs: vec!["osimertinib".to_string()],
                    cancer_type: Some("Lung adenocarcinoma".to_string()),
                    note: None,
                },
                TreatmentImplication {
                    level: "Level 2".to_string(),
                    drugs: vec!["afatinib".to_string()],
                    cancer_type: Some("Lung adenocarcinoma".to_string()),
                    note: Some("(and 2 more)".to_string()),
                },
            ],
        };

        let markdown = variant_oncokb_markdown(&result);
        assert!(markdown.contains("| Drug | Level | Cancer Type | Note |"));
        assert!(markdown.contains("(and 2 more)"));
    }

    #[test]
    fn pagination_footer_offset_suppresses_more_when_complete_single_result() {
        let footer = pagination_footer(PaginationFooterMode::Offset, 0, 10, 1, Some(1), None);
        assert!(footer.contains("Showing 1 of 1 results."));
        assert!(!footer.contains("Use --offset"));
    }

    #[test]
    fn pagination_footer_offset_keeps_more_when_additional_rows_exist() {
        let footer = pagination_footer(PaginationFooterMode::Offset, 0, 2, 2, Some(10), None);
        assert!(footer.contains("Showing 1-2 of 10 results."));
        assert!(footer.contains("Use --offset 2 for more."));
    }

    #[test]
    fn pagination_footer_offset_suppresses_more_on_last_page() {
        let footer = pagination_footer(PaginationFooterMode::Offset, 8, 2, 2, Some(10), None);
        assert!(footer.contains("Showing 9-10 of 10 results."));
        assert!(!footer.contains("Use --offset"));
    }

    #[test]
    fn pagination_footer_cursor_prefers_offset_guidance_without_placeholder() {
        let footer = pagination_footer(
            PaginationFooterMode::Cursor,
            0,
            1,
            1,
            Some(20),
            Some("abc123"),
        );
        assert!(footer.contains("Use --offset 1 for more."));
        assert!(footer.contains("--next-page is also supported"));
        assert!(!footer.contains("<TOKEN>"));
    }

    #[test]
    fn related_article_uses_article_entities_helper_command() {
        let article = Article {
            pmid: Some("22663011".to_string()),
            pmcid: None,
            doi: None,
            title: "Improved survival with MEK inhibition in BRAF-mutated melanoma.".to_string(),
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
                genes: vec![
                    AnnotationCount {
                        text: "serine-threonine protein kinase".to_string(),
                        count: 7,
                    },
                    AnnotationCount {
                        text: "BRAF".to_string(),
                        count: 5,
                    },
                    AnnotationCount {
                        text: "MEK".to_string(),
                        count: 3,
                    },
                    AnnotationCount {
                        text: "B-RAF".to_string(),
                        count: 1,
                    },
                ],
                diseases: vec![
                    AnnotationCount {
                        text: "melanoma".to_string(),
                        count: 2,
                    },
                    AnnotationCount {
                        text: "metastatic melanoma".to_string(),
                        count: 1,
                    },
                ],
                chemicals: vec![AnnotationCount {
                    text: "trametinib".to_string(),
                    count: 8,
                }],
                mutations: Vec::new(),
            }),
            semantic_scholar: None,
            pubtator_fallback: false,
        };

        let related = related_article(&article);
        assert_eq!(related[0], "biomcp article entities 22663011");
        let braf = related
            .iter()
            .position(|cmd| cmd == "biomcp search gene -q BRAF")
            .expect("curated BRAF pivot should be promoted");
        let mek = related
            .iter()
            .position(|cmd| cmd == "biomcp search gene -q MEK")
            .expect("curated MEK pivot should be promoted");
        let melanoma = related
            .iter()
            .position(|cmd| cmd == "biomcp search disease --query melanoma")
            .expect("disease pivot should be promoted");
        let trametinib = related
            .iter()
            .position(|cmd| cmd == "biomcp get drug trametinib")
            .expect("drug pivot should be promoted");
        let references = related
            .iter()
            .position(|cmd| cmd == "biomcp article references 22663011 --limit 3")
            .expect("references command should remain available");
        let citations = related
            .iter()
            .position(|cmd| cmd == "biomcp article citations 22663011 --limit 3")
            .expect("citations command should remain available");
        let recommendations = related
            .iter()
            .position(|cmd| cmd == "biomcp article recommendations 22663011 --limit 3")
            .expect("recommendations command should remain available");

        assert!(braf < references);
        assert!(mek < references);
        assert!(melanoma < citations);
        assert!(trametinib < recommendations);
        assert!(references < citations);
        assert!(citations < recommendations);
        assert!(
            !related
                .iter()
                .any(|cmd| cmd == "biomcp get gene serine-threonine protein kinase")
        );
        assert!(
            !related
                .iter()
                .any(|cmd| cmd == "biomcp search gene -q \"serine-threonine protein kinase\"")
        );
        assert!(!related.iter().any(|cmd| cmd.contains("biomcp get article")));

        let rendered = format_related_block(related);
        assert!(rendered.contains("standardized entity extraction"));
        assert!(rendered.contains(
            "background evidence this paper builds on; use if the primary paper lacks context"
        ));
        assert!(rendered.contains(
            "later papers that cite this article; use only if the primary paper lacks your answer"
        ));
        assert!(rendered.contains(
            "related papers to broaden coverage; use only if the primary paper lacks your answer"
        ));
    }

    #[test]
    fn article_entities_markdown_uses_safe_gene_search_commands() {
        let annotations = ArticleAnnotations {
            genes: vec![
                AnnotationCount {
                    text: "BRAF".to_string(),
                    count: 5,
                },
                AnnotationCount {
                    text: "serine-threonine protein kinase".to_string(),
                    count: 1,
                },
            ],
            diseases: Vec::new(),
            chemicals: Vec::new(),
            mutations: vec![AnnotationCount {
                text: "V600E".to_string(),
                count: 2,
            }],
        };

        let markdown =
            article_entities_markdown("22663011", Some(&annotations), Some(5)).expect("markdown");
        assert!(markdown.contains("`biomcp search gene -q BRAF`"));
        assert!(markdown.contains("`biomcp search gene -q \"serine-threonine protein kinase\"`"));
        assert!(!markdown.contains("`biomcp get gene serine-threonine protein kinase`"));
        assert!(markdown.contains("`biomcp get variant V600E`"));
    }

    #[test]
    fn related_trial_promotes_results_search_for_completed_or_terminated_studies() {
        let trial = crate::entities::trial::Trial {
            nct_id: "NCT02576665".to_string(),
            source: None,
            title: "A Study of Toca 511, a Retroviral Replicating Vector, Combined With Toca FC in Patients With Solid Tumors or Lymphoma (Toca 6)".to_string(),
            status: "TERMINATED".to_string(),
            phase: None,
            study_type: None,
            age_range: None,
            conditions: vec!["Colorectal Cancer".to_string()],
            interventions: vec!["Toca 511".to_string()],
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

        let related = related_trial(&trial);
        assert_eq!(
            related[0],
            "biomcp search article --drug \"Toca 511\" -q \"NCT02576665 A Study of Toca 511, a\" --limit 5"
        );
        assert_eq!(
            related[1],
            "biomcp search disease --query \"Colorectal Cancer\""
        );

        let rendered = format_related_block(related.clone());
        assert!(rendered.contains(
            "find publications or conference reports from this completed/terminated trial"
        ));
        assert_eq!(
            related_command_description(&related[0]),
            Some("find publications or conference reports from this completed/terminated trial")
        );
        assert_eq!(
            related_command_description("biomcp search article --drug pembrolizumab --limit 5"),
            None
        );
    }

    #[test]
    fn related_trial_keeps_recruiting_order_without_results_search() {
        let trial = crate::entities::trial::Trial {
            nct_id: "NCT01234567".to_string(),
            source: None,
            title: "Example trial".to_string(),
            status: "Recruiting".to_string(),
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

        let related = related_trial(&trial);
        assert_eq!(related[0], "biomcp search disease --query melanoma");
        assert!(!related.iter().any(|cmd| {
            cmd.starts_with("biomcp search article --drug ") && cmd.contains(" --limit 5")
        }));
    }

    #[test]
    fn related_trial_completed_promotes_results_search_before_condition_pivots() {
        let trial = crate::entities::trial::Trial {
            nct_id: "NCT01234567".to_string(),
            source: None,
            title: "Example completed trial".to_string(),
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

        let related = related_trial(&trial);
        assert_eq!(
            related[0],
            "biomcp search article --drug dabrafenib -q \"NCT01234567 Example completed trial\" --limit 5"
        );
        assert_eq!(related[1], "biomcp search disease --query melanoma");
    }

    #[test]
    fn related_trial_results_search_without_intervention_keeps_seed_quoted() {
        let trial = crate::entities::trial::Trial {
            nct_id: "NCT09999999".to_string(),
            source: None,
            title: "   ".to_string(),
            status: "Completed".to_string(),
            phase: None,
            study_type: None,
            age_range: None,
            conditions: vec!["melanoma".to_string()],
            interventions: Vec::new(),
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

        let related = related_trial(&trial);
        assert_eq!(
            related[0],
            "biomcp search article -q \"NCT09999999\" --limit 5"
        );
        assert_eq!(
            related_command_description(&related[0]),
            Some("find publications or conference reports from this completed/terminated trial")
        );
    }

    #[test]
    fn article_markdown_renders_semantic_scholar_section() {
        let article = Article {
            pmid: Some("22663011".to_string()),
            pmcid: None,
            doi: Some("10.1000/example".to_string()),
            title: "Example".to_string(),
            authors: Vec::new(),
            journal: Some("Example Journal".to_string()),
            date: Some("2024-01-01".to_string()),
            citation_count: Some(12),
            publication_type: None,
            open_access: Some(true),
            abstract_text: None,
            full_text_path: None,
            full_text_note: None,
            annotations: None,
            semantic_scholar: Some(crate::entities::article::ArticleSemanticScholar {
                paper_id: Some("paper-1".to_string()),
                tldr: Some("A concise summary.".to_string()),
                citation_count: Some(20),
                influential_citation_count: Some(4),
                reference_count: Some(10),
                is_open_access: Some(true),
                open_access_pdf: Some(crate::entities::article::ArticleSemanticScholarPdf {
                    url: "https://example.org/paper.pdf".to_string(),
                    status: Some("GREEN".to_string()),
                    license: Some("CC-BY".to_string()),
                }),
            }),
            pubtator_fallback: false,
        };

        let markdown =
            article_markdown(&article, &["tldr".to_string()]).expect("markdown should render");
        assert!(markdown.contains("## Semantic Scholar"));
        assert!(markdown.contains("TLDR: A concise summary."));
        assert!(markdown.contains("Influential citations: 4"));
        assert!(markdown.contains("Open-access PDF: https://example.org/paper.pdf"));
    }

    #[test]
    fn article_graph_markdown_renders_expected_table_headers() {
        let result = crate::entities::article::ArticleGraphResult {
            article: crate::entities::article::ArticleRelatedPaper {
                paper_id: Some("paper-1".to_string()),
                pmid: Some("22663011".to_string()),
                doi: None,
                arxiv_id: None,
                title: "Seed".to_string(),
                journal: None,
                year: Some(2012),
            },
            edges: vec![crate::entities::article::ArticleGraphEdge {
                paper: crate::entities::article::ArticleRelatedPaper {
                    paper_id: Some("paper-2".to_string()),
                    pmid: Some("24200969".to_string()),
                    doi: None,
                    arxiv_id: None,
                    title: "Related paper".to_string(),
                    journal: Some("Nature".to_string()),
                    year: Some(2014),
                },
                intents: vec!["Background".to_string()],
                contexts: vec!["Important supporting context".to_string()],
                is_influential: true,
            }],
        };

        let markdown = article_graph_markdown("Citations", &result).expect("graph markdown");
        assert!(markdown.contains("# Citations for PMID 22663011"));
        assert!(markdown.contains("| PMID | Title | Intents | Influential | Context |"));
        assert!(markdown.contains(
            "| 24200969 | Related paper | Background | yes | Important supporting context |"
        ));
    }

    #[test]
    fn article_batch_markdown_renders_compact_rows() {
        let rows = vec![
            crate::entities::article::ArticleBatchItem {
                requested_id: "22663011".to_string(),
                pmid: Some("22663011".to_string()),
                pmcid: None,
                doi: Some("10.1056/NEJMoa1203421".to_string()),
                title: "Improved survival with vemurafenib".to_string(),
                journal: Some("NEJM".to_string()),
                year: Some(2012),
                entity_summary: Some(crate::entities::article::ArticleBatchEntitySummary {
                    genes: vec![crate::entities::article::AnnotationCount {
                        text: "BRAF".to_string(),
                        count: 4,
                    }],
                    diseases: vec![crate::entities::article::AnnotationCount {
                        text: "melanoma".to_string(),
                        count: 2,
                    }],
                    chemicals: Vec::new(),
                    mutations: Vec::new(),
                }),
                tldr: Some("BRAF inhibitor benefit in melanoma.".to_string()),
                citation_count: Some(120),
                influential_citation_count: Some(18),
            },
            crate::entities::article::ArticleBatchItem {
                requested_id: "PMC9984800".to_string(),
                pmid: Some("24200969".to_string()),
                pmcid: Some("PMC9984800".to_string()),
                doi: None,
                title: "Follow-up trial".to_string(),
                journal: Some("Nature".to_string()),
                year: Some(2014),
                entity_summary: None,
                tldr: None,
                citation_count: None,
                influential_citation_count: None,
            },
        ];

        let markdown = article_batch_markdown(&rows).expect("batch markdown");
        assert!(markdown.contains("# Article Batch (2)"));
        assert!(markdown.contains("## 1. Improved survival with vemurafenib"));
        assert!(markdown.contains("PMID: 22663011"));
        assert!(markdown.contains("Entities: Genes: BRAF (4); Diseases: melanoma (2)"));
        assert!(markdown.contains("TLDR: BRAF inhibitor benefit in melanoma."));
        assert!(markdown.contains("Citations: 120 (influential: 18)"));
        assert!(markdown.contains("## 2. Follow-up trial"));
        assert!(markdown.contains("PMID: 24200969"));
        // Absent optional fields are omitted, not printed as placeholders
        assert!(!markdown.contains("TLDR: -"));
        assert!(!markdown.contains("Entities: -"));
    }

    #[test]
    fn related_pgx_uses_search_flags() {
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

        let related = related_pgx(&pgx);
        assert!(related.contains(&"biomcp search pgx -g CYP2D6".to_string()));
        assert!(related.contains(&"biomcp search pgx -d \"warfarin sodium\"".to_string()));
    }

    #[test]
    fn gene_evidence_urls_include_ensembl_and_omim() {
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

        let urls = gene_evidence_urls(&gene);
        assert!(urls.contains(&(
            "Ensembl",
            "https://www.ensembl.org/Homo_sapiens/Gene/Summary?g=ENSG00000157764".to_string()
        )));
        assert!(urls.contains(&("OMIM", "https://www.omim.org/entry/164757".to_string())));
    }

    #[test]
    fn disease_markdown_section_only_shows_disgenet_section() {
        let disease = Disease {
            id: "MONDO:0007254".to_string(),
            name: "breast cancer".to_string(),
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
            disgenet: Some(crate::entities::disease::DiseaseDisgenet {
                associations: vec![crate::entities::disease::DiseaseDisgenetAssociation {
                    symbol: "TP53".to_string(),
                    entrez_id: Some(7157),
                    score: 0.91,
                    publication_count: Some(1234),
                    clinical_trial_count: Some(4),
                    evidence_index: Some(0.72),
                    evidence_level: Some("Definitive".to_string()),
                }],
            }),
            funding: None,
            funding_note: None,
            xrefs: std::collections::HashMap::new(),
        };

        let markdown =
            disease_markdown(&disease, &["disgenet".to_string()]).expect("rendered markdown");

        assert!(markdown.contains("# breast cancer - disgenet"));
        assert!(markdown.contains("## DisGeNET"));
        assert!(markdown.contains("| Gene | Entrez ID | Score | PMIDs | Trials | EL | EI |"));
        assert!(markdown.contains("| TP53 | 7157 | 0.910 | 1234 | 4 | Definitive | 0.720 |"));
    }

    #[test]
    fn disease_markdown_disgenet_renders_sparse_optional_fields() {
        let disease = Disease {
            id: "MONDO:0000001".to_string(),
            name: "sparse disease".to_string(),
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
            disgenet: Some(crate::entities::disease::DiseaseDisgenet {
                associations: vec![crate::entities::disease::DiseaseDisgenetAssociation {
                    symbol: "KYNU".to_string(),
                    entrez_id: None,
                    score: 0.23,
                    publication_count: None,
                    clinical_trial_count: None,
                    evidence_index: None,
                    evidence_level: None,
                }],
            }),
            funding: None,
            funding_note: None,
            xrefs: std::collections::HashMap::new(),
        };

        let markdown =
            disease_markdown(&disease, &["disgenet".to_string()]).expect("rendered markdown");

        assert!(markdown.contains("| Gene | Entrez ID | Score | PMIDs | Trials | EL | EI |"));
        assert!(markdown.contains("| KYNU | - | 0.230 | - | - | - | - |"));
    }

    #[test]
    fn disease_markdown_funding_renders_truthful_notes_without_table() {
        let mut disease = Disease {
            id: "MONDO:0007947".to_string(),
            name: "Marfan syndrome".to_string(),
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
            funding: Some(crate::sources::nih_reporter::NihReporterFundingSection {
                query: "Marfan syndrome".to_string(),
                fiscal_years: vec![2022, 2023, 2024, 2025, 2026],
                matching_project_years: 0,
                grants: Vec::new(),
            }),
            funding_note: Some("No NIH funding data found for this query.".to_string()),
            xrefs: std::collections::HashMap::new(),
        };

        let no_hit =
            disease_markdown(&disease, &["funding".to_string()]).expect("no-hit funding markdown");
        assert!(no_hit.contains("## Funding (NIH Reporter)"));
        assert!(no_hit.contains("No NIH funding data found for this query."));
        assert!(!no_hit.contains("| Project | PI | Organization | FY | Amount |"));

        disease.funding = None;
        disease.funding_note =
            Some("NIH Reporter funding data is temporarily unavailable.".to_string());

        let unavailable = disease_markdown(&disease, &["funding".to_string()])
            .expect("unavailable funding markdown");
        assert!(unavailable.contains("## Funding (NIH Reporter)"));
        assert!(unavailable.contains("NIH Reporter funding data is temporarily unavailable."));
        assert!(!unavailable.contains("| Project | PI | Organization | FY | Amount |"));
    }

    #[test]
    fn disease_markdown_all_keeps_opt_in_sections_hidden() {
        let disease = Disease {
            id: "MONDO:0007947".to_string(),
            name: "Marfan syndrome".to_string(),
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

        let markdown = disease_markdown(&disease, &["all".to_string()]).expect("all markdown");

        assert!(!markdown.contains("## Funding (NIH Reporter)"));
        assert!(!markdown.contains("## DisGeNET"));
    }

    #[test]
    fn variant_evidence_urls_include_dbsnp_and_cosmic() {
        let variant: Variant = serde_json::from_value(serde_json::json!({
            "id": "chr7:g.140453136A>T",
            "gene": "BRAF",
            "rsid": "rs113488022",
            "cosmic_id": "COSM476",
            "clinvar_id": "13961"
        }))
        .expect("variant should deserialize");

        let urls = variant_evidence_urls(&variant);
        assert!(urls.contains(&(
            "dbSNP",
            "https://www.ncbi.nlm.nih.gov/snp/rs113488022".to_string()
        )));
        assert!(urls.contains(&(
            "COSMIC",
            "https://cancer.sanger.ac.uk/cosmic/mutation/overview?id=COSM476".to_string()
        )));
    }

    #[test]
    fn variant_evidence_urls_include_gnomad_for_population_data() {
        let variant: Variant = serde_json::from_value(serde_json::json!({
            "id": "chr11:g.5227002A>T",
            "gene": "HBB",
            "rsid": "rs334",
            "gnomad_af": 0.042
        }))
        .expect("variant should deserialize");

        let urls = variant_evidence_urls(&variant);
        assert!(urls.contains(&(
            "gnomAD",
            "https://gnomad.broadinstitute.org/variant/rs334".to_string()
        )));
    }

    #[test]
    fn variant_evidence_urls_fall_back_to_hgvs_slug_for_population_data() {
        let variant: Variant = serde_json::from_value(serde_json::json!({
            "id": "chr7:g.140453136A>T",
            "gene": "BRAF",
            "population_breakdown": {
                "populations": [{"population": "global", "af": 0.01}]
            }
        }))
        .expect("variant should deserialize");

        let urls = variant_evidence_urls(&variant);
        assert!(urls.contains(&(
            "gnomAD",
            "https://gnomad.broadinstitute.org/variant/7-140453136-A-T".to_string()
        )));
    }

    #[test]
    fn disease_markdown_links_source_cells_and_footer_evidence_urls() {
        let disease = Disease {
            id: "MONDO:0009061".to_string(),
            name: "cystic fibrosis".to_string(),
            definition: None,
            synonyms: Vec::new(),
            parents: Vec::new(),
            associated_genes: Vec::new(),
            gene_associations: vec![crate::entities::disease::DiseaseGeneAssociation {
                gene: "CFTR".to_string(),
                relationship: Some("gene associated with condition".to_string()),
                source: Some("infores:orphanet".to_string()),
                opentargets_score: None,
            }],
            top_genes: Vec::new(),
            top_gene_scores: Vec::new(),
            treatment_landscape: Vec::new(),
            recruiting_trial_count: None,
            pathways: Vec::new(),
            phenotypes: vec![crate::entities::disease::DiseasePhenotype {
                hpo_id: "HP:0001945".to_string(),
                name: Some("Dehydration".to_string()),
                evidence: None,
                frequency: None,
                frequency_qualifier: None,
                onset_qualifier: None,
                sex_qualifier: None,
                stage_qualifier: None,
                qualifiers: Vec::new(),
                source: Some("infores:omim".to_string()),
            }],
            key_features: Vec::new(),
            variants: Vec::new(),
            top_variant: None,
            models: vec![crate::entities::disease::DiseaseModelAssociation {
                model: "Cftr tm1Unc".to_string(),
                model_id: Some("MGI:3698752".to_string()),
                organism: Some("Mus musculus".to_string()),
                relationship: Some("model of".to_string()),
                source: Some("infores:mgi".to_string()),
                evidence_count: Some(3),
            }],
            prevalence: Vec::new(),
            prevalence_note: None,
            survival: None,
            survival_note: None,
            civic: None,
            disgenet: None,
            funding: None,
            funding_note: None,
            xrefs: std::collections::HashMap::from([
                ("Orphanet".to_string(), "586".to_string()),
                ("OMIM".to_string(), "219700".to_string()),
            ]),
        };

        let markdown = disease_markdown(&disease, &["all".to_string()]).expect("markdown");
        assert!(
            markdown.contains("[infores:orphanet](https://www.orpha.net/en/disease/detail/586)")
        );
        assert!(markdown.contains("[infores:omim](https://www.omim.org/entry/219700)"));
        assert!(markdown.contains("[infores:mgi](https://www.informatics.jax.org/accession/MGI:"));
        assert!(markdown.contains("[Orphanet](https://www.orpha.net/en/disease/detail/586)"));
        assert!(markdown.contains("[OMIM](https://www.omim.org/entry/219700)"));
        assert!(markdown.contains("[MGI](https://www.informatics.jax.org/accession/MGI:"));
    }

    #[test]
    fn disease_evidence_urls_include_record_links() {
        let disease = Disease {
            id: "MONDO:0009061".to_string(),
            name: "cystic fibrosis".to_string(),
            definition: None,
            synonyms: Vec::new(),
            parents: Vec::new(),
            associated_genes: Vec::new(),
            gene_associations: vec![crate::entities::disease::DiseaseGeneAssociation {
                gene: "CFTR".to_string(),
                relationship: None,
                source: Some("infores:orphanet".to_string()),
                opentargets_score: None,
            }],
            top_genes: Vec::new(),
            top_gene_scores: Vec::new(),
            treatment_landscape: Vec::new(),
            recruiting_trial_count: None,
            pathways: Vec::new(),
            phenotypes: vec![crate::entities::disease::DiseasePhenotype {
                hpo_id: "HP:0001945".to_string(),
                name: Some("Dehydration".to_string()),
                evidence: None,
                frequency: None,
                frequency_qualifier: None,
                onset_qualifier: None,
                sex_qualifier: None,
                stage_qualifier: None,
                qualifiers: Vec::new(),
                source: Some("infores:omim".to_string()),
            }],
            key_features: Vec::new(),
            variants: Vec::new(),
            top_variant: None,
            models: vec![crate::entities::disease::DiseaseModelAssociation {
                model: "MGI:3698752".to_string(),
                model_id: None,
                organism: Some("Mus musculus".to_string()),
                relationship: Some("model of".to_string()),
                source: Some("infores:mgi".to_string()),
                evidence_count: Some(2),
            }],
            prevalence: Vec::new(),
            prevalence_note: None,
            survival: None,
            survival_note: None,
            civic: None,
            disgenet: None,
            funding: None,
            funding_note: None,
            xrefs: std::collections::HashMap::from([
                ("Orphanet".to_string(), "586".to_string()),
                ("OMIM".to_string(), "219700".to_string()),
            ]),
        };

        let urls = disease_evidence_urls(&disease);
        assert!(urls.contains(&(
            "Orphanet",
            "https://www.orpha.net/en/disease/detail/586".to_string()
        )));
        assert!(urls.contains(&("OMIM", "https://www.omim.org/entry/219700".to_string())));
        assert!(urls.iter().any(|(label, url)| {
            *label == "MGI" && url.starts_with("https://www.informatics.jax.org/accession/MGI:")
        }));
    }

    #[test]
    fn drug_evidence_urls_include_chembl() {
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
            targets: Vec::new(),
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

        let urls = drug_evidence_urls(&drug);
        assert!(urls.contains(&(
            "ChEMBL",
            "https://www.ebi.ac.uk/chembl/compound_report_card/CHEMBL3353410".to_string()
        )));
    }

    #[test]
    fn drug_evidence_urls_include_faers_and_dailymed_when_sections_exist() {
        let drug = Drug {
            name: "ivacaftor".to_string(),
            drugbank_id: None,
            chembl_id: None,
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
            targets: Vec::new(),
            variant_targets: Vec::new(),
            target_family: None,
            target_family_name: None,
            indications: Vec::new(),
            interactions: Vec::new(),
            interaction_text: None,
            pharm_classes: Vec::new(),
            top_adverse_events: vec!["Rash".to_string()],
            faers_query: Some(
                "(patient.drug.openfda.generic_name:\"ivacaftor\") AND patient.drug.drugcharacterization:1"
                    .to_string(),
            ),
            label: Some(crate::entities::drug::DrugLabel {
                indication_summary: Vec::new(),
                indications: None,
                warnings: Some("Warnings".to_string()),
                dosage: None,
            }),
            label_set_id: Some("set-123".to_string()),
            shortage: None,
            approvals: None,
            us_safety_warnings: None,
            ema_regulatory: None,
            ema_safety: None,
            ema_shortage: None,
            who_prequalification: None,
            civic: None,
        };

        let urls = drug_evidence_urls(&drug);
        assert!(urls.iter().any(|(label, url)| {
            *label == "OpenFDA FAERS"
                && url.starts_with("https://api.fda.gov/drug/event.json?search=")
                && url.contains("count=patient.reaction.reactionmeddrapt.exact")
        }));
        assert!(urls.iter().any(|(label, url)| {
            *label == "DailyMed" && url.contains("/drugInfo.cfm?setid=set-123")
        }));
    }

    #[test]
    fn drug_markdown_uses_label_interaction_text_before_public_unavailable_fallback() {
        let drug = Drug {
            name: "warfarin".to_string(),
            drugbank_id: Some("DB00682".to_string()),
            chembl_id: None,
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
            targets: Vec::new(),
            variant_targets: Vec::new(),
            target_family: None,
            target_family_name: None,
            indications: Vec::new(),
            interactions: Vec::new(),
            interaction_text: Some(
                "DRUG INTERACTIONS\n\nWarfarin interacts with aspirin.".to_string(),
            ),
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

        let markdown = drug_markdown(&drug, &["interactions".to_string()]).expect("markdown");
        assert!(markdown.contains("## Interactions"));
        assert!(markdown.contains("DRUG INTERACTIONS"));
        assert!(!markdown.contains("No known drug-drug interactions found."));
    }

    #[test]
    fn drug_markdown_uses_truthful_public_unavailable_interactions_message() {
        let drug = Drug {
            name: "pembrolizumab".to_string(),
            drugbank_id: Some("DB09037".to_string()),
            chembl_id: None,
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
            targets: Vec::new(),
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

        let markdown = drug_markdown(&drug, &["interactions".to_string()]).expect("markdown");
        assert!(markdown.contains("Interaction details not available from public sources."));
        assert!(!markdown.contains("No known drug-drug interactions found."));
    }

    #[test]
    fn drug_markdown_shows_target_family_and_members_when_present() {
        let drug = Drug {
            name: "olaparib".to_string(),
            drugbank_id: Some("DB09074".to_string()),
            chembl_id: Some("CHEMBL1789941".to_string()),
            unii: None,
            drug_type: Some("small molecule".to_string()),
            mechanism: Some("PARP inhibitor".to_string()),
            mechanisms: vec!["PARP inhibitor".to_string()],
            approval_date: None,
            approval_date_raw: None,
            approval_date_display: None,
            approval_summary: None,
            brand_names: Vec::new(),
            route: None,
            targets: vec![
                "PARP1".to_string(),
                "PARP2".to_string(),
                "PARP3".to_string(),
            ],
            variant_targets: Vec::new(),
            target_family: Some("PARP".to_string()),
            target_family_name: Some("poly(ADP-ribose) polymerase".to_string()),
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

        let markdown = drug_markdown(&drug, &["targets".to_string()]).expect("markdown");
        assert!(markdown.contains("Family: PARP (poly(ADP-ribose) polymerase)"));
        assert!(markdown.contains("Members: PARP1, PARP2, PARP3"));
    }

    #[test]
    fn drug_markdown_renders_variant_targets_as_additive_line() {
        let drug = Drug {
            name: "rindopepimut".to_string(),
            drugbank_id: None,
            chembl_id: Some("CHEMBL2108508".to_string()),
            unii: None,
            drug_type: Some("vaccine".to_string()),
            mechanism: None,
            mechanisms: Vec::new(),
            approval_date: None,
            approval_date_raw: None,
            approval_date_display: None,
            approval_summary: None,
            brand_names: Vec::new(),
            route: None,
            targets: vec!["EGFR".to_string()],
            variant_targets: vec!["EGFRvIII".to_string()],
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

        let markdown = drug_markdown(&drug, &["targets".to_string()]).expect("markdown");
        assert!(markdown.contains("## Targets (ChEMBL / Open Targets)"));
        assert!(markdown.contains("EGFR"));
        assert!(markdown.contains("Variant Targets (CIViC): EGFRvIII"));
    }

    #[test]
    fn drug_markdown_omits_target_family_for_mixed_targets() {
        let drug = Drug {
            name: "imatinib".to_string(),
            drugbank_id: Some("DB00619".to_string()),
            chembl_id: Some("CHEMBL941".to_string()),
            unii: None,
            drug_type: Some("small-molecule".to_string()),
            mechanism: Some("Inhibitor of BCR-ABL".to_string()),
            mechanisms: vec!["Inhibitor of BCR-ABL".to_string()],
            approval_date: None,
            approval_date_raw: None,
            approval_date_display: None,
            approval_summary: None,
            brand_names: Vec::new(),
            route: None,
            targets: vec!["ABL1".to_string(), "KIT".to_string(), "PDGFRB".to_string()],
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

        let markdown = drug_markdown(&drug, &["targets".to_string()]).expect("markdown");
        assert!(!markdown.contains("Family:"));
        assert!(!markdown.contains("Members:"));
        assert!(markdown.contains("ABL1, KIT, PDGFRB"));
    }

    #[test]
    fn drug_markdown_with_region_all_keeps_us_and_eu_blocks_separate() {
        let drug = Drug {
            name: "pembrolizumab".to_string(),
            drugbank_id: Some("DB09037".to_string()),
            chembl_id: None,
            unii: None,
            drug_type: None,
            mechanism: None,
            mechanisms: Vec::new(),
            approval_date: None,
            approval_date_raw: None,
            approval_date_display: None,
            approval_summary: None,
            brand_names: vec!["Keytruda".to_string()],
            route: None,
            targets: Vec::new(),
            variant_targets: Vec::new(),
            target_family: None,
            target_family_name: None,
            indications: Vec::new(),
            interactions: Vec::new(),
            interaction_text: None,
            pharm_classes: Vec::new(),
            top_adverse_events: vec!["Rash".to_string()],
            faers_query: None,
            label: None,
            label_set_id: None,
            shortage: Some(vec![crate::entities::drug::DrugShortageEntry {
                status: Some("Current".to_string()),
                availability: Some("Limited".to_string()),
                company_name: Some("Example Pharma".to_string()),
                generic_name: Some("pembrolizumab".to_string()),
                related_info: Some("https://example.org/us-shortage".to_string()),
                update_date: Some("2026-01-13".to_string()),
                initial_posting_date: None,
            }]),
            approvals: Some(vec![DrugApproval {
                application_number: "BLA125514".to_string(),
                sponsor_name: Some("Merck Sharp & Dohme".to_string()),
                openfda_brand_names: vec!["Keytruda".to_string()],
                openfda_generic_names: vec!["pembrolizumab".to_string()],
                products: Vec::new(),
                submissions: Vec::new(),
            }]),
            us_safety_warnings: Some("Immune-mediated adverse reactions.".to_string()),
            ema_regulatory: Some(vec![EmaRegulatoryRow {
                medicine_name: "Keytruda".to_string(),
                active_substance: "pembrolizumab".to_string(),
                ema_product_number: "EMEA/H/C/003820".to_string(),
                status: "Authorised".to_string(),
                holder: Some("Merck Sharp & Dohme B.V.".to_string()),
                recent_activity: vec![crate::entities::drug::EmaRegulatoryActivity {
                    first_published_date: "27/02/2026".to_string(),
                    last_updated_date: None,
                }],
            }]),
            ema_safety: Some(EmaSafetyInfo {
                dhpcs: vec![crate::entities::drug::EmaDhpcEntry {
                    medicine_name: "Keytruda".to_string(),
                    dhpc_type: Some("DHPC".to_string()),
                    regulatory_outcome: Some("Updated safety communication".to_string()),
                    first_published_date: Some("15/01/2026".to_string()),
                    last_updated_date: None,
                }],
                referrals: Vec::new(),
                psusas: Vec::new(),
            }),
            ema_shortage: Some(vec![EmaShortageEntry {
                medicine_affected: "Keytruda".to_string(),
                status: Some("Resolved".to_string()),
                availability_of_alternatives: Some("Yes".to_string()),
                first_published_date: Some("10/01/2026".to_string()),
                last_updated_date: Some("13/01/2026".to_string()),
            }]),
            who_prequalification: None,
            civic: None,
        };

        let markdown =
            drug_markdown_with_region(&drug, &["all".to_string()], DrugRegion::All, false)
                .expect("markdown");
        assert!(markdown.contains("## Regulatory (US - Drugs@FDA)"));
        assert!(markdown.contains("## Regulatory (EU - EMA)"));
        assert!(markdown.contains("## Safety (US - OpenFDA)"));
        assert!(markdown.contains("## Safety (EU - EMA)"));
        assert!(markdown.contains("## Shortage (US - OpenFDA Drug Shortages)"));
        assert!(markdown.contains("## Shortage (EU - EMA)"));
        assert!(markdown.contains("BLA125514"));
        assert!(markdown.contains("EMEA/H/C/003820"));
        assert!(markdown.contains("Immune-mediated adverse reactions."));
        assert!(markdown.contains("Resolved"));
    }

    #[test]
    fn drug_markdown_with_region_who_renders_regulatory_block() {
        let drug = Drug {
            name: "trastuzumab".to_string(),
            drugbank_id: Some("DB00072".to_string()),
            chembl_id: None,
            unii: None,
            drug_type: None,
            mechanism: None,
            mechanisms: Vec::new(),
            approval_date: None,
            approval_date_raw: None,
            approval_date_display: None,
            approval_summary: None,
            brand_names: vec!["Herceptin".to_string()],
            route: None,
            targets: Vec::new(),
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
            who_prequalification: Some(vec![WhoPrequalificationEntry {
                who_reference_number: "BT-ON001".to_string(),
                inn: "Trastuzumab".to_string(),
                presentation: "Trastuzumab Powder for concentrate for solution for infusion 150 mg"
                    .to_string(),
                dosage_form: "Powder for concentrate for solution for infusion".to_string(),
                product_type: "Biotherapeutic Product".to_string(),
                therapeutic_area: "Oncology".to_string(),
                applicant: "Samsung Bioepis NL B.V.".to_string(),
                listing_basis: "Prequalification - Abridged".to_string(),
                alternative_listing_basis: None,
                prequalification_date: Some("2019-12-18".to_string()),
            }]),
            civic: None,
        };

        let markdown =
            drug_markdown_with_region(&drug, &["regulatory".to_string()], DrugRegion::Who, false)
                .expect("markdown");

        assert!(markdown.contains("## Regulatory (WHO Prequalification)"));
        assert!(markdown.contains("| WHO Ref | Presentation | Dosage Form |"));
        assert!(markdown.contains("BT-ON001"));
        assert!(markdown.contains("Samsung Bioepis NL B.V."));
        assert!(markdown.contains("2019-12-18"));
    }

    #[test]
    fn drug_search_all_region_markdown_includes_who_block() {
        let markdown = drug_search_markdown_with_region(
            "trastuzumab",
            DrugRegion::All,
            &[crate::entities::drug::DrugSearchResult {
                name: "trastuzumab".to_string(),
                drugbank_id: None,
                mechanism: None,
                target: Some("ERBB2".to_string()),
                drug_type: None,
            }],
            Some(1),
            &[crate::entities::drug::EmaDrugSearchResult {
                name: "Herzuma".to_string(),
                active_substance: "trastuzumab".to_string(),
                ema_product_number: "EMEA/H/C/004123".to_string(),
                status: "Authorised".to_string(),
            }],
            Some(1),
            &[crate::entities::drug::WhoPrequalificationSearchResult {
                inn: "Trastuzumab".to_string(),
                therapeutic_area: "Oncology".to_string(),
                dosage_form: "Powder for concentrate for solution for infusion".to_string(),
                applicant: "Samsung Bioepis NL B.V.".to_string(),
                who_reference_number: "BT-ON001".to_string(),
                listing_basis: "Prequalification - Abridged".to_string(),
                prequalification_date: Some("2019-12-18".to_string()),
            }],
            Some(1),
            "",
        )
        .expect("markdown");

        assert!(markdown.contains("## US (MyChem.info / OpenFDA)"));
        assert!(markdown.contains("## EU (EMA)"));
        assert!(markdown.contains("## WHO (WHO Prequalification)"));
        assert!(markdown.contains("BT-ON001"));
        assert!(markdown.contains("EMEA/H/C/004123"));
    }

    #[test]
    fn drug_markdown_with_region_eu_all_suppresses_us_header_facts() {
        // Criterion 9: `get drug <name> all --region eu` must not show US-specific
        // header lines (FDA Approved, Safety FAERS) even though the full card is rendered.
        let drug = Drug {
            name: "pembrolizumab".to_string(),
            drugbank_id: Some("DB09037".to_string()),
            chembl_id: None,
            unii: None,
            drug_type: None,
            mechanism: None,
            mechanisms: Vec::new(),
            approval_date: Some("2014-09-04".to_string()),
            approval_date_raw: Some("20140904".to_string()),
            approval_date_display: Some("September 4, 2014".to_string()),
            approval_summary: Some("FDA approved September 4, 2014".to_string()),
            brand_names: vec!["Keytruda".to_string()],
            route: None,
            targets: Vec::new(),
            variant_targets: Vec::new(),
            target_family: None,
            target_family_name: None,
            indications: Vec::new(),
            interactions: Vec::new(),
            interaction_text: None,
            pharm_classes: Vec::new(),
            top_adverse_events: vec!["Fatigue".to_string(), "Rash".to_string()],
            faers_query: None,
            label: None,
            label_set_id: None,
            shortage: None,
            approvals: None,
            us_safety_warnings: None,
            ema_regulatory: Some(vec![EmaRegulatoryRow {
                medicine_name: "Keytruda".to_string(),
                active_substance: "pembrolizumab".to_string(),
                ema_product_number: "EMEA/H/C/003820".to_string(),
                status: "Authorised".to_string(),
                holder: None,
                recent_activity: Vec::new(),
            }]),
            ema_safety: Some(EmaSafetyInfo {
                dhpcs: Vec::new(),
                referrals: Vec::new(),
                psusas: Vec::new(),
            }),
            ema_shortage: Some(Vec::new()),
            who_prequalification: None,
            civic: None,
        };

        let markdown =
            drug_markdown_with_region(&drug, &["all".to_string()], DrugRegion::Eu, false)
                .expect("markdown");

        // EU EMA section must be present
        assert!(markdown.contains("## Regulatory (EU - EMA)"));
        assert!(markdown.contains("EMEA/H/C/003820"));

        // US-specific header facts must be absent
        assert!(
            !markdown.contains("FDA Approved"),
            "US approval date must not appear in EU-only output"
        );
        assert!(
            !markdown.contains("Safety (OpenFDA FAERS)"),
            "US FAERS safety line must not appear in EU-only output"
        );
    }

    #[test]
    fn drug_markdown_with_region_eu_safety_shows_truthful_empty_subsections() {
        let drug = Drug {
            name: "semaglutide".to_string(),
            drugbank_id: Some("DB13928".to_string()),
            chembl_id: None,
            unii: None,
            drug_type: None,
            mechanism: None,
            mechanisms: Vec::new(),
            approval_date: None,
            approval_date_raw: None,
            approval_date_display: None,
            approval_summary: None,
            brand_names: vec!["Ozempic".to_string()],
            route: None,
            targets: Vec::new(),
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
            ema_safety: Some(EmaSafetyInfo {
                dhpcs: vec![crate::entities::drug::EmaDhpcEntry {
                    medicine_name: "Ozempic".to_string(),
                    dhpc_type: Some("DHPC".to_string()),
                    regulatory_outcome: Some("Medicine shortage".to_string()),
                    first_published_date: Some("10/01/2026".to_string()),
                    last_updated_date: Some("13/01/2026".to_string()),
                }],
                referrals: Vec::new(),
                psusas: Vec::new(),
            }),
            ema_shortage: None,
            who_prequalification: None,
            civic: None,
        };

        let markdown =
            drug_markdown_with_region(&drug, &["safety".to_string()], DrugRegion::Eu, false)
                .expect("markdown");
        assert!(markdown.contains("## Safety (EU - EMA)"));
        assert!(markdown.contains("### DHPCs"));
        assert!(markdown.contains("Medicine shortage"));
        assert!(markdown.contains("### Referrals"));
        assert!(markdown.contains("### PSUSAs"));
        assert!(markdown.contains("No data found (EMA)"));
    }

    #[test]
    fn drug_search_empty_state_frames_zero_indication_miss_as_regulatory_signal() {
        let markdown = drug_search_markdown_with_region(
            "indication=Marfan syndrome",
            DrugRegion::Us,
            &[],
            Some(0),
            &[],
            None,
            &[],
            None,
            "",
        )
        .expect("markdown");

        assert!(markdown.contains("U.S. regulatory data"));
        assert!(markdown.contains("This absence is informative"));
        assert!(markdown.contains(
            "biomcp search article -k \"Marfan syndrome treatment\" --type review --limit 5"
        ));
        assert!(markdown.contains("Try: biomcp discover \"Marfan syndrome\""));
        assert!(!markdown.contains("No drugs found\n"));
    }

    #[test]
    fn disease_search_empty_state_includes_discover_hint() {
        let markdown = disease_search_markdown_with_footer(
            "definitelynotarealdisease",
            "definitelynotarealdisease",
            &[],
            false,
            "",
        )
        .expect("markdown");

        assert!(markdown.contains("Try: biomcp discover definitelynotarealdisease"));
    }

    #[test]
    fn disease_search_empty_state_uses_raw_query_in_discover_hint() {
        let markdown = disease_search_markdown_with_footer(
            "Arnold Chiari syndrome",
            "Arnold Chiari syndrome, offset=5",
            &[],
            false,
            "",
        )
        .expect("markdown");

        assert!(markdown.contains("Try: biomcp discover \"Arnold Chiari syndrome\""));
        assert!(!markdown.contains("offset=5\""));
    }

    #[test]
    fn disease_search_fallback_renders_provenance_columns() {
        let markdown = disease_search_markdown_with_footer(
            "Arnold Chiari syndrome",
            "Arnold Chiari syndrome",
            &[DiseaseSearchResult {
                id: "MONDO:0000115".into(),
                name: "Arnold-Chiari malformation".into(),
                synonyms_preview: Some("Chiari malformation".into()),
                resolved_via: Some("MESH crosswalk".into()),
                source_id: Some("MESH:D001139".into()),
            }],
            true,
            "",
        )
        .expect("markdown");

        assert!(markdown.contains("Resolved via discover + crosswalk"));
        assert!(markdown.contains("| ID | Name | Resolved via | Source ID |"));
        assert!(markdown.contains("MESH crosswalk"));
        assert!(markdown.contains("MESH:D001139"));
    }

    #[test]
    fn drug_search_standard_empty_state_includes_discover_hint() {
        let markdown =
            drug_search_markdown_with_footer("MK-3475", &[], Some(0), "").expect("markdown");

        assert!(markdown.contains("Try: biomcp discover MK-3475"));
    }

    #[test]
    fn drug_search_eu_empty_state_includes_discover_hint() {
        let markdown = drug_search_markdown_with_region(
            "MK-3475",
            DrugRegion::Eu,
            &[],
            None,
            &[],
            Some(0),
            &[],
            None,
            "",
        )
        .expect("markdown");

        assert!(markdown.contains("Try: biomcp discover MK-3475"));
    }

    #[test]
    fn drug_search_all_region_empty_state_calls_out_regulatory_absence() {
        let markdown = drug_search_markdown_with_region(
            "indication=Marfan syndrome",
            DrugRegion::All,
            &[],
            Some(0),
            &[],
            Some(0),
            &[],
            Some(0),
            "",
        )
        .expect("markdown");

        assert!(
            markdown
                .contains("specific to the structured regulatory portion of the combined search")
        );
        assert!(markdown.contains("## US (MyChem.info / OpenFDA)"));
        assert!(markdown.contains("## EU (EMA)"));
    }

    #[test]
    fn drug_search_all_region_empty_state_includes_discover_only_when_both_regions_are_empty() {
        let empty_markdown = drug_search_markdown_with_region(
            "MK-3475",
            DrugRegion::All,
            &[],
            Some(0),
            &[],
            Some(0),
            &[],
            Some(0),
            "",
        )
        .expect("markdown");
        assert!(empty_markdown.contains("Try: biomcp discover MK-3475"));

        let us_only_markdown = drug_search_markdown_with_region(
            "MK-3475",
            DrugRegion::All,
            &[crate::entities::drug::DrugSearchResult {
                name: "pembrolizumab".to_string(),
                drugbank_id: None,
                mechanism: None,
                target: None,
                drug_type: None,
            }],
            Some(1),
            &[],
            Some(0),
            &[],
            Some(0),
            "",
        )
        .expect("markdown");
        assert!(!us_only_markdown.contains("Try: biomcp discover MK-3475"));

        let eu_only_markdown = drug_search_markdown_with_region(
            "MK-3475",
            DrugRegion::All,
            &[],
            Some(0),
            &[crate::entities::drug::EmaDrugSearchResult {
                name: "Keytruda".to_string(),
                active_substance: "pembrolizumab".to_string(),
                ema_product_number: "EMEA/H/C/003820".to_string(),
                status: "Authorized".to_string(),
            }],
            Some(1),
            &[],
            Some(0),
            "",
        )
        .expect("markdown");
        assert!(!eu_only_markdown.contains("Try: biomcp discover MK-3475"));
    }

    #[test]
    fn drug_search_eu_indication_empty_state_includes_discover_hint() {
        let markdown = drug_search_markdown_with_region(
            "indication=Marfan syndrome",
            DrugRegion::Eu,
            &[],
            None,
            &[],
            Some(0),
            &[],
            None,
            "",
        )
        .expect("markdown");

        assert!(markdown.contains("Try: biomcp discover \"Marfan syndrome\""));
    }

    #[test]
    fn related_disease_malformed_study_lookup_falls_back_to_download_list() {
        let _guard = crate::test_support::env_lock().blocking_lock();
        let disease = Disease {
            id: "MONDO:0005105".to_string(),
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
            civic: Some(crate::sources::civic::CivicContext::default()),
            disgenet: None,
            funding: None,
            funding_note: None,
            xrefs: std::collections::HashMap::new(),
        };

        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "biomcp-render-study-malformed-{}-{unique}",
            std::process::id()
        ));
        let study_dir = root.join("broken-study");
        std::fs::create_dir_all(&study_dir).expect("create malformed study dir");
        std::fs::write(
            study_dir.join("meta_study.txt"),
            "name: Missing identifier\n",
        )
        .expect("write malformed meta");
        std::fs::write(
            study_dir.join("data_mutations.txt"),
            "Hugo_Symbol\tTumor_Sample_Barcode\tVariant_Classification\tHGVSp_Short\nTP53\tS1\tMissense_Mutation\tp.R175H\n",
        )
        .expect("write mutations");

        let original = std::env::var_os("BIOMCP_STUDY_DIR");
        unsafe { std::env::set_var("BIOMCP_STUDY_DIR", &root) };
        let related = related_disease(&disease);
        match original {
            Some(value) => unsafe { std::env::set_var("BIOMCP_STUDY_DIR", value) },
            None => unsafe { std::env::remove_var("BIOMCP_STUDY_DIR") },
        }
        let _ = std::fs::remove_dir_all(&root);

        assert!(related.contains(&"biomcp study download --list".to_string()));
    }

    #[test]
    fn pgx_markdown_includes_evidence_links() {
        let pgx = Pgx {
            query: "CYP2D6".to_string(),
            gene: Some("CYP2D6".to_string()),
            drug: Some("warfarin".to_string()),
            interactions: Vec::new(),
            recommendations: Vec::new(),
            frequencies: Vec::new(),
            guidelines: Vec::new(),
            annotations: Vec::new(),
            annotations_note: None,
        };

        let markdown = pgx_markdown(&pgx, &[]).expect("rendered markdown");
        assert!(markdown.contains("[CPIC](https://cpicpgx.org/genes/cyp2d6/)"));
        assert!(markdown.contains("[PharmGKB](https://www.pharmgkb.org/gene/CYP2D6)"));
        assert!(markdown.contains("[PharmGKB](https://www.pharmgkb.org/chemical/warfarin)"));
    }

    #[test]
    fn related_device_event_uses_supported_search_subcommands() {
        let event = DeviceEvent {
            report_id: "MDR-123".to_string(),
            report_number: None,
            device: "Infusion Pump".to_string(),
            manufacturer: None,
            event_type: None,
            date: None,
            description: None,
        };

        let related = related_device_event(&event);
        assert!(related.contains(
            &"biomcp search adverse-event --type device --device \"Infusion Pump\"".to_string()
        ));
        assert!(related.contains(
            &"biomcp search adverse-event --type recall --classification \"Class I\"".to_string()
        ));
    }

    #[test]
    fn related_protein_includes_complexes_follow_up() {
        let protein = Protein {
            accession: "P15056".to_string(),
            entry_id: Some("BRAF_HUMAN".to_string()),
            name: "Serine/threonine-protein kinase B-raf".to_string(),
            gene_symbol: Some("BRAF".to_string()),
            organism: Some("Homo sapiens".to_string()),
            length: Some(766),
            function: None,
            structures: vec!["6V34".to_string()],
            structure_count: Some(1),
            domains: Vec::new(),
            interactions: Vec::new(),
            complexes: Vec::new(),
        };

        let related = related_protein(&protein, &[]);
        assert!(related.contains(&"biomcp get protein P15056 structures".to_string()));
        assert!(related.contains(&"biomcp get protein P15056 complexes".to_string()));
        assert!(related.contains(&"biomcp get gene BRAF".to_string()));
    }

    #[test]
    fn related_protein_excludes_requested_sections() {
        let protein = Protein {
            accession: "P15056".to_string(),
            entry_id: Some("BRAF_HUMAN".to_string()),
            name: "Serine/threonine-protein kinase B-raf".to_string(),
            gene_symbol: Some("BRAF".to_string()),
            organism: Some("Homo sapiens".to_string()),
            length: Some(766),
            function: None,
            structures: vec!["6V34".to_string()],
            structure_count: Some(1),
            domains: Vec::new(),
            interactions: Vec::new(),
            complexes: Vec::new(),
        };

        let related = related_protein(
            &protein,
            &["complexes".to_string(), "structures".to_string()],
        );
        assert!(!related.contains(&"biomcp get protein P15056 structures".to_string()));
        assert!(!related.contains(&"biomcp get protein P15056 complexes".to_string()));
        assert!(related.contains(&"biomcp get gene BRAF".to_string()));
    }

    #[test]
    fn protein_markdown_renders_complexes_summary_and_detail_bullets() {
        let protein = Protein {
            accession: "P15056".to_string(),
            entry_id: Some("BRAF_HUMAN".to_string()),
            name: "Serine/threonine-protein kinase B-raf".to_string(),
            gene_symbol: Some("BRAF".to_string()),
            organism: Some("Homo sapiens".to_string()),
            length: Some(766),
            function: None,
            structures: Vec::new(),
            structure_count: None,
            domains: Vec::new(),
            interactions: Vec::new(),
            complexes: vec![
                ProteinComplex {
                    accession: "CPX-1234".to_string(),
                    name: "BRAF signaling complex with an intentionally long display label that should truncate in the summary table".to_string(),
                    description: Some("Signals through MAPK.".to_string()),
                    curation: ProteinComplexCuration::Curated,
                    components: vec![
                        ProteinComplexComponent {
                            accession: "P15056".to_string(),
                            name: "BRAF".to_string(),
                            stoichiometry: Some("1".to_string()),
                        },
                        ProteinComplexComponent {
                            accession: "Q02750".to_string(),
                            name: "MAP2K1".to_string(),
                            stoichiometry: None,
                        },
                        ProteinComplexComponent {
                            accession: "P10398".to_string(),
                            name: "RAF1".to_string(),
                            stoichiometry: None,
                        },
                        ProteinComplexComponent {
                            accession: "P07900".to_string(),
                            name: "HSP90AA1".to_string(),
                            stoichiometry: None,
                        },
                        ProteinComplexComponent {
                            accession: "Q16543".to_string(),
                            name: "CDC37".to_string(),
                            stoichiometry: None,
                        },
                        ProteinComplexComponent {
                            accession: "Q9Y243".to_string(),
                            name: "AKT3".to_string(),
                            stoichiometry: None,
                        },
                        ProteinComplexComponent {
                            accession: "P31749".to_string(),
                            name: "AKT1".to_string(),
                            stoichiometry: None,
                        },
                    ],
                },
                ProteinComplex {
                    accession: "CPX-5678".to_string(),
                    name: "Complex with no listed members".to_string(),
                    description: None,
                    curation: ProteinComplexCuration::Predicted,
                    components: Vec::new(),
                },
            ],
        };

        let markdown = protein_markdown(&protein, &["complexes".to_string()]).expect("markdown");
        assert!(markdown.contains("## Complexes"));
        assert!(markdown.contains("| ID | Name | Members | Curation |"));
        assert!(!markdown.contains("| ID | Name | Components | Curation |"));
        assert!(
            markdown.contains(
                "| CPX-1234 | BRAF signaling complex with an intentionally long di… | 7 | curated |"
            ),
            "expected truncated complex summary row, got:\n{markdown}"
        );
        assert!(markdown.contains(
            "- `CPX-1234` members (7): BRAF (1), MAP2K1, RAF1, HSP90AA1, CDC37, +2 more\n  Description: Signals through MAPK."
        ));
        assert!(markdown.contains("- `CPX-5678` members (0): none listed"));
        assert!(!markdown.contains("- `CPX-1234`: Signals through MAPK."));
        assert!(!markdown.contains("AKT3"));
        assert!(!markdown.contains("AKT1"));
        assert!(!markdown.contains("See also: biomcp get protein P15056 complexes"));
    }

    #[test]
    fn article_search_markdown_preserves_rank_order_and_shows_rationale() {
        let rows = vec![
            ArticleSearchResult {
                pmid: "1".into(),
                title: "Entity-ranked".into(),
                pmcid: Some("PMC1".into()),
                doi: Some("10.1000/one".into()),
                journal: Some("Journal A".into()),
                date: Some("2025-01-01".into()),
                citation_count: Some(10),
                influential_citation_count: Some(4),
                source: ArticleSource::PubTator,
                score: Some(99.1),
                is_retracted: Some(false),
                abstract_snippet: Some("Abstract one".into()),
                ranking: Some(crate::entities::article::ArticleRankingMetadata {
                    directness_tier: 3,
                    anchor_count: 2,
                    title_anchor_hits: 2,
                    abstract_anchor_hits: 0,
                    combined_anchor_hits: 2,
                    all_anchors_in_title: true,
                    all_anchors_in_text: true,
                    study_or_review_cue: false,
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
                matched_sources: vec![ArticleSource::PubTator, ArticleSource::SemanticScholar],
                normalized_title: "entity-ranked".into(),
                normalized_abstract: "abstract one".into(),
                publication_type: None,
                source_local_position: 0,
            },
            ArticleSearchResult {
                pmid: "2".into(),
                title: "Field-ranked".into(),
                pmcid: None,
                doi: None,
                journal: Some("Journal B".into()),
                date: Some("2025-01-02".into()),
                citation_count: Some(12),
                influential_citation_count: Some(1),
                source: ArticleSource::EuropePmc,
                score: None,
                is_retracted: Some(false),
                abstract_snippet: Some("Abstract two".into()),
                ranking: Some(crate::entities::article::ArticleRankingMetadata {
                    directness_tier: 2,
                    anchor_count: 2,
                    title_anchor_hits: 1,
                    abstract_anchor_hits: 1,
                    combined_anchor_hits: 2,
                    all_anchors_in_title: false,
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
                matched_sources: vec![ArticleSource::EuropePmc],
                normalized_title: "field-ranked".into(),
                normalized_abstract: "abstract two".into(),
                publication_type: Some("Review".into()),
                source_local_position: 1,
            },
        ];

        let markdown = article_search_markdown_with_footer_and_context(
            "gene=BRAF",
            &rows,
            "",
            &article_filters_for_test(crate::entities::article::ArticleSort::Relevance),
            true,
            Some(
                "Note: --type restricts article search to Europe PMC and PubMed. PubTator3, LitSense2, and Semantic Scholar do not support publication-type filtering.",
            ),
            None,
        )
        .expect("markdown should render");
        assert!(markdown.contains(
            "> Note: --type restricts article search to Europe PMC and PubMed. PubTator3, LitSense2, and Semantic Scholar do not support publication-type filtering."
        ));
        assert!(markdown.contains("Semantic Scholar: enabled"));
        assert!(markdown.contains("Ranking: calibrated PubMed rescue + lexical directness"));
        assert!(markdown.contains("| PMID | Title | Source(s) | Date | Why | Cit. |"));
        assert!(markdown.contains("PubTator3, Semantic Scholar"));
        assert!(markdown.contains("title 2/2"));
        assert!(markdown.contains("title+abstract 2/2"));
        assert!(
            markdown.contains(
                "--date-from/--date-to <YYYY|YYYY-MM|YYYY-MM-DD> (alias: --since/--until)"
            )
        );
        assert!(!markdown.contains("## PubTator3"));
        assert!(!markdown.contains("## Europe PMC"));
        assert!(markdown.find("|1|").unwrap() < markdown.find("|2|").unwrap());
    }

    #[test]
    fn article_ranking_why_tier1_mixed_shows_title_plus_abstract() {
        let row = ArticleSearchResult {
            pmid: "1".into(),
            title: "Partial coverage".into(),
            pmcid: None,
            doi: None,
            journal: None,
            date: None,
            citation_count: None,
            influential_citation_count: None,
            source: ArticleSource::EuropePmc,
            matched_sources: vec![ArticleSource::EuropePmc],
            score: None,
            is_retracted: None,
            abstract_snippet: None,
            ranking: Some(crate::entities::article::ArticleRankingMetadata {
                directness_tier: 1,
                anchor_count: 3,
                title_anchor_hits: 1,
                abstract_anchor_hits: 1,
                combined_anchor_hits: 2,
                all_anchors_in_title: false,
                all_anchors_in_text: false,
                study_or_review_cue: false,
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
            normalized_title: "partial coverage".into(),
            normalized_abstract: String::new(),
            publication_type: None,
            source_local_position: 0,
        };
        let why = article_ranking_why(&row, &article_filters_for_test(ArticleSort::Relevance));
        assert_eq!(why, "title+abstract 2/3");
    }

    #[test]
    fn article_ranking_why_rescue_composes_with_lexical_reason() {
        let row = ArticleSearchResult {
            pmid: "1".into(),
            title: "Rescued partial coverage".into(),
            pmcid: None,
            doi: None,
            journal: None,
            date: None,
            citation_count: None,
            influential_citation_count: None,
            source: ArticleSource::PubMed,
            matched_sources: vec![ArticleSource::PubMed],
            score: None,
            is_retracted: None,
            abstract_snippet: None,
            ranking: Some(crate::entities::article::ArticleRankingMetadata {
                directness_tier: 1,
                anchor_count: 3,
                title_anchor_hits: 1,
                abstract_anchor_hits: 1,
                combined_anchor_hits: 2,
                all_anchors_in_title: false,
                all_anchors_in_text: false,
                study_or_review_cue: false,
                pubmed_rescue: true,
                pubmed_rescue_kind: Some(crate::entities::article::ArticlePubMedRescueKind::Unique),
                pubmed_source_position: Some(0),
                mode: Some(crate::entities::article::ArticleRankingMode::Lexical),
                semantic_score: None,
                lexical_score: None,
                citation_score: None,
                position_score: None,
                composite_score: None,
                avg_source_rank: None,
            }),
            normalized_title: "rescued partial coverage".into(),
            normalized_abstract: String::new(),
            publication_type: None,
            source_local_position: 0,
        };

        let why = article_ranking_why(&row, &article_filters_for_test(ArticleSort::Relevance));
        assert_eq!(why, "pubmed-rescue + title+abstract 2/3");
    }

    #[test]
    fn article_ranking_why_semantic_includes_score_and_lexical_context() {
        let row = ArticleSearchResult {
            pmid: "1".into(),
            title: "Semantic lead".into(),
            pmcid: None,
            doi: None,
            journal: None,
            date: None,
            citation_count: None,
            influential_citation_count: None,
            source: ArticleSource::EuropePmc,
            matched_sources: vec![ArticleSource::EuropePmc],
            score: Some(0.81234),
            is_retracted: None,
            abstract_snippet: None,
            ranking: Some(crate::entities::article::ArticleRankingMetadata {
                directness_tier: 2,
                anchor_count: 3,
                title_anchor_hits: 2,
                abstract_anchor_hits: 0,
                combined_anchor_hits: 2,
                all_anchors_in_title: true,
                all_anchors_in_text: false,
                study_or_review_cue: false,
                pubmed_rescue: false,
                pubmed_rescue_kind: None,
                pubmed_source_position: None,
                mode: Some(crate::entities::article::ArticleRankingMode::Semantic),
                semantic_score: Some(0.81234),
                lexical_score: None,
                citation_score: None,
                position_score: None,
                composite_score: None,
                avg_source_rank: None,
            }),
            normalized_title: "semantic lead".into(),
            normalized_abstract: String::new(),
            publication_type: None,
            source_local_position: 0,
        };

        let why = article_ranking_why(&row, &article_filters_for_test(ArticleSort::Relevance));
        assert_eq!(why, "semantic 0.812 + title 2/3");
    }

    #[test]
    fn article_ranking_why_hybrid_includes_score_and_lexical_context() {
        let row = ArticleSearchResult {
            pmid: "1".into(),
            title: "Hybrid lead".into(),
            pmcid: None,
            doi: None,
            journal: None,
            date: None,
            citation_count: None,
            influential_citation_count: None,
            source: ArticleSource::EuropePmc,
            matched_sources: vec![ArticleSource::EuropePmc],
            score: Some(0.9),
            is_retracted: None,
            abstract_snippet: None,
            ranking: Some(crate::entities::article::ArticleRankingMetadata {
                directness_tier: 1,
                anchor_count: 3,
                title_anchor_hits: 1,
                abstract_anchor_hits: 1,
                combined_anchor_hits: 2,
                all_anchors_in_title: false,
                all_anchors_in_text: false,
                study_or_review_cue: false,
                pubmed_rescue: false,
                pubmed_rescue_kind: None,
                pubmed_source_position: None,
                mode: Some(crate::entities::article::ArticleRankingMode::Hybrid),
                semantic_score: Some(0.9),
                lexical_score: Some(1.0 / 3.0),
                citation_score: Some(0.1),
                position_score: Some(0.4),
                composite_score: Some(0.61234),
                avg_source_rank: Some(1.0),
            }),
            normalized_title: "hybrid lead".into(),
            normalized_abstract: String::new(),
            publication_type: None,
            source_local_position: 0,
        };

        let why = article_ranking_why(&row, &article_filters_for_test(ArticleSort::Relevance));
        assert_eq!(why, "hybrid 0.612 + title+abstract 2/3");
    }

    #[test]
    fn search_all_markdown_renders_section_note() {
        let results = crate::cli::search_all::SearchAllResults {
            query: "gene=EGFR disease=non-small cell lung cancer".to_string(),
            sections: vec![crate::cli::search_all::SearchAllSection {
                entity: "variant".to_string(),
                label: "Variants".to_string(),
                count: 1,
                total: Some(1),
                error: None,
                note: Some(
                    "No disease-filtered variants found; showing top gene variants.".to_string(),
                ),
                results: vec![serde_json::json!({
                    "id": "rs121434568",
                    "gene": "EGFR",
                    "hgvs_p": "L858R",
                    "significance": "Pathogenic",
                })],
                links: Vec::new(),
            }],
            searches_dispatched: 1,
            searches_with_results: 1,
            wall_time_ms: 42,
            debug_plan: None,
        };

        let markdown = search_all_markdown(&results, false).expect("markdown should render");
        assert!(
            markdown.contains("> No disease-filtered variants found; showing top gene variants.")
        );
    }

    #[test]
    fn article_search_markdown_prepends_debug_plan_block() {
        let debug_plan = DebugPlan {
            surface: "search_article",
            query: "gene=BRAF".to_string(),
            anchor: None,
            legs: vec![crate::cli::debug_plan::DebugPlanLeg {
                leg: "article".to_string(),
                entity: "article".to_string(),
                filters: vec!["gene=BRAF".to_string()],
                routing: vec!["planner=federated".to_string()],
                sources: vec!["PubTator3".to_string(), "Europe PMC".to_string()],
                matched_sources: vec!["PubTator3".to_string()],
                count: 1,
                total: Some(1),
                note: None,
                error: None,
            }],
        };
        let rows = vec![ArticleSearchResult {
            pmid: "1".into(),
            title: "Entity-ranked".into(),
            pmcid: None,
            doi: None,
            journal: Some("Journal A".into()),
            date: Some("2025-01-01".into()),
            citation_count: Some(10),
            influential_citation_count: Some(4),
            source: ArticleSource::PubTator,
            score: Some(99.1),
            is_retracted: Some(false),
            abstract_snippet: Some("Abstract one".into()),
            ranking: None,
            matched_sources: vec![ArticleSource::PubTator],
            normalized_title: "entity-ranked".into(),
            normalized_abstract: "abstract one".into(),
            publication_type: None,
            source_local_position: 0,
        }];

        let markdown = article_search_markdown_with_footer_and_context(
            "gene=BRAF",
            &rows,
            "",
            &article_filters_for_test(crate::entities::article::ArticleSort::Relevance),
            true,
            None,
            Some(&debug_plan),
        )
        .expect("markdown should render");

        assert!(markdown.starts_with("## Debug plan"));
        assert!(markdown.contains("\"surface\": \"search_article\""));
        assert!(markdown.contains("# Articles: gene=BRAF"));
    }

    #[test]
    fn study_list_markdown_renders_study_table() {
        let markdown = study_list_markdown(&[StudyInfo {
            study_id: "msk_impact_2017".to_string(),
            name: "MSK-IMPACT".to_string(),
            cancer_type: Some("mixed".to_string()),
            citation: Some("Zehir et al.".to_string()),
            sample_count: Some(10945),
            available_data: vec!["mutations".to_string(), "cna".to_string()],
        }]);

        assert!(markdown.contains("# Study Datasets"));
        assert!(markdown.contains("| Study ID | Name | Cancer Type | Samples | Available Data |"));
        assert!(markdown.contains("msk_impact_2017"));
        assert!(markdown.contains("mutations, cna"));
    }

    #[test]
    fn study_query_markdown_renders_mutation_shape() {
        let markdown = study_query_markdown(&StudyQueryResult::MutationFrequency(
            StudyMutationFrequencyResult {
                study_id: "msk_impact_2017".to_string(),
                gene: "TP53".to_string(),
                mutation_count: 10,
                unique_samples: 9,
                total_samples: 100,
                frequency: 0.09,
                top_variant_classes: vec![("Missense_Mutation".to_string(), 8)],
                top_protein_changes: vec![("p.R175H".to_string(), 3)],
            },
        ));

        assert!(markdown.contains("# Study Mutation Frequency: TP53 (msk_impact_2017)"));
        assert!(markdown.contains("| Mutation records | 10 |"));
        assert!(markdown.contains("## Top Variant Classes"));
        assert!(markdown.contains("## Top Protein Changes"));
    }

    #[test]
    fn study_query_markdown_renders_cna_and_expression_shapes() {
        let cna = study_query_markdown(&StudyQueryResult::CnaDistribution(
            StudyCnaDistributionResult {
                study_id: "brca_tcga_pan_can_atlas_2018".to_string(),
                gene: "ERBB2".to_string(),
                total_samples: 20,
                deep_deletion: 1,
                shallow_deletion: 2,
                diploid: 10,
                gain: 4,
                amplification: 3,
            },
        ));
        assert!(cna.contains("# Study CNA Distribution: ERBB2 (brca_tcga_pan_can_atlas_2018)"));
        assert!(cna.contains("| Amplification (2) | 3 |"));

        let expression = study_query_markdown(&StudyQueryResult::ExpressionDistribution(
            StudyExpressionDistributionResult {
                study_id: "paad_qcmg_uq_2016".to_string(),
                gene: "KRAS".to_string(),
                file: "data_mrna_seq_v2_rsem_zscores_ref_all_samples.txt".to_string(),
                sample_count: 50,
                mean: 0.2,
                median: 0.1,
                min: -2.0,
                max: 2.5,
                q1: -0.4,
                q3: 0.5,
            },
        ));
        assert!(expression.contains("# Study Expression Distribution: KRAS (paad_qcmg_uq_2016)"));
        assert!(expression.contains("| Sample count | 50 |"));
    }

    #[test]
    fn study_filter_markdown_renders_tables_and_samples() {
        let markdown = study_filter_markdown(&StudyFilterResult {
            study_id: "brca_tcga_pan_can_atlas_2018".to_string(),
            criteria: vec![
                crate::entities::study::FilterCriterionSummary {
                    description: "mutated TP53".to_string(),
                    matched_count: 3,
                },
                crate::entities::study::FilterCriterionSummary {
                    description: "amplified ERBB2".to_string(),
                    matched_count: 2,
                },
            ],
            total_study_samples: Some(4),
            matched_count: 2,
            matched_sample_ids: vec!["S2".to_string(), "S3".to_string()],
        });

        assert!(markdown.contains("# Study Filter: brca_tcga_pan_can_atlas_2018"));
        assert!(markdown.contains("## Criteria"));
        assert!(markdown.contains("| Filter | Matching Samples |"));
        assert!(markdown.contains("| mutated TP53 | 3 |"));
        assert!(markdown.contains("## Result"));
        assert!(markdown.contains("| Study Total Samples | 4 |"));
        assert!(markdown.contains("| Intersection | 2 |"));
        assert!(markdown.contains("## Matched Samples"));
        assert!(markdown.contains("S2"));
        assert!(markdown.contains("S3"));
    }

    #[test]
    fn study_filter_markdown_renders_empty_results_and_unknown_totals() {
        let markdown = study_filter_markdown(&StudyFilterResult {
            study_id: "demo_study".to_string(),
            criteria: vec![crate::entities::study::FilterCriterionSummary {
                description: "expression > 1.5 for MYC".to_string(),
                matched_count: 0,
            }],
            total_study_samples: None,
            matched_count: 0,
            matched_sample_ids: Vec::new(),
        });

        assert!(markdown.contains("| Study Total Samples | - |"));
        assert!(markdown.contains("| Intersection | 0 |"));
        assert!(markdown.contains("## Matched Samples"));
        assert!(markdown.contains("\nNone\n"));
    }

    #[test]
    fn study_filter_markdown_truncates_long_sample_lists() {
        let markdown = study_filter_markdown(&StudyFilterResult {
            study_id: "long_study".to_string(),
            criteria: vec![crate::entities::study::FilterCriterionSummary {
                description: "mutated TP53".to_string(),
                matched_count: 55,
            }],
            total_study_samples: Some(100),
            matched_count: 55,
            matched_sample_ids: (1..=55).map(|idx| format!("S{idx}")).collect(),
        });

        assert!(markdown.contains("S1"));
        assert!(markdown.contains("S50"));
        assert!(!markdown.contains("S51\n"));
        assert!(markdown.contains("... and 5 more (use --json for full list)"));
    }

    #[test]
    fn study_co_occurrence_markdown_renders_pair_table() {
        let markdown = study_co_occurrence_markdown(&StudyCoOccurrenceResult {
            study_id: "msk_impact_2017".to_string(),
            genes: vec!["TP53".to_string(), "KRAS".to_string()],
            total_samples: 100,
            sample_universe_basis: StudySampleUniverseBasis::ClinicalSampleFile,
            pairs: vec![StudyCoOccurrencePair {
                gene_a: "TP53".to_string(),
                gene_b: "KRAS".to_string(),
                both_mutated: 10,
                a_only: 20,
                b_only: 15,
                neither: 55,
                log_odds_ratio: Some(0.1234),
                p_value: Some(6.0e-22),
            }],
        });

        assert!(markdown.contains("# Study Co-occurrence: msk_impact_2017"));
        assert!(markdown.contains("Sample universe: clinical sample file"));
        assert!(markdown.contains(
            "| Gene A | Gene B | Both | A only | B only | Neither | Log Odds Ratio | p-value |"
        ));
        assert!(markdown.contains("| TP53 | KRAS | 10 | 20 | 15 | 55 | 0.123400 | 6.000e-22 |"));
    }

    #[test]
    fn study_co_occurrence_markdown_marks_mutation_observed_fallback() {
        let markdown = study_co_occurrence_markdown(&StudyCoOccurrenceResult {
            study_id: "fallback_study".to_string(),
            genes: vec!["TP53".to_string(), "KRAS".to_string()],
            total_samples: 3,
            sample_universe_basis: StudySampleUniverseBasis::MutationObserved,
            pairs: vec![],
        });

        assert!(markdown.contains(
            "Sample universe: mutation-observed samples only (clinical sample file unavailable)"
        ));
    }

    #[test]
    fn study_cohort_markdown_renders_group_counts() {
        let markdown = study_cohort_markdown(&StudyCohortResult {
            study_id: "brca_tcga_pan_can_atlas_2018".to_string(),
            gene: "TP53".to_string(),
            stratification: "mutation".to_string(),
            mutant_samples: 348,
            wildtype_samples: 736,
            mutant_patients: 348,
            wildtype_patients: 736,
            total_samples: 1084,
            total_patients: 1084,
        });

        assert!(markdown.contains("# Study Cohort: TP53 (brca_tcga_pan_can_atlas_2018)"));
        assert!(markdown.contains("Stratification: mutation status"));
        assert!(markdown.contains("| Group | Samples | Patients |"));
        assert!(markdown.contains("| TP53-mutant | 348 | 348 |"));
        assert!(markdown.contains("| Total | 1084 | 1084 |"));
    }

    #[test]
    fn study_survival_markdown_renders_group_table() {
        let markdown = study_survival_markdown(&StudySurvivalResult {
            study_id: "brca_tcga_pan_can_atlas_2018".to_string(),
            gene: "TP53".to_string(),
            endpoint: StudySurvivalEndpoint::Os,
            groups: vec![
                StudySurvivalGroupResult {
                    group_name: "TP53-mutant".to_string(),
                    n_patients: 340,
                    n_events: 48,
                    n_censored: 292,
                    km_median_months: Some(85.2),
                    survival_1yr: Some(0.91),
                    survival_3yr: Some(0.72),
                    survival_5yr: None,
                    event_rate: 0.141176,
                    km_curve_points: Vec::new(),
                },
                StudySurvivalGroupResult {
                    group_name: "TP53-wildtype".to_string(),
                    n_patients: 720,
                    n_events: 64,
                    n_censored: 656,
                    km_median_months: None,
                    survival_1yr: Some(0.97),
                    survival_3yr: Some(0.88),
                    survival_5yr: Some(0.74),
                    event_rate: 0.088889,
                    km_curve_points: Vec::new(),
                },
            ],
            log_rank_p: Some(0.0042),
        });

        assert!(markdown.contains("# Study Survival: TP53 (brca_tcga_pan_can_atlas_2018)"));
        assert!(markdown.contains("Endpoint: Overall Survival (OS)"));
        assert!(markdown.contains(
            "| Group | N | Events | Censored | Event Rate | KM Median | 1yr | 3yr | 5yr |"
        ));
        assert!(
            markdown
                .contains("| TP53-mutant | 340 | 48 | 292 | 0.141176 | 85.2 | 0.910 | 0.720 | - |")
        );
        assert!(markdown.contains("Log-rank p-value: 0.004"));
    }

    #[test]
    fn study_download_markdown_renders_result_table() {
        let markdown = study_download_markdown(&StudyDownloadResult {
            study_id: "msk_impact_2017".to_string(),
            path: "/tmp/studies/msk_impact_2017".to_string(),
            downloaded: true,
        });

        assert!(markdown.contains("# Study Download: msk_impact_2017"));
        assert!(markdown.contains("| Study ID | msk_impact_2017 |"));
        assert!(markdown.contains("| Downloaded | yes |"));
    }

    #[test]
    fn study_download_catalog_markdown_renders_remote_ids() {
        let markdown = study_download_catalog_markdown(&StudyDownloadCatalog {
            study_ids: vec![
                "msk_impact_2017".to_string(),
                "brca_tcga_pan_can_atlas_2018".to_string(),
            ],
        });

        assert!(markdown.contains("# Downloadable cBioPortal Studies"));
        assert!(markdown.contains("| Study ID |"));
        assert!(markdown.contains("| msk_impact_2017 |"));
        assert!(markdown.contains("| brca_tcga_pan_can_atlas_2018 |"));
    }

    #[test]
    fn study_compare_expression_markdown_renders_distribution_table() {
        let markdown = study_compare_expression_markdown(&StudyExpressionComparisonResult {
            study_id: "brca_tcga_pan_can_atlas_2018".to_string(),
            stratify_gene: "TP53".to_string(),
            target_gene: "ERBB2".to_string(),
            groups: vec![
                StudyExpressionGroupStats {
                    group_name: "TP53-mutant".to_string(),
                    sample_count: 345,
                    mean: 0.234,
                    median: 0.112,
                    min: -2.1,
                    max: 4.5,
                    q1: -0.45,
                    q3: 0.78,
                },
                StudyExpressionGroupStats {
                    group_name: "TP53-wildtype".to_string(),
                    sample_count: 730,
                    mean: -0.089,
                    median: -0.156,
                    min: -3.2,
                    max: 5.1,
                    q1: -0.67,
                    q3: 0.34,
                },
            ],
            mann_whitney_u: Some(9821.0),
            mann_whitney_p: Some(0.003),
        });

        assert!(markdown.contains("# Study Group Comparison: Expression"));
        assert!(markdown.contains(
            "Stratify gene: TP53 | Target gene: ERBB2 | Study: brca_tcga_pan_can_atlas_2018"
        ));
        assert!(markdown.contains("| Group | N | Mean | Median | Q1 | Q3 | Min | Max |"));
        assert!(markdown.contains("Mann-Whitney U: 9821.000"));
        assert!(markdown.contains("Mann-Whitney p-value: 0.003"));
        assert!(markdown.contains(
            "| TP53-wildtype | 730 | -0.089 | -0.156 | -0.670 | 0.340 | -3.200 | 5.100 |"
        ));
    }

    #[test]
    fn study_compare_mutations_markdown_renders_rate_table() {
        let markdown = study_compare_mutations_markdown(&StudyMutationComparisonResult {
            study_id: "brca_tcga_pan_can_atlas_2018".to_string(),
            stratify_gene: "TP53".to_string(),
            target_gene: "PIK3CA".to_string(),
            groups: vec![
                StudyMutationGroupStats {
                    group_name: "TP53-mutant".to_string(),
                    sample_count: 348,
                    mutated_count: 120,
                    mutation_rate: 0.344828,
                },
                StudyMutationGroupStats {
                    group_name: "TP53-wildtype".to_string(),
                    sample_count: 736,
                    mutated_count: 220,
                    mutation_rate: 0.298913,
                },
            ],
        });

        assert!(markdown.contains("# Study Group Comparison: Mutation Rate"));
        assert!(markdown.contains(
            "Stratify gene: TP53 | Target gene: PIK3CA | Study: brca_tcga_pan_can_atlas_2018"
        ));
        assert!(markdown.contains("| Group | N | Mutated | Mutation Rate |"));
        assert!(markdown.contains("| TP53-mutant | 348 | 120 | 0.344828 |"));
    }
}
