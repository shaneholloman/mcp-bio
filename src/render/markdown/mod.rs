//! Markdown renderers exposed through the stable markdown facade.

mod adverse_event;
mod evidence;
mod funding;
mod pathway;
mod pgx;
mod protein;
mod related;
#[cfg(test)]
mod root_tests;
mod sections;
mod study;
mod support;
#[cfg(test)]
mod test_support;
#[cfg(test)]
pub(crate) mod tests;
mod trial;

#[allow(unused_imports)]
use self::{evidence::*, funding::*, related::*, sections::*, support::*};

#[allow(unused_imports)]
pub use self::adverse_event::{
    adverse_event_count_markdown, adverse_event_markdown, adverse_event_search_markdown,
    adverse_event_search_markdown_with_footer, device_event_markdown, device_event_search_markdown,
    device_event_search_markdown_with_footer, recall_search_markdown,
    recall_search_markdown_with_footer,
};
pub(crate) use self::evidence::{
    adverse_event_evidence_urls, article_evidence_urls, device_event_evidence_urls,
    discover_evidence_urls, disease_evidence_urls, drug_evidence_urls, gene_evidence_urls,
    pathway_evidence_urls, pgx_evidence_urls, protein_evidence_urls, trial_evidence_urls,
    variant_evidence_urls,
};
#[allow(unused_imports)]
pub use self::pathway::{
    pathway_markdown, pathway_search_markdown, pathway_search_markdown_with_footer,
};
#[allow(unused_imports)]
pub use self::pgx::{pgx_markdown, pgx_search_markdown, pgx_search_markdown_with_footer};
#[allow(unused_imports)]
pub use self::protein::{
    protein_markdown, protein_search_markdown, protein_search_markdown_with_footer,
};
pub(crate) use self::related::{
    related_adverse_event, related_article, related_device_event, related_disease, related_drug,
    related_gene, related_pathway, related_pgx, related_phenotype_search_results, related_protein,
    related_trial, related_variant, related_variant_search_results,
};
#[allow(unused_imports)]
pub use self::study::{
    study_co_occurrence_markdown, study_cohort_markdown, study_compare_expression_markdown,
    study_compare_mutations_markdown, study_download_catalog_markdown, study_download_markdown,
    study_filter_markdown, study_list_markdown, study_query_markdown, study_survival_markdown,
    study_top_mutated_markdown,
};
pub(crate) use self::support::{alias_fallback_suggestion, quote_arg, variant_guidance_suggestion};
#[allow(unused_imports)]
pub use self::trial::{trial_markdown, trial_search_markdown, trial_search_markdown_with_footer};
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
    env.add_template("gene.md.j2", include_str!("../../../templates/gene.md.j2"))?;
    env.add_template(
        "gene_search.md.j2",
        include_str!("../../../templates/gene_search.md.j2"),
    )?;
    env.add_template(
        "article.md.j2",
        include_str!("../../../templates/article.md.j2"),
    )?;
    env.add_template(
        "article_entities.md.j2",
        include_str!("../../../templates/article_entities.md.j2"),
    )?;
    env.add_template(
        "article_search.md.j2",
        include_str!("../../../templates/article_search.md.j2"),
    )?;
    env.add_template(
        "disease.md.j2",
        include_str!("../../../templates/disease.md.j2"),
    )?;
    env.add_template(
        "disease_search.md.j2",
        include_str!("../../../templates/disease_search.md.j2"),
    )?;
    env.add_template("pgx.md.j2", include_str!("../../../templates/pgx.md.j2"))?;
    env.add_template(
        "pgx_search.md.j2",
        include_str!("../../../templates/pgx_search.md.j2"),
    )?;
    env.add_template(
        "trial.md.j2",
        include_str!("../../../templates/trial.md.j2"),
    )?;
    env.add_template(
        "trial_search.md.j2",
        include_str!("../../../templates/trial_search.md.j2"),
    )?;
    env.add_template(
        "variant.md.j2",
        include_str!("../../../templates/variant.md.j2"),
    )?;
    env.add_template(
        "variant_search.md.j2",
        include_str!("../../../templates/variant_search.md.j2"),
    )?;
    env.add_template(
        "phenotype_search.md.j2",
        include_str!("../../../templates/phenotype_search.md.j2"),
    )?;
    env.add_template(
        "gwas_search.md.j2",
        include_str!("../../../templates/gwas_search.md.j2"),
    )?;
    env.add_template("drug.md.j2", include_str!("../../../templates/drug.md.j2"))?;
    env.add_template(
        "drug_search.md.j2",
        include_str!("../../../templates/drug_search.md.j2"),
    )?;
    env.add_template(
        "pathway.md.j2",
        include_str!("../../../templates/pathway.md.j2"),
    )?;
    env.add_template(
        "pathway_search.md.j2",
        include_str!("../../../templates/pathway_search.md.j2"),
    )?;
    env.add_template(
        "protein.md.j2",
        include_str!("../../../templates/protein.md.j2"),
    )?;
    env.add_template(
        "protein_search.md.j2",
        include_str!("../../../templates/protein_search.md.j2"),
    )?;
    env.add_template(
        "adverse_event.md.j2",
        include_str!("../../../templates/adverse_event.md.j2"),
    )?;
    env.add_template(
        "adverse_event_search.md.j2",
        include_str!("../../../templates/adverse_event_search.md.j2"),
    )?;
    env.add_template(
        "device_event.md.j2",
        include_str!("../../../templates/device_event.md.j2"),
    )?;
    env.add_template(
        "device_event_search.md.j2",
        include_str!("../../../templates/device_event_search.md.j2"),
    )?;
    env.add_template(
        "recall_search.md.j2",
        include_str!("../../../templates/recall_search.md.j2"),
    )?;
    env.add_template(
        "search_all.md.j2",
        include_str!("../../../templates/search_all.md.j2"),
    )?;
    env.add_template(
        "discover.md.j2",
        include_str!("../../../templates/discover.md.j2"),
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
