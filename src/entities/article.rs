use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use futures::future::try_join_all;
use serde::{Deserialize, Serialize};

use crate::entities::SearchPage;
use crate::error::BioMcpError;
use crate::sources::europepmc::{
    EuropePmcClient, EuropePmcResult, EuropePmcSearchResponse, EuropePmcSort,
};
use crate::sources::litsense2::LitSense2Client;
use crate::sources::ncbi_idconv::NcbiIdConverterClient;
use crate::sources::pmc_oa::PmcOaClient;
use crate::sources::pubmed::{PubMedClient, PubMedESearchParams};
use crate::sources::pubtator::PubTatorClient;
use crate::sources::semantic_scholar::{
    SemanticScholarCitationEdge, SemanticScholarClient, SemanticScholarPaper,
    SemanticScholarReferenceEdge,
};
use crate::transform;
use crate::utils::date::validate_since;
use crate::utils::download;
use tracing::warn;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pmid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pmcid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doi: Option<String>,
    pub title: String,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub citation_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publication_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_access: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abstract_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_text_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_text_note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<ArticleAnnotations>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_scholar: Option<ArticleSemanticScholar>,
    #[serde(default)]
    pub pubtator_fallback: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArticleSemanticScholar {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paper_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tldr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub citation_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub influential_citation_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_open_access: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_access_pdf: Option<ArticleSemanticScholarPdf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArticleSemanticScholarPdf {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArticleAnnotations {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub genes: Vec<AnnotationCount>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diseases: Vec<AnnotationCount>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chemicals: Vec<AnnotationCount>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mutations: Vec<AnnotationCount>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnnotationCount {
    pub text: String,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArticleBatchItem {
    pub requested_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pmid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pmcid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doi: Option<String>,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_summary: Option<ArticleBatchEntitySummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tldr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub citation_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub influential_citation_count: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArticleBatchEntitySummary {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub genes: Vec<AnnotationCount>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diseases: Vec<AnnotationCount>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chemicals: Vec<AnnotationCount>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mutations: Vec<AnnotationCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleSearchResult {
    pub pmid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pmcid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doi: Option<String>,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub citation_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub influential_citation_count: Option<u64>,
    pub source: ArticleSource,
    #[serde(default)]
    pub matched_sources: Vec<ArticleSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    #[serde(default)]
    pub is_retracted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abstract_snippet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ranking: Option<ArticleRankingMetadata>,
    #[serde(skip)]
    pub normalized_title: String,
    #[serde(skip)]
    pub normalized_abstract: String,
    #[serde(skip)]
    pub publication_type: Option<String>,
    #[serde(skip)]
    pub source_local_position: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArticlePubMedRescueKind {
    Unique,
    Led,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArticleRankingMetadata {
    pub directness_tier: u8,
    pub anchor_count: u8,
    pub title_anchor_hits: u8,
    pub abstract_anchor_hits: u8,
    pub combined_anchor_hits: u8,
    pub all_anchors_in_title: bool,
    pub all_anchors_in_text: bool,
    pub study_or_review_cue: bool,
    #[serde(default)]
    pub pubmed_rescue: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pubmed_rescue_kind: Option<ArticlePubMedRescueKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pubmed_source_position: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<ArticleRankingMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_score: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lexical_score: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub citation_score: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position_score: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composite_score: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avg_source_rank: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ArticleSourcePosition {
    source: ArticleSource,
    local_position: usize,
}

#[derive(Debug, Clone)]
struct ArticleCandidate {
    row: ArticleSearchResult,
    source_positions: Vec<ArticleSourcePosition>,
    semantic_signal: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArticleRelatedPaper {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paper_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pmid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doi: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arxiv_id: Option<String>,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArticleGraphEdge {
    pub paper: ArticleRelatedPaper,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub intents: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contexts: Vec<String>,
    pub is_influential: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArticleGraphResult {
    pub article: ArticleRelatedPaper,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<ArticleGraphEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArticleRecommendationsResult {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub positive_seeds: Vec<ArticleRelatedPaper>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub negative_seeds: Vec<ArticleRelatedPaper>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recommendations: Vec<ArticleRelatedPaper>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArticleSource {
    PubTator,
    EuropePmc,
    SemanticScholar,
    PubMed,
    LitSense2,
}

impl ArticleSource {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::PubTator => "PubTator3",
            Self::EuropePmc => "Europe PMC",
            Self::SemanticScholar => "Semantic Scholar",
            Self::PubMed => "PubMed",
            Self::LitSense2 => "LitSense2",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArticleSourceFilter {
    #[default]
    All,
    PubTator,
    EuropePmc,
    PubMed,
    LitSense2,
}

impl ArticleSourceFilter {
    pub fn from_flag(value: &str) -> Result<Self, BioMcpError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "all" => Ok(Self::All),
            "pubtator" => Ok(Self::PubTator),
            "europepmc" | "europe-pmc" => Ok(Self::EuropePmc),
            "pubmed" => Ok(Self::PubMed),
            "litsense2" => Ok(Self::LitSense2),
            other => Err(BioMcpError::InvalidArgument(format!(
                "Unknown --source '{other}'. Expected one of: all, pubtator, europepmc, pubmed, litsense2."
            ))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::PubTator => "pubtator",
            Self::EuropePmc => "europepmc",
            Self::PubMed => "pubmed",
            Self::LitSense2 => "litsense2",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArticleSort {
    Date,
    Citations,
    #[default]
    Relevance,
}

impl ArticleSort {
    pub fn from_flag(value: &str) -> Result<Self, BioMcpError> {
        let value = value.trim();
        match value.to_ascii_lowercase().as_str() {
            "date" => Ok(Self::Date),
            "citations" => Ok(Self::Citations),
            "relevance" => Ok(Self::Relevance),
            _ => Err(BioMcpError::InvalidArgument(
                "Invalid article sort. Expected one of: date, citations, relevance".into(),
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Date => "date",
            Self::Citations => "citations",
            Self::Relevance => "relevance",
        }
    }

    fn as_europepmc_sort(self) -> EuropePmcSort {
        match self {
            Self::Date => EuropePmcSort::Date,
            Self::Citations => EuropePmcSort::Citations,
            Self::Relevance => EuropePmcSort::Relevance,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArticleRankingMode {
    Lexical,
    Semantic,
    Hybrid,
}

impl ArticleRankingMode {
    pub fn from_flag(value: &str) -> Result<Self, BioMcpError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "lexical" => Ok(Self::Lexical),
            "semantic" => Ok(Self::Semantic),
            "hybrid" => Ok(Self::Hybrid),
            _ => Err(BioMcpError::InvalidArgument(
                "Invalid article ranking mode. Expected one of: lexical, semantic, hybrid".into(),
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lexical => "lexical",
            Self::Semantic => "semantic",
            Self::Hybrid => "hybrid",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArticleRankingWeights {
    pub semantic: f64,
    pub lexical: f64,
    pub citations: f64,
    pub position: f64,
}

impl Default for ArticleRankingWeights {
    fn default() -> Self {
        Self {
            semantic: 0.4,
            lexical: 0.3,
            citations: 0.2,
            position: 0.1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ArticleRankingOptions {
    pub requested_mode: Option<ArticleRankingMode>,
    pub weights: ArticleRankingWeights,
    pub weights_overridden: bool,
}

impl ArticleRankingOptions {
    pub fn from_inputs(
        requested_mode: Option<&str>,
        weight_semantic: Option<f64>,
        weight_lexical: Option<f64>,
        weight_citations: Option<f64>,
        weight_position: Option<f64>,
    ) -> Result<Self, BioMcpError> {
        let defaults = ArticleRankingWeights::default();
        Ok(Self {
            requested_mode: requested_mode
                .map(ArticleRankingMode::from_flag)
                .transpose()?,
            weights: ArticleRankingWeights {
                semantic: weight_semantic.unwrap_or(defaults.semantic),
                lexical: weight_lexical.unwrap_or(defaults.lexical),
                citations: weight_citations.unwrap_or(defaults.citations),
                position: weight_position.unwrap_or(defaults.position),
            },
            weights_overridden: weight_semantic.is_some()
                || weight_lexical.is_some()
                || weight_citations.is_some()
                || weight_position.is_some(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct ArticleSearchFilters {
    pub gene: Option<String>,
    pub gene_anchored: bool,
    pub disease: Option<String>,
    pub drug: Option<String>,
    pub author: Option<String>,
    pub keyword: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub article_type: Option<String>,
    pub journal: Option<String>,
    pub open_access: bool,
    pub no_preprints: bool,
    pub exclude_retracted: bool,
    pub max_per_source: Option<usize>,
    pub sort: ArticleSort,
    pub ranking: ArticleRankingOptions,
}

const ARTICLE_SECTION_ANNOTATIONS: &str = "annotations";
const ARTICLE_SECTION_FULLTEXT: &str = "fulltext";
const ARTICLE_SECTION_TLDR: &str = "tldr";
const ARTICLE_SECTION_ALL: &str = "all";

pub const ARTICLE_SECTION_NAMES: &[&str] = &[
    ARTICLE_SECTION_ANNOTATIONS,
    ARTICLE_SECTION_FULLTEXT,
    ARTICLE_SECTION_TLDR,
    ARTICLE_SECTION_ALL,
];

const MAX_SEARCH_LIMIT: usize = 50;
pub const ARTICLE_BATCH_MAX_IDS: usize = 20;
const EUROPE_PMC_PAGE_SIZE: usize = 25;
const PUBTATOR_PAGE_SIZE: usize = 25;
const PUBMED_PAGE_SIZE: usize = 100;
const MAX_PAGE_FETCHES: usize = 50;
const WARN_PAGE_THRESHOLD: usize = 20;
const SEMANTIC_SCHOLAR_BATCH_LOOKUP_MAX_IDS: usize = 500;
const FEDERATED_PAGE_SIZE_CAP: usize = if EUROPE_PMC_PAGE_SIZE < PUBTATOR_PAGE_SIZE {
    EUROPE_PMC_PAGE_SIZE
} else {
    PUBTATOR_PAGE_SIZE
};
const MAX_FEDERATED_FETCH_RESULTS: usize = MAX_PAGE_FETCHES * FEDERATED_PAGE_SIZE_CAP;
const FULLTEXT_CACHE_VERSION: &str = "jats-v1";
const PUBMED_RESCUE_POSITION_MAX: usize = 0;
const INVALID_ARTICLE_ID_MSG: &str = "\
Unsupported identifier format. BioMCP resolves PMID (digits only, e.g., 22663011), \
PMCID (starts with PMC, e.g., PMC9984800), and DOI (starts with 10., \
e.g., 10.1056/NEJMoa1203421). publisher PIIs (e.g., S1535610826000103) are not \
indexed by PubMed or Europe PMC and cannot be resolved.";
pub const ARTICLE_RELEVANCE_RANKING_POLICY: &str = "calibrated PubMed rescue + lexical directness (top-ranked weak PubMed unique/led rows with at least one anchor hit > title coverage > title+abstract coverage > study/review cue > citation support > source-local position)";
pub const ARTICLE_SEMANTIC_RANKING_POLICY: &str =
    "semantic relevance (semantic score first, lexical directness fallback)";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackendPlan {
    EuropeOnly,
    PubTatorOnly,
    PubMedOnly,
    LitSense2Only,
    TypeCapable,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntityBiotype {
    Gene,
    Disease,
    Chemical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArticleSourceCapMode {
    Default(usize),
    Explicit(usize),
    Disabled,
}

fn is_doi(id: &str) -> bool {
    let id = id.trim();
    if !id.starts_with("10.") {
        return false;
    }
    id.contains('/')
}

fn parse_pmid(id: &str) -> Option<u32> {
    let id = id.trim();
    if id.is_empty() {
        return None;
    }
    if !id.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    id.parse::<u32>().ok()
}

fn parse_pmcid(id: &str) -> Option<String> {
    let mut id = id.trim();
    if id.len() > 6
        && let Some(prefix) = id.get(..6)
        && prefix.eq_ignore_ascii_case("PMCID:")
        && let Some(rest) = id.get(6..)
    {
        id = rest.trim();
    }
    if id.len() < 4 {
        return None;
    }
    let prefix = id.get(..3)?;
    if !prefix.eq_ignore_ascii_case("PMC") {
        return None;
    }
    let rest = id.get(3..)?.trim();
    if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(format!("PMC{rest}"))
}

#[derive(Debug, Clone)]
enum ArticleIdType {
    Pmc(String),
    Doi(String),
    Pmid(u32),
    Invalid,
}

fn parse_article_id(id: &str) -> ArticleIdType {
    let id = id.trim();
    if let Some(pmcid) = parse_pmcid(id) {
        return ArticleIdType::Pmc(pmcid);
    }
    if is_doi(id) {
        return ArticleIdType::Doi(id.to_string());
    }
    if let Some(pmid) = parse_pmid(id) {
        return ArticleIdType::Pmid(pmid);
    }
    ArticleIdType::Invalid
}

fn parse_arxiv_id(id: &str) -> Option<String> {
    let id = id.trim();
    if id.len() <= 6 {
        return None;
    }
    let prefix = id.get(..6)?;
    if !prefix.eq_ignore_ascii_case("arxiv:") {
        return None;
    }
    let rest = id.get(6..)?.trim();
    if rest.is_empty() {
        return None;
    }
    Some(format!("ARXIV:{rest}"))
}

fn fulltext_cache_key(id: &str) -> String {
    format!("article-fulltext-{FULLTEXT_CACHE_VERSION}:{id}")
}

async fn render_fulltext_xml(xml: String) -> Result<String, BioMcpError> {
    tokio::task::spawn_blocking(move || transform::article::extract_text_from_xml(&xml))
        .await
        .map_err(|err| BioMcpError::Api {
            api: "article".to_string(),
            message: format!("Full text render worker failed: {err}"),
        })
}

fn is_semantic_scholar_paper_id(id: &str) -> bool {
    id.len() == 40 && id.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn semantic_scholar_lookup_id(id: &str) -> Option<String> {
    let id = id.trim();
    if let Some(pmid) = parse_pmid(id) {
        return Some(format!("PMID:{pmid}"));
    }
    if is_doi(id) {
        return Some(format!("DOI:{id}"));
    }
    if let Some(arxiv) = parse_arxiv_id(id) {
        return Some(arxiv);
    }
    if is_semantic_scholar_paper_id(id) {
        return Some(id.to_string());
    }
    None
}

fn is_preprint_journal(journal: &str) -> bool {
    let j = journal.to_ascii_lowercase();
    j.contains("biorxiv") || j.contains("medrxiv") || j.contains("arxiv")
}

fn europepmc_escape(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return String::new();
    }

    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        if matches!(
            ch,
            '\\' | '\"'
                | '+'
                | '-'
                | '!'
                | '('
                | ')'
                | '{'
                | '}'
                | '['
                | ']'
                | '^'
                | '~'
                | '*'
                | '?'
                | ':'
                | '|'
        ) {
            escaped.push('\\');
        }
        escaped.push(ch);
    }

    escaped
}

fn europepmc_phrase(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return String::new();
    }
    let escaped = europepmc_escape(value);
    if value.chars().any(|c| c.is_whitespace()) || value.contains('/') {
        format!("\"{escaped}\"")
    } else {
        escaped
    }
}

fn europepmc_keyword(value: &str) -> String {
    europepmc_escape(value)
}

fn normalize_article_type(value: &str) -> Result<&'static str, BioMcpError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "review" => Ok("review"),
        "research" | "research-article" => Ok("research-article"),
        "case-reports" => Ok("case-reports"),
        "meta-analysis" | "metaanalysis" => Ok("meta-analysis"),
        _ => Err(BioMcpError::InvalidArgument(
            "--type must be one of: review, research, research-article, case-reports, meta-analysis".into(),
        )),
    }
}

fn relabel_date_argument_error(err: BioMcpError, flag_name: &str) -> BioMcpError {
    if let BioMcpError::InvalidArgument(message) = err {
        BioMcpError::InvalidArgument(message.replace("--since", flag_name))
    } else {
        err
    }
}

fn normalized_date_bound(
    value: Option<&str>,
    flag_name: &str,
) -> Result<Option<String>, BioMcpError> {
    value
        .map(|value| {
            validate_since(value).map_err(|err| relabel_date_argument_error(err, flag_name))
        })
        .transpose()
}

fn validate_search_filter_values(filters: &ArticleSearchFilters) -> Result<(), BioMcpError> {
    if let Some(article_type) = filters
        .article_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        normalize_article_type(article_type)?;
    }
    Ok(())
}

fn validate_required_search_filters(filters: &ArticleSearchFilters) -> Result<(), BioMcpError> {
    if filters.gene.is_none()
        && filters.disease.is_none()
        && filters.drug.is_none()
        && filters.author.is_none()
        && filters.keyword.is_none()
        && filters.article_type.is_none()
        && !filters.open_access
    {
        return Err(BioMcpError::InvalidArgument(
            "At least one filter is required. Example: biomcp search article -g BRAF".into(),
        ));
    }
    Ok(())
}

fn normalized_date_bounds(
    filters: &ArticleSearchFilters,
) -> Result<(Option<String>, Option<String>), BioMcpError> {
    let normalized_date_from = normalized_date_bound(filters.date_from.as_deref(), "--date-from")?;
    let normalized_date_to = normalized_date_bound(filters.date_to.as_deref(), "--date-to")?;
    if let (Some(from), Some(to)) = (
        normalized_date_from.as_deref(),
        normalized_date_to.as_deref(),
    ) && from > to
    {
        return Err(BioMcpError::InvalidArgument(
            "--date-from must be <= --date-to".into(),
        ));
    }
    Ok((normalized_date_from, normalized_date_to))
}

fn has_article_type_filter(filters: &ArticleSearchFilters) -> bool {
    filters
        .article_type
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}

fn has_keyword_query(filters: &ArticleSearchFilters) -> bool {
    filters
        .keyword
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}

#[derive(Debug, Clone, Copy)]
struct ResolvedArticleRanking {
    mode: ArticleRankingMode,
    weights: ArticleRankingWeights,
}

fn resolve_article_ranking(filters: &ArticleSearchFilters) -> ResolvedArticleRanking {
    ResolvedArticleRanking {
        mode: filters.ranking.requested_mode.unwrap_or_else(|| {
            if has_keyword_query(filters) {
                ArticleRankingMode::Hybrid
            } else {
                ArticleRankingMode::Lexical
            }
        }),
        weights: filters.ranking.weights,
    }
}

pub(crate) fn article_effective_ranking_mode(
    filters: &ArticleSearchFilters,
) -> Option<ArticleRankingMode> {
    (filters.sort == ArticleSort::Relevance).then(|| resolve_article_ranking(filters).mode)
}

fn format_article_ranking_number(value: f64) -> String {
    let mut out = format!("{value:.3}");
    while out.contains('.') && out.ends_with('0') {
        out.pop();
    }
    if out.ends_with('.') {
        out.pop();
    }
    if out == "-0" { "0".to_string() } else { out }
}

pub(crate) fn article_relevance_ranking_policy(filters: &ArticleSearchFilters) -> Option<String> {
    if filters.sort != ArticleSort::Relevance {
        return None;
    }

    let ranking = resolve_article_ranking(filters);
    Some(match ranking.mode {
        ArticleRankingMode::Lexical => ARTICLE_RELEVANCE_RANKING_POLICY.to_string(),
        ArticleRankingMode::Semantic => ARTICLE_SEMANTIC_RANKING_POLICY.to_string(),
        ArticleRankingMode::Hybrid => format!(
            "hybrid relevance (score = {}*semantic + {}*lexical + {}*citations + {}*position)",
            format_article_ranking_number(ranking.weights.semantic),
            format_article_ranking_number(ranking.weights.lexical),
            format_article_ranking_number(ranking.weights.citations),
            format_article_ranking_number(ranking.weights.position),
        ),
    })
}

fn validate_article_ranking_options(filters: &ArticleSearchFilters) -> Result<(), BioMcpError> {
    let requested_mode = filters.ranking.requested_mode;
    let weights_overridden = filters.ranking.weights_overridden;
    if filters.sort != ArticleSort::Relevance {
        if requested_mode.is_some() || weights_overridden {
            return Err(BioMcpError::InvalidArgument(
                "--ranking-mode and --weight-* require --sort relevance".into(),
            ));
        }
        return Ok(());
    }

    for (flag, value) in [
        ("--weight-semantic", filters.ranking.weights.semantic),
        ("--weight-lexical", filters.ranking.weights.lexical),
        ("--weight-citations", filters.ranking.weights.citations),
        ("--weight-position", filters.ranking.weights.position),
    ] {
        if !value.is_finite() {
            return Err(BioMcpError::InvalidArgument(format!(
                "{flag} must be finite"
            )));
        }
        if value < 0.0 {
            return Err(BioMcpError::InvalidArgument(format!("{flag} must be >= 0")));
        }
    }

    let resolved = resolve_article_ranking(filters);
    if weights_overridden && resolved.mode != ArticleRankingMode::Hybrid {
        return Err(BioMcpError::InvalidArgument(
            "--weight-* flags require --ranking-mode hybrid or no explicit ranking mode".into(),
        ));
    }

    if resolved.mode == ArticleRankingMode::Hybrid
        && filters.ranking.weights.semantic == 0.0
        && filters.ranking.weights.lexical == 0.0
        && filters.ranking.weights.citations == 0.0
        && filters.ranking.weights.position == 0.0
    {
        return Err(BioMcpError::InvalidArgument(
            "At least one hybrid ranking weight must be > 0".into(),
        ));
    }

    Ok(())
}

fn pubmed_filter_compatible(filters: &ArticleSearchFilters) -> bool {
    !filters.open_access && !filters.no_preprints
}

fn has_strict_europepmc_filters(filters: &ArticleSearchFilters) -> bool {
    filters.open_access || has_article_type_filter(filters)
}

fn pubtator_strict_filter_error(filters: &ArticleSearchFilters) -> BioMcpError {
    if has_article_type_filter(filters) && pubmed_filter_compatible(filters) {
        return BioMcpError::InvalidArgument(
            "--source pubtator does not support --type. Use --source europepmc, --source pubmed, or remove --type.".into(),
        );
    }
    if filters.open_access {
        return BioMcpError::InvalidArgument(
            "--source pubtator does not support --open-access. Use --source europepmc or relax --open-access.".into(),
        );
    }
    BioMcpError::InvalidArgument(
        "--source pubtator does not support --type with --no-preprints. Use --source europepmc or relax --no-preprints.".into(),
    )
}

fn pubmed_source_filter_error(filters: &ArticleSearchFilters) -> BioMcpError {
    match (filters.open_access, filters.no_preprints) {
        (true, true) => BioMcpError::InvalidArgument(
            "--source pubmed does not support --open-access or --no-preprints. Use --source europepmc or relax the selected filters.".into(),
        ),
        (true, false) => BioMcpError::InvalidArgument(
            "--source pubmed does not support --open-access. Use --source europepmc or remove --open-access.".into(),
        ),
        (false, true) => BioMcpError::InvalidArgument(
            "--source pubmed does not support --no-preprints. Use --source europepmc or remove --no-preprints.".into(),
        ),
        (false, false) => unreachable!("pubmed_source_filter_error called with compatible filters"),
    }
}

fn litsense2_source_filter_error(filters: &ArticleSearchFilters) -> BioMcpError {
    if !has_keyword_query(filters) {
        return BioMcpError::InvalidArgument(
            "--source litsense2 requires a keyword query. Add -k/--keyword (or a positional query) or use --source all.".into(),
        );
    }
    if has_article_type_filter(filters) {
        return BioMcpError::InvalidArgument(
            "--source litsense2 does not support --type. Use --source europepmc, --source pubmed, or remove --type.".into(),
        );
    }
    if filters.open_access {
        return BioMcpError::InvalidArgument(
            "--source litsense2 does not support --open-access. Use --source europepmc or remove --open-access.".into(),
        );
    }
    unreachable!("litsense2_source_filter_error called with compatible filters");
}

fn plan_backends(
    filters: &ArticleSearchFilters,
    source: ArticleSourceFilter,
) -> Result<BackendPlan, BioMcpError> {
    match source {
        ArticleSourceFilter::EuropePmc => Ok(BackendPlan::EuropeOnly),
        ArticleSourceFilter::PubTator => {
            if has_strict_europepmc_filters(filters) {
                return Err(pubtator_strict_filter_error(filters));
            }
            Ok(BackendPlan::PubTatorOnly)
        }
        ArticleSourceFilter::PubMed => {
            if !pubmed_filter_compatible(filters) {
                return Err(pubmed_source_filter_error(filters));
            }
            Ok(BackendPlan::PubMedOnly)
        }
        ArticleSourceFilter::LitSense2 => {
            if !has_keyword_query(filters)
                || has_article_type_filter(filters)
                || filters.open_access
            {
                return Err(litsense2_source_filter_error(filters));
            }
            Ok(BackendPlan::LitSense2Only)
        }
        ArticleSourceFilter::All => {
            if filters.open_access {
                Ok(BackendPlan::EuropeOnly)
            } else if has_article_type_filter(filters) {
                if pubmed_filter_compatible(filters) {
                    Ok(BackendPlan::TypeCapable)
                } else {
                    Ok(BackendPlan::EuropeOnly)
                }
            } else {
                Ok(BackendPlan::Both)
            }
        }
    }
}

pub fn semantic_scholar_search_enabled(
    filters: &ArticleSearchFilters,
    source: ArticleSourceFilter,
) -> bool {
    source == ArticleSourceFilter::All && !has_strict_europepmc_filters(filters)
}

pub fn litsense2_search_enabled(
    filters: &ArticleSearchFilters,
    source: ArticleSourceFilter,
) -> bool {
    source == ArticleSourceFilter::All
        && !has_strict_europepmc_filters(filters)
        && has_keyword_query(filters)
}

pub(crate) fn article_type_limitation_note(
    filters: &ArticleSearchFilters,
    source: ArticleSourceFilter,
) -> Option<String> {
    if source != ArticleSourceFilter::All || !has_article_type_filter(filters) {
        return None;
    }
    if pubmed_filter_compatible(filters) {
        Some(
            "Note: --type restricts article search to Europe PMC and PubMed. PubTator3, LitSense2, and Semantic Scholar do not support publication-type filtering."
                .into(),
        )
    } else {
        Some(
            "Note: --type restricts this article search to Europe PMC. PubTator3, LitSense2, and Semantic Scholar do not support publication-type filtering, and PubMed does not support the other selected filters."
                .into(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArticleSearchDebugSummary {
    pub routing: Vec<String>,
    pub sources: Vec<String>,
    pub matched_sources: Vec<String>,
}

pub(crate) fn summarize_debug_plan(
    filters: &ArticleSearchFilters,
    source: ArticleSourceFilter,
    results: &[ArticleSearchResult],
) -> Result<ArticleSearchDebugSummary, BioMcpError> {
    let plan = plan_backends(filters, source)?;
    let planner = match (plan, source) {
        (BackendPlan::EuropeOnly, ArticleSourceFilter::All)
            if filters.open_access
                || (has_article_type_filter(filters) && !pubmed_filter_compatible(filters)) =>
        {
            "planner=europe_only_strict_filters"
        }
        (BackendPlan::EuropeOnly, _) => "planner=europe_only",
        (BackendPlan::PubTatorOnly, _) => "planner=pubtator_only",
        (BackendPlan::PubMedOnly, _) => "planner=pubmed_only",
        (BackendPlan::LitSense2Only, _) => "planner=litsense2_only",
        (BackendPlan::TypeCapable, _) => "planner=type_capable",
        (BackendPlan::Both, _) => "planner=federated",
    };

    let mut sources = match plan {
        BackendPlan::EuropeOnly => vec!["Europe PMC".to_string()],
        BackendPlan::PubTatorOnly => vec!["PubTator3".to_string()],
        BackendPlan::PubMedOnly => vec!["PubMed".to_string()],
        BackendPlan::LitSense2Only => vec!["LitSense2".to_string()],
        BackendPlan::TypeCapable => vec!["Europe PMC".to_string(), "PubMed".to_string()],
        BackendPlan::Both => {
            let mut sources = vec!["PubTator3".to_string(), "Europe PMC".to_string()];
            if pubmed_filter_compatible(filters) {
                sources.push("PubMed".to_string());
            }
            if litsense2_search_enabled(filters, source) {
                sources.push("LitSense2".to_string());
            }
            sources
        }
    };
    if semantic_scholar_search_enabled(filters, source) {
        sources.push("Semantic Scholar".to_string());
    }

    let matched_sources = [
        ArticleSource::PubTator,
        ArticleSource::EuropePmc,
        ArticleSource::PubMed,
        ArticleSource::SemanticScholar,
        ArticleSource::LitSense2,
    ]
    .into_iter()
    .filter(|candidate| {
        results.iter().any(|row| {
            row.source == *candidate || row.matched_sources.iter().any(|source| source == candidate)
        })
    })
    .map(|source| source.display_name().to_string())
    .collect();

    Ok(ArticleSearchDebugSummary {
        routing: vec![planner.to_string()],
        sources,
        matched_sources,
    })
}

fn matches_entity_biotype(value: Option<&str>, expected: EntityBiotype) -> bool {
    let Some(value) = value else {
        return false;
    };
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }
    match expected {
        EntityBiotype::Gene => normalized.contains("gene"),
        EntityBiotype::Disease => normalized.contains("disease"),
        EntityBiotype::Chemical => normalized.contains("chemical") || normalized.contains("drug"),
    }
}

async fn normalize_entity_token(
    pubtator: &PubTatorClient,
    token: Option<&str>,
    expected: EntityBiotype,
) -> Option<String> {
    let token = token.map(str::trim).filter(|value| !value.is_empty())?;
    match pubtator.entity_autocomplete(token).await {
        Ok(rows) => rows
            .iter()
            .find(|row| matches_entity_biotype(row.biotype.as_deref(), expected))
            .and_then(|row| row.id.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
            .or_else(|| Some(token.to_string())),
        Err(err) => {
            warn!(
                ?err,
                token, "pubtator autocomplete failed; falling back to raw token"
            );
            Some(token.to_string())
        }
    }
}

fn pubtator_sort(sort: ArticleSort) -> Option<&'static str> {
    match sort {
        ArticleSort::Date => Some("date desc"),
        ArticleSort::Citations | ArticleSort::Relevance => None,
    }
}

fn parse_row_date(value: Option<&str>) -> Option<String> {
    let value = value.map(str::trim).filter(|v| !v.is_empty())?;
    let truncated = value.get(0..10).unwrap_or(value);
    match truncated.len() {
        4 => Some(format!("{truncated}-01-01")),
        7 => Some(format!("{truncated}-01")),
        _ => Some(truncated.to_string()),
    }
}

fn matches_optional_journal_filter(
    row_journal: Option<&str>,
    expected_journal: Option<&str>,
) -> bool {
    let Some(expected) = expected_journal
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return true;
    };
    let Some(actual) = row_journal.map(str::trim).filter(|value| !value.is_empty()) else {
        return false;
    };
    actual
        .to_ascii_lowercase()
        .contains(&expected.to_ascii_lowercase())
}

fn matches_optional_date_filter(
    row_date: Option<&str>,
    date_from: Option<&str>,
    date_to: Option<&str>,
) -> bool {
    if date_from.is_none() && date_to.is_none() {
        return true;
    }
    let Some(value) = parse_row_date(row_date) else {
        return false;
    };
    if let Some(from) = date_from
        && value.as_str() < from
    {
        return false;
    }
    if let Some(to) = date_to
        && value.as_str() > to
    {
        return false;
    }
    true
}

fn matches_result_filters(
    row: &ArticleSearchResult,
    filters: &ArticleSearchFilters,
    date_from: Option<&str>,
    date_to: Option<&str>,
) -> bool {
    if filters.no_preprints && row.journal.as_deref().is_some_and(is_preprint_journal) {
        return false;
    }
    // `None` means the source does not expose retraction metadata; default exclusion
    // only hides rows that are positively confirmed as retracted.
    if filters.exclude_retracted && row.is_retracted == Some(true) {
        return false;
    }
    if !matches_optional_journal_filter(row.journal.as_deref(), filters.journal.as_deref()) {
        return false;
    }
    if !matches_optional_date_filter(row.date.as_deref(), date_from, date_to) {
        return false;
    }
    true
}

fn normalize_row_identifier(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

fn article_source_priority(source: ArticleSource) -> u8 {
    match source {
        ArticleSource::PubTator => 0,
        ArticleSource::EuropePmc => 1,
        ArticleSource::PubMed => 2,
        ArticleSource::SemanticScholar => 3,
        ArticleSource::LitSense2 => 4,
    }
}

fn stable_article_identifier(row: &ArticleSearchResult) -> String {
    normalize_row_identifier(Some(&row.pmid))
        .or_else(|| normalize_row_identifier(row.pmcid.as_deref()))
        .or_else(|| normalize_row_identifier(row.doi.as_deref()))
        .unwrap_or_else(|| row.title.to_ascii_lowercase())
}

fn validate_article_source_cap(
    filters: &ArticleSearchFilters,
    limit: usize,
) -> Result<(), BioMcpError> {
    if let Some(max_per_source) = filters.max_per_source
        && max_per_source > limit
    {
        return Err(BioMcpError::InvalidArgument(
            "--max-per-source must be <= --limit".into(),
        ));
    }
    Ok(())
}

fn resolve_article_source_cap(
    filters: &ArticleSearchFilters,
    limit: usize,
) -> ArticleSourceCapMode {
    match filters.max_per_source {
        None | Some(0) => ArticleSourceCapMode::Default((limit.saturating_mul(40) / 100).max(1)),
        Some(value) if value == limit => ArticleSourceCapMode::Disabled,
        Some(value) => ArticleSourceCapMode::Explicit(value),
    }
}

fn ensure_matched_sources(row: &mut ArticleSearchResult) {
    if !row.matched_sources.contains(&row.source) {
        row.matched_sources.push(row.source);
    }
    row.matched_sources
        .sort_by_key(|source| article_source_priority(*source));
    row.matched_sources.dedup();
}

fn article_candidate_from_row(mut row: ArticleSearchResult) -> ArticleCandidate {
    ensure_matched_sources(&mut row);
    ArticleCandidate {
        source_positions: vec![ArticleSourcePosition {
            source: row.source,
            local_position: row.source_local_position,
        }],
        semantic_signal: (row.source == ArticleSource::LitSense2)
            .then(|| row.score.unwrap_or(0.0).clamp(0.0, 1.0)),
        row,
    }
}

fn collapse_source_positions(source_positions: &mut Vec<ArticleSourcePosition>) {
    source_positions
        .sort_by_key(|entry| (article_source_priority(entry.source), entry.local_position));
    source_positions.dedup_by_key(|entry| entry.source);
}

fn min_source_local_position(source_positions: &[ArticleSourcePosition]) -> Option<usize> {
    source_positions
        .iter()
        .map(|entry| entry.local_position)
        .min()
}

fn article_rows_overlap(left: &ArticleSearchResult, right: &ArticleSearchResult) -> bool {
    let left_pmid = normalize_row_identifier(Some(&left.pmid));
    let right_pmid = normalize_row_identifier(Some(&right.pmid));
    let left_pmcid = normalize_row_identifier(left.pmcid.as_deref());
    let right_pmcid = normalize_row_identifier(right.pmcid.as_deref());
    let left_doi = normalize_row_identifier(left.doi.as_deref());
    let right_doi = normalize_row_identifier(right.doi.as_deref());

    left_pmid.is_some() && left_pmid == right_pmid
        || left_pmcid.is_some() && left_pmcid == right_pmcid
        || left_doi.is_some() && left_doi == right_doi
}

fn merge_missing_string(target: &mut Option<String>, incoming: Option<String>) {
    if target
        .as_deref()
        .map(str::trim)
        .is_none_or(|value| value.is_empty())
    {
        *target = incoming
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
    }
}

fn merge_missing_u64(target: &mut Option<u64>, incoming: Option<u64>) {
    if target.is_none() {
        *target = incoming;
    }
}

fn merge_article_candidate(target: &mut ArticleCandidate, incoming: ArticleCandidate) {
    let ArticleCandidate {
        row: incoming_row,
        mut source_positions,
        semantic_signal,
    } = incoming;
    let target_row = &mut target.row;
    merge_missing_string(&mut target_row.pmcid, incoming_row.pmcid);
    merge_missing_string(&mut target_row.doi, incoming_row.doi);
    if target_row.pmid.trim().is_empty() && !incoming_row.pmid.trim().is_empty() {
        target_row.pmid = incoming_row.pmid;
    }
    if target_row.title.trim().is_empty() && !incoming_row.title.trim().is_empty() {
        target_row.title = incoming_row.title;
    }
    merge_missing_string(&mut target_row.journal, incoming_row.journal);
    merge_missing_string(&mut target_row.date, incoming_row.date);
    merge_missing_u64(&mut target_row.citation_count, incoming_row.citation_count);
    merge_missing_u64(
        &mut target_row.influential_citation_count,
        incoming_row.influential_citation_count,
    );
    if target_row.score.is_none() {
        target_row.score = incoming_row.score;
    }
    if target_row.is_retracted.is_none() && incoming_row.is_retracted.is_some() {
        target_row.is_retracted = incoming_row.is_retracted;
    }
    merge_missing_string(
        &mut target_row.abstract_snippet,
        incoming_row.abstract_snippet,
    );
    if target_row.ranking.is_none() {
        target_row.ranking = incoming_row.ranking;
    }
    if target_row.normalized_title.is_empty() && !incoming_row.normalized_title.is_empty() {
        target_row.normalized_title = incoming_row.normalized_title;
    }
    if target_row.normalized_abstract.is_empty() && !incoming_row.normalized_abstract.is_empty() {
        target_row.normalized_abstract = incoming_row.normalized_abstract;
    }
    merge_missing_string(
        &mut target_row.publication_type,
        incoming_row.publication_type,
    );
    target_row
        .matched_sources
        .extend(incoming_row.matched_sources);
    ensure_matched_sources(target_row);
    target.source_positions.append(&mut source_positions);
    collapse_source_positions(&mut target.source_positions);
    if target.semantic_signal.is_none() {
        target.semantic_signal = semantic_signal;
    }
    if let Some(local_position) = min_source_local_position(&target.source_positions) {
        target_row.source_local_position = local_position;
    }
}

fn merge_article_candidates(results: Vec<ArticleSearchResult>) -> Vec<ArticleCandidate> {
    let mut merged: Vec<ArticleCandidate> = Vec::with_capacity(results.len());

    for row in results {
        let row = article_candidate_from_row(row);
        let matches = merged
            .iter()
            .enumerate()
            .filter_map(|(idx, existing)| {
                article_rows_overlap(&existing.row, &row.row).then_some(idx)
            })
            .collect::<Vec<_>>();

        if matches.is_empty() {
            merged.push(row);
            continue;
        }

        let keep_idx = matches[0];
        merge_article_candidate(&mut merged[keep_idx], row);
        for idx in matches.into_iter().skip(1).rev() {
            let duplicate = merged.remove(idx);
            merge_article_candidate(&mut merged[keep_idx], duplicate);
        }
    }

    merged
}

fn primary_source_native_position(candidate: &ArticleCandidate) -> usize {
    candidate
        .source_positions
        .iter()
        .find(|entry| entry.source == candidate.row.source)
        .map(|entry| entry.local_position)
        .unwrap_or(candidate.row.source_local_position)
}

fn distinct_primary_source_count(candidates: &[ArticleCandidate]) -> usize {
    let mut seen = Vec::new();
    for candidate in candidates {
        if !seen.contains(&candidate.row.source) {
            seen.push(candidate.row.source);
        }
    }
    seen.len()
}

fn cap_article_candidates_by_source(
    candidates: Vec<ArticleCandidate>,
    cap_mode: ArticleSourceCapMode,
) -> Vec<ArticleCandidate> {
    let source_count = distinct_primary_source_count(&candidates);
    let effective_cap = match cap_mode {
        ArticleSourceCapMode::Disabled => return candidates,
        ArticleSourceCapMode::Default(_) if source_count < 3 => return candidates,
        ArticleSourceCapMode::Default(cap) | ArticleSourceCapMode::Explicit(cap)
            if source_count < 2 =>
        {
            return candidates;
        }
        ArticleSourceCapMode::Default(cap) | ArticleSourceCapMode::Explicit(cap) => cap,
    };

    let mut buckets: Vec<(ArticleSource, Vec<ArticleCandidate>)> = Vec::new();
    for candidate in candidates {
        if let Some((_, rows)) = buckets
            .iter_mut()
            .find(|(source, _)| *source == candidate.row.source)
        {
            rows.push(candidate);
        } else {
            buckets.push((candidate.row.source, vec![candidate]));
        }
    }

    let mut retained = Vec::new();
    for (_, mut bucket) in buckets {
        bucket.sort_by(|left, right| {
            primary_source_native_position(left)
                .cmp(&primary_source_native_position(right))
                .then_with(|| {
                    stable_article_identifier(&left.row).cmp(&stable_article_identifier(&right.row))
                })
        });
        retained.extend(bucket.into_iter().take(effective_cap));
    }
    retained
}

fn compare_optional_dates_desc(
    left: Option<&ArticleSearchResult>,
    right: Option<&ArticleSearchResult>,
) -> Ordering {
    match (
        left.and_then(|row| parse_row_date(row.date.as_deref())),
        right.and_then(|row| parse_row_date(row.date.as_deref())),
    ) {
        (Some(left), Some(right)) => right.cmp(&left),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn compare_optional_citations_desc(
    left: Option<&ArticleSearchResult>,
    right: Option<&ArticleSearchResult>,
) -> Ordering {
    match (
        left.and_then(|row| row.citation_count),
        right.and_then(|row| row.citation_count),
    ) {
        (Some(left), Some(right)) => right.cmp(&left),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn build_anchor_set(filters: &ArticleSearchFilters) -> Vec<String> {
    let mut anchors = Vec::new();
    let mut seen = HashSet::new();
    for value in [
        filters.gene.as_deref(),
        filters.disease.as_deref(),
        filters.drug.as_deref(),
    ] {
        let Some(anchor) = value
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(transform::article::normalize_article_search_text)
        else {
            continue;
        };
        if seen.insert(anchor.clone()) {
            anchors.push(anchor);
        }
    }

    if let Some(keyword) = filters.keyword.as_deref() {
        for token in keyword.split_whitespace() {
            let anchor = transform::article::normalize_article_search_text(token);
            if !anchor.is_empty() && seen.insert(anchor.clone()) {
                anchors.push(anchor);
            }
        }
    }

    anchors
}

fn anchor_matches_text(text: &str, anchor: &str) -> bool {
    if anchor.is_empty() || text.is_empty() {
        return false;
    }
    if anchor.chars().any(|ch| ch.is_whitespace()) {
        return text.contains(anchor);
    }

    for (idx, _) in text.match_indices(anchor) {
        let start_ok = text[..idx]
            .chars()
            .next_back()
            .is_none_or(|ch| !ch.is_ascii_alphanumeric());
        let end_idx = idx + anchor.len();
        let end_ok = text[end_idx..]
            .chars()
            .next()
            .is_none_or(|ch| !ch.is_ascii_alphanumeric());
        if start_ok && end_ok {
            return true;
        }
    }
    false
}

fn has_study_or_review_cue(row: &ArticleSearchResult) -> bool {
    let title = row.normalized_title.as_str();
    let publication_type = row
        .publication_type
        .as_deref()
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    [
        "review",
        "meta-analysis",
        "meta analysis",
        "systematic review",
        "clinical trial",
    ]
    .into_iter()
    .any(|cue| title.contains(cue) || publication_type.contains(cue))
}

fn pubmed_rescue_metadata(
    row: &ArticleSearchResult,
    source_positions: &[ArticleSourcePosition],
    directness_tier: u8,
    combined_hits: usize,
) -> (bool, Option<ArticlePubMedRescueKind>, Option<usize>) {
    let Some(pubmed_position) = source_positions
        .iter()
        .find(|entry| entry.source == ArticleSource::PubMed)
        .map(|entry| entry.local_position)
    else {
        return (false, None, None);
    };

    if pubmed_position > PUBMED_RESCUE_POSITION_MAX || directness_tier > 1 || combined_hits == 0 {
        return (false, None, None);
    }

    let pubmed_unique = matches!(row.matched_sources.as_slice(), [ArticleSource::PubMed]);
    let has_non_pubmed_source = source_positions
        .iter()
        .any(|entry| entry.source != ArticleSource::PubMed);
    let pubmed_led = row.matched_sources.contains(&ArticleSource::PubMed)
        && has_non_pubmed_source
        && source_positions
            .iter()
            .filter(|entry| entry.source != ArticleSource::PubMed)
            .all(|entry| pubmed_position < entry.local_position);

    let pubmed_rescue_kind = if pubmed_unique {
        Some(ArticlePubMedRescueKind::Unique)
    } else if pubmed_led {
        Some(ArticlePubMedRescueKind::Led)
    } else {
        None
    };

    (
        pubmed_rescue_kind.is_some(),
        pubmed_rescue_kind,
        pubmed_rescue_kind.map(|_| pubmed_position),
    )
}

fn lexical_ranking_metadata(
    row: &ArticleSearchResult,
    source_positions: &[ArticleSourcePosition],
    anchors: &[String],
) -> ArticleRankingMetadata {
    let title_hits = anchors
        .iter()
        .filter(|anchor| anchor_matches_text(&row.normalized_title, anchor))
        .count();
    let abstract_hits = anchors
        .iter()
        .filter(|anchor| anchor_matches_text(&row.normalized_abstract, anchor))
        .count();
    let combined_hits = anchors
        .iter()
        .filter(|anchor| {
            anchor_matches_text(&row.normalized_title, anchor)
                || anchor_matches_text(&row.normalized_abstract, anchor)
        })
        .count();
    let anchor_count = anchors.len();
    let all_anchors_in_title = anchor_count > 0 && title_hits == anchor_count;
    let all_anchors_in_text = anchor_count > 0 && combined_hits == anchor_count;
    let directness_tier = if all_anchors_in_title {
        3
    } else if all_anchors_in_text {
        2
    } else if combined_hits > 0 {
        1
    } else {
        0
    };
    let study_or_review_cue = has_study_or_review_cue(row);
    let (pubmed_rescue, pubmed_rescue_kind, pubmed_source_position) =
        pubmed_rescue_metadata(row, source_positions, directness_tier, combined_hits);

    ArticleRankingMetadata {
        directness_tier,
        anchor_count: anchor_count.min(u8::MAX as usize) as u8,
        title_anchor_hits: title_hits.min(u8::MAX as usize) as u8,
        abstract_anchor_hits: abstract_hits.min(u8::MAX as usize) as u8,
        combined_anchor_hits: combined_hits.min(u8::MAX as usize) as u8,
        all_anchors_in_title,
        all_anchors_in_text,
        study_or_review_cue,
        pubmed_rescue,
        pubmed_rescue_kind,
        pubmed_source_position,
        mode: None,
        semantic_score: None,
        lexical_score: None,
        citation_score: None,
        position_score: None,
        composite_score: None,
        avg_source_rank: None,
    }
}

fn populate_lexical_ranking_metadata(
    rows: &mut [ArticleCandidate],
    filters: &ArticleSearchFilters,
) {
    let anchors = build_anchor_set(filters);

    for row in rows.iter_mut() {
        ensure_matched_sources(&mut row.row);
        row.row.ranking = Some(lexical_ranking_metadata(
            &row.row,
            &row.source_positions,
            &anchors,
        ));
    }
}

fn compare_article_candidates_lexical(
    left: &ArticleCandidate,
    right: &ArticleCandidate,
) -> Ordering {
    let left_ranking = left.row.ranking.as_ref();
    let right_ranking = right.row.ranking.as_ref();
    right_ranking
        .map(|ranking| ranking.pubmed_rescue)
        .cmp(&left_ranking.map(|ranking| ranking.pubmed_rescue))
        .then_with(|| {
            right_ranking
                .map(|ranking| ranking.directness_tier)
                .cmp(&left_ranking.map(|ranking| ranking.directness_tier))
        })
        .then_with(|| {
            right_ranking
                .map(|ranking| ranking.title_anchor_hits)
                .cmp(&left_ranking.map(|ranking| ranking.title_anchor_hits))
        })
        .then_with(|| {
            right_ranking
                .map(|ranking| ranking.combined_anchor_hits)
                .cmp(&left_ranking.map(|ranking| ranking.combined_anchor_hits))
        })
        .then_with(|| {
            right_ranking
                .map(|ranking| ranking.study_or_review_cue)
                .cmp(&left_ranking.map(|ranking| ranking.study_or_review_cue))
        })
        .then_with(|| compare_optional_citations_desc(Some(&left.row), Some(&right.row)))
        .then_with(|| {
            right
                .row
                .influential_citation_count
                .cmp(&left.row.influential_citation_count)
        })
        .then_with(|| {
            left.row
                .source_local_position
                .cmp(&right.row.source_local_position)
        })
        .then_with(|| {
            stable_article_identifier(&left.row).cmp(&stable_article_identifier(&right.row))
        })
}

fn semantic_signal(candidate: &ArticleCandidate) -> f64 {
    candidate.semantic_signal.unwrap_or(0.0)
}

fn avg_source_rank(source_positions: &[ArticleSourcePosition], fallback_position: usize) -> f64 {
    if source_positions.is_empty() {
        return fallback_position as f64 + 1.0;
    }
    source_positions
        .iter()
        .map(|entry| entry.local_position as f64 + 1.0)
        .sum::<f64>()
        / source_positions.len() as f64
}

fn normalized_citation_score(citation_count: Option<u64>, max_citation_count: u64) -> f64 {
    if max_citation_count == 0 {
        return 0.0;
    }
    let citations = citation_count.unwrap_or(0) as f64;
    (1.0 + citations).ln() / (1.0 + max_citation_count as f64).ln()
}

fn normalized_position_score(avg_source_rank: f64, max_avg_source_rank: f64) -> f64 {
    if max_avg_source_rank <= 1.0 {
        0.0
    } else {
        1.0 - (avg_source_rank / max_avg_source_rank)
    }
}

fn rank_articles_by_directness(rows: &mut [ArticleCandidate], filters: &ArticleSearchFilters) {
    populate_lexical_ranking_metadata(rows, filters);
    for row in rows.iter_mut() {
        if let Some(ranking) = row.row.ranking.as_mut() {
            ranking.mode = Some(ArticleRankingMode::Lexical);
        }
    }
    rows.sort_by(compare_article_candidates_lexical);
}

fn rank_articles_by_semantic(rows: &mut [ArticleCandidate], filters: &ArticleSearchFilters) {
    populate_lexical_ranking_metadata(rows, filters);
    for row in rows.iter_mut() {
        let semantic_score = semantic_signal(row);
        if let Some(ranking) = row.row.ranking.as_mut() {
            ranking.mode = Some(ArticleRankingMode::Semantic);
            ranking.semantic_score = Some(semantic_score);
        }
    }
    rows.sort_by(|left, right| {
        semantic_signal(right)
            .total_cmp(&semantic_signal(left))
            .then_with(|| compare_article_candidates_lexical(left, right))
    });
}

fn rank_articles_hybrid(rows: &mut [ArticleCandidate], filters: &ArticleSearchFilters) {
    populate_lexical_ranking_metadata(rows, filters);
    let ranking = resolve_article_ranking(filters);
    let max_citation_count = rows
        .iter()
        .filter_map(|candidate| candidate.row.citation_count)
        .max()
        .unwrap_or(0);
    let max_avg_source_rank = rows
        .iter()
        .map(|candidate| {
            avg_source_rank(
                &candidate.source_positions,
                candidate.row.source_local_position,
            )
        })
        .fold(0.0, f64::max);

    for row in rows.iter_mut() {
        let semantic_score = semantic_signal(row);
        let lexical_score = row
            .row
            .ranking
            .as_ref()
            .map(|entry| entry.directness_tier as f64 / 3.0)
            .unwrap_or(0.0);
        let citation_score = normalized_citation_score(row.row.citation_count, max_citation_count);
        let avg_source_rank = avg_source_rank(&row.source_positions, row.row.source_local_position);
        let position_score = normalized_position_score(avg_source_rank, max_avg_source_rank);
        let composite_score = ranking.weights.semantic * semantic_score
            + ranking.weights.lexical * lexical_score
            + ranking.weights.citations * citation_score
            + ranking.weights.position * position_score;

        if let Some(metadata) = row.row.ranking.as_mut() {
            metadata.mode = Some(ArticleRankingMode::Hybrid);
            metadata.semantic_score = Some(semantic_score);
            metadata.lexical_score = Some(lexical_score);
            metadata.citation_score = Some(citation_score);
            metadata.position_score = Some(position_score);
            metadata.composite_score = Some(composite_score);
            metadata.avg_source_rank = Some(avg_source_rank);
        }
    }

    rows.sort_by(|left, right| {
        let left_score = left
            .row
            .ranking
            .as_ref()
            .and_then(|ranking| ranking.composite_score)
            .unwrap_or(0.0);
        let right_score = right
            .row
            .ranking
            .as_ref()
            .and_then(|ranking| ranking.composite_score)
            .unwrap_or(0.0);
        right_score.total_cmp(&left_score).then_with(|| {
            stable_article_identifier(&left.row).cmp(&stable_article_identifier(&right.row))
        })
    });
}

fn sort_article_rows(
    rows: &mut [ArticleCandidate],
    sort: ArticleSort,
    filters: &ArticleSearchFilters,
) {
    match sort {
        ArticleSort::Relevance => match resolve_article_ranking(filters).mode {
            ArticleRankingMode::Lexical => rank_articles_by_directness(rows, filters),
            ArticleRankingMode::Semantic => rank_articles_by_semantic(rows, filters),
            ArticleRankingMode::Hybrid => rank_articles_hybrid(rows, filters),
        },
        ArticleSort::Citations => rows.sort_by(|left, right| {
            compare_optional_citations_desc(Some(&left.row), Some(&right.row))
                .then_with(|| compare_optional_dates_desc(Some(&left.row), Some(&right.row)))
                .then_with(|| left.row.pmid.cmp(&right.row.pmid))
        }),
        ArticleSort::Date => rows.sort_by(|left, right| {
            compare_optional_dates_desc(Some(&left.row), Some(&right.row))
                .then_with(|| compare_optional_citations_desc(Some(&left.row), Some(&right.row)))
                .then_with(|| left.row.pmid.cmp(&right.row.pmid))
        }),
    }
}

fn build_search_query(filters: &ArticleSearchFilters) -> Result<String, BioMcpError> {
    validate_required_search_filters(filters)?;
    validate_article_ranking_options(filters)?;
    let (normalized_date_from, normalized_date_to) = normalized_date_bounds(filters)?;
    let mut terms: Vec<String> = Vec::new();

    if let Some(gene) = filters
        .gene
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        if filters.gene_anchored {
            terms.push(format!("GENE_PROTEIN:{}", europepmc_phrase(gene)));
        } else {
            terms.push(europepmc_phrase(gene));
        }
    }
    if let Some(disease) = filters
        .disease
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        terms.push(europepmc_phrase(disease));
    }
    if let Some(drug) = filters
        .drug
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        terms.push(europepmc_phrase(drug));
    }
    if let Some(author) = filters
        .author
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        terms.push(format!("AUTH:{}", europepmc_phrase(author)));
    }
    if let Some(keyword) = filters
        .keyword
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        terms.push(europepmc_keyword(keyword));
    }

    if let Some(article_type) = filters
        .article_type
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let normalized = normalize_article_type(article_type)?;
        terms.push(format!("PUB_TYPE:\"{normalized}\""));
    }

    if let Some(journal) = filters
        .journal
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        terms.push(format!("JOURNAL:{}", europepmc_phrase(journal)));
    }

    if filters.open_access {
        terms.push("OPEN_ACCESS:y".into());
    }

    if let Some(from) = normalized_date_from.as_deref() {
        let to = normalized_date_to.as_deref().unwrap_or("*");
        terms.push(format!("FIRST_PDATE:[{from} TO {to}]"));
    } else if let Some(to) = normalized_date_to.as_deref() {
        terms.push(format!("FIRST_PDATE:[* TO {to}]"));
    }

    if filters.no_preprints {
        terms.push("NOT SRC:PPR".into());
    }
    if filters.exclude_retracted {
        terms.push("NOT PUB_TYPE:\"retracted publication\"".into());
    }

    Ok(terms.join(" AND "))
}

fn build_pubmed_search_term(filters: &ArticleSearchFilters) -> Result<String, BioMcpError> {
    validate_required_search_filters(filters)?;
    validate_search_filter_values(filters)?;

    let mut clauses: Vec<String> = Vec::new();
    for value in [
        filters.gene.as_deref(),
        filters.disease.as_deref(),
        filters.drug.as_deref(),
        filters.keyword.as_deref(),
    ] {
        if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
            clauses.push(value.to_string());
        }
    }

    if let Some(author) = filters
        .author
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        clauses.push(format!("\"{author}\"[author]"));
    }

    if let Some(journal) = filters
        .journal
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        clauses.push(format!("\"{journal}\"[journal]"));
    }

    if let Some(article_type) = filters
        .article_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let normalized = normalize_article_type(article_type)?;
        let clause = match normalized {
            "review" => "review[pt]",
            "research-article" => "journal article[pt]",
            "case-reports" => "case reports[pt]",
            "meta-analysis" => "meta-analysis[pt]",
            _ => {
                return Err(BioMcpError::InvalidArgument(
                    "--type must be one of: review, research, research-article, case-reports, meta-analysis".into(),
                ))
            }
        };
        clauses.push(clause.to_string());
    }

    let base = clauses.join(" AND ");
    if filters.exclude_retracted {
        if base.is_empty() {
            return Ok("NOT retracted publication[pt]".to_string());
        }
        return Ok(format!("{base} NOT retracted publication[pt]"));
    }

    Ok(base)
}

#[cfg_attr(not(test), allow(dead_code))]
fn build_pubmed_esearch_params(
    filters: &ArticleSearchFilters,
    limit: usize,
    offset: usize,
) -> Result<PubMedESearchParams, BioMcpError> {
    let term = build_pubmed_search_term(filters)?;
    let (date_from, date_to) = normalized_date_bounds(filters)?;

    if limit == 0 || limit > MAX_FEDERATED_FETCH_RESULTS {
        return Err(BioMcpError::InvalidArgument(format!(
            "--limit must be between 1 and {MAX_FEDERATED_FETCH_RESULTS}"
        )));
    }

    let fetch_count = limit.saturating_add(offset);
    if fetch_count > MAX_FEDERATED_FETCH_RESULTS {
        return Err(BioMcpError::InvalidArgument(format!(
            "--offset + --limit must be <= {MAX_FEDERATED_FETCH_RESULTS} for federated article search"
        )));
    }

    if filters.open_access {
        return Err(BioMcpError::InvalidArgument(
            "PubMed ESearch does not support --open-access filtering".into(),
        ));
    }
    if filters.no_preprints {
        return Err(BioMcpError::InvalidArgument(
            "PubMed ESearch does not support --no-preprints filtering".into(),
        ));
    }

    Ok(PubMedESearchParams {
        term,
        retstart: offset,
        retmax: limit,
        date_from,
        date_to,
    })
}

async fn search_pubmed_page(
    filters: &ArticleSearchFilters,
    limit: usize,
    offset: usize,
) -> Result<SearchPage<ArticleSearchResult>, BioMcpError> {
    if limit == 0 || limit > MAX_FEDERATED_FETCH_RESULTS {
        return Err(BioMcpError::InvalidArgument(format!(
            "--limit must be between 1 and {MAX_FEDERATED_FETCH_RESULTS}"
        )));
    }
    if filters.open_access {
        return Err(BioMcpError::InvalidArgument(
            "PubMed ESearch does not support --open-access filtering".into(),
        ));
    }
    if filters.no_preprints {
        return Err(BioMcpError::InvalidArgument(
            "PubMed ESearch does not support --no-preprints filtering".into(),
        ));
    }

    let term = build_pubmed_search_term(filters)?;
    let (normalized_date_from, normalized_date_to) = normalized_date_bounds(filters)?;
    let client = PubMedClient::new()?;

    let mut out: Vec<ArticleSearchResult> = Vec::with_capacity(limit.min(10));
    let mut seen_pmids: HashSet<String> = HashSet::with_capacity(limit.min(10));
    let mut total: Option<usize> = None;
    let mut batch_start = 0usize;
    let mut visible_skipped = 0usize;
    let mut source_position = 0usize;
    let mut fetched_pages = 0usize;
    while out.len() < limit && fetched_pages < MAX_PAGE_FETCHES {
        fetched_pages = fetched_pages.saturating_add(1);
        if fetched_pages == WARN_PAGE_THRESHOLD + 1 {
            tracing::warn!(
                "article search is deep (>{WARN_PAGE_THRESHOLD} page fetches); continuing up to {MAX_PAGE_FETCHES} — consider narrowing your query"
            );
        }

        let response = client
            .esearch(&PubMedESearchParams {
                term: term.clone(),
                retstart: batch_start,
                retmax: PUBMED_PAGE_SIZE,
                date_from: normalized_date_from.clone(),
                date_to: normalized_date_to.clone(),
            })
            .await?;
        if total.is_none() {
            total = Some(response.count as usize);
            if total.is_some_and(|value| offset >= value) {
                return Ok(SearchPage::offset(Vec::new(), total));
            }
        }
        if response.idlist.is_empty() {
            break;
        }

        let batch_len = response.idlist.len();
        let entries = client.esummary(&response.idlist).await?;
        for entry in entries {
            let mut row =
                transform::article::from_pubmed_esummary_entry(&entry).ok_or_else(|| {
                    BioMcpError::Api {
                        api: "pubmed-eutils".to_string(),
                        message: format!(
                            "ESummary entry for PMID {} has blank title after cleaning",
                            entry.uid
                        ),
                    }
                })?;
            if !matches_result_filters(
                &row,
                filters,
                normalized_date_from.as_deref(),
                normalized_date_to.as_deref(),
            ) {
                continue;
            }
            if !seen_pmids.insert(row.pmid.clone()) {
                continue;
            }
            row.source_local_position = source_position;
            source_position = source_position.saturating_add(1);
            if visible_skipped < offset {
                visible_skipped = visible_skipped.saturating_add(1);
                continue;
            }
            out.push(row);
            if out.len() >= limit {
                break;
            }
        }

        batch_start = batch_start.saturating_add(batch_len);
    }

    Ok(SearchPage::offset(out, total))
}

async fn build_pubtator_query(
    filters: &ArticleSearchFilters,
    pubtator: &PubTatorClient,
) -> Result<String, BioMcpError> {
    validate_required_search_filters(filters)?;
    let gene = normalize_entity_token(pubtator, filters.gene.as_deref(), EntityBiotype::Gene).await;
    let disease =
        normalize_entity_token(pubtator, filters.disease.as_deref(), EntityBiotype::Disease).await;
    let drug =
        normalize_entity_token(pubtator, filters.drug.as_deref(), EntityBiotype::Chemical).await;

    let mut terms: Vec<String> = Vec::new();
    if let Some(gene) = gene {
        terms.push(gene);
    }
    if let Some(disease) = disease {
        terms.push(disease);
    }
    if let Some(drug) = drug {
        terms.push(drug);
    }
    if let Some(author) = filters
        .author
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        terms.push(author.to_string());
    }
    if let Some(keyword) = filters
        .keyword
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        terms.push(keyword.to_string());
    }

    if terms.is_empty() {
        return Err(BioMcpError::InvalidArgument(
            "At least one queryable token is required for --source pubtator.".into(),
        ));
    }

    Ok(terms.join(" "))
}

#[derive(Debug, Clone, Copy, Default)]
struct ArticleSections {
    include_annotations: bool,
    include_fulltext: bool,
    include_tldr: bool,
    include_all: bool,
}

fn parse_sections(sections: &[String]) -> Result<ArticleSections, BioMcpError> {
    let mut out = ArticleSections::default();

    for raw in sections {
        let section = raw.trim().to_ascii_lowercase();
        if section.is_empty() {
            continue;
        }
        if section == "--json" || section == "-j" {
            continue;
        }

        match section.as_str() {
            ARTICLE_SECTION_ANNOTATIONS => out.include_annotations = true,
            ARTICLE_SECTION_FULLTEXT => out.include_fulltext = true,
            ARTICLE_SECTION_TLDR => out.include_tldr = true,
            ARTICLE_SECTION_ALL => out.include_all = true,
            _ => {
                return Err(BioMcpError::InvalidArgument(format!(
                    "Unknown section \"{section}\" for article. Available: {}",
                    ARTICLE_SECTION_NAMES.join(", ")
                )));
            }
        }
    }

    if out.include_all {
        out.include_annotations = true;
        out.include_fulltext = true;
        out.include_tldr = true;
    }

    Ok(out)
}

fn is_section_only_request(sections: &[String], include_all: bool) -> bool {
    if include_all {
        return false;
    }
    sections.iter().any(|s| {
        let value = s.trim().to_ascii_lowercase();
        !value.is_empty() && value != "--json" && value != "-j"
    })
}

fn article_not_found(id: &str, suggestion_id: &str) -> BioMcpError {
    BioMcpError::NotFound {
        entity: "article".into(),
        id: id.to_string(),
        suggestion: format!("Try searching: biomcp search article -q \"{suggestion_id}\""),
    }
}

fn semantic_scholar_invalid_id(id: &str) -> BioMcpError {
    BioMcpError::InvalidArgument(format!(
        "Unsupported identifier format for Semantic Scholar article helpers: '{id}'. Supported: PMID, PMCID, DOI, arXiv, or a Semantic Scholar paper ID."
    ))
}

fn first_europepmc_hit(search: EuropePmcSearchResponse) -> Option<EuropePmcResult> {
    search.result_list.and_then(|l| l.result.into_iter().next())
}

fn related_paper_from_semantic_scholar(paper: &SemanticScholarPaper) -> ArticleRelatedPaper {
    let external_ids = paper.external_ids.as_ref();
    ArticleRelatedPaper {
        paper_id: paper.paper_id.clone(),
        pmid: external_ids.and_then(|ids| ids.pubmed.clone()),
        doi: external_ids.and_then(|ids| ids.doi.clone()),
        arxiv_id: external_ids.and_then(|ids| ids.arxiv.clone()),
        title: paper.title.clone().unwrap_or_default().trim().to_string(),
        journal: paper
            .venue
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        year: paper.year,
    }
}

fn semantic_scholar_enrichment_from_paper(
    paper: &SemanticScholarPaper,
) -> Option<ArticleSemanticScholar> {
    let open_access_pdf = paper.open_access_pdf.as_ref().and_then(|pdf| {
        let url = pdf
            .url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())?;
        Some(ArticleSemanticScholarPdf {
            url: url.to_string(),
            status: pdf
                .status
                .as_ref()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            license: pdf
                .license
                .as_ref()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
        })
    });
    let tldr = paper
        .tldr
        .as_ref()
        .and_then(|value| value.text.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    if paper.paper_id.is_none()
        && tldr.is_none()
        && paper.citation_count.is_none()
        && paper.influential_citation_count.is_none()
        && paper.reference_count.is_none()
        && paper.is_open_access.is_none()
        && open_access_pdf.is_none()
    {
        return None;
    }

    Some(ArticleSemanticScholar {
        paper_id: paper.paper_id.clone(),
        tldr,
        citation_count: paper.citation_count,
        influential_citation_count: paper.influential_citation_count,
        reference_count: paper.reference_count,
        is_open_access: paper.is_open_access,
        open_access_pdf,
    })
}

async fn resolve_semantic_scholar_input_id(
    id: &str,
    europe: &EuropePmcClient,
) -> Result<String, BioMcpError> {
    if let Some(id) = semantic_scholar_lookup_id(id) {
        return Ok(id);
    }

    if let Some(pmcid) = parse_pmcid(id) {
        let search = europe.search_by_pmcid(&pmcid).await?;
        let hit = first_europepmc_hit(search).ok_or_else(|| article_not_found(&pmcid, id))?;
        if let Some(pmid) = hit.pmid.as_deref().and_then(parse_pmid) {
            return Ok(format!("PMID:{pmid}"));
        }
        if let Some(doi) = hit
            .doi
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Ok(format!("DOI:{doi}"));
        }
        return Err(article_not_found(&pmcid, id));
    }

    Err(semantic_scholar_invalid_id(id))
}

async fn resolve_semantic_scholar_seed(
    id: &str,
    client: &SemanticScholarClient,
    europe: &EuropePmcClient,
) -> Result<ArticleRelatedPaper, BioMcpError> {
    let lookup_id = resolve_semantic_scholar_input_id(id, europe).await?;
    let mut rows = client.paper_batch(&[lookup_id]).await?;
    let paper = rows
        .pop()
        .flatten()
        .ok_or_else(|| article_not_found(id, id))?;
    Ok(related_paper_from_semantic_scholar(&paper))
}

fn dedup_related_papers(rows: Vec<ArticleRelatedPaper>) -> Vec<ArticleRelatedPaper> {
    let mut seen: HashSet<String> = HashSet::with_capacity(rows.len());
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let key = row
            .paper_id
            .as_deref()
            .map(str::to_string)
            .or_else(|| row.pmid.as_deref().map(|value| format!("pmid:{value}")))
            .or_else(|| row.doi.as_deref().map(|value| format!("doi:{value}")))
            .or_else(|| {
                row.arxiv_id
                    .as_deref()
                    .map(|value| format!("arxiv:{value}"))
            })
            .unwrap_or_else(|| row.title.clone());
        if seen.insert(key) {
            out.push(row);
        }
    }
    out
}

async fn resolve_semantic_scholar_seeds(
    ids: &[String],
    client: &SemanticScholarClient,
    europe: &EuropePmcClient,
) -> Result<Vec<ArticleRelatedPaper>, BioMcpError> {
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        out.push(resolve_semantic_scholar_seed(id, client, europe).await?);
    }
    Ok(dedup_related_papers(out))
}

fn is_pubtator_lag_error(err: &BioMcpError) -> bool {
    matches!(
        err,
        BioMcpError::Api { api, message }
            if api == "pubtator3" && (message.contains("HTTP 400") || message.contains("HTTP 404"))
    )
}

async fn resolve_article_from_pmid(
    pmid: u32,
    not_found_id: &str,
    suggestion_id: &str,
    pubtator: &PubTatorClient,
    europe: &EuropePmcClient,
    europe_hint: Option<&EuropePmcResult>,
) -> Result<Article, BioMcpError> {
    match pubtator.export_biocjson(pmid).await {
        Ok(resp) => {
            let doc = resp
                .documents
                .into_iter()
                .next()
                .ok_or_else(|| article_not_found(not_found_id, suggestion_id))?;

            let mut article = transform::article::from_pubtator_document(&doc);
            if let Some(hit) = europe_hint {
                transform::article::merge_europepmc_metadata(&mut article, hit);
            } else if let Ok(search) = europe.search_by_pmid(&pmid.to_string()).await
                && let Some(hit) = first_europepmc_hit(search)
            {
                transform::article::merge_europepmc_metadata(&mut article, &hit);
            }
            article.annotations = transform::article::extract_annotations(&doc);
            Ok(article)
        }
        Err(err) => {
            if !is_pubtator_lag_error(&err) {
                return Err(err);
            }

            let hit = match europe_hint.cloned() {
                Some(hit) => hit,
                None => {
                    let search = europe.search_by_pmid(&pmid.to_string()).await?;
                    first_europepmc_hit(search)
                        .ok_or_else(|| article_not_found(not_found_id, suggestion_id))?
                }
            };
            let mut article = transform::article::from_europepmc_result(&hit);
            article.pubtator_fallback = true;
            Ok(article)
        }
    }
}

async fn get_article_base_with_clients(
    id: &str,
    pubtator: &PubTatorClient,
    europe: &EuropePmcClient,
) -> Result<Article, BioMcpError> {
    let id = id.trim();
    if id.is_empty() {
        return Err(BioMcpError::InvalidArgument(
            "ID is required. Example: biomcp get article 22663011".into(),
        ));
    }
    if id.len() > 512 {
        return Err(BioMcpError::InvalidArgument("ID is too long.".into()));
    }

    match parse_article_id(id) {
        ArticleIdType::Pmid(pmid) => {
            resolve_article_from_pmid(pmid, id, id, pubtator, europe, None).await
        }
        ArticleIdType::Doi(doi) => {
            let search = europe.search_by_doi(&doi).await?;
            if search.hit_count.unwrap_or(0) == 0 {
                return Err(article_not_found(&doi, id));
            }
            let hit = first_europepmc_hit(search).ok_or_else(|| article_not_found(&doi, id))?;

            if let Some(pmid) = hit.pmid.as_deref().and_then(parse_pmid) {
                resolve_article_from_pmid(pmid, &doi, id, pubtator, europe, Some(&hit)).await
            } else {
                Ok(transform::article::from_europepmc_result(&hit))
            }
        }
        ArticleIdType::Pmc(pmcid) => {
            let search = europe.search_by_pmcid(&pmcid).await?;
            if search.hit_count.unwrap_or(0) == 0 {
                return Err(article_not_found(&pmcid, id));
            }
            let hit = first_europepmc_hit(search).ok_or_else(|| article_not_found(&pmcid, id))?;

            if let Some(pmid) = hit.pmid.as_deref().and_then(parse_pmid) {
                resolve_article_from_pmid(pmid, &pmcid, id, pubtator, europe, Some(&hit)).await
            } else {
                Ok(transform::article::from_europepmc_result(&hit))
            }
        }
        ArticleIdType::Invalid => Err(BioMcpError::InvalidArgument(INVALID_ARTICLE_ID_MSG.into())),
    }
}

async fn get_article_base(id: &str) -> Result<Article, BioMcpError> {
    let pubtator = PubTatorClient::new()?;
    let europe = EuropePmcClient::new()?;
    get_article_base_with_clients(id, &pubtator, &europe).await
}

fn trimmed_opt(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn article_batch_title(article: &Article, requested_id: &str) -> String {
    let title = article.title.trim();
    if !title.is_empty() {
        return title.to_string();
    }
    article
        .pmid
        .as_deref()
        .or(article.pmcid.as_deref())
        .or(article.doi.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(requested_id)
        .to_string()
}

fn article_batch_entity_summary(
    annotations: &ArticleAnnotations,
) -> Option<ArticleBatchEntitySummary> {
    fn top_three(rows: &[AnnotationCount]) -> Vec<AnnotationCount> {
        rows.iter().take(3).cloned().collect()
    }

    let summary = ArticleBatchEntitySummary {
        genes: top_three(&annotations.genes),
        diseases: top_three(&annotations.diseases),
        chemicals: top_three(&annotations.chemicals),
        mutations: top_three(&annotations.mutations),
    };

    if summary.genes.is_empty()
        && summary.diseases.is_empty()
        && summary.chemicals.is_empty()
        && summary.mutations.is_empty()
    {
        None
    } else {
        Some(summary)
    }
}

fn article_batch_year(article: &Article) -> Option<u32> {
    let normalized = parse_row_date(article.date.as_deref())?;
    normalized.get(..4)?.parse::<u32>().ok()
}

fn article_batch_semantic_scholar_lookup_id(item: &ArticleBatchItem) -> Option<String> {
    item.pmid
        .as_deref()
        .map(|pmid| format!("PMID:{pmid}"))
        .or_else(|| item.doi.as_deref().map(|doi| format!("DOI:{doi}")))
}

fn article_batch_item_from_article(requested_id: &str, article: &Article) -> ArticleBatchItem {
    let requested_id = requested_id.trim();
    ArticleBatchItem {
        requested_id: requested_id.to_string(),
        pmid: trimmed_opt(article.pmid.as_deref()),
        pmcid: trimmed_opt(article.pmcid.as_deref()),
        doi: trimmed_opt(article.doi.as_deref()),
        title: article_batch_title(article, requested_id),
        journal: trimmed_opt(article.journal.as_deref()),
        year: article_batch_year(article),
        entity_summary: article
            .annotations
            .as_ref()
            .and_then(article_batch_entity_summary),
        tldr: None,
        citation_count: None,
        influential_citation_count: None,
    }
}

fn merge_semantic_scholar_compact_rows(
    items: &mut [ArticleBatchItem],
    item_positions: &[usize],
    rows: Vec<Option<SemanticScholarPaper>>,
) {
    for (idx, paper) in item_positions.iter().zip(rows.into_iter()) {
        let Some(paper) = paper else {
            continue;
        };
        let item = &mut items[*idx];
        item.tldr = paper
            .tldr
            .as_ref()
            .and_then(|value| value.text.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        item.citation_count = paper.citation_count;
        item.influential_citation_count = paper.influential_citation_count;
    }
}

async fn enrich_article_batch_with_semantic_scholar(
    items: &mut [ArticleBatchItem],
) -> Result<(), BioMcpError> {
    let client = SemanticScholarClient::new()?;

    let mut lookup_ids = Vec::new();
    let mut item_positions = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        if let Some(lookup_id) = article_batch_semantic_scholar_lookup_id(item) {
            item_positions.push(idx);
            lookup_ids.push(lookup_id);
        }
    }
    if lookup_ids.is_empty() {
        return Ok(());
    }

    match client.paper_batch_compact(&lookup_ids).await {
        Ok(rows) => merge_semantic_scholar_compact_rows(items, &item_positions, rows),
        Err(err) => warn!(?err, "Semantic Scholar batch enrichment failed"),
    }

    Ok(())
}

pub async fn get_batch_compact(ids: &[String]) -> Result<Vec<ArticleBatchItem>, BioMcpError> {
    if ids.len() > ARTICLE_BATCH_MAX_IDS {
        return Err(BioMcpError::InvalidArgument(format!(
            "Article batch is limited to {ARTICLE_BATCH_MAX_IDS} IDs"
        )));
    }

    let pubtator = PubTatorClient::new()?;
    let europe = EuropePmcClient::new()?;
    let articles = try_join_all(
        ids.iter()
            .map(|id| get_article_base_with_clients(id, &pubtator, &europe)),
    )
    .await?;

    let mut items = ids
        .iter()
        .zip(articles.iter())
        .map(|(requested_id, article)| article_batch_item_from_article(requested_id, article))
        .collect::<Vec<_>>();
    enrich_article_batch_with_semantic_scholar(&mut items).await?;
    Ok(items)
}

async fn enrich_article_with_semantic_scholar(article: &mut Article) -> Result<(), BioMcpError> {
    let client = SemanticScholarClient::new()?;

    let lookup_id = article
        .pmid
        .as_deref()
        .map(|pmid| format!("PMID:{pmid}"))
        .or_else(|| article.doi.as_deref().map(|doi| format!("DOI:{doi}")));
    let Some(lookup_id) = lookup_id else {
        return Ok(());
    };

    match client.paper_detail(&lookup_id).await {
        Ok(paper) => article.semantic_scholar = semantic_scholar_enrichment_from_paper(&paper),
        Err(err) => warn!(?err, lookup_id, "Semantic Scholar enrichment failed"),
    }

    Ok(())
}
fn graph_edge_from_citation(edge: SemanticScholarCitationEdge) -> ArticleGraphEdge {
    ArticleGraphEdge {
        paper: related_paper_from_semantic_scholar(&edge.citing_paper),
        intents: edge.intents,
        contexts: edge.contexts,
        is_influential: edge.is_influential.unwrap_or(false),
    }
}

fn graph_edge_from_reference(edge: SemanticScholarReferenceEdge) -> ArticleGraphEdge {
    ArticleGraphEdge {
        paper: related_paper_from_semantic_scholar(&edge.cited_paper),
        intents: edge.intents,
        contexts: edge.contexts,
        is_influential: edge.is_influential.unwrap_or(false),
    }
}

pub async fn search(
    filters: &ArticleSearchFilters,
    limit: usize,
) -> Result<Vec<ArticleSearchResult>, BioMcpError> {
    Ok(search_page(filters, limit, 0, ArticleSourceFilter::All)
        .await?
        .results)
}

async fn search_europepmc_page(
    filters: &ArticleSearchFilters,
    limit: usize,
    offset: usize,
) -> Result<SearchPage<ArticleSearchResult>, BioMcpError> {
    let europe = EuropePmcClient::new()?;
    let query = build_search_query(filters)?;
    let europepmc_sort = filters.sort.as_europepmc_sort();
    let (normalized_date_from, normalized_date_to) = normalized_date_bounds(filters)?;

    let mut out: Vec<ArticleSearchResult> = Vec::with_capacity(limit.min(10));
    let mut seen_pmids: HashSet<String> = HashSet::with_capacity(limit.min(10));
    let mut total: Option<usize> = None;
    let mut page: usize = (offset / EUROPE_PMC_PAGE_SIZE) + 1;
    let mut local_skip = offset % EUROPE_PMC_PAGE_SIZE;
    let mut source_position = 0usize;
    let mut fetched_pages = 0usize;
    while out.len() < limit && fetched_pages < MAX_PAGE_FETCHES {
        fetched_pages = fetched_pages.saturating_add(1);
        if fetched_pages == WARN_PAGE_THRESHOLD + 1 {
            tracing::warn!(
                "article search is deep (>{WARN_PAGE_THRESHOLD} page fetches); continuing up to {MAX_PAGE_FETCHES} — consider narrowing your query"
            );
        }
        let resp = europe
            .search_query_with_sort(&query, page, EUROPE_PMC_PAGE_SIZE, europepmc_sort)
            .await?;
        if total.is_none() {
            total = resp.hit_count.map(|v| v as usize);
            if total.is_some_and(|value| offset >= value) {
                return Ok(SearchPage::offset(Vec::new(), total));
            }
        }
        let Some(results) = resp.result_list.map(|v| v.result) else {
            break;
        };
        if results.is_empty() {
            break;
        }

        for hit in results {
            if local_skip > 0 {
                local_skip -= 1;
                continue;
            }

            let Some(mut row) = transform::article::from_europepmc_search_result(&hit) else {
                continue;
            };
            if !matches_result_filters(
                &row,
                filters,
                normalized_date_from.as_deref(),
                normalized_date_to.as_deref(),
            ) {
                continue;
            }
            if !seen_pmids.insert(row.pmid.clone()) {
                continue;
            }
            row.source_local_position = source_position;
            source_position = source_position.saturating_add(1);
            out.push(row);
            if out.len() >= limit {
                break;
            }
        }

        page += 1;
    }

    // Safety-first default: when date-sorted results contain no visible retraction marker,
    // try adding one matched retracted publication if available.
    if !filters.exclude_retracted
        && filters.sort == ArticleSort::Date
        && !out.iter().any(|row| row.is_retracted == Some(true))
    {
        let retracted_query = format!("({query}) AND PUB_TYPE:\"retracted publication\"");
        if let Ok(resp) = europe
            .search_query_with_sort(&retracted_query, 1, 10, europepmc_sort)
            .await
        {
            let replacement = resp
                .result_list
                .map(|v| v.result)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|hit| transform::article::from_europepmc_search_result(&hit))
                .find(|row| {
                    row.is_retracted == Some(true)
                        && !seen_pmids.contains(&row.pmid)
                        && matches_result_filters(
                            row,
                            filters,
                            normalized_date_from.as_deref(),
                            normalized_date_to.as_deref(),
                        )
                });
            if let Some(mut row) = replacement {
                if out.len() >= limit && !out.is_empty() {
                    out.pop();
                }
                if out.len() < limit {
                    row.source_local_position = out.len();
                    seen_pmids.insert(row.pmid.clone());
                    out.push(row);
                }
            }
        }
    }

    Ok(SearchPage::offset(out, total))
}

async fn search_pubtator_page(
    filters: &ArticleSearchFilters,
    limit: usize,
    offset: usize,
) -> Result<SearchPage<ArticleSearchResult>, BioMcpError> {
    let pubtator = PubTatorClient::new()?;
    let query = build_pubtator_query(filters, &pubtator).await?;
    let sort = pubtator_sort(filters.sort);
    let (normalized_date_from, normalized_date_to) = normalized_date_bounds(filters)?;

    let mut out: Vec<ArticleSearchResult> = Vec::with_capacity(limit.min(10));
    let mut seen_pmids: HashSet<String> = HashSet::with_capacity(limit.min(10));
    let mut total: Option<usize> = None;
    let mut page: usize = (offset / PUBTATOR_PAGE_SIZE) + 1;
    let mut local_skip = offset % PUBTATOR_PAGE_SIZE;
    let mut source_position = 0usize;
    let mut fetched_pages = 0usize;
    while out.len() < limit && fetched_pages < MAX_PAGE_FETCHES {
        fetched_pages = fetched_pages.saturating_add(1);
        let resp = pubtator
            .search(&query, page, PUBTATOR_PAGE_SIZE, sort)
            .await?;
        if total.is_none() {
            total = resp.count.map(|v| v as usize);
            if total.is_some_and(|value| offset >= value) {
                return Ok(SearchPage::offset(Vec::new(), total));
            }
        }

        if resp.results.is_empty() {
            break;
        }

        for hit in resp.results {
            if local_skip > 0 {
                local_skip -= 1;
                continue;
            }
            let Some(mut row) = transform::article::from_pubtator_search_result(&hit) else {
                continue;
            };
            if !matches_result_filters(
                &row,
                filters,
                normalized_date_from.as_deref(),
                normalized_date_to.as_deref(),
            ) {
                continue;
            }
            if !seen_pmids.insert(row.pmid.clone()) {
                continue;
            }
            row.source_local_position = source_position;
            source_position = source_position.saturating_add(1);
            out.push(row);
            if out.len() >= limit {
                break;
            }
        }
        page += 1;
    }

    Ok(SearchPage::offset(out, total))
}

fn build_free_text_article_query(filters: &ArticleSearchFilters) -> String {
    [
        filters.gene.as_deref(),
        filters.disease.as_deref(),
        filters.drug.as_deref(),
        filters.keyword.as_deref(),
        filters.author.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>()
    .join(" ")
}

async fn search_semantic_scholar_candidates(
    filters: &ArticleSearchFilters,
    limit: usize,
) -> Result<Vec<ArticleSearchResult>, BioMcpError> {
    let client = SemanticScholarClient::new()?;

    let query = build_free_text_article_query(filters);
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let (normalized_date_from, normalized_date_to) = normalized_date_bounds(filters)?;

    let response = match client.paper_search(&query, limit).await {
        Ok(response) => response,
        Err(err) => {
            warn!(?err, query, "Semantic Scholar article search leg failed");
            return Ok(Vec::new());
        }
    };

    let mut rows = Vec::with_capacity(response.data.len());
    let mut source_position = 0usize;
    for paper in response.data {
        let external_ids = paper.external_ids.as_ref();
        let title = paper
            .title
            .as_deref()
            .map(transform::article::clean_title)
            .unwrap_or_default();
        let abstract_text = paper
            .abstract_text
            .as_deref()
            .map(transform::article::clean_abstract);
        let mut row = ArticleSearchResult {
            pmid: external_ids
                .and_then(|ids| ids.pubmed.clone())
                .unwrap_or_default()
                .trim()
                .to_string(),
            pmcid: external_ids
                .and_then(|ids| ids.pmcid.clone())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            doi: external_ids
                .and_then(|ids| ids.doi.clone())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            title,
            journal: paper
                .venue
                .as_ref()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            date: paper.year.map(|year| year.to_string()),
            citation_count: paper.citation_count,
            influential_citation_count: paper.influential_citation_count,
            source: ArticleSource::SemanticScholar,
            matched_sources: vec![ArticleSource::SemanticScholar],
            score: None,
            is_retracted: None,
            abstract_snippet: abstract_text
                .as_deref()
                .and_then(transform::article::article_search_abstract_snippet),
            ranking: None,
            normalized_title: paper
                .title
                .as_deref()
                .map(transform::article::normalize_article_search_text)
                .unwrap_or_default(),
            normalized_abstract: abstract_text
                .as_deref()
                .map(transform::article::normalize_article_search_text)
                .unwrap_or_default(),
            publication_type: None,
            source_local_position: 0,
        };
        if matches_result_filters(
            &row,
            filters,
            normalized_date_from.as_deref(),
            normalized_date_to.as_deref(),
        ) {
            row.source_local_position = source_position;
            source_position = source_position.saturating_add(1);
            rows.push(row);
        }
    }

    Ok(rows)
}

async fn search_litsense2_candidates(
    filters: &ArticleSearchFilters,
    limit: usize,
) -> Result<Vec<ArticleSearchResult>, BioMcpError> {
    let query = build_free_text_article_query(filters);
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let (normalized_date_from, normalized_date_to) = normalized_date_bounds(filters)?;
    let response = LitSense2Client::new()?.sentence_search(&query).await?;

    let mut deduped: HashMap<u64, (crate::sources::litsense2::LitSense2SearchHit, usize)> =
        HashMap::new();
    for (index, hit) in response.into_iter().enumerate() {
        match deduped.get_mut(&hit.pmid) {
            Some((best, _)) if hit.score > best.score => *best = hit,
            Some(_) => {}
            None => {
                deduped.insert(hit.pmid, (hit, index));
            }
        }
    }

    let mut deduped = deduped.into_values().collect::<Vec<_>>();
    deduped.sort_by(
        |(left_hit, left_first_seen), (right_hit, right_first_seen)| {
            right_hit
                .score
                .total_cmp(&left_hit.score)
                .then_with(|| left_first_seen.cmp(right_first_seen))
        },
    );

    let pmids = deduped
        .iter()
        .map(|(hit, _)| hit.pmid.to_string())
        .collect::<Vec<_>>();
    let mut hydrated = PubMedClient::new()?
        .esummary(&pmids)
        .await?
        .into_iter()
        .filter_map(|entry| {
            transform::article::from_pubmed_esummary_entry(&entry)
                .map(|row| (row.pmid.clone(), row))
        })
        .collect::<HashMap<_, _>>();

    let mut rows = Vec::with_capacity(deduped.len());
    let mut source_position = 0usize;
    for (hit, _) in deduped {
        let pmid = hit.pmid.to_string();
        let cleaned_text = transform::article::clean_abstract(&hit.text);
        let fallback_title = if cleaned_text.is_empty() {
            format!("PMID {pmid}")
        } else {
            transform::article::article_search_fallback_title(&cleaned_text)
        };
        let mut row = hydrated
            .remove(&pmid)
            .unwrap_or_else(|| ArticleSearchResult {
                pmid: pmid.clone(),
                pmcid: hit.pmcid.clone(),
                doi: None,
                title: fallback_title.clone(),
                journal: None,
                date: None,
                citation_count: None,
                influential_citation_count: None,
                source: ArticleSource::LitSense2,
                matched_sources: vec![ArticleSource::LitSense2],
                score: Some(hit.score),
                is_retracted: None,
                abstract_snippet: transform::article::article_search_abstract_snippet(
                    &cleaned_text,
                ),
                ranking: None,
                normalized_title: transform::article::normalize_article_search_text(
                    &fallback_title,
                ),
                normalized_abstract: transform::article::normalize_article_search_text(
                    &cleaned_text,
                ),
                publication_type: None,
                source_local_position: 0,
            });
        row.source = ArticleSource::LitSense2;
        row.matched_sources = vec![ArticleSource::LitSense2];
        row.score = Some(hit.score);
        if row.pmcid.is_none() {
            row.pmcid = hit.pmcid.clone();
        }
        if row.title.trim().is_empty() {
            row.title = fallback_title.clone();
        }
        row.abstract_snippet = transform::article::article_search_abstract_snippet(&cleaned_text);
        row.normalized_title = transform::article::normalize_article_search_text(&row.title);
        row.normalized_abstract = transform::article::normalize_article_search_text(&cleaned_text);
        row.is_retracted = None;
        row.publication_type = None;
        if !matches_result_filters(
            &row,
            filters,
            normalized_date_from.as_deref(),
            normalized_date_to.as_deref(),
        ) {
            continue;
        }
        row.source_local_position = source_position;
        source_position = source_position.saturating_add(1);
        rows.push(row);
        if rows.len() >= limit {
            break;
        }
    }

    Ok(rows)
}

fn article_search_semantic_scholar_lookup_id(row: &ArticleSearchResult) -> Option<String> {
    let pmid = row.pmid.trim();
    if !pmid.is_empty() {
        return Some(format!("PMID:{pmid}"));
    }
    row.doi
        .as_deref()
        .map(str::trim)
        .filter(|doi| !doi.is_empty())
        .map(|doi| format!("DOI:{doi}"))
}

fn article_search_row_needs_semantic_scholar_enrichment(row: &ArticleSearchResult) -> bool {
    row.source != ArticleSource::SemanticScholar
        && (row.citation_count.is_none()
            || row.influential_citation_count.is_none()
            || row
                .abstract_snippet
                .as_deref()
                .is_none_or(|snippet| snippet.trim().is_empty())
            || row.normalized_abstract.trim().is_empty())
}

fn merge_semantic_scholar_search_citation(target: &mut Option<u64>, incoming: Option<u64>) {
    match (*target, incoming) {
        (None, Some(value)) | (Some(0), Some(value)) => *target = Some(value),
        _ => {}
    }
}

fn merge_article_search_row_abstract_text(row: &mut ArticleSearchResult, abstract_text: &str) {
    let cleaned_abstract = transform::article::clean_abstract(abstract_text);
    if cleaned_abstract.is_empty() {
        return;
    }

    if row
        .abstract_snippet
        .as_deref()
        .is_none_or(|snippet| snippet.trim().is_empty())
    {
        row.abstract_snippet =
            transform::article::article_search_abstract_snippet(&cleaned_abstract);
    }
    if row.normalized_abstract.trim().is_empty() {
        row.normalized_abstract =
            transform::article::normalize_article_search_text(&cleaned_abstract);
    }
}

fn merge_article_search_row_with_semantic_scholar(
    row: &mut ArticleSearchResult,
    paper: &SemanticScholarPaper,
) {
    merge_semantic_scholar_search_citation(&mut row.citation_count, paper.citation_count);
    merge_semantic_scholar_search_citation(
        &mut row.influential_citation_count,
        paper.influential_citation_count,
    );

    let Some(abstract_text) = paper.abstract_text.as_deref() else {
        return;
    };
    merge_article_search_row_abstract_text(row, abstract_text);
}

async fn enrich_article_search_rows_with_semantic_scholar(rows: &mut [ArticleSearchResult]) {
    let mut lookup_ids = Vec::new();
    let mut lookup_positions: HashMap<String, Vec<usize>> = HashMap::new();

    for (idx, row) in rows.iter().enumerate() {
        if !article_search_row_needs_semantic_scholar_enrichment(row) {
            continue;
        }
        let Some(lookup_id) = article_search_semantic_scholar_lookup_id(row) else {
            continue;
        };
        match lookup_positions.get_mut(&lookup_id) {
            Some(positions) => positions.push(idx),
            None => {
                lookup_positions.insert(lookup_id.clone(), vec![idx]);
                lookup_ids.push(lookup_id);
            }
        }
    }

    if lookup_ids.is_empty() {
        return;
    }

    let client = match SemanticScholarClient::new() {
        Ok(client) => client,
        Err(err) => {
            warn!(?err, "Semantic Scholar search-row enrichment unavailable");
            return;
        }
    };

    for (chunk_idx, chunk) in lookup_ids
        .chunks(SEMANTIC_SCHOLAR_BATCH_LOOKUP_MAX_IDS)
        .enumerate()
    {
        let chunk_start = chunk_idx * SEMANTIC_SCHOLAR_BATCH_LOOKUP_MAX_IDS;
        let chunk_end = chunk_start + chunk.len();
        match client.paper_batch_search_enrichment(chunk).await {
            Ok(papers) => {
                for (lookup_id, paper) in chunk.iter().zip(papers.into_iter()) {
                    let Some(paper) = paper else {
                        continue;
                    };
                    let Some(row_positions) = lookup_positions.get(lookup_id) else {
                        continue;
                    };
                    for row_idx in row_positions {
                        merge_article_search_row_with_semantic_scholar(&mut rows[*row_idx], &paper);
                    }
                }
            }
            Err(err) => {
                warn!(
                    ?err,
                    chunk_start,
                    chunk_end,
                    "Semantic Scholar article-search batch enrichment failed",
                );
                break;
            }
        }
    }
}

fn article_search_row_needs_visible_article_fallback(row: &ArticleSearchResult) -> bool {
    (row.source == ArticleSource::PubMed || row.matched_sources.contains(&ArticleSource::PubMed))
        && parse_pmid(&row.pmid).is_some()
        && (row.citation_count.is_none()
            || matches!(row.citation_count, Some(0))
            || row
                .abstract_snippet
                .as_deref()
                .is_none_or(|snippet| snippet.trim().is_empty())
            || row.normalized_abstract.trim().is_empty())
}

fn merge_article_search_row_with_article_base(row: &mut ArticleSearchResult, article: &Article) {
    merge_semantic_scholar_search_citation(&mut row.citation_count, article.citation_count);
    if let Some(abstract_text) = article.abstract_text.as_deref() {
        merge_article_search_row_abstract_text(row, abstract_text);
    }
}

async fn enrich_visible_article_search_rows_with_article_base(rows: &mut [ArticleSearchResult]) {
    let lookup_positions = rows
        .iter()
        .enumerate()
        .filter_map(|(idx, row)| {
            article_search_row_needs_visible_article_fallback(row)
                .then(|| parse_pmid(&row.pmid).map(|pmid| (idx, pmid)))
                .flatten()
        })
        .collect::<Vec<_>>();
    if lookup_positions.is_empty() {
        return;
    }

    let pubtator = match PubTatorClient::new() {
        Ok(client) => client,
        Err(err) => {
            warn!(?err, "PubTator visible-row metadata fallback unavailable");
            return;
        }
    };
    let europe = match EuropePmcClient::new() {
        Ok(client) => client,
        Err(err) => {
            warn!(?err, "Europe PMC visible-row metadata fallback unavailable");
            return;
        }
    };

    for (row_idx, pmid) in lookup_positions {
        let lookup_id = rows[row_idx].pmid.clone();
        match resolve_article_from_pmid(pmid, &lookup_id, &lookup_id, &pubtator, &europe, None)
            .await
        {
            Ok(article) => merge_article_search_row_with_article_base(&mut rows[row_idx], &article),
            Err(err) => warn!(
                ?err,
                pmid = lookup_id,
                "Visible article-search metadata fallback failed",
            ),
        }
    }
}

async fn enrich_and_finalize_article_candidates(
    mut rows: Vec<ArticleSearchResult>,
    limit: usize,
    offset: usize,
    total: Option<usize>,
    filters: &ArticleSearchFilters,
) -> SearchPage<ArticleSearchResult> {
    enrich_article_search_rows_with_semantic_scholar(&mut rows).await;
    let mut page = finalize_article_candidates(rows, limit, offset, total, filters);
    enrich_visible_article_search_rows_with_article_base(&mut page.results).await;
    page
}

async fn enrich_visible_article_search_page(
    mut page: SearchPage<ArticleSearchResult>,
) -> SearchPage<ArticleSearchResult> {
    enrich_article_search_rows_with_semantic_scholar(&mut page.results).await;
    enrich_visible_article_search_rows_with_article_base(&mut page.results).await;
    page
}

fn finalize_article_candidates(
    mut rows: Vec<ArticleSearchResult>,
    limit: usize,
    offset: usize,
    total: Option<usize>,
    filters: &ArticleSearchFilters,
) -> SearchPage<ArticleSearchResult> {
    for row in rows.iter_mut() {
        ensure_matched_sources(row);
    }

    let mut rows = merge_article_candidates(rows);
    rows.retain(|candidate| !candidate.row.pmid.trim().is_empty());
    rows = cap_article_candidates_by_source(rows, resolve_article_source_cap(filters, limit));
    sort_article_rows(&mut rows, filters.sort, filters);
    let mut rows = rows
        .into_iter()
        .map(|candidate| candidate.row)
        .collect::<Vec<_>>();
    rows.drain(0..offset.min(rows.len()));
    rows.truncate(limit);
    SearchPage::offset(rows, total)
}

async fn search_federated_page(
    filters: &ArticleSearchFilters,
    limit: usize,
    offset: usize,
) -> Result<SearchPage<ArticleSearchResult>, BioMcpError> {
    let fetch_count = limit.saturating_add(offset);
    if fetch_count > MAX_FEDERATED_FETCH_RESULTS {
        return Err(BioMcpError::InvalidArgument(format!(
            "--offset + --limit must be <= {MAX_FEDERATED_FETCH_RESULTS} for federated article search"
        )));
    }
    let include_pubmed = pubmed_filter_compatible(filters);
    let include_litsense2 = litsense2_search_enabled(filters, ArticleSourceFilter::All);
    let (pubtator_leg, europe_leg, pubmed_leg, semantic_scholar_leg, litsense2_leg) = tokio::join!(
        search_pubtator_page(filters, fetch_count, 0),
        search_europepmc_page(filters, fetch_count, 0),
        async {
            if include_pubmed {
                Some(search_pubmed_page(filters, fetch_count, 0).await)
            } else {
                None
            }
        },
        search_semantic_scholar_candidates(filters, fetch_count),
        async {
            if include_litsense2 {
                search_litsense2_candidates(filters, fetch_count).await
            } else {
                Ok(Vec::new())
            }
        }
    );

    let rows = collect_federated_article_rows(
        pubtator_leg,
        europe_leg,
        pubmed_leg,
        semantic_scholar_leg,
        litsense2_leg,
    )?;

    Ok(enrich_and_finalize_article_candidates(rows, limit, offset, None, filters).await)
}

#[allow(clippy::too_many_arguments)]
fn collect_federated_article_rows(
    pubtator_leg: Result<SearchPage<ArticleSearchResult>, BioMcpError>,
    europe_leg: Result<SearchPage<ArticleSearchResult>, BioMcpError>,
    pubmed_leg: Option<Result<SearchPage<ArticleSearchResult>, BioMcpError>>,
    semantic_scholar_leg: Result<Vec<ArticleSearchResult>, BioMcpError>,
    litsense2_leg: Result<Vec<ArticleSearchResult>, BioMcpError>,
) -> Result<Vec<ArticleSearchResult>, BioMcpError> {
    let semantic_scholar_rows = match semantic_scholar_leg {
        Ok(rows) => rows,
        Err(err) => {
            warn!(
                ?err,
                "Semantic Scholar search leg failed; continuing without it"
            );
            Vec::new()
        }
    };
    let litsense2_rows = match litsense2_leg {
        Ok(rows) => rows,
        Err(err) => {
            warn!(?err, "LitSense2 search leg failed; continuing without it");
            Vec::new()
        }
    };
    let pubmed_rows = match pubmed_leg {
        Some(Ok(page)) => page.results,
        Some(Err(err)) => {
            warn!(?err, "PubMed search leg failed; continuing without it");
            Vec::new()
        }
        None => Vec::new(),
    };

    match (pubtator_leg, europe_leg) {
        (Ok(pubtator_page), Ok(europe_page)) => {
            let mut merged = pubtator_page.results;
            merged.extend(europe_page.results);
            merged.extend(pubmed_rows);
            merged.extend(semantic_scholar_rows);
            merged.extend(litsense2_rows);
            Ok(merged)
        }
        (Ok(pubtator_page), Err(err)) => {
            warn!(
                ?err,
                "Europe PMC search leg failed; returning PubTator-only results"
            );
            let mut rows = pubtator_page.results;
            rows.extend(pubmed_rows);
            rows.extend(semantic_scholar_rows);
            rows.extend(litsense2_rows);
            Ok(rows)
        }
        (Err(err), Ok(europe_page)) => {
            warn!(
                ?err,
                "PubTator search leg failed; returning Europe PMC-only results"
            );
            let mut rows = europe_page.results;
            rows.extend(pubmed_rows);
            rows.extend(semantic_scholar_rows);
            rows.extend(litsense2_rows);
            Ok(rows)
        }
        (Err(pubtator_err), Err(europe_err)) => {
            warn!(?europe_err, "Europe PMC leg also failed");
            Err(pubtator_err)
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
#[allow(clippy::too_many_arguments)]
fn merge_federated_pages(
    pubtator_leg: Result<SearchPage<ArticleSearchResult>, BioMcpError>,
    europe_leg: Result<SearchPage<ArticleSearchResult>, BioMcpError>,
    pubmed_leg: Option<Result<SearchPage<ArticleSearchResult>, BioMcpError>>,
    semantic_scholar_leg: Result<Vec<ArticleSearchResult>, BioMcpError>,
    litsense2_leg: Result<Vec<ArticleSearchResult>, BioMcpError>,
    limit: usize,
    offset: usize,
    filters: &ArticleSearchFilters,
) -> Result<SearchPage<ArticleSearchResult>, BioMcpError> {
    let rows = collect_federated_article_rows(
        pubtator_leg,
        europe_leg,
        pubmed_leg,
        semantic_scholar_leg,
        litsense2_leg,
    )?;
    Ok(finalize_article_candidates(
        rows, limit, offset, None, filters,
    ))
}

async fn search_type_capable_page(
    filters: &ArticleSearchFilters,
    fetch_count: usize,
    limit: usize,
    offset: usize,
) -> Result<SearchPage<ArticleSearchResult>, BioMcpError> {
    let (europe_leg, pubmed_leg) = tokio::join!(
        search_europepmc_page(filters, fetch_count, 0),
        search_pubmed_page(filters, fetch_count, 0),
    );

    match (europe_leg, pubmed_leg) {
        (Ok(europe_page), Ok(pubmed_page)) => {
            let mut rows = europe_page.results;
            rows.extend(pubmed_page.results);
            Ok(enrich_and_finalize_article_candidates(rows, limit, offset, None, filters).await)
        }
        (Ok(europe_page), Err(err)) => {
            warn!(
                ?err,
                "PubMed type-capable leg failed; returning Europe PMC-only results"
            );
            Ok(enrich_and_finalize_article_candidates(
                europe_page.results,
                limit,
                offset,
                europe_page.total,
                filters,
            )
            .await)
        }
        (Err(err), Ok(pubmed_page)) => {
            warn!(
                ?err,
                "Europe PMC type-capable leg failed; returning PubMed-only results"
            );
            Ok(enrich_and_finalize_article_candidates(
                pubmed_page.results,
                limit,
                offset,
                pubmed_page.total,
                filters,
            )
            .await)
        }
        (Err(europe_err), Err(pubmed_err)) => {
            warn!(?pubmed_err, "PubMed type-capable leg also failed");
            Err(europe_err)
        }
    }
}

async fn search_relevance_page(
    filters: &ArticleSearchFilters,
    limit: usize,
    offset: usize,
    plan: BackendPlan,
) -> Result<SearchPage<ArticleSearchResult>, BioMcpError> {
    let fetch_count = limit.saturating_add(offset);
    if fetch_count > MAX_FEDERATED_FETCH_RESULTS {
        return Err(BioMcpError::InvalidArgument(format!(
            "--offset + --limit must be <= {MAX_FEDERATED_FETCH_RESULTS} for federated article search"
        )));
    }

    match plan {
        BackendPlan::EuropeOnly => {
            let page = search_europepmc_page(filters, fetch_count, 0).await?;
            Ok(enrich_and_finalize_article_candidates(
                page.results,
                limit,
                offset,
                page.total,
                filters,
            )
            .await)
        }
        BackendPlan::PubTatorOnly => {
            let page = search_pubtator_page(filters, fetch_count, 0).await?;
            Ok(enrich_and_finalize_article_candidates(
                page.results,
                limit,
                offset,
                page.total,
                filters,
            )
            .await)
        }
        BackendPlan::PubMedOnly => {
            let page = search_pubmed_page(filters, fetch_count, 0).await?;
            Ok(enrich_and_finalize_article_candidates(
                page.results,
                limit,
                offset,
                page.total,
                filters,
            )
            .await)
        }
        BackendPlan::LitSense2Only => {
            let rows = search_litsense2_candidates(filters, fetch_count).await?;
            Ok(enrich_and_finalize_article_candidates(rows, limit, offset, None, filters).await)
        }
        BackendPlan::TypeCapable => {
            search_type_capable_page(filters, fetch_count, limit, offset).await
        }
        BackendPlan::Both => search_federated_page(filters, limit, offset).await,
    }
}

pub async fn search_page(
    filters: &ArticleSearchFilters,
    limit: usize,
    offset: usize,
    source: ArticleSourceFilter,
) -> Result<SearchPage<ArticleSearchResult>, BioMcpError> {
    if limit == 0 || limit > MAX_SEARCH_LIMIT {
        return Err(BioMcpError::InvalidArgument(format!(
            "--limit must be between 1 and {MAX_SEARCH_LIMIT}"
        )));
    }
    validate_article_source_cap(filters, limit)?;
    validate_required_search_filters(filters)?;
    normalized_date_bounds(filters)?;
    validate_search_filter_values(filters)?;
    validate_article_ranking_options(filters)?;
    let plan = plan_backends(filters, source)?;
    if filters.sort == ArticleSort::Relevance {
        return search_relevance_page(filters, limit, offset, plan).await;
    }
    match plan {
        BackendPlan::EuropeOnly => {
            let page = search_europepmc_page(filters, limit, offset).await?;
            Ok(enrich_visible_article_search_page(page).await)
        }
        BackendPlan::PubTatorOnly => {
            let page = search_pubtator_page(filters, limit, offset).await?;
            Ok(enrich_visible_article_search_page(page).await)
        }
        BackendPlan::PubMedOnly | BackendPlan::LitSense2Only | BackendPlan::TypeCapable => {
            search_relevance_page(filters, limit, offset, plan).await
        }
        BackendPlan::Both => search_federated_page(filters, limit, offset).await,
    }
}

pub async fn get(id: &str, sections: &[String]) -> Result<Article, BioMcpError> {
    let id = id.trim();
    let section_flags = parse_sections(sections)?;
    let full_text = section_flags.include_fulltext;
    let section_only = is_section_only_request(sections, section_flags.include_all);
    let europe = EuropePmcClient::new()?;
    let mut article = get_article_base(id).await?;

    enrich_article_with_semantic_scholar(&mut article).await?;

    if section_only && !section_flags.include_annotations {
        article.annotations = None;
    }
    if section_only && !section_flags.include_tldr {
        article.semantic_scholar = None;
    }

    if full_text {
        let mut full_text_err: Option<BioMcpError> = None;
        let mut resolved_pmcid = article.pmcid.clone();

        if resolved_pmcid.is_none() {
            match NcbiIdConverterClient::new() {
                Ok(ncbi) => {
                    if let Some(pmid) = article.pmid.as_deref() {
                        match ncbi.pmid_to_pmcid(pmid).await {
                            Ok(v) => resolved_pmcid = v,
                            Err(err) => full_text_err = Some(err),
                        }
                    } else if let Some(doi) = article.doi.as_deref() {
                        match ncbi.doi_to_pmcid(doi).await {
                            Ok(v) => resolved_pmcid = v,
                            Err(err) => full_text_err = Some(err),
                        }
                    }
                }
                Err(err) => full_text_err = Some(err),
            }
        }

        if article.pmcid.is_none() {
            article.pmcid = resolved_pmcid.clone();
        }

        let mut xml: Option<String> = None;
        if let Some(pmcid) = resolved_pmcid.as_deref() {
            match europe.get_full_text_xml("PMC", pmcid).await {
                Ok(v) => xml = v,
                Err(err) => full_text_err = Some(err),
            }
        }
        if xml.is_none()
            && let Some(pmcid) = resolved_pmcid.as_deref()
        {
            match PmcOaClient::new() {
                Ok(pmc_oa) => match pmc_oa.get_full_text_xml(pmcid).await {
                    Ok(v) => xml = v,
                    Err(err) => full_text_err = Some(err),
                },
                Err(err) => full_text_err = Some(err),
            }
        }
        if xml.is_none()
            && let Some(pmid) = article.pmid.as_deref()
        {
            match europe.get_full_text_xml("MED", pmid).await {
                Ok(v) => xml = v,
                Err(err) => full_text_err = Some(err),
            }
        }

        if let Some(xml) = xml {
            let text = render_fulltext_xml(xml).await?;
            let key = article
                .pmid
                .as_deref()
                .or(article.doi.as_deref())
                .or(article.pmcid.as_deref())
                .unwrap_or(id);
            let path = download::save_atomic(&fulltext_cache_key(key), &text).await?;
            article.full_text_path = Some(path);
            article.full_text_note = None;
        } else if let Some(err) = full_text_err {
            warn!(?err, id, "Full text retrieval failed");
            article.full_text_note = Some("Full text not available: API error".into());
        } else if article.pmcid.is_none() {
            article.full_text_note =
                Some("Full text not available: Article not in PubMed Central".into());
        } else {
            article.full_text_note =
                Some("Full text not available: Full text not available from Europe PMC".into());
        }
    }

    Ok(article)
}

pub async fn citations(id: &str, limit: usize) -> Result<ArticleGraphResult, BioMcpError> {
    let client = SemanticScholarClient::new()?;
    let europe = EuropePmcClient::new()?;
    let article = resolve_semantic_scholar_seed(id, &client, &europe).await?;
    let graph_id = article
        .paper_id
        .as_deref()
        .map(str::to_string)
        .ok_or_else(|| article_not_found(id, id))?;
    let response = client.paper_citations(&graph_id, limit).await?;

    Ok(ArticleGraphResult {
        article,
        edges: response
            .data
            .into_iter()
            .map(graph_edge_from_citation)
            .collect(),
    })
}

pub async fn references(id: &str, limit: usize) -> Result<ArticleGraphResult, BioMcpError> {
    let client = SemanticScholarClient::new()?;
    let europe = EuropePmcClient::new()?;
    let article = resolve_semantic_scholar_seed(id, &client, &europe).await?;
    let graph_id = article
        .paper_id
        .as_deref()
        .map(str::to_string)
        .ok_or_else(|| article_not_found(id, id))?;
    let response = client.paper_references(&graph_id, limit).await?;

    Ok(ArticleGraphResult {
        article,
        edges: response
            .data
            .into_iter()
            .map(graph_edge_from_reference)
            .collect(),
    })
}

pub async fn recommendations(
    ids: &[String],
    negative: &[String],
    limit: usize,
) -> Result<ArticleRecommendationsResult, BioMcpError> {
    let client = SemanticScholarClient::new()?;
    let europe = EuropePmcClient::new()?;
    let positive_seeds = resolve_semantic_scholar_seeds(ids, &client, &europe).await?;
    let negative_seeds = resolve_semantic_scholar_seeds(negative, &client, &europe).await?;
    if positive_seeds.is_empty() {
        return Err(BioMcpError::InvalidArgument(
            "At least one positive article seed is required. Example: biomcp article recommendations 22663011".into(),
        ));
    }

    let positive_ids: Vec<String> = positive_seeds
        .iter()
        .filter_map(|paper| paper.paper_id.clone())
        .collect();
    let negative_ids: Vec<String> = negative_seeds
        .iter()
        .filter_map(|paper| paper.paper_id.clone())
        .collect();
    let positive_set: HashSet<&str> = positive_ids.iter().map(String::as_str).collect();
    if let Some(conflict) = negative_ids
        .iter()
        .map(String::as_str)
        .find(|paper_id| positive_set.contains(paper_id))
    {
        return Err(BioMcpError::InvalidArgument(format!(
            "The same paper cannot appear in both positive and negative recommendation seeds ({conflict})"
        )));
    }

    let response = if positive_ids.len() == 1 && negative_ids.is_empty() {
        client
            .recommendations_for_paper(&positive_ids[0], limit)
            .await?
    } else {
        client
            .recommendations(&positive_ids, &negative_ids, limit)
            .await?
    };

    Ok(ArticleRecommendationsResult {
        positive_seeds,
        negative_seeds,
        recommendations: response
            .recommended_papers
            .into_iter()
            .map(|paper| related_paper_from_semantic_scholar(&paper))
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_string_contains, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn lock_env() -> tokio::sync::MutexGuard<'static, ()> {
        crate::test_support::env_lock().lock().await
    }

    struct EnvVarGuard {
        name: &'static str,
        previous: Option<String>,
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            // Safety: tests serialize environment mutation with `lock_env()`.
            unsafe {
                match &self.previous {
                    Some(value) => std::env::set_var(self.name, value),
                    None => std::env::remove_var(self.name),
                }
            }
        }
    }

    fn set_env_var(name: &'static str, value: Option<&str>) -> EnvVarGuard {
        let previous = std::env::var(name).ok();
        // Safety: tests serialize environment mutation with `lock_env()`.
        unsafe {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
        EnvVarGuard { name, previous }
    }

    fn empty_filters() -> ArticleSearchFilters {
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
            no_preprints: false,
            exclude_retracted: false,
            max_per_source: None,
            sort: ArticleSort::Relevance,
            ranking: ArticleRankingOptions::default(),
        }
    }

    #[test]
    fn article_sort_default_is_relevance() {
        let default: ArticleSort = Default::default();
        assert_eq!(default, ArticleSort::Relevance);
    }

    #[test]
    fn pubtator_sort_omits_param_for_relevance() {
        assert_eq!(pubtator_sort(ArticleSort::Relevance), None);
    }

    #[test]
    fn pubtator_sort_sends_param_for_date() {
        assert_eq!(pubtator_sort(ArticleSort::Date), Some("date desc"));
    }

    #[test]
    fn empty_filters_default_sort_is_relevance() {
        let filters = empty_filters();
        assert_eq!(filters.sort, ArticleSort::Relevance);
    }

    #[test]
    fn article_section_names_include_tldr() {
        assert!(ARTICLE_SECTION_NAMES.contains(&"tldr"));
    }

    #[test]
    fn fulltext_cache_key_is_versioned() {
        let key = fulltext_cache_key("22663011");
        assert!(key.starts_with("article-fulltext-jats-v1:"));
        assert!(key.ends_with("22663011"));
    }

    #[test]
    fn parse_sections_supports_tldr_and_all() {
        let tldr_only = parse_sections(&["tldr".to_string()]).expect("tldr should parse");
        assert!(tldr_only.include_tldr);
        assert!(!tldr_only.include_annotations);
        assert!(!tldr_only.include_fulltext);

        let all = parse_sections(&["all".to_string()]).expect("all should parse");
        assert!(all.include_tldr);
        assert!(all.include_annotations);
        assert!(all.include_fulltext);
    }

    #[test]
    fn semantic_scholar_lookup_id_supports_arxiv_and_paper_ids() {
        assert_eq!(
            semantic_scholar_lookup_id("arXiv:2401.01234"),
            Some("ARXIV:2401.01234".to_string())
        );
        assert_eq!(
            semantic_scholar_lookup_id("0123456789abcdef0123456789abcdef01234567"),
            Some("0123456789abcdef0123456789abcdef01234567".to_string())
        );
    }

    #[test]
    fn is_doi_basic() {
        assert!(is_doi("10.1056/NEJMoa1203421"));
        assert!(is_doi("10.1056/nejmoa1203421"));
        assert!(!is_doi("22663011"));
        assert!(!is_doi("doi:10.1056/NEJMoa1203421"));
    }

    #[test]
    fn parse_pmid_basic() {
        assert_eq!(parse_pmid("22663011"), Some(22663011));
        assert_eq!(parse_pmid(" 22663011 "), Some(22663011));
        assert_eq!(parse_pmid(""), None);
        assert_eq!(parse_pmid("10.1056/NEJMoa1203421"), None);
        assert_eq!(parse_pmid("abc"), None);
    }

    #[test]
    fn parse_pmcid_basic() {
        assert_eq!(parse_pmcid("PMC9984800"), Some("PMC9984800".into()));
        assert_eq!(parse_pmcid("pmc9984800"), Some("PMC9984800".into()));
        assert_eq!(parse_pmcid("PMCID:PMC9984800"), Some("PMC9984800".into()));
        assert_eq!(parse_pmcid(" PMC9984800 "), Some("PMC9984800".into()));
        assert_eq!(parse_pmcid("PMC"), None);
        assert_eq!(parse_pmcid("PMCX"), None);
        assert_eq!(parse_pmcid("PMC-123"), None);
        assert_eq!(parse_pmcid("22663011"), None);
    }

    #[test]
    fn parse_article_id_basic() {
        match parse_article_id("PMC9984800") {
            ArticleIdType::Pmc(v) => assert_eq!(v, "PMC9984800"),
            _ => panic!("expected PMCID"),
        }
        match parse_article_id("10.1056/NEJMoa1203421") {
            ArticleIdType::Doi(v) => assert_eq!(v, "10.1056/NEJMoa1203421"),
            _ => panic!("expected DOI"),
        }
        match parse_article_id("22663011") {
            ArticleIdType::Pmid(v) => assert_eq!(v, 22663011),
            _ => panic!("expected PMID"),
        }
        assert!(matches!(
            parse_article_id("doi:10.1056/NEJMoa1203421"),
            ArticleIdType::Invalid
        ));
    }

    #[test]
    fn parse_article_id_publisher_pii_is_invalid() {
        assert!(matches!(
            parse_article_id("S1535610826000103"),
            ArticleIdType::Invalid
        ));
    }

    #[test]
    fn article_error_copy_and_warn_threshold_match_contract() {
        assert_eq!(WARN_PAGE_THRESHOLD, 20);
        assert_eq!(
            INVALID_ARTICLE_ID_MSG,
            "Unsupported identifier format. BioMCP resolves PMID (digits only, e.g., 22663011), PMCID (starts with PMC, e.g., PMC9984800), and DOI (starts with 10., e.g., 10.1056/NEJMoa1203421). publisher PIIs (e.g., S1535610826000103) are not indexed by PubMed or Europe PMC and cannot be resolved."
        );
    }

    #[test]
    fn invalid_article_id_error_names_supported_types_and_publisher_limit() {
        assert!(INVALID_ARTICLE_ID_MSG.contains("PMID"));
        assert!(INVALID_ARTICLE_ID_MSG.contains("PMCID"));
        assert!(INVALID_ARTICLE_ID_MSG.contains("DOI"));
        assert!(
            INVALID_ARTICLE_ID_MSG.contains("PII") || INVALID_ARTICLE_ID_MSG.contains("publisher")
        );
    }

    #[test]
    fn europepmc_keyword_does_not_quote_whitespace() {
        let term = europepmc_keyword("large language model clinical trials");
        assert_eq!(term, "large language model clinical trials");
    }

    #[test]
    fn build_search_query_keeps_phrase_quoting_for_entity_filters() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF V600E".into());
        filters.author = Some("Jane Doe".into());

        let query = build_search_query(&filters).expect("query should build");
        assert!(query.contains("\"BRAF V600E\""));
        assert!(query.contains("AUTH:\"Jane Doe\""));
    }

    #[test]
    fn build_search_query_uses_gene_anchor_field_when_requested() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());
        filters.gene_anchored = true;
        let query = build_search_query(&filters).expect("query should build");
        assert!(query.contains("GENE_PROTEIN:BRAF"));
    }

    #[test]
    fn build_search_query_combines_keyword_and_since() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());
        filters.keyword = Some("large language model".into());
        filters.date_from = Some("2024-01-01".into());
        filters.no_preprints = true;

        let query = build_search_query(&filters).expect("query should build");
        assert!(query.contains("BRAF"));
        assert!(query.contains("large language model"));
        assert!(query.contains("FIRST_PDATE:[2024-01-01 TO *]"));
        assert!(query.contains("NOT SRC:PPR"));
    }

    #[test]
    fn build_search_query_excludes_retracted_when_requested() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());
        filters.exclude_retracted = true;
        let query = build_search_query(&filters).expect("query should build");
        assert!(query.contains("NOT PUB_TYPE:\"retracted publication\""));
    }

    #[test]
    fn normalized_date_bounds_normalizes_partial_dates() {
        let mut filters = empty_filters();
        filters.date_from = Some("2020".into());
        filters.date_to = Some("2024-12".into());

        let (date_from, date_to) =
            normalized_date_bounds(&filters).expect("partial dates should normalize");

        assert_eq!(date_from.as_deref(), Some("2020-01-01"));
        assert_eq!(date_to.as_deref(), Some("2024-12-01"));
    }

    #[test]
    fn normalized_date_bounds_rejects_bad_month() {
        let mut filters = empty_filters();
        filters.date_from = Some("2024-13-01".into());

        let err = normalized_date_bounds(&filters).expect_err("invalid month should fail");

        assert_eq!(
            err.to_string(),
            "Invalid argument: Invalid month 13 in --date-from (must be 01-12)"
        );
    }

    #[test]
    fn normalized_date_bounds_rejects_bad_date_to_with_flag_name() {
        let mut filters = empty_filters();
        filters.date_to = Some("2024-99".into());

        let err = normalized_date_bounds(&filters).expect_err("invalid date-to should fail");

        assert_eq!(
            err.to_string(),
            "Invalid argument: Invalid month 99 in --date-to (must be 01-12)"
        );
    }

    #[test]
    fn normalized_date_bounds_rejects_inverted_range() {
        let mut filters = empty_filters();
        filters.date_from = Some("2024-06-01".into());
        filters.date_to = Some("2020-01-01".into());

        let err = normalized_date_bounds(&filters).expect_err("inverted range should fail");

        assert_eq!(
            err.to_string(),
            "Invalid argument: --date-from must be <= --date-to"
        );
    }

    #[test]
    fn normalize_article_type_accepts_aliases() {
        assert_eq!(
            normalize_article_type("review").expect("review should normalize"),
            "review"
        );
        assert_eq!(
            normalize_article_type("research").expect("research alias should normalize"),
            "research-article"
        );
        assert_eq!(
            normalize_article_type("research-article").expect("research-article should normalize"),
            "research-article"
        );
        assert_eq!(
            normalize_article_type("case-reports").expect("case-reports should normalize"),
            "case-reports"
        );
        assert_eq!(
            normalize_article_type("metaanalysis").expect("metaanalysis alias should normalize"),
            "meta-analysis"
        );
    }

    #[test]
    fn build_search_query_rejects_unknown_article_type() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());
        filters.article_type = Some("invalid".into());

        let err = build_search_query(&filters).expect_err("invalid article type should fail");
        let msg = err.to_string();
        assert!(msg.contains("Invalid argument"));
        assert!(msg.contains("case-reports"));
    }

    #[tokio::test]
    async fn search_page_rejects_unknown_article_type_before_backend_planning() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());
        filters.article_type = Some("invalid".into());

        let err = search_page(&filters, 1, 0, ArticleSourceFilter::PubTator)
            .await
            .expect_err("invalid article type should fail before planner-specific errors");

        assert_eq!(
            err.to_string(),
            "Invalid argument: --type must be one of: review, research, research-article, case-reports, meta-analysis"
        );
    }

    #[tokio::test]
    async fn search_page_rejects_max_per_source_above_limit_before_backend_planning() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());
        filters.open_access = true;
        filters.max_per_source = Some(11);

        let err = search_page(&filters, 10, 0, ArticleSourceFilter::PubTator)
            .await
            .expect_err("invalid max-per-source should fail before planner-specific errors");

        assert_eq!(
            err.to_string(),
            "Invalid argument: --max-per-source must be <= --limit"
        );
    }

    #[test]
    fn article_sort_parses_supported_values() {
        assert_eq!(
            ArticleSort::from_flag("date").expect("date should parse"),
            ArticleSort::Date
        );
        assert_eq!(
            ArticleSort::from_flag("citations").expect("citations should parse"),
            ArticleSort::Citations
        );
        assert_eq!(
            ArticleSort::from_flag("relevance").expect("relevance should parse"),
            ArticleSort::Relevance
        );
        assert!(ArticleSort::from_flag("newest").is_err());
    }

    #[test]
    fn article_ranking_mode_parses_supported_values() {
        assert_eq!(
            ArticleRankingMode::from_flag("lexical").expect("lexical should parse"),
            ArticleRankingMode::Lexical
        );
        assert_eq!(
            ArticleRankingMode::from_flag("semantic").expect("semantic should parse"),
            ArticleRankingMode::Semantic
        );
        assert_eq!(
            ArticleRankingMode::from_flag("hybrid").expect("hybrid should parse"),
            ArticleRankingMode::Hybrid
        );
        assert!(ArticleRankingMode::from_flag("auto").is_err());
    }

    #[test]
    fn default_ranking_mode_depends_on_keyword_presence() {
        let mut keyword_filters = empty_filters();
        keyword_filters.keyword = Some("melanoma".into());
        assert_eq!(
            resolve_article_ranking(&keyword_filters).mode,
            ArticleRankingMode::Hybrid
        );

        let mut entity_filters = empty_filters();
        entity_filters.gene = Some("BRAF".into());
        assert_eq!(
            resolve_article_ranking(&entity_filters).mode,
            ArticleRankingMode::Lexical
        );
    }

    #[test]
    fn article_relevance_ranking_policy_formats_modes() {
        let mut lexical_filters = empty_filters();
        lexical_filters.gene = Some("BRAF".into());
        assert_eq!(
            article_effective_ranking_mode(&lexical_filters),
            Some(ArticleRankingMode::Lexical)
        );
        assert_eq!(
            article_relevance_ranking_policy(&lexical_filters).as_deref(),
            Some(ARTICLE_RELEVANCE_RANKING_POLICY)
        );

        let mut semantic_filters = empty_filters();
        semantic_filters.keyword = Some("melanoma".into());
        semantic_filters.ranking.requested_mode = Some(ArticleRankingMode::Semantic);
        assert_eq!(
            article_effective_ranking_mode(&semantic_filters),
            Some(ArticleRankingMode::Semantic)
        );
        assert_eq!(
            article_relevance_ranking_policy(&semantic_filters).as_deref(),
            Some(ARTICLE_SEMANTIC_RANKING_POLICY)
        );

        let mut hybrid_filters = empty_filters();
        hybrid_filters.keyword = Some("melanoma".into());
        hybrid_filters.ranking = ArticleRankingOptions::from_inputs(
            Some("hybrid"),
            Some(0.5),
            Some(0.25),
            Some(0.2),
            Some(0.05),
        )
        .expect("hybrid options should parse");
        assert_eq!(
            article_effective_ranking_mode(&hybrid_filters),
            Some(ArticleRankingMode::Hybrid)
        );
        assert_eq!(
            article_relevance_ranking_policy(&hybrid_filters).as_deref(),
            Some(
                "hybrid relevance (score = 0.5*semantic + 0.25*lexical + 0.2*citations + 0.05*position)"
            )
        );
    }

    #[test]
    fn search_article_ranking_flags_validate_cleanly() {
        let mut non_relevance = empty_filters();
        non_relevance.gene = Some("BRAF".into());
        non_relevance.sort = ArticleSort::Date;
        non_relevance.ranking.requested_mode = Some(ArticleRankingMode::Hybrid);
        let err = validate_article_ranking_options(&non_relevance)
            .expect_err("ranking mode should be rejected outside relevance sort");
        assert_eq!(
            err.to_string(),
            "Invalid argument: --ranking-mode and --weight-* require --sort relevance"
        );

        let mut lexical_weights = empty_filters();
        lexical_weights.keyword = Some("melanoma".into());
        lexical_weights.ranking =
            ArticleRankingOptions::from_inputs(Some("lexical"), Some(0.5), None, None, None)
                .expect("options should parse");
        let err = validate_article_ranking_options(&lexical_weights)
            .expect_err("weights should require hybrid mode");
        assert_eq!(
            err.to_string(),
            "Invalid argument: --weight-* flags require --ranking-mode hybrid or no explicit ranking mode"
        );

        let mut entity_default_weights = empty_filters();
        entity_default_weights.gene = Some("BRAF".into());
        entity_default_weights.ranking =
            ArticleRankingOptions::from_inputs(None, Some(0.5), None, None, None)
                .expect("options should parse");
        let err = validate_article_ranking_options(&entity_default_weights)
            .expect_err("entity-only default lexical mode should reject weights");
        assert_eq!(
            err.to_string(),
            "Invalid argument: --weight-* flags require --ranking-mode hybrid or no explicit ranking mode"
        );

        let mut zero_weights = empty_filters();
        zero_weights.keyword = Some("melanoma".into());
        zero_weights.ranking = ArticleRankingOptions::from_inputs(
            Some("hybrid"),
            Some(0.0),
            Some(0.0),
            Some(0.0),
            Some(0.0),
        )
        .expect("options should parse");
        let err = validate_article_ranking_options(&zero_weights)
            .expect_err("hybrid weights must not all be zero");
        assert_eq!(
            err.to_string(),
            "Invalid argument: At least one hybrid ranking weight must be > 0"
        );

        let mut negative_weight = empty_filters();
        negative_weight.keyword = Some("melanoma".into());
        negative_weight.ranking =
            ArticleRankingOptions::from_inputs(Some("hybrid"), Some(-0.1), None, None, None)
                .expect("options should parse");
        let err = validate_article_ranking_options(&negative_weight)
            .expect_err("negative weights should fail validation");
        assert_eq!(
            err.to_string(),
            "Invalid argument: --weight-semantic must be >= 0"
        );

        let mut invalid_weight = empty_filters();
        invalid_weight.keyword = Some("melanoma".into());
        invalid_weight.ranking =
            ArticleRankingOptions::from_inputs(Some("hybrid"), Some(f64::NAN), None, None, None)
                .expect("options should parse");
        let err = validate_article_ranking_options(&invalid_weight)
            .expect_err("non-finite weights should fail validation");
        assert_eq!(
            err.to_string(),
            "Invalid argument: --weight-semantic must be finite"
        );
    }

    #[test]
    fn article_source_filter_parses_supported_values() {
        assert_eq!(
            ArticleSourceFilter::from_flag("all").expect("all should parse"),
            ArticleSourceFilter::All
        );
        assert_eq!(
            ArticleSourceFilter::from_flag("pubtator").expect("pubtator should parse"),
            ArticleSourceFilter::PubTator
        );
        assert_eq!(
            ArticleSourceFilter::from_flag("europepmc").expect("europepmc should parse"),
            ArticleSourceFilter::EuropePmc
        );
        assert_eq!(
            ArticleSourceFilter::from_flag("pubmed").expect("pubmed should parse"),
            ArticleSourceFilter::PubMed
        );
        assert!(
            ArticleSourceFilter::from_flag("litsense2").is_ok(),
            "litsense2 should parse"
        );
    }

    #[test]
    fn planner_routes_all_to_europepmc_for_strict_filters() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());
        filters.open_access = true;

        let plan = plan_backends(&filters, ArticleSourceFilter::All).expect("planner should work");
        assert!(matches!(plan, BackendPlan::EuropeOnly));
    }

    #[test]
    fn planner_routes_pubmed_to_pubmed_only() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());

        let plan =
            plan_backends(&filters, ArticleSourceFilter::PubMed).expect("planner should work");
        assert!(matches!(plan, BackendPlan::PubMedOnly));
    }

    #[test]
    fn planner_routes_litsense2_to_litsense2_only() {
        let mut filters = empty_filters();
        filters.keyword = Some("Hirschsprung disease".into());

        let plan =
            plan_backends(&filters, ArticleSourceFilter::LitSense2).expect("planner should work");
        assert!(matches!(plan, BackendPlan::LitSense2Only));
    }

    #[test]
    fn build_free_text_article_query_preserves_mixed_semantic_anchors() {
        let mut filters = empty_filters();
        filters.gene = Some(" RET ".into());
        filters.disease = Some(" Hirschsprung disease ".into());
        filters.drug = Some(" selpercatinib ".into());
        filters.keyword = Some(" ganglion cells ".into());
        filters.author = Some(" Alice Smith ".into());

        let query = build_free_text_article_query(&filters);

        assert_eq!(
            query,
            "RET Hirschsprung disease selpercatinib ganglion cells Alice Smith"
        );
    }

    #[test]
    fn planner_routes_all_with_type_to_type_capable() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());
        filters.article_type = Some("review".into());

        let plan = plan_backends(&filters, ArticleSourceFilter::All).expect("planner should work");
        assert!(matches!(plan, BackendPlan::TypeCapable));
    }

    #[test]
    fn planner_routes_all_with_type_and_no_preprints_to_europe_only() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());
        filters.article_type = Some("review".into());
        filters.no_preprints = true;

        let plan = plan_backends(&filters, ArticleSourceFilter::All).expect("planner should work");
        assert!(matches!(plan, BackendPlan::EuropeOnly));
    }

    #[test]
    fn planner_rejects_pubtator_type_with_pubmed_compatible_filters_and_suggests_supported_routes()
    {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());
        filters.article_type = Some("review".into());

        let err = plan_backends(&filters, ArticleSourceFilter::PubTator)
            .expect_err("planner should reject strict-only filter on pubtator");
        let msg = err.to_string();
        assert!(msg.contains("--type"));
        assert!(msg.contains("--source europepmc"));
        assert!(msg.contains("--source pubmed"));
        assert!(msg.contains("remove --type"));
    }

    #[test]
    fn planner_rejects_pubtator_open_access_without_suggesting_pubmed() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());
        filters.open_access = true;

        let err = plan_backends(&filters, ArticleSourceFilter::PubTator)
            .expect_err("planner should reject open-access on pubtator");
        let msg = err.to_string();
        assert!(msg.contains("--open-access"));
        assert!(msg.contains("--source europepmc"));
        assert!(!msg.contains("--source pubmed"));
    }

    #[test]
    fn planner_rejects_pubtator_type_and_no_preprints_without_suggesting_pubmed() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());
        filters.article_type = Some("review".into());
        filters.no_preprints = true;

        let err = plan_backends(&filters, ArticleSourceFilter::PubTator)
            .expect_err("planner should reject incompatible type/no-preprints mix");
        let msg = err.to_string();
        assert!(msg.contains("--source europepmc"));
        assert!(msg.contains("--no-preprints"));
        assert!(!msg.contains("--source pubmed"));
    }

    #[test]
    fn planner_rejects_litsense2_without_keyword() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());

        let err = plan_backends(&filters, ArticleSourceFilter::LitSense2)
            .expect_err("planner should reject keyword-less litsense2 source");
        let msg = err.to_string();
        assert!(msg.contains("--source litsense2"));
        assert!(msg.contains("keyword"));
    }

    #[test]
    fn planner_rejects_litsense2_type_filter() {
        let mut filters = empty_filters();
        filters.keyword = Some("melanoma".into());
        filters.article_type = Some("review".into());

        let err = plan_backends(&filters, ArticleSourceFilter::LitSense2)
            .expect_err("planner should reject --type for litsense2");
        let msg = err.to_string();
        assert!(msg.contains("--source litsense2"));
        assert!(msg.contains("--type"));
    }

    #[test]
    fn planner_rejects_litsense2_open_access_filter() {
        let mut filters = empty_filters();
        filters.keyword = Some("melanoma".into());
        filters.open_access = true;

        let err = plan_backends(&filters, ArticleSourceFilter::LitSense2)
            .expect_err("planner should reject --open-access for litsense2");
        let msg = err.to_string();
        assert!(msg.contains("--source litsense2"));
        assert!(msg.contains("--open-access"));
    }

    #[test]
    fn build_pubmed_esearch_params_reuses_article_type_aliases() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());
        filters.keyword = Some("melanoma".into());
        filters.author = Some("Alice Smith".into());
        filters.journal = Some("Nature".into());
        filters.article_type = Some("research".into());
        filters.date_from = Some("2020".into());
        filters.date_to = Some("2024-12".into());
        filters.exclude_retracted = true;

        let params =
            build_pubmed_esearch_params(&filters, 5, 10).expect("pubmed params should build");

        assert_eq!(
            params.term,
            "BRAF AND melanoma AND \"Alice Smith\"[author] AND \"Nature\"[journal] AND journal article[pt] NOT retracted publication[pt]"
        );
        assert_eq!(params.retstart, 10);
        assert_eq!(params.retmax, 5);
        assert_eq!(params.date_from.as_deref(), Some("2020-01-01"));
        assert_eq!(params.date_to.as_deref(), Some("2024-12-01"));
    }

    #[test]
    fn build_pubmed_search_term_uses_standalone_not_for_retraction_filter() {
        let mut filters = empty_filters();
        filters.gene = Some("WDR5".into());
        filters.exclude_retracted = true;

        let term = build_pubmed_search_term(&filters).expect("pubmed term should build");

        assert_eq!(term, "WDR5 NOT retracted publication[pt]");
        assert!(
            !term.contains("AND NOT"),
            "term must not contain 'AND NOT': {term:?}"
        );
    }

    #[test]
    fn build_pubmed_esearch_params_allows_federated_windows_above_user_limit() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());

        let params =
            build_pubmed_esearch_params(&filters, 75, 0).expect("pubmed params should build");

        assert_eq!(params.retmax, 75);
        assert_eq!(params.retstart, 0);
    }

    #[test]
    fn article_source_pubmed_display_name() {
        assert_eq!(ArticleSource::PubMed.display_name(), "PubMed");
    }

    #[test]
    fn article_source_litsense2_display_name() {
        assert_eq!(ArticleSource::LitSense2.display_name(), "LitSense2");
    }

    #[test]
    fn article_source_pubmed_priority() {
        assert_eq!(article_source_priority(ArticleSource::PubMed), 2);
    }

    #[test]
    fn article_source_litsense2_priority() {
        assert_eq!(article_source_priority(ArticleSource::LitSense2), 4);
    }

    #[test]
    fn build_pubmed_esearch_params_rejects_open_access() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());
        filters.open_access = true;

        let err = build_pubmed_esearch_params(&filters, 5, 0)
            .expect_err("open-access should be rejected for PubMed builder");

        assert!(err.to_string().contains("--open-access"));
        assert!(err.to_string().contains("PubMed"));
    }

    #[test]
    fn build_pubmed_esearch_params_rejects_no_preprints() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());
        filters.no_preprints = true;

        let err = build_pubmed_esearch_params(&filters, 5, 0)
            .expect_err("no-preprints should be rejected for PubMed builder");

        assert!(err.to_string().contains("--no-preprints"));
        assert!(err.to_string().contains("PubMed"));
    }

    #[test]
    fn build_pubmed_esearch_params_rejects_federated_window_overflow() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());

        let err = build_pubmed_esearch_params(&filters, 1, MAX_FEDERATED_FETCH_RESULTS)
            .expect_err("offset + limit overflow should be rejected");

        assert!(
            err.to_string()
                .contains("--offset + --limit must be <= 1250 for federated article search")
        );
    }

    #[tokio::test]
    async fn search_pubmed_page_rejects_open_access() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());
        filters.open_access = true;

        let err = search_pubmed_page(&filters, 5, 0)
            .await
            .expect_err("open-access should be rejected for PubMed page helper");

        assert!(err.to_string().contains("--open-access"));
        assert!(err.to_string().contains("PubMed"));
    }

    #[tokio::test]
    async fn search_pubmed_page_rejects_no_preprints() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());
        filters.no_preprints = true;

        let err = search_pubmed_page(&filters, 5, 0)
            .await
            .expect_err("no-preprints should be rejected for PubMed page helper");

        assert!(err.to_string().contains("--no-preprints"));
        assert!(err.to_string().contains("PubMed"));
    }

    #[tokio::test]
    async fn search_pubmed_page_sends_standalone_not_retraction_term() {
        let _guard = lock_env().await;
        let server = MockServer::start().await;
        let _pubmed_base = set_env_var("BIOMCP_PUBMED_BASE", Some(&server.uri()));
        let mut filters = empty_filters();
        filters.gene = Some("WDR5".into());
        filters.exclude_retracted = true;

        Mock::given(method("GET"))
            .and(path("/esearch.fcgi"))
            .and(query_param("db", "pubmed"))
            .and(query_param("retmode", "json"))
            .and(query_param("retstart", "0"))
            .and(query_param("retmax", "100"))
            .and(query_param("term", "WDR5 NOT retracted publication[pt]"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "esearchresult": {
                    "count": "0",
                    "idlist": []
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let page = search_pubmed_page(&filters, 1, 0)
            .await
            .expect("pubmed page should accept standalone NOT query");

        assert!(page.results.is_empty());
        assert_eq!(page.total, Some(0));
    }

    #[tokio::test]
    async fn search_pubmed_page_refills_across_batches_after_filtering() {
        let _guard = lock_env().await;
        let server = MockServer::start().await;
        let _pubmed_base = set_env_var("BIOMCP_PUBMED_BASE", Some(&server.uri()));
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());
        filters.journal = Some("Nature".into());

        Mock::given(method("GET"))
            .and(path("/esearch.fcgi"))
            .and(query_param("db", "pubmed"))
            .and(query_param("retmode", "json"))
            .and(query_param("retstart", "0"))
            .and(query_param("retmax", "100"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "esearchresult": {
                    "count": "4",
                    "idlist": ["1", "2"]
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/esummary.fcgi"))
            .and(query_param("db", "pubmed"))
            .and(query_param("retmode", "json"))
            .and(query_param("id", "1,2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "result": {
                    "uids": ["1", "2"],
                    "1": {
                        "uid": "1",
                        "title": "Filtered title",
                        "sortpubdate": "2024/01/01 00:00",
                        "fulljournalname": "Other Journal",
                        "source": "Other J"
                    },
                    "2": {
                        "uid": "2",
                        "title": "First visible title",
                        "sortpubdate": "2024/01/02 00:00",
                        "fulljournalname": "Nature",
                        "source": "Nature"
                    }
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/esearch.fcgi"))
            .and(query_param("db", "pubmed"))
            .and(query_param("retmode", "json"))
            .and(query_param("retstart", "2"))
            .and(query_param("retmax", "100"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "esearchresult": {
                    "count": "4",
                    "idlist": ["3", "4"]
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/esummary.fcgi"))
            .and(query_param("db", "pubmed"))
            .and(query_param("retmode", "json"))
            .and(query_param("id", "3,4"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "result": {
                    "uids": ["3", "4"],
                    "3": {
                        "uid": "3",
                        "title": "Second visible title",
                        "sortpubdate": "2024/01/03 00:00",
                        "fulljournalname": "Nature",
                        "source": "Nature"
                    },
                    "4": {
                        "uid": "4",
                        "title": "Third visible title",
                        "sortpubdate": "2024/01/04 00:00",
                        "fulljournalname": "Nature",
                        "source": "Nature"
                    }
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let page = search_pubmed_page(&filters, 2, 0)
            .await
            .expect("pubmed page should fill visible results");

        assert_eq!(page.total, Some(4));
        assert_eq!(page.results.len(), 2);
        assert_eq!(page.results[0].pmid, "2");
        assert_eq!(page.results[1].pmid, "3");
    }

    #[tokio::test]
    async fn search_pubmed_page_applies_offset_after_filtering() {
        let _guard = lock_env().await;
        let server = MockServer::start().await;
        let _pubmed_base = set_env_var("BIOMCP_PUBMED_BASE", Some(&server.uri()));
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());
        filters.journal = Some("Nature".into());

        Mock::given(method("GET"))
            .and(path("/esearch.fcgi"))
            .and(query_param("db", "pubmed"))
            .and(query_param("retmode", "json"))
            .and(query_param("retstart", "0"))
            .and(query_param("retmax", "100"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "esearchresult": {
                    "count": "4",
                    "idlist": ["1", "2"]
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/esummary.fcgi"))
            .and(query_param("db", "pubmed"))
            .and(query_param("retmode", "json"))
            .and(query_param("id", "1,2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "result": {
                    "uids": ["1", "2"],
                    "1": {
                        "uid": "1",
                        "title": "Filtered title",
                        "sortpubdate": "2024/01/01 00:00",
                        "fulljournalname": "Other Journal",
                        "source": "Other J"
                    },
                    "2": {
                        "uid": "2",
                        "title": "First visible title",
                        "sortpubdate": "2024/01/02 00:00",
                        "fulljournalname": "Nature",
                        "source": "Nature"
                    }
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/esearch.fcgi"))
            .and(query_param("db", "pubmed"))
            .and(query_param("retmode", "json"))
            .and(query_param("retstart", "2"))
            .and(query_param("retmax", "100"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "esearchresult": {
                    "count": "4",
                    "idlist": ["3", "4"]
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/esummary.fcgi"))
            .and(query_param("db", "pubmed"))
            .and(query_param("retmode", "json"))
            .and(query_param("id", "3,4"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "result": {
                    "uids": ["3", "4"],
                    "3": {
                        "uid": "3",
                        "title": "Second visible title",
                        "sortpubdate": "2024/01/03 00:00",
                        "fulljournalname": "Nature",
                        "source": "Nature"
                    },
                    "4": {
                        "uid": "4",
                        "title": "Third visible title",
                        "sortpubdate": "2024/01/04 00:00",
                        "fulljournalname": "Nature",
                        "source": "Nature"
                    }
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let page = search_pubmed_page(&filters, 2, 1)
            .await
            .expect("offset should apply after filtering");

        assert_eq!(page.total, Some(4));
        assert_eq!(page.results.len(), 2);
        assert_eq!(page.results[0].pmid, "3");
        assert_eq!(page.results[1].pmid, "4");
        assert_eq!(page.results[0].source_local_position, 1);
        assert_eq!(page.results[1].source_local_position, 2);
    }

    #[tokio::test]
    async fn search_pubmed_page_hard_fails_on_blank_title() {
        let _guard = lock_env().await;
        let server = MockServer::start().await;
        let _pubmed_base = set_env_var("BIOMCP_PUBMED_BASE", Some(&server.uri()));
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());

        Mock::given(method("GET"))
            .and(path("/esearch.fcgi"))
            .and(query_param("db", "pubmed"))
            .and(query_param("retmode", "json"))
            .and(query_param("retstart", "0"))
            .and(query_param("retmax", "100"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "esearchresult": {
                    "count": "1",
                    "idlist": ["1"]
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/esummary.fcgi"))
            .and(query_param("db", "pubmed"))
            .and(query_param("retmode", "json"))
            .and(query_param("id", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "result": {
                    "uids": ["1"],
                    "1": {
                        "uid": "1",
                        "title": "   ",
                        "sortpubdate": "2024/01/01 00:00",
                        "fulljournalname": "Nature",
                        "source": "Nature"
                    }
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let err = search_pubmed_page(&filters, 1, 0)
            .await
            .expect_err("blank title should be a contract error");

        let msg = err.to_string();
        assert!(msg.contains("pubmed-eutils"));
        assert!(msg.contains("1"));
        assert!(msg.contains("title"));
    }

    #[test]
    fn pubmed_only_routes_use_common_finalizer_for_sorting() {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        runtime.block_on(async {
            let _guard = lock_env().await;
            let server = MockServer::start().await;
            let _pubmed_base = set_env_var("BIOMCP_PUBMED_BASE", Some(&server.uri()));
            let mut filters = empty_filters();
            filters.gene = Some("BRAF".into());
            filters.sort = ArticleSort::Date;

            Mock::given(method("GET"))
                .and(path("/esearch.fcgi"))
                .and(query_param("db", "pubmed"))
                .and(query_param("retmode", "json"))
                .and(query_param("retstart", "0"))
                .and(query_param("retmax", "100"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "esearchresult": {
                        "count": "2",
                        "idlist": ["1", "2"]
                    }
                })))
                .expect(1)
                .mount(&server)
                .await;

            Mock::given(method("GET"))
                .and(path("/esummary.fcgi"))
                .and(query_param("db", "pubmed"))
                .and(query_param("retmode", "json"))
                .and(query_param("id", "1,2"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "result": {
                        "uids": ["1", "2"],
                        "1": {
                            "uid": "1",
                            "title": "Older title",
                            "sortpubdate": "2024/01/01 00:00",
                            "fulljournalname": "Nature",
                            "source": "Nature"
                        },
                        "2": {
                            "uid": "2",
                            "title": "Newer title",
                            "sortpubdate": "2025/02/01 00:00",
                            "fulljournalname": "Nature",
                            "source": "Nature"
                        }
                    }
                })))
                .expect(1)
                .mount(&server)
                .await;

            let page = search_page(&filters, 2, 0, ArticleSourceFilter::PubMed)
                .await
                .expect("pubmed search should succeed");
            let pmids: Vec<&str> = page.results.iter().map(|row| row.pmid.as_str()).collect();
            assert_eq!(pmids, vec!["2", "1"]);
        });
    }

    #[test]
    fn article_type_limitation_note_tracks_compatible_source_sets() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());
        filters.article_type = Some("review".into());

        assert_eq!(
            article_type_limitation_note(&filters, ArticleSourceFilter::All),
            Some(
                "Note: --type restricts article search to Europe PMC and PubMed. PubTator3, LitSense2, and Semantic Scholar do not support publication-type filtering."
                    .into()
            )
        );
        assert_eq!(
            article_type_limitation_note(&filters, ArticleSourceFilter::EuropePmc),
            None
        );
        assert_eq!(
            article_type_limitation_note(&filters, ArticleSourceFilter::PubTator),
            None
        );
        assert_eq!(
            article_type_limitation_note(&filters, ArticleSourceFilter::PubMed),
            None
        );

        filters.no_preprints = true;
        assert_eq!(
            article_type_limitation_note(&filters, ArticleSourceFilter::All),
            Some(
                "Note: --type restricts this article search to Europe PMC. PubTator3, LitSense2, and Semantic Scholar do not support publication-type filtering, and PubMed does not support the other selected filters."
                    .into()
            )
        );
    }

    #[tokio::test]
    async fn semantic_scholar_search_is_enabled_without_api_key_for_federated_queries() {
        let _guard = lock_env().await;
        let _s2_key = set_env_var("S2_API_KEY", None);

        assert!(semantic_scholar_search_enabled(
            &empty_filters(),
            ArticleSourceFilter::All
        ));
    }

    #[tokio::test]
    async fn citations_work_without_api_key() {
        let _guard = lock_env().await;
        let server = MockServer::start().await;
        let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&server.uri()));
        let _s2_key = set_env_var("S2_API_KEY", None);

        Mock::given(method("POST"))
            .and(path("/graph/v1/paper/batch"))
            .and(query_param(
                "fields",
                "paperId,externalIds,title,venue,year",
            ))
            .and(body_string_contains("\"PMID:22663011\""))
            .and(|request: &wiremock::Request| !request.headers.contains_key("x-api-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "paperId": "paper-1",
                    "externalIds": {"PubMed": "22663011"},
                    "title": "Seed paper",
                    "venue": "Science",
                    "year": 2012
                }
            ])))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/graph/v1/paper/paper-1/citations"))
            .and(query_param(
                "fields",
                "contexts,intents,isInfluential,citingPaper.paperId,citingPaper.externalIds,citingPaper.title,citingPaper.venue,citingPaper.year",
            ))
            .and(query_param("limit", "1"))
            .and(|request: &wiremock::Request| !request.headers.contains_key("x-api-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [{
                    "contexts": ["Example context"],
                    "intents": ["Background"],
                    "isInfluential": false,
                    "citingPaper": {
                        "paperId": "paper-2",
                        "externalIds": {"PubMed": "24200969"},
                        "title": "Related paper",
                        "venue": "Nature",
                        "year": 2024
                    }
                }]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let result = citations("22663011", 1)
            .await
            .expect("no-key citations should succeed");

        assert_eq!(result.article.paper_id.as_deref(), Some("paper-1"));
        assert_eq!(result.edges.len(), 1);
        assert_eq!(result.edges[0].paper.pmid.as_deref(), Some("24200969"));
    }

    #[tokio::test]
    async fn references_work_without_api_key() {
        let _guard = lock_env().await;
        let server = MockServer::start().await;
        let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&server.uri()));
        let _s2_key = set_env_var("S2_API_KEY", None);

        Mock::given(method("POST"))
            .and(path("/graph/v1/paper/batch"))
            .and(query_param(
                "fields",
                "paperId,externalIds,title,venue,year",
            ))
            .and(body_string_contains("\"PMID:22663011\""))
            .and(|request: &wiremock::Request| !request.headers.contains_key("x-api-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "paperId": "paper-1",
                    "externalIds": {"PubMed": "22663011"},
                    "title": "Seed paper",
                    "venue": "Science",
                    "year": 2012
                }
            ])))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/graph/v1/paper/paper-1/references"))
            .and(query_param(
                "fields",
                "contexts,intents,isInfluential,citedPaper.paperId,citedPaper.externalIds,citedPaper.title,citedPaper.venue,citedPaper.year",
            ))
            .and(query_param("limit", "1"))
            .and(|request: &wiremock::Request| !request.headers.contains_key("x-api-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [{
                    "contexts": ["Example context"],
                    "intents": ["Background"],
                    "isInfluential": false,
                    "citedPaper": {
                        "paperId": "paper-2",
                        "externalIds": {"PubMed": "19424861"},
                        "title": "Referenced paper",
                        "venue": "Cell",
                        "year": 2009
                    }
                }]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let result = references("22663011", 1)
            .await
            .expect("no-key references should succeed");

        assert_eq!(result.article.paper_id.as_deref(), Some("paper-1"));
        assert_eq!(result.edges.len(), 1);
        assert_eq!(result.edges[0].paper.pmid.as_deref(), Some("19424861"));
    }

    #[tokio::test]
    async fn recommendations_work_without_api_key() {
        let _guard = lock_env().await;
        let server = MockServer::start().await;
        let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&server.uri()));
        let _s2_key = set_env_var("S2_API_KEY", None);

        Mock::given(method("POST"))
            .and(path("/graph/v1/paper/batch"))
            .and(query_param(
                "fields",
                "paperId,externalIds,title,venue,year",
            ))
            .and(body_string_contains("\"PMID:22663011\""))
            .and(|request: &wiremock::Request| !request.headers.contains_key("x-api-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "paperId": "paper-1",
                    "externalIds": {"PubMed": "22663011"},
                    "title": "Seed paper",
                    "venue": "Science",
                    "year": 2012
                }
            ])))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/recommendations/v1/papers/forpaper/paper-1"))
            .and(query_param(
                "fields",
                "paperId,externalIds,title,venue,year",
            ))
            .and(query_param("limit", "1"))
            .and(|request: &wiremock::Request| !request.headers.contains_key("x-api-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "recommendedPapers": [{
                    "paperId": "paper-3",
                    "externalIds": {"PubMed": "28052061"},
                    "title": "Recommended paper",
                    "venue": "Nature Medicine",
                    "year": 2017
                }]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let result = recommendations(&["22663011".to_string()], &[], 1)
            .await
            .expect("no-key recommendations should succeed");

        assert_eq!(result.positive_seeds.len(), 1);
        assert_eq!(result.recommendations.len(), 1);
        assert_eq!(result.recommendations[0].pmid.as_deref(), Some("28052061"));
    }

    #[test]
    fn summarize_debug_plan_reports_federated_sources_and_matches() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());
        let results = vec![ArticleSearchResult {
            pmid: "22663011".into(),
            pmcid: None,
            doi: None,
            title: "BRAF melanoma".into(),
            journal: None,
            date: None,
            citation_count: None,
            influential_citation_count: None,
            source: ArticleSource::PubMed,
            matched_sources: vec![
                ArticleSource::PubTator,
                ArticleSource::PubMed,
                ArticleSource::SemanticScholar,
            ],
            score: None,
            is_retracted: Some(false),
            abstract_snippet: None,
            ranking: None,
            normalized_title: "braf melanoma".into(),
            normalized_abstract: String::new(),
            publication_type: None,
            source_local_position: 0,
        }];

        let summary =
            summarize_debug_plan(&filters, ArticleSourceFilter::All, &results).expect("summary");

        assert_eq!(summary.routing, vec!["planner=federated".to_string()]);
        assert!(summary.sources.contains(&"PubTator3".to_string()));
        assert!(summary.sources.contains(&"Europe PMC".to_string()));
        assert!(summary.sources.contains(&"PubMed".to_string()));
        assert_eq!(
            summary.matched_sources,
            vec![
                "PubTator3".to_string(),
                "PubMed".to_string(),
                "Semantic Scholar".to_string()
            ]
        );
    }

    #[test]
    fn summarize_debug_plan_strict_filter_emits_europe_only_strict() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());
        filters.open_access = true;

        let summary =
            summarize_debug_plan(&filters, ArticleSourceFilter::All, &[]).expect("summary");

        assert_eq!(
            summary.routing,
            vec!["planner=europe_only_strict_filters".to_string()]
        );
        assert_eq!(summary.sources, vec!["Europe PMC".to_string()]);
        assert!(summary.matched_sources.is_empty());
    }

    #[test]
    fn summarize_debug_plan_explicit_pubtator_emits_pubtator_only() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());

        let summary =
            summarize_debug_plan(&filters, ArticleSourceFilter::PubTator, &[]).expect("summary");

        assert_eq!(summary.routing, vec!["planner=pubtator_only".to_string()]);
        assert_eq!(summary.sources, vec!["PubTator3".to_string()]);
        assert!(summary.matched_sources.is_empty());
    }

    #[test]
    fn summarize_debug_plan_no_preprints_omits_pubmed() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());
        filters.no_preprints = true;

        let summary =
            summarize_debug_plan(&filters, ArticleSourceFilter::All, &[]).expect("summary");

        assert_eq!(summary.routing, vec!["planner=federated".to_string()]);
        assert!(summary.sources.contains(&"PubTator3".to_string()));
        assert!(summary.sources.contains(&"Europe PMC".to_string()));
        assert!(
            !summary.sources.contains(&"PubMed".to_string()),
            "PubMed should be excluded when no_preprints is set"
        );
    }

    #[test]
    fn summarize_debug_plan_keyword_enables_litsense2() {
        let mut filters = empty_filters();
        filters.keyword = Some("Hirschsprung disease".into());

        let summary =
            summarize_debug_plan(&filters, ArticleSourceFilter::All, &[]).expect("summary");

        assert!(
            summary.sources.contains(&"LitSense2".to_string()),
            "keyword-driven federated debug plan should advertise LitSense2"
        );
    }

    #[test]
    fn litsense2_search_enabled_requires_keyword_and_non_strict_filters() {
        let mut keyword_filters = empty_filters();
        keyword_filters.keyword = Some("Hirschsprung disease".into());
        assert!(litsense2_search_enabled(
            &keyword_filters,
            ArticleSourceFilter::All
        ));

        let mut strict_filters = keyword_filters.clone();
        strict_filters.article_type = Some("review".into());
        assert!(!litsense2_search_enabled(
            &strict_filters,
            ArticleSourceFilter::All
        ));

        let mut no_keyword_filters = empty_filters();
        no_keyword_filters.gene = Some("RET".into());
        assert!(!litsense2_search_enabled(
            &no_keyword_filters,
            ArticleSourceFilter::All
        ));
    }

    #[test]
    fn pubmed_unique_row_survives_first_page_in_mixed_federation() {
        // Design: "construct a mixed candidate set with one PubMed-only row that
        // has stronger title-anchor coverage than some competing rows, run it
        // through the common finalizer, and assert that the PubMed-only row
        // survives in the first returned page."
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());

        // PubMed-only row with strong title-anchor coverage (gene in title)
        let pubmed_row = ArticleSearchResult {
            pmid: "99999".into(),
            pmcid: None,
            doi: None,
            title: "BRAF V600E mutations in melanoma".into(),
            journal: Some("Nature".into()),
            date: Some("2025-01-01".into()),
            citation_count: Some(5),
            influential_citation_count: None,
            source: ArticleSource::PubMed,
            matched_sources: vec![ArticleSource::PubMed],
            score: None,
            is_retracted: Some(false),
            abstract_snippet: None,
            ranking: None,
            normalized_title: "braf v600e mutations in melanoma".into(),
            normalized_abstract: String::new(),
            publication_type: None,
            source_local_position: 0,
        };

        // Competing rows from other backends with weaker title-anchor coverage
        let weak_rows: Vec<ArticleSearchResult> = (1..=5)
            .map(|i| ArticleSearchResult {
                pmid: format!("{i}"),
                pmcid: None,
                doi: None,
                title: format!("Unrelated oncology study {i}"),
                journal: Some("Journal".into()),
                date: Some("2025-01-01".into()),
                citation_count: Some(100),
                influential_citation_count: None,
                source: ArticleSource::EuropePmc,
                matched_sources: vec![ArticleSource::EuropePmc],
                score: None,
                is_retracted: Some(false),
                abstract_snippet: None,
                ranking: None,
                normalized_title: format!("unrelated oncology study {i}"),
                normalized_abstract: String::new(),
                publication_type: None,
                source_local_position: 3,
            })
            .collect();

        let mut candidates = weak_rows;
        candidates.push(pubmed_row);

        let page = finalize_article_candidates(candidates, 5, 0, None, &filters);

        assert!(
            page.results.iter().any(|r| r.pmid == "99999"),
            "PubMed-unique row should survive in the first visible page"
        );
        // It should rank high because "BRAF" is in the title
        let pubmed_pos = page
            .results
            .iter()
            .position(|r| r.pmid == "99999")
            .expect("PubMed row must be present");
        assert_eq!(
            pubmed_pos, 0,
            "PubMed row with title-anchor match should rank first among rows without anchor coverage"
        );
    }

    #[test]
    fn pubtator_lag_error_is_400_or_404_only() {
        let err_400 = BioMcpError::Api {
            api: "pubtator3".into(),
            message: "HTTP 400 Bad Request: pending".into(),
        };
        let err_404 = BioMcpError::Api {
            api: "pubtator3".into(),
            message: "HTTP 404 Not Found: pending".into(),
        };
        let err_500 = BioMcpError::Api {
            api: "pubtator3".into(),
            message: "HTTP 500 Internal Server Error".into(),
        };
        let other_api_400 = BioMcpError::Api {
            api: "europepmc".into(),
            message: "HTTP 400 Bad Request".into(),
        };

        assert!(is_pubtator_lag_error(&err_400));
        assert!(is_pubtator_lag_error(&err_404));
        assert!(!is_pubtator_lag_error(&err_500));
        assert!(!is_pubtator_lag_error(&other_api_400));
    }

    #[test]
    fn finalize_article_candidates_preserves_source_local_position() {
        let mut filters = empty_filters();
        filters.sort = ArticleSort::Date;

        let mut first = row("100", ArticleSource::EuropePmc);
        first.source_local_position = 7;
        first.date = Some("2024-01-01".into());

        let mut second = row("200", ArticleSource::PubMed);
        second.source_local_position = 3;
        second.date = Some("2025-01-01".into());

        let page = finalize_article_candidates(vec![first, second], 10, 0, None, &filters);

        assert_eq!(
            page.results
                .iter()
                .find(|row| row.pmid == "100")
                .expect("first row should remain")
                .source_local_position,
            7
        );
        assert_eq!(
            page.results
                .iter()
                .find(|row| row.pmid == "200")
                .expect("second row should remain")
                .source_local_position,
            3
        );
    }

    fn count_primary_source(rows: &[ArticleSearchResult], source: ArticleSource) -> usize {
        rows.iter().filter(|row| row.source == source).count()
    }

    fn row(pmid: &str, source: ArticleSource) -> ArticleSearchResult {
        row_with(pmid, source, Some("2025-01-01"), Some(1), Some(false))
    }

    fn row_with(
        pmid: &str,
        source: ArticleSource,
        date: Option<&str>,
        citation_count: Option<u64>,
        is_retracted: Option<bool>,
    ) -> ArticleSearchResult {
        ArticleSearchResult {
            pmid: pmid.to_string(),
            pmcid: None,
            doi: None,
            title: format!("title-{pmid}"),
            journal: Some("Journal".into()),
            date: date.map(str::to_string),
            citation_count,
            influential_citation_count: None,
            source,
            matched_sources: vec![source],
            score: (source == ArticleSource::PubTator).then_some(42.0),
            is_retracted,
            abstract_snippet: None,
            ranking: None,
            normalized_title: format!("title-{pmid}"),
            normalized_abstract: String::new(),
            publication_type: None,
            source_local_position: 0,
        }
    }

    fn rank_result_rows_by_directness(
        rows: &mut [ArticleSearchResult],
        filters: &ArticleSearchFilters,
    ) {
        let mut candidates = rows
            .iter()
            .cloned()
            .map(article_candidate_from_row)
            .collect::<Vec<_>>();
        rank_articles_by_directness(&mut candidates, filters);
        for (slot, candidate) in rows.iter_mut().zip(candidates.into_iter()) {
            *slot = candidate.row;
        }
    }

    #[test]
    fn finalize_article_candidates_default_cap_skips_two_source_pools() {
        let mut filters = empty_filters();
        filters.sort = ArticleSort::Date;

        let mut rows = Vec::new();
        for (idx, pmid) in ["100", "101", "102"].into_iter().enumerate() {
            let mut row = row(pmid, ArticleSource::PubTator);
            row.source_local_position = idx;
            rows.push(row);
        }
        for (idx, pmid) in ["200", "201", "202"].into_iter().enumerate() {
            let mut row = row(pmid, ArticleSource::EuropePmc);
            row.source_local_position = idx;
            rows.push(row);
        }

        let page = finalize_article_candidates(rows, 5, 0, None, &filters);

        assert_eq!(
            page.results.len(),
            5,
            "default capping should not shrink a two-source federated pool"
        );
    }

    #[test]
    fn finalize_article_candidates_default_cap_limits_three_source_pool() {
        let mut filters = empty_filters();
        filters.sort = ArticleSort::Date;

        let mut rows = Vec::new();
        for (idx, pmid) in ["100", "101", "102", "103"].into_iter().enumerate() {
            let mut row = row(pmid, ArticleSource::PubTator);
            row.source_local_position = idx;
            rows.push(row);
        }
        for (idx, pmid) in ["200", "201"].into_iter().enumerate() {
            let mut row = row(pmid, ArticleSource::EuropePmc);
            row.source_local_position = idx;
            rows.push(row);
        }
        let mut pubmed = row("300", ArticleSource::PubMed);
        pubmed.source_local_position = 0;
        rows.push(pubmed);

        let page = finalize_article_candidates(rows, 5, 0, None, &filters);

        assert_eq!(
            count_primary_source(&page.results, ArticleSource::PubTator),
            2,
            "default cap should keep at most floor(40% of limit) rows from one source when three primary sources survive"
        );
    }

    #[test]
    fn finalize_article_candidates_explicit_cap_applies_on_two_source_pools() {
        let mut filters = empty_filters();
        filters.sort = ArticleSort::Date;
        filters.max_per_source = Some(1);

        let mut rows = Vec::new();
        for (idx, pmid) in ["100", "101", "102"].into_iter().enumerate() {
            let mut row = row(pmid, ArticleSource::PubTator);
            row.source_local_position = idx;
            rows.push(row);
        }
        for (idx, pmid) in ["200", "201", "202"].into_iter().enumerate() {
            let mut row = row(pmid, ArticleSource::EuropePmc);
            row.source_local_position = idx;
            rows.push(row);
        }

        let page = finalize_article_candidates(rows, 5, 0, None, &filters);

        assert_eq!(page.results.len(), 2);
        assert_eq!(
            count_primary_source(&page.results, ArticleSource::PubTator),
            1
        );
        assert_eq!(
            count_primary_source(&page.results, ArticleSource::EuropePmc),
            1
        );
    }

    #[test]
    fn finalize_article_candidates_explicit_cap_uses_primary_source_native_position() {
        let mut filters = empty_filters();
        filters.sort = ArticleSort::Date;
        filters.max_per_source = Some(2);

        let mut pubmed_duplicate = row("100", ArticleSource::PubMed);
        pubmed_duplicate.source_local_position = 8;

        let mut europe_duplicate = row("100", ArticleSource::EuropePmc);
        europe_duplicate.source_local_position = 1;

        let mut pubmed_best = row("101", ArticleSource::PubMed);
        pubmed_best.source_local_position = 2;

        let mut pubmed_second = row("102", ArticleSource::PubMed);
        pubmed_second.source_local_position = 4;

        let mut europe = row("201", ArticleSource::EuropePmc);
        europe.source_local_position = 0;

        let mut pubtator = row("301", ArticleSource::PubTator);
        pubtator.source_local_position = 0;

        let page = finalize_article_candidates(
            vec![
                pubmed_duplicate,
                europe_duplicate,
                pubmed_best,
                pubmed_second,
                europe,
                pubtator,
            ],
            10,
            0,
            None,
            &filters,
        );

        let pmids = page
            .results
            .iter()
            .map(|row| row.pmid.as_str())
            .collect::<Vec<_>>();
        assert!(
            pmids.contains(&"101") && pmids.contains(&"102"),
            "the two best PubMed-primary rows should survive the explicit per-source cap"
        );
        assert!(
            !pmids.contains(&"100"),
            "the merged row should be capped by the PubMed primary-source position, not the min merged position"
        );
    }

    #[test]
    fn finalize_article_candidates_explicit_cap_equal_limit_disables_capping() {
        let mut filters = empty_filters();
        filters.sort = ArticleSort::Citations;
        filters.max_per_source = Some(5);

        let mut rows = Vec::new();
        for (idx, (pmid, citations)) in [
            ("100", 1_u64),
            ("101", 2),
            ("102", 3),
            ("103", 4),
            ("104", 5),
            ("105", 500),
        ]
        .into_iter()
        .enumerate()
        {
            let mut row = row(pmid, ArticleSource::PubTator);
            row.source_local_position = idx;
            row.citation_count = Some(citations);
            rows.push(row);
        }

        let mut europe = row("200", ArticleSource::EuropePmc);
        europe.citation_count = Some(10);
        rows.push(europe);

        let mut pubmed = row("300", ArticleSource::PubMed);
        pubmed.citation_count = Some(9);
        rows.push(pubmed);

        let page = finalize_article_candidates(rows, 5, 0, None, &filters);
        let pmids = page
            .results
            .iter()
            .map(|row| row.pmid.as_str())
            .collect::<Vec<_>>();

        assert!(
            pmids.contains(&"105"),
            "setting --max-per-source equal to --limit should disable capping before ranking"
        );
    }

    #[test]
    fn finalize_article_candidates_default_cap_ignores_empty_pmid_rows() {
        let mut filters = empty_filters();
        filters.sort = ArticleSort::Date;

        let mut pubtator_first = row("100", ArticleSource::PubTator);
        pubtator_first.source_local_position = 0;

        let mut pubtator_empty = row("", ArticleSource::PubTator);
        pubtator_empty.title = "title-empty".into();
        pubtator_empty.normalized_title = "title-empty".into();
        pubtator_empty.source_local_position = 1;

        let mut pubtator_second = row("101", ArticleSource::PubTator);
        pubtator_second.source_local_position = 2;

        let mut europe_first = row("200", ArticleSource::EuropePmc);
        europe_first.source_local_position = 0;

        let mut europe_second = row("201", ArticleSource::EuropePmc);
        europe_second.source_local_position = 1;

        let mut pubmed = row("300", ArticleSource::PubMed);
        pubmed.source_local_position = 0;

        let page = finalize_article_candidates(
            vec![
                pubtator_first,
                pubtator_empty,
                pubtator_second,
                europe_first,
                europe_second,
                pubmed,
            ],
            5,
            0,
            None,
            &filters,
        );

        let pmids = page
            .results
            .iter()
            .map(|row| row.pmid.as_str())
            .collect::<Vec<_>>();
        assert!(pmids.contains(&"100"));
        assert!(pmids.contains(&"101"));
        assert!(!pmids.iter().any(|pmid| pmid.trim().is_empty()));
    }

    #[tokio::test]
    async fn pubmed_source_search_enriches_citation_count_and_abstract_from_semantic_scholar_batch()
    {
        let _guard = lock_env().await;
        let pubmed = MockServer::start().await;
        let s2 = MockServer::start().await;
        let _pubmed_base = set_env_var("BIOMCP_PUBMED_BASE", Some(&pubmed.uri()));
        let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&s2.uri()));
        let _s2_key = set_env_var("S2_API_KEY", None);

        Mock::given(method("GET"))
            .and(path("/esearch.fcgi"))
            .and(query_param("db", "pubmed"))
            .and(query_param("retmode", "json"))
            .and(query_param("retstart", "0"))
            .and(query_param("retmax", "100"))
            .and(query_param("term", "GDNF RET Hirschsprung 1996"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "esearchresult": {
                    "count": "1",
                    "idlist": ["8896569"]
                }
            })))
            .expect(1)
            .mount(&pubmed)
            .await;

        Mock::given(method("GET"))
            .and(path("/esummary.fcgi"))
            .and(query_param("db", "pubmed"))
            .and(query_param("retmode", "json"))
            .and(query_param("id", "8896569"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "result": {
                    "uids": ["8896569"],
                    "8896569": {
                        "uid": "8896569",
                        "title": "A mutation of the RET proto-oncogene in Hirschsprung's disease increases its sensitivity to glial cell line-derived neurotrophic factor",
                        "sortpubdate": "1996/10/24 00:00",
                        "pubdate": "1996 Oct 24",
                        "fulljournalname": "Nature",
                        "source": "Nature"
                    }
                }
            })))
            .expect(1)
            .mount(&pubmed)
            .await;

        Mock::given(method("POST"))
            .and(path("/graph/v1/paper/batch"))
            .and(query_param(
                "fields",
                "paperId,externalIds,citationCount,influentialCitationCount,abstract",
            ))
            .and(body_string_contains("\"PMID:8896569\""))
            .and(|request: &wiremock::Request| !request.headers.contains_key("x-api-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "paperId": "paper-8896569",
                    "externalIds": {"PubMed": "8896569"},
                    "citationCount": 231,
                    "influentialCitationCount": 17,
                    "abstract": "Glial cell line-derived neurotrophic factor signaling through RET is increased by the Hirschsprung disease mutation."
                }
            ])))
            .expect(1)
            .mount(&s2)
            .await;

        let page = search_page(
            &ArticleSearchFilters {
                keyword: Some("GDNF RET Hirschsprung 1996".into()),
                ..empty_filters()
            },
            1,
            0,
            ArticleSourceFilter::PubMed,
        )
        .await
        .expect("pubmed source search should succeed");

        assert_eq!(page.results.len(), 1);
        let row = &page.results[0];
        assert_eq!(row.pmid, "8896569");
        assert_eq!(row.source, ArticleSource::PubMed);
        assert_eq!(row.matched_sources, vec![ArticleSource::PubMed]);
        assert_eq!(row.citation_count, Some(231));
        assert_eq!(row.influential_citation_count, Some(17));
        assert!(
            row.abstract_snippet
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
        );
        assert!(!row.normalized_abstract.is_empty());
    }

    #[tokio::test]
    async fn pubmed_search_falls_back_to_article_base_when_s2_returns_null_abstract() {
        let _guard = lock_env().await;
        let pubmed = MockServer::start().await;
        let pubtator = MockServer::start().await;
        let europepmc = MockServer::start().await;
        let s2 = MockServer::start().await;
        let _pubmed_base = set_env_var("BIOMCP_PUBMED_BASE", Some(&pubmed.uri()));
        let _pubtator_base = set_env_var("BIOMCP_PUBTATOR_BASE", Some(&pubtator.uri()));
        let _europepmc_base = set_env_var("BIOMCP_EUROPEPMC_BASE", Some(&europepmc.uri()));
        let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&s2.uri()));
        let _s2_key = set_env_var("S2_API_KEY", None);

        Mock::given(method("GET"))
            .and(path("/esearch.fcgi"))
            .and(query_param("db", "pubmed"))
            .and(query_param("retmode", "json"))
            .and(query_param("retstart", "0"))
            .and(query_param("retmax", "100"))
            .and(query_param("term", "GDNF RET Hirschsprung 1996"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "esearchresult": {"count": "1", "idlist": ["8896569"]}
            })))
            .expect(1)
            .mount(&pubmed)
            .await;

        Mock::given(method("GET"))
            .and(path("/esummary.fcgi"))
            .and(query_param("db", "pubmed"))
            .and(query_param("retmode", "json"))
            .and(query_param("id", "8896569"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "result": {
                    "uids": ["8896569"],
                    "8896569": {
                        "uid": "8896569",
                        "title": "A mutation of the RET proto-oncogene in Hirschsprung's disease",
                        "sortpubdate": "1996/10/24 00:00",
                        "pubdate": "1996 Oct 24",
                        "fulljournalname": "Nature",
                        "source": "Nature"
                    }
                }
            })))
            .expect(1)
            .mount(&pubmed)
            .await;

        // S2 batch returns citation count but no abstract (null)
        Mock::given(method("POST"))
            .and(path("/graph/v1/paper/batch"))
            .and(query_param(
                "fields",
                "paperId,externalIds,citationCount,influentialCitationCount,abstract",
            ))
            .and(body_string_contains("\"PMID:8896569\""))
            .and(|request: &wiremock::Request| !request.headers.contains_key("x-api-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "paperId": "paper-8896569",
                    "externalIds": {"PubMed": "8896569"},
                    "citationCount": 231,
                    "influentialCitationCount": 17,
                    "abstract": null
                }
            ])))
            .expect(1)
            .mount(&s2)
            .await;

        // Article-base fallback: PubTator provides the abstract
        // Note: PubTatorInfons uses `#[serde(rename = "type")]` for the `kind` field,
        // so the JSON key must be "type", not "kind".
        Mock::given(method("GET"))
            .and(path("/publications/export/biocjson"))
            .and(query_param("pmids", "8896569"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "PubTator3": [{
                    "pmid": 8896569,
                    "passages": [
                        {
                            "infons": {"type": "abstract"},
                            "text": "Hirschsprung disease is a congenital malformation of the enteric nervous system."
                        }
                    ]
                }]
            })))
            .expect(1)
            .mount(&pubtator)
            .await;

        // EuropePMC lookup triggered by resolve_article_from_pmid to merge citation metadata
        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param("query", "EXT_ID:8896569 AND SRC:MED"))
            .and(query_param("format", "json"))
            .and(query_param("page", "1"))
            .and(query_param("pageSize", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "hitCount": 0,
                "resultList": {"result": []}
            })))
            .mount(&europepmc)
            .await;

        let page = search_page(
            &ArticleSearchFilters {
                keyword: Some("GDNF RET Hirschsprung 1996".into()),
                ..empty_filters()
            },
            1,
            0,
            ArticleSourceFilter::PubMed,
        )
        .await
        .expect("search should succeed with article-base abstract fallback");

        assert_eq!(page.results.len(), 1);
        let row = &page.results[0];
        assert_eq!(row.pmid, "8896569");
        assert_eq!(row.source, ArticleSource::PubMed);
        assert_eq!(row.matched_sources, vec![ArticleSource::PubMed]);
        // S2 citation count is preserved
        assert_eq!(row.citation_count, Some(231));
        assert_eq!(row.influential_citation_count, Some(17));
        // Abstract comes from PubTator fallback since S2 returned null
        assert!(
            row.abstract_snippet
                .as_deref()
                .is_some_and(|value| value.contains("Hirschsprung"))
        );
        assert!(row.normalized_abstract.contains("hirschsprung"));
    }

    #[tokio::test]
    async fn federated_search_enrichment_overwrites_europepmc_zero_citation_count() {
        let _guard = lock_env().await;
        let pubtator = MockServer::start().await;
        let europepmc = MockServer::start().await;
        let pubmed = MockServer::start().await;
        let s2 = MockServer::start().await;
        let litsense2 = MockServer::start().await;
        let _pubtator_base = set_env_var("BIOMCP_PUBTATOR_BASE", Some(&pubtator.uri()));
        let _europepmc_base = set_env_var("BIOMCP_EUROPEPMC_BASE", Some(&europepmc.uri()));
        let _pubmed_base = set_env_var("BIOMCP_PUBMED_BASE", Some(&pubmed.uri()));
        let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&s2.uri()));
        let _litsense2_base = set_env_var("BIOMCP_LITSENSE2_BASE", Some(&litsense2.uri()));
        let _s2_key = set_env_var("S2_API_KEY", None);

        Mock::given(method("GET"))
            .and(query_param("page", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "results": [],
                "count": 0,
                "total_pages": 1,
                "current": 1,
                "page_size": 25,
                "facets": {}
            })))
            .expect(1)
            .mount(&pubtator)
            .await;

        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param("format", "json"))
            .and(query_param("page", "1"))
            .and(query_param("pageSize", "25"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "hitCount": 2,
                "resultList": {
                    "result": [
                        {
                            "id": "EP-1",
                            "pmid": "8896569",
                            "title": "RET mutation study",
                            "journalTitle": "Nature",
                            "firstPublicationDate": "1996-10-24",
                            "citedByCount": 0,
                            "abstractText": "Primary source abstract.",
                            "pubType": "journal article"
                        },
                        {
                            "id": "EP-2",
                            "pmid": "99900001",
                            "title": "Comparator study",
                            "journalTitle": "Science",
                            "firstPublicationDate": "1995-01-01",
                            "citedByCount": 5,
                            "abstractText": "Comparator abstract.",
                            "pubType": "journal article"
                        }
                    ]
                }
            })))
            .expect(1)
            .mount(&europepmc)
            .await;

        Mock::given(method("GET"))
            .and(path("/esearch.fcgi"))
            .and(query_param("db", "pubmed"))
            .and(query_param("retmode", "json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "esearchresult": {
                    "count": "0",
                    "idlist": []
                }
            })))
            .expect(1)
            .mount(&pubmed)
            .await;

        Mock::given(method("GET"))
            .and(path("/graph/v1/paper/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "total": 0,
                "data": []
            })))
            .expect(1)
            .mount(&s2)
            .await;

        Mock::given(method("GET"))
            .and(path("/sentences/"))
            .and(query_param("query", "GDNF RET Hirschsprung 1996"))
            .and(query_param("rerank", "true"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .expect(1)
            .mount(&litsense2)
            .await;

        Mock::given(method("POST"))
            .and(path("/graph/v1/paper/batch"))
            .and(query_param(
                "fields",
                "paperId,externalIds,citationCount,influentialCitationCount,abstract",
            ))
            .and(body_string_contains("\"PMID:8896569\""))
            .and(body_string_contains("\"PMID:99900001\""))
            .and(|request: &wiremock::Request| !request.headers.contains_key("x-api-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "paperId": "paper-8896569",
                    "externalIds": {"PubMed": "8896569"},
                    "citationCount": 231,
                    "influentialCitationCount": 17,
                    "abstract": "Semantic Scholar abstract for RET mutation study."
                },
                {
                    "paperId": "paper-99900001",
                    "externalIds": {"PubMed": "99900001"},
                    "citationCount": 5,
                    "influentialCitationCount": 1,
                    "abstract": "Semantic Scholar abstract for comparator study."
                }
            ])))
            .expect(1)
            .mount(&s2)
            .await;

        let page = search_page(
            &ArticleSearchFilters {
                keyword: Some("GDNF RET Hirschsprung 1996".into()),
                sort: ArticleSort::Citations,
                ..empty_filters()
            },
            2,
            0,
            ArticleSourceFilter::All,
        )
        .await
        .expect("federated search should succeed");

        assert_eq!(
            page.results
                .iter()
                .map(|row| row.pmid.as_str())
                .collect::<Vec<_>>(),
            vec!["8896569", "99900001"]
        );
        assert_eq!(page.results[0].source, ArticleSource::EuropePmc);
        assert_eq!(
            page.results[0].matched_sources,
            vec![ArticleSource::EuropePmc]
        );
        assert_eq!(page.results[0].citation_count, Some(231));
    }

    #[tokio::test]
    async fn article_search_enrichment_preserves_existing_nonempty_primary_metadata() {
        let _guard = lock_env().await;
        let s2 = MockServer::start().await;
        let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&s2.uri()));
        let _s2_key = set_env_var("S2_API_KEY", None);

        Mock::given(method("POST"))
            .and(path("/graph/v1/paper/batch"))
            .and(query_param(
                "fields",
                "paperId,externalIds,citationCount,influentialCitationCount,abstract",
            ))
            .and(body_string_contains("\"PMID:8896569\""))
            .and(|request: &wiremock::Request| !request.headers.contains_key("x-api-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "paperId": "paper-8896569",
                    "externalIds": {"PubMed": "8896569"},
                    "citationCount": 231,
                    "influentialCitationCount": 17,
                    "abstract": "Semantic Scholar replacement abstract."
                }
            ])))
            .expect(1)
            .mount(&s2)
            .await;

        let primary_abstract = "Primary source abstract.";
        let primary_normalized =
            crate::transform::article::normalize_article_search_text(primary_abstract);
        let mut rows = vec![ArticleSearchResult {
            abstract_snippet: Some(primary_abstract.into()),
            normalized_abstract: primary_normalized.clone(),
            ..row_with(
                "8896569",
                ArticleSource::PubMed,
                Some("1996-10-24"),
                Some(99),
                Some(false),
            )
        }];

        enrich_article_search_rows_with_semantic_scholar(&mut rows).await;

        let row = &rows[0];
        assert_eq!(row.citation_count, Some(99));
        assert_eq!(row.influential_citation_count, Some(17));
        assert_eq!(row.abstract_snippet.as_deref(), Some(primary_abstract));
        assert_eq!(row.normalized_abstract, primary_normalized);
    }

    #[tokio::test]
    async fn article_search_semantic_scholar_batch_failure_is_non_fatal() {
        let _guard = lock_env().await;
        let pubtator = MockServer::start().await;
        let s2 = MockServer::start().await;
        let _pubtator_base = set_env_var("BIOMCP_PUBTATOR_BASE", Some(&pubtator.uri()));
        let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&s2.uri()));
        let _s2_key = set_env_var("S2_API_KEY", None);

        Mock::given(method("GET"))
            .and(query_param("page", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "results": [{
                    "_id": "pt-1",
                    "pmid": 22663011,
                    "title": "Alternative microexon splicing in metastasis",
                    "journal": "Cancer Cell",
                    "date": "2025-01-01",
                    "score": 42.0
                }],
                "count": 1,
                "total_pages": 1,
                "current": 1,
                "page_size": 25,
                "facets": {}
            })))
            .expect(1)
            .mount(&pubtator)
            .await;

        Mock::given(method("POST"))
            .and(path("/graph/v1/paper/batch"))
            .and(query_param(
                "fields",
                "paperId,externalIds,citationCount,influentialCitationCount,abstract",
            ))
            .and(body_string_contains("\"PMID:22663011\""))
            .and(|request: &wiremock::Request| !request.headers.contains_key("x-api-key"))
            .respond_with(ResponseTemplate::new(429).set_body_string("shared rate limit"))
            .expect(1)
            .mount(&s2)
            .await;

        let page = search_page(
            &ArticleSearchFilters {
                keyword: Some("alternative microexon splicing metastasis".into()),
                sort: ArticleSort::Date,
                ..empty_filters()
            },
            1,
            0,
            ArticleSourceFilter::PubTator,
        )
        .await
        .expect("search should survive Semantic Scholar batch failures");

        assert_eq!(page.results.len(), 1);
        assert_eq!(page.results[0].source, ArticleSource::PubTator);
        assert_eq!(
            page.results[0].matched_sources,
            vec![ArticleSource::PubTator]
        );
        assert_eq!(page.results[0].citation_count, None);
        assert_eq!(page.results[0].abstract_snippet, None);
    }

    #[tokio::test]
    async fn article_search_semantic_scholar_batch_enrichment_chunks_after_500_ids() {
        let _guard = lock_env().await;
        let s2 = MockServer::start().await;
        let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&s2.uri()));
        let _s2_key = set_env_var("S2_API_KEY", None);

        let first_chunk_body = (1..=SEMANTIC_SCHOLAR_BATCH_LOOKUP_MAX_IDS)
            .map(|pmid| {
                serde_json::json!({
                    "paperId": format!("paper-{pmid}"),
                    "citationCount": pmid as u64,
                    "influentialCitationCount": 1,
                    "abstract": format!("Abstract for PMID {pmid}.")
                })
            })
            .collect::<Vec<_>>();
        Mock::given(method("POST"))
            .and(path("/graph/v1/paper/batch"))
            .and(query_param(
                "fields",
                "paperId,externalIds,citationCount,influentialCitationCount,abstract",
            ))
            .and(body_string_contains("\"PMID:1\""))
            .and(body_string_contains("\"PMID:500\""))
            .and(|request: &wiremock::Request| {
                !String::from_utf8_lossy(&request.body).contains("\"PMID:501\"")
            })
            .respond_with(ResponseTemplate::new(200).set_body_json(first_chunk_body))
            .expect(1)
            .mount(&s2)
            .await;

        Mock::given(method("POST"))
            .and(path("/graph/v1/paper/batch"))
            .and(query_param(
                "fields",
                "paperId,externalIds,citationCount,influentialCitationCount,abstract",
            ))
            .and(body_string_contains("\"PMID:501\""))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "paperId": "paper-501",
                    "citationCount": 501,
                    "influentialCitationCount": 1,
                    "abstract": "Abstract for PMID 501."
                }
            ])))
            .expect(1)
            .mount(&s2)
            .await;

        let mut rows = (1..=501)
            .map(|pmid| {
                row_with(
                    &pmid.to_string(),
                    ArticleSource::PubMed,
                    Some("2025-01-01"),
                    None,
                    Some(false),
                )
            })
            .collect::<Vec<_>>();

        enrich_article_search_rows_with_semantic_scholar(&mut rows).await;

        assert_eq!(rows[0].citation_count, Some(1));
        assert_eq!(rows[499].citation_count, Some(500));
        assert_eq!(rows[500].citation_count, Some(501));
        assert!(
            rows[500]
                .abstract_snippet
                .as_deref()
                .is_some_and(|value| value.contains("Abstract"))
        );
        assert!(!rows[500].normalized_abstract.is_empty());
    }

    #[test]
    fn merge_federated_pages_dedups_with_pubtator_priority() {
        let pubtator_page = SearchPage::offset(
            vec![
                row("100", ArticleSource::PubTator),
                row("200", ArticleSource::PubTator),
            ],
            Some(2),
        );
        let europe_page = SearchPage::offset(
            vec![
                row("200", ArticleSource::EuropePmc),
                row("300", ArticleSource::EuropePmc),
            ],
            Some(2),
        );

        let merged = merge_federated_pages(
            Ok(pubtator_page),
            Ok(europe_page),
            None,
            Ok(Vec::new()),
            Ok(Vec::new()),
            3,
            0,
            &empty_filters(),
        )
        .expect("federated merge should succeed");
        assert_eq!(merged.results.len(), 3);
        assert_eq!(merged.results[0].pmid, "100");
        assert_eq!(merged.results[1].pmid, "200");
        assert_eq!(merged.results[2].pmid, "300");
        assert_eq!(merged.results[1].source, ArticleSource::PubTator);
        assert_eq!(merged.total, None);
    }

    #[test]
    fn merge_federated_pages_records_litsense2_in_matched_sources() {
        let pubtator_page = SearchPage::offset(vec![row("100", ArticleSource::PubTator)], Some(1));
        let europe_page = SearchPage::offset(Vec::new(), Some(0));
        let litsense2_rows = vec![row("100", ArticleSource::LitSense2)];

        let merged = merge_federated_pages(
            Ok(pubtator_page),
            Ok(europe_page),
            None,
            Ok(Vec::new()),
            Ok(litsense2_rows),
            10,
            0,
            &empty_filters(),
        )
        .expect("federated merge should succeed");

        assert_eq!(merged.results.len(), 1);
        assert_eq!(merged.results[0].source, ArticleSource::PubTator);
        assert_eq!(
            merged.results[0].matched_sources,
            vec![ArticleSource::PubTator, ArticleSource::LitSense2]
        );
    }

    #[test]
    fn federated_relevance_uses_source_local_position_not_merge_order() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());
        filters.keyword = Some("melanoma".into());

        let mut europe_first = row("100", ArticleSource::EuropePmc);
        europe_first.title = "BRAF melanoma study".into();
        europe_first.normalized_title = "braf melanoma study".into();
        europe_first.citation_count = Some(5);
        europe_first.source_local_position = 0;

        let mut europe_second = row("200", ArticleSource::EuropePmc);
        europe_second.title = "BRAF melanoma study".into();
        europe_second.normalized_title = "braf melanoma study".into();
        europe_second.citation_count = Some(5);
        europe_second.source_local_position = 1;

        let mut europe_third = row("300", ArticleSource::EuropePmc);
        europe_third.title = "BRAF melanoma study".into();
        europe_third.normalized_title = "braf melanoma study".into();
        europe_third.citation_count = Some(5);
        europe_third.source_local_position = 2;

        let mut pubmed_first = row("900", ArticleSource::PubMed);
        pubmed_first.title = "BRAF melanoma study".into();
        pubmed_first.normalized_title = "braf melanoma study".into();
        pubmed_first.citation_count = Some(5);
        pubmed_first.source_local_position = 0;

        let page = finalize_article_candidates(
            vec![europe_first, europe_second, europe_third, pubmed_first],
            10,
            0,
            None,
            &filters,
        );

        let pubmed_rank = page
            .results
            .iter()
            .position(|row| row.pmid == "900")
            .expect("pubmed row should remain in the ranked output");
        assert!(
            pubmed_rank <= 1,
            "a source-local first PubMed row should rank with other source-local first rows"
        );
    }

    #[test]
    fn merge_federated_pages_returns_surviving_pubtator_leg() {
        let pubtator_page = SearchPage::offset(
            vec![
                row("100", ArticleSource::PubTator),
                row("200", ArticleSource::PubTator),
            ],
            Some(50),
        );
        let europe_err = BioMcpError::Api {
            api: "europepmc".into(),
            message: "HTTP 500: upstream".into(),
        };

        let merged = merge_federated_pages(
            Ok(pubtator_page),
            Err(europe_err),
            None,
            Ok(Vec::new()),
            Ok(Vec::new()),
            2,
            0,
            &empty_filters(),
        )
        .expect("fallback should return pubtator rows");
        assert_eq!(merged.results.len(), 2);
        assert!(
            merged
                .results
                .iter()
                .all(|r| r.source == ArticleSource::PubTator)
        );
        assert_eq!(merged.total, None);
    }

    #[test]
    fn merge_federated_pages_returns_surviving_europe_leg() {
        let pubtator_err = BioMcpError::Api {
            api: "pubtator3".into(),
            message: "HTTP 500: upstream".into(),
        };
        let europe_page = SearchPage::offset(
            vec![
                row("100", ArticleSource::EuropePmc),
                row("200", ArticleSource::EuropePmc),
                row("300", ArticleSource::EuropePmc),
            ],
            Some(50),
        );

        let merged = merge_federated_pages(
            Err(pubtator_err),
            Ok(europe_page),
            None,
            Ok(Vec::new()),
            Ok(Vec::new()),
            2,
            0,
            &empty_filters(),
        )
        .expect("fallback should return europe rows");
        assert_eq!(merged.results.len(), 2);
        assert!(
            merged
                .results
                .iter()
                .all(|r| r.source == ArticleSource::EuropePmc)
        );
        assert_eq!(merged.total, None);
    }

    #[test]
    fn merge_federated_pages_sorts_surviving_leg_before_offset() {
        let pubtator_err = BioMcpError::Api {
            api: "pubtator3".into(),
            message: "HTTP 500: upstream".into(),
        };
        let europe_page = SearchPage::offset(
            vec![
                row_with(
                    "100",
                    ArticleSource::EuropePmc,
                    Some("2024-01-01"),
                    Some(1),
                    Some(false),
                ),
                row_with(
                    "200",
                    ArticleSource::EuropePmc,
                    Some("2025-01-01"),
                    Some(1),
                    Some(false),
                ),
                row_with(
                    "300",
                    ArticleSource::EuropePmc,
                    Some("2023-01-01"),
                    Some(1),
                    Some(false),
                ),
            ],
            Some(3),
        );

        let merged = merge_federated_pages(
            Err(pubtator_err),
            Ok(europe_page),
            None,
            Ok(Vec::new()),
            Ok(Vec::new()),
            1,
            1,
            &ArticleSearchFilters {
                sort: ArticleSort::Date,
                ..empty_filters()
            },
        )
        .expect("fallback should sort surviving rows before offset");
        assert_eq!(merged.results.len(), 1);
        assert_eq!(merged.results[0].pmid, "100");
    }

    #[test]
    fn merge_federated_pages_returns_first_error_when_both_fail() {
        let pubtator_err = BioMcpError::Api {
            api: "pubtator3".into(),
            message: "HTTP 500: pubtator failed".into(),
        };
        let europe_err = BioMcpError::Api {
            api: "europepmc".into(),
            message: "HTTP 500: europe failed".into(),
        };

        let err = merge_federated_pages(
            Err(pubtator_err),
            Err(europe_err),
            None,
            Ok(Vec::new()),
            Ok(Vec::new()),
            10,
            0,
            &empty_filters(),
        )
        .expect_err("both failing legs should return first error");
        let msg = err.to_string();
        assert!(msg.contains("pubtator"));
    }

    #[test]
    fn federated_offset_applied_after_merge_not_per_leg() {
        let pubtator_page = SearchPage::offset(
            vec![
                row("100", ArticleSource::PubTator),
                row("200", ArticleSource::PubTator),
                row("300", ArticleSource::PubTator),
                row("400", ArticleSource::PubTator),
                row("500", ArticleSource::PubTator),
            ],
            Some(5),
        );
        let europe_page = SearchPage::offset(
            vec![
                row("600", ArticleSource::EuropePmc),
                row("700", ArticleSource::EuropePmc),
            ],
            Some(2),
        );

        let merged = merge_federated_pages(
            Ok(pubtator_page),
            Ok(europe_page),
            None,
            Ok(Vec::new()),
            Ok(Vec::new()),
            2,
            3,
            &empty_filters(),
        )
        .expect("federated merge should succeed");

        let pmids: Vec<&str> = merged.results.iter().map(|row| row.pmid.as_str()).collect();
        assert_eq!(pmids, vec!["400", "500"]);
    }

    #[test]
    fn federated_sort_orders_merged_results_for_citations_and_date() {
        let citation_pubtator_page = SearchPage::offset(
            vec![
                row_with(
                    "100",
                    ArticleSource::PubTator,
                    Some("2025-02-01"),
                    Some(50),
                    Some(false),
                ),
                row_with(
                    "200",
                    ArticleSource::PubTator,
                    Some("2024-01-01"),
                    Some(5),
                    Some(false),
                ),
            ],
            Some(2),
        );
        let citation_europe_page = SearchPage::offset(
            vec![
                row_with(
                    "300",
                    ArticleSource::EuropePmc,
                    Some("2025-03-01"),
                    Some(100),
                    Some(false),
                ),
                row_with(
                    "400",
                    ArticleSource::EuropePmc,
                    Some("2024-06-01"),
                    Some(10),
                    Some(false),
                ),
            ],
            Some(2),
        );

        let citation_merged = merge_federated_pages(
            Ok(citation_pubtator_page),
            Ok(citation_europe_page),
            None,
            Ok(Vec::new()),
            Ok(Vec::new()),
            10,
            0,
            &ArticleSearchFilters {
                sort: ArticleSort::Citations,
                ..empty_filters()
            },
        )
        .expect("citation merge should succeed");
        let citation_pmids: Vec<&str> = citation_merged
            .results
            .iter()
            .map(|row| row.pmid.as_str())
            .collect();
        assert_eq!(citation_pmids, vec!["300", "100", "400", "200"]);

        let date_pubtator_page = SearchPage::offset(
            vec![
                row_with(
                    "500",
                    ArticleSource::PubTator,
                    Some("2025"),
                    Some(25),
                    Some(false),
                ),
                row_with(
                    "600",
                    ArticleSource::PubTator,
                    Some("2024-12-31"),
                    Some(30),
                    Some(false),
                ),
            ],
            Some(2),
        );
        let date_europe_page = SearchPage::offset(
            vec![
                row_with(
                    "700",
                    ArticleSource::EuropePmc,
                    Some("2025-06-01"),
                    Some(10),
                    Some(false),
                ),
                row_with("800", ArticleSource::EuropePmc, None, Some(99), Some(false)),
            ],
            Some(2),
        );

        let date_merged = merge_federated_pages(
            Ok(date_pubtator_page),
            Ok(date_europe_page),
            None,
            Ok(Vec::new()),
            Ok(Vec::new()),
            10,
            0,
            &ArticleSearchFilters {
                sort: ArticleSort::Date,
                ..empty_filters()
            },
        )
        .expect("date merge should succeed");
        let date_pmids: Vec<&str> = date_merged
            .results
            .iter()
            .map(|row| row.pmid.as_str())
            .collect();
        assert_eq!(date_pmids, vec!["700", "500", "600", "800"]);
    }

    #[test]
    fn partial_date_normalization_and_filtering_are_consistent() {
        assert_eq!(parse_row_date(Some("2024")), Some("2024-01-01".into()));
        assert_eq!(parse_row_date(Some("2024-06")), Some("2024-06-01".into()));
        assert_eq!(
            parse_row_date(Some("2024-06-15")),
            Some("2024-06-15".into())
        );

        assert!(matches_optional_date_filter(
            Some("2024"),
            Some("2024-01-01"),
            None,
        ));
        assert!(!matches_optional_date_filter(
            Some("2023"),
            Some("2024-01-01"),
            None,
        ));
        assert!(matches_optional_date_filter(
            Some("2024-06"),
            None,
            Some("2024-12-31"),
        ));
    }

    #[test]
    fn article_batch_item_projection_keeps_requested_id_year_and_top_entities() {
        let article = Article {
            pmid: Some("22663011".to_string()),
            pmcid: Some("PMC9984800".to_string()),
            doi: Some("10.1056/NEJMoa1203421".to_string()),
            title: " Improved survival with vemurafenib ".to_string(),
            authors: Vec::new(),
            journal: Some("NEJM".to_string()),
            date: Some("2012-06-07".to_string()),
            citation_count: Some(77),
            publication_type: None,
            open_access: None,
            abstract_text: None,
            full_text_path: None,
            full_text_note: None,
            annotations: Some(ArticleAnnotations {
                genes: vec![
                    AnnotationCount {
                        text: "BRAF".to_string(),
                        count: 4,
                    },
                    AnnotationCount {
                        text: "NRAS".to_string(),
                        count: 3,
                    },
                    AnnotationCount {
                        text: "MAP2K1".to_string(),
                        count: 2,
                    },
                    AnnotationCount {
                        text: "PTEN".to_string(),
                        count: 1,
                    },
                ],
                diseases: vec![AnnotationCount {
                    text: "melanoma".to_string(),
                    count: 2,
                }],
                chemicals: vec![AnnotationCount {
                    text: "vemurafenib".to_string(),
                    count: 2,
                }],
                mutations: vec![AnnotationCount {
                    text: "V600E".to_string(),
                    count: 3,
                }],
            }),
            semantic_scholar: Some(ArticleSemanticScholar {
                paper_id: Some("paper-1".to_string()),
                tldr: Some("BRAF inhibitor benefit in melanoma.".to_string()),
                citation_count: Some(120),
                influential_citation_count: Some(18),
                reference_count: None,
                is_open_access: None,
                open_access_pdf: None,
            }),
            pubtator_fallback: false,
        };

        let item = article_batch_item_from_article(" 10.1056/NEJMoa1203421 ", &article);
        assert_eq!(item.requested_id, "10.1056/NEJMoa1203421");
        assert_eq!(item.pmid.as_deref(), Some("22663011"));
        assert_eq!(item.pmcid.as_deref(), Some("PMC9984800"));
        assert_eq!(item.doi.as_deref(), Some("10.1056/NEJMoa1203421"));
        assert_eq!(item.title, "Improved survival with vemurafenib");
        assert_eq!(item.journal.as_deref(), Some("NEJM"));
        assert_eq!(item.year, Some(2012));
        assert_eq!(item.tldr, None);
        assert_eq!(item.citation_count, None);
        assert_eq!(item.influential_citation_count, None);

        let entity_summary = item.entity_summary.expect("entity summary");
        assert_eq!(entity_summary.genes.len(), 3);
        assert_eq!(
            entity_summary
                .genes
                .iter()
                .map(|row| row.text.as_str())
                .collect::<Vec<_>>(),
            vec!["BRAF", "NRAS", "MAP2K1"]
        );
        assert_eq!(entity_summary.diseases[0].text, "melanoma");
        assert_eq!(entity_summary.chemicals[0].text, "vemurafenib");
        assert_eq!(entity_summary.mutations[0].text, "V600E");
    }

    #[tokio::test]
    async fn article_batch_rejects_more_than_max_ids_before_network() {
        let ids = (0..ARTICLE_BATCH_MAX_IDS + 1)
            .map(|idx| format!("{}", 22000000 + idx))
            .collect::<Vec<_>>();

        let err = get_batch_compact(&ids)
            .await
            .expect_err("batch over the max should fail");
        assert_eq!(
            err.to_string(),
            format!("Invalid argument: Article batch is limited to {ARTICLE_BATCH_MAX_IDS} IDs")
        );
    }

    #[test]
    fn batch_semantic_scholar_merge_fills_fields_and_skips_none_rows_and_pmcid_only() {
        use crate::sources::semantic_scholar::{SemanticScholarPaper, SemanticScholarTldr};

        fn blank_item(requested_id: &str) -> ArticleBatchItem {
            ArticleBatchItem {
                requested_id: requested_id.to_string(),
                pmid: None,
                pmcid: None,
                doi: None,
                title: String::new(),
                journal: None,
                year: None,
                entity_summary: None,
                tldr: None,
                citation_count: None,
                influential_citation_count: None,
            }
        }

        let mut items = vec![
            ArticleBatchItem {
                pmid: Some("22663011".to_string()),
                ..blank_item("22663011")
            },
            // PMCID-only: not in the S2 lookup list (no PMID or DOI)
            ArticleBatchItem {
                pmcid: Some("PMC9984800".to_string()),
                ..blank_item("PMC9984800")
            },
            // Second PMID lookup — S2 returns None (paper not found)
            ArticleBatchItem {
                pmid: Some("00000000".to_string()),
                ..blank_item("00000000")
            },
        ];

        // positions 0 and 2 have PMIDs; position 1 is PMCID-only and not looked up
        let item_positions = vec![0usize, 2usize];
        let rows: Vec<Option<SemanticScholarPaper>> = vec![
            Some(SemanticScholarPaper {
                tldr: Some(SemanticScholarTldr {
                    text: Some("  Compact summary  ".to_string()),
                    model: None,
                }),
                citation_count: Some(120),
                influential_citation_count: Some(18),
                ..Default::default()
            }),
            None, // S2 returned no match for position 2
        ];

        merge_semantic_scholar_compact_rows(&mut items, &item_positions, rows);

        // Item 0: enriched
        assert_eq!(items[0].tldr.as_deref(), Some("Compact summary")); // whitespace trimmed
        assert_eq!(items[0].citation_count, Some(120));
        assert_eq!(items[0].influential_citation_count, Some(18));

        // Item 1: PMCID-only, not in lookup, untouched
        assert_eq!(items[1].tldr, None);
        assert_eq!(items[1].citation_count, None);

        // Item 2: None row, fields stay unset
        assert_eq!(items[2].tldr, None);
        assert_eq!(items[2].citation_count, None);
    }

    #[test]
    fn exclude_retracted_only_filters_confirmed_retractions() {
        let confirmed_retracted = row_with(
            "100",
            ArticleSource::PubTator,
            Some("2025-01-01"),
            Some(1),
            Some(true),
        );
        let confirmed_not_retracted = row_with(
            "101",
            ArticleSource::PubTator,
            Some("2025-01-01"),
            Some(1),
            Some(false),
        );
        let exclude_filters = ArticleSearchFilters {
            exclude_retracted: true,
            ..empty_filters()
        };
        let include_filters = ArticleSearchFilters {
            exclude_retracted: false,
            ..empty_filters()
        };

        assert!(!matches_result_filters(
            &confirmed_retracted,
            &exclude_filters,
            None,
            None
        ));
        assert!(matches_result_filters(
            &confirmed_retracted,
            &include_filters,
            None,
            None
        ));
        assert!(matches_result_filters(
            &confirmed_not_retracted,
            &exclude_filters,
            None,
            None
        ));
    }

    #[test]
    fn exclude_retracted_keeps_unknown_retraction_status() {
        let row = row_with(
            "100",
            ArticleSource::PubTator,
            Some("2025-01-01"),
            Some(1),
            None,
        );
        let exclude_filters = ArticleSearchFilters {
            exclude_retracted: true,
            ..empty_filters()
        };
        let include_filters = ArticleSearchFilters {
            exclude_retracted: false,
            ..empty_filters()
        };

        assert!(matches_result_filters(&row, &exclude_filters, None, None));
        assert!(matches_result_filters(&row, &include_filters, None, None));
    }

    #[tokio::test]
    async fn source_specific_pubtator_search_uses_default_retraction_filter() {
        let _guard = lock_env().await;
        let server = MockServer::start().await;
        let _pubtator_base = set_env_var("BIOMCP_PUBTATOR_BASE", Some(&server.uri()));

        Mock::given(method("GET"))
            .and(query_param("page", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "results": [{
                    "_id": "pt-1",
                    "pmid": 22663011,
                    "title": "Alternative microexon splicing in metastasis",
                    "journal": "Cancer Cell",
                    "date": "2025-01-01",
                    "score": 42.0
                }],
                "count": 1,
                "total_pages": 1,
                "current": 1,
                "page_size": 25,
                "facets": {}
            })))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(query_param("page", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "results": [],
                "count": 1,
                "total_pages": 1,
                "current": 2,
                "page_size": 25,
                "facets": {}
            })))
            .expect(1)
            .mount(&server)
            .await;

        let page = search_page(
            &ArticleSearchFilters {
                keyword: Some("alternative microexon splicing metastasis".into()),
                exclude_retracted: true,
                ..empty_filters()
            },
            3,
            0,
            ArticleSourceFilter::PubTator,
        )
        .await
        .expect("pubtator search should succeed");

        assert_eq!(page.results.len(), 1);
        assert_eq!(page.results[0].source, ArticleSource::PubTator);
        assert_eq!(page.results[0].pmid, "22663011");
    }

    #[tokio::test]
    async fn semantic_scholar_candidates_keep_unknown_retraction_rows() {
        let _guard = lock_env().await;
        let server = MockServer::start().await;
        let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&server.uri()));
        let _s2_key = set_env_var("S2_API_KEY", Some("dummy-key"));

        Mock::given(method("GET"))
            .and(path("/graph/v1/paper/search"))
            .and(query_param(
                "query",
                "alternative microexon splicing metastasis",
            ))
            .and(query_param("limit", "3"))
            .and(header("x-api-key", "dummy-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "total": 1,
                "data": [{
                    "paperId": "paper-1",
                    "externalIds": {
                        "PubMed": "22663011",
                        "DOI": "10.1000/example"
                    },
                    "title": "Alternative microexon splicing in metastasis",
                    "venue": "Cancer Cell",
                    "year": 2025,
                    "citationCount": 12,
                    "influentialCitationCount": 4,
                    "abstract": "Microexon splicing contributes to metastatic progression."
                }]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let rows = search_semantic_scholar_candidates(
            &ArticleSearchFilters {
                keyword: Some("alternative microexon splicing metastasis".into()),
                exclude_retracted: true,
                ..empty_filters()
            },
            3,
        )
        .await
        .expect("semantic scholar search should succeed");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].source, ArticleSource::SemanticScholar);
        assert_eq!(rows[0].is_retracted, None);
    }

    #[tokio::test]
    async fn litsense2_candidates_deduplicate_and_hydrate_pubmed_metadata() {
        let _guard = lock_env().await;
        let litsense2 = MockServer::start().await;
        let pubmed = MockServer::start().await;
        let _litsense2_base = set_env_var("BIOMCP_LITSENSE2_BASE", Some(&litsense2.uri()));
        let _pubmed_base = set_env_var("BIOMCP_PUBMED_BASE", Some(&pubmed.uri()));

        Mock::given(method("GET"))
            .and(path("/sentences/"))
            .and(query_param("query", "Hirschsprung disease ganglion cells"))
            .and(query_param("rerank", "true"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "pmid": 22663011,
                    "pmcid": "PMC9984800",
                    "text": "First weaker sentence",
                    "score": 0.5,
                    "section": "INTRO",
                    "annotations": ["0|12|disease|MESH:D006627"]
                },
                {
                    "pmid": 22663011,
                    "pmcid": "PMC9984800",
                    "text": "Stronger sentence for the same PMID",
                    "score": 0.9,
                    "section": "RESULTS",
                    "annotations": []
                },
                {
                    "pmid": 24200969,
                    "pmcid": null,
                    "text": "Fallback title text that should be truncated when PubMed has no title",
                    "score": 0.7,
                    "section": null,
                    "annotations": null
                }
            ])))
            .expect(1)
            .mount(&litsense2)
            .await;

        Mock::given(method("GET"))
            .and(path("/esummary.fcgi"))
            .and(query_param("db", "pubmed"))
            .and(query_param("retmode", "json"))
            .and(query_param("id", "22663011,24200969"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "result": {
                    "uids": ["22663011", "24200969"],
                    "22663011": {
                        "uid": "22663011",
                        "title": "Hydrated LitSense2 title",
                        "sortpubdate": "2024/01/15 00:00",
                        "pubdate": "2024 Jan 15",
                        "fulljournalname": "Journal One",
                        "source": "J1"
                    },
                    "24200969": {
                        "uid": "24200969",
                        "title": " ",
                        "sortpubdate": null,
                        "pubdate": null,
                        "fulljournalname": null,
                        "source": null
                    }
                }
            })))
            .expect(1)
            .mount(&pubmed)
            .await;

        let rows = search_litsense2_candidates(
            &ArticleSearchFilters {
                keyword: Some("Hirschsprung disease ganglion cells".into()),
                exclude_retracted: true,
                ..empty_filters()
            },
            10,
        )
        .await
        .expect("litsense2 search should succeed");

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].pmid, "22663011");
        assert_eq!(rows[0].title, "Hydrated LitSense2 title");
        assert_eq!(rows[0].pmcid.as_deref(), Some("PMC9984800"));
        assert_eq!(rows[0].journal.as_deref(), Some("Journal One"));
        assert_eq!(rows[0].date.as_deref(), Some("2024-01-15"));
        assert_eq!(rows[0].score, Some(0.9));
        assert_eq!(rows[0].source, ArticleSource::LitSense2);
        assert_eq!(rows[0].matched_sources, vec![ArticleSource::LitSense2]);
        assert_eq!(rows[0].source_local_position, 0);
        assert!(
            rows[0]
                .abstract_snippet
                .as_deref()
                .is_some_and(|snippet| snippet.contains("Stronger sentence for the same PMID"))
        );
        assert_eq!(rows[1].pmid, "24200969");
        assert!(!rows[1].title.trim().is_empty());
        assert_eq!(rows[1].score, Some(0.7));
        assert_eq!(rows[1].source_local_position, 1);
        assert!(
            rows[1]
                .abstract_snippet
                .as_deref()
                .is_some_and(|snippet| snippet.contains("Fallback title text"))
        );
    }

    #[tokio::test]
    async fn litsense2_candidates_apply_hydrated_journal_and_date_filters() {
        let _guard = lock_env().await;
        let litsense2 = MockServer::start().await;
        let pubmed = MockServer::start().await;
        let _litsense2_base = set_env_var("BIOMCP_LITSENSE2_BASE", Some(&litsense2.uri()));
        let _pubmed_base = set_env_var("BIOMCP_PUBMED_BASE", Some(&pubmed.uri()));

        Mock::given(method("GET"))
            .and(path("/sentences/"))
            .and(query_param("query", "Hirschsprung disease"))
            .and(query_param("rerank", "true"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "pmid": 22663011,
                    "pmcid": "PMC9984800",
                    "text": "Hydrated row",
                    "score": 0.9,
                    "section": "INTRO",
                    "annotations": []
                },
                {
                    "pmid": 24200969,
                    "pmcid": null,
                    "text": "Fallback row",
                    "score": 0.7,
                    "section": null,
                    "annotations": null
                }
            ])))
            .expect(1)
            .mount(&litsense2)
            .await;

        Mock::given(method("GET"))
            .and(path("/esummary.fcgi"))
            .and(query_param("db", "pubmed"))
            .and(query_param("retmode", "json"))
            .and(query_param("id", "22663011,24200969"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "result": {
                    "uids": ["22663011", "24200969"],
                    "22663011": {
                        "uid": "22663011",
                        "title": "Hydrated LitSense2 title",
                        "sortpubdate": "2024/01/15 00:00",
                        "pubdate": "2024 Jan 15",
                        "fulljournalname": "Journal One",
                        "source": "J1"
                    },
                    "24200969": {
                        "uid": "24200969",
                        "title": "Fallback title",
                        "sortpubdate": "2023/01/15 00:00",
                        "pubdate": "2023 Jan 15",
                        "fulljournalname": "Journal Two",
                        "source": "J2"
                    }
                }
            })))
            .expect(1)
            .mount(&pubmed)
            .await;

        let rows = search_litsense2_candidates(
            &ArticleSearchFilters {
                keyword: Some("Hirschsprung disease".into()),
                journal: Some("Journal One".into()),
                date_from: Some("2024".into()),
                date_to: Some("2024-12".into()),
                exclude_retracted: true,
                ..empty_filters()
            },
            10,
        )
        .await
        .expect("litsense2 search should respect hydrated filters");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].pmid, "22663011");
        assert_eq!(rows[0].journal.as_deref(), Some("Journal One"));
        assert_eq!(rows[0].date.as_deref(), Some("2024-01-15"));
    }

    #[tokio::test]
    async fn federated_search_keeps_non_europepmc_matches_under_default_retraction_filter() {
        let _guard = lock_env().await;
        let pubtator = MockServer::start().await;
        let europepmc = MockServer::start().await;
        let s2 = MockServer::start().await;
        let _pubtator_base = set_env_var("BIOMCP_PUBTATOR_BASE", Some(&pubtator.uri()));
        let _europepmc_base = set_env_var("BIOMCP_EUROPEPMC_BASE", Some(&europepmc.uri()));
        let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&s2.uri()));
        let _s2_key = set_env_var("S2_API_KEY", None);

        // S2 is now enabled without a key; return empty results so S2 doesn't
        // interfere with the PubTator/EuropePMC assertion below.
        Mock::given(method("GET"))
            .and(path("/graph/v1/paper/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "total": 0,
                "data": []
            })))
            .mount(&s2)
            .await;

        Mock::given(method("GET"))
            .and(query_param("page", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "results": [{
                    "_id": "pt-1",
                    "pmid": 22663011,
                    "title": "Alternative microexon splicing in metastasis",
                    "journal": "Cancer Cell",
                    "date": "2025-01-01",
                    "score": 42.0
                }],
                "count": 1,
                "total_pages": 1,
                "current": 1,
                "page_size": 25,
                "facets": {}
            })))
            .expect(1)
            .mount(&pubtator)
            .await;

        Mock::given(method("GET"))
            .and(query_param("page", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "results": [],
                "count": 1,
                "total_pages": 1,
                "current": 2,
                "page_size": 25,
                "facets": {}
            })))
            .expect(1)
            .mount(&pubtator)
            .await;

        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param(
                "query",
                "alternative microexon splicing metastasis AND NOT PUB_TYPE:\"retracted publication\"",
            ))
            .and(query_param("format", "json"))
            .and(query_param("page", "1"))
            .and(query_param("pageSize", "25"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "hitCount": 1,
                "resultList": {
                    "result": [{
                        "id": "EP-1",
                        "pmid": "22663012",
                        "title": "Europe PMC match",
                        "journalTitle": "Nature",
                        "firstPublicationDate": "2024-01-01",
                        "citedByCount": 25,
                        "pubType": "journal article"
                    }]
                }
            })))
            .expect(1)
            .mount(&europepmc)
            .await;

        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param("page", "2"))
            .and(query_param("format", "json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "hitCount": 1,
                "resultList": {
                    "result": []
                }
            })))
            .expect(1)
            .mount(&europepmc)
            .await;

        let page = search_page(
            &ArticleSearchFilters {
                keyword: Some("alternative microexon splicing metastasis".into()),
                exclude_retracted: true,
                ..empty_filters()
            },
            5,
            0,
            ArticleSourceFilter::All,
        )
        .await
        .expect("federated search should succeed");

        assert!(!page.results.is_empty());
        assert!(page.results.iter().any(|row| {
            row.source == ArticleSource::PubTator
                || row.matched_sources.contains(&ArticleSource::PubTator)
        }));
    }

    #[tokio::test]
    async fn federated_search_keyword_includes_litsense2_matches() {
        let _guard = lock_env().await;
        let pubtator = MockServer::start().await;
        let europepmc = MockServer::start().await;
        let pubmed = MockServer::start().await;
        let s2 = MockServer::start().await;
        let litsense2 = MockServer::start().await;
        let _pubtator_base = set_env_var("BIOMCP_PUBTATOR_BASE", Some(&pubtator.uri()));
        let _europepmc_base = set_env_var("BIOMCP_EUROPEPMC_BASE", Some(&europepmc.uri()));
        let _pubmed_base = set_env_var("BIOMCP_PUBMED_BASE", Some(&pubmed.uri()));
        let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&s2.uri()));
        let _litsense2_base = set_env_var("BIOMCP_LITSENSE2_BASE", Some(&litsense2.uri()));
        let _s2_key = set_env_var("S2_API_KEY", None);

        Mock::given(method("GET"))
            .and(path("/graph/v1/paper/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "total": 0,
                "data": []
            })))
            .mount(&s2)
            .await;

        Mock::given(method("GET"))
            .and(query_param("page", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "results": [],
                "count": 0,
                "total_pages": 1,
                "current": 1,
                "page_size": 25,
                "facets": {}
            })))
            .expect(1)
            .mount(&pubtator)
            .await;

        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param("page", "1"))
            .and(query_param("format", "json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "hitCount": 0,
                "resultList": {
                    "result": []
                }
            })))
            .expect(1)
            .mount(&europepmc)
            .await;

        Mock::given(method("GET"))
            .and(path("/esearch.fcgi"))
            .and(query_param("db", "pubmed"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "esearchresult": {
                    "count": "0",
                    "idlist": []
                }
            })))
            .expect(1)
            .mount(&pubmed)
            .await;

        Mock::given(method("GET"))
            .and(path("/sentences/"))
            .and(query_param("query", "Hirschsprung disease"))
            .and(query_param("rerank", "true"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "pmid": 22663011,
                    "pmcid": "PMC9984800",
                    "text": "Hirschsprung disease semantic hit",
                    "score": 0.8,
                    "section": "INTRO",
                    "annotations": []
                }
            ])))
            .expect(1)
            .mount(&litsense2)
            .await;

        Mock::given(method("GET"))
            .and(path("/esummary.fcgi"))
            .and(query_param("db", "pubmed"))
            .and(query_param("retmode", "json"))
            .and(query_param("id", "22663011"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "result": {
                    "uids": ["22663011"],
                    "22663011": {
                        "uid": "22663011",
                        "title": "Hydrated LitSense2 federated title",
                        "sortpubdate": "2024/01/15 00:00",
                        "pubdate": "2024 Jan 15",
                        "fulljournalname": "Journal One",
                        "source": "J1"
                    }
                }
            })))
            .expect(1)
            .mount(&pubmed)
            .await;

        let page = search_page(
            &ArticleSearchFilters {
                keyword: Some("Hirschsprung disease".into()),
                exclude_retracted: true,
                ..empty_filters()
            },
            5,
            0,
            ArticleSourceFilter::All,
        )
        .await
        .expect("federated search should succeed");

        assert_eq!(page.results.len(), 1);
        assert_eq!(page.results[0].source, ArticleSource::LitSense2);
        assert_eq!(
            page.results[0].matched_sources,
            vec![ArticleSource::LitSense2]
        );
        assert_eq!(page.results[0].score, Some(0.8));
    }

    #[tokio::test]
    async fn federated_search_gene_only_skips_litsense2() {
        let _guard = lock_env().await;
        let pubtator = MockServer::start().await;
        let europepmc = MockServer::start().await;
        let pubmed = MockServer::start().await;
        let s2 = MockServer::start().await;
        let litsense2 = MockServer::start().await;
        let _pubtator_base = set_env_var("BIOMCP_PUBTATOR_BASE", Some(&pubtator.uri()));
        let _europepmc_base = set_env_var("BIOMCP_EUROPEPMC_BASE", Some(&europepmc.uri()));
        let _pubmed_base = set_env_var("BIOMCP_PUBMED_BASE", Some(&pubmed.uri()));
        let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&s2.uri()));
        let _litsense2_base = set_env_var("BIOMCP_LITSENSE2_BASE", Some(&litsense2.uri()));
        let _s2_key = set_env_var("S2_API_KEY", None);

        Mock::given(method("GET"))
            .and(path("/graph/v1/paper/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "total": 0,
                "data": []
            })))
            .mount(&s2)
            .await;

        Mock::given(method("GET"))
            .and(query_param("page", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "results": [],
                "count": 0,
                "total_pages": 1,
                "current": 1,
                "page_size": 25,
                "facets": {}
            })))
            .expect(1)
            .mount(&pubtator)
            .await;

        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param("page", "1"))
            .and(query_param("format", "json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "hitCount": 0,
                "resultList": {
                    "result": []
                }
            })))
            .expect(1)
            .mount(&europepmc)
            .await;

        Mock::given(method("GET"))
            .and(path("/esearch.fcgi"))
            .and(query_param("db", "pubmed"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "esearchresult": {
                    "count": "0",
                    "idlist": []
                }
            })))
            .expect(1)
            .mount(&pubmed)
            .await;

        Mock::given(method("GET"))
            .and(path("/sentences/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .expect(0)
            .mount(&litsense2)
            .await;

        let page = search_page(
            &ArticleSearchFilters {
                gene: Some("BRAF".into()),
                exclude_retracted: true,
                ..empty_filters()
            },
            5,
            0,
            ArticleSourceFilter::All,
        )
        .await
        .expect("federated search should succeed");

        assert!(page.results.is_empty());
    }

    #[tokio::test]
    async fn federated_search_includes_pubmed_rows_in_matched_sources() {
        let _guard = lock_env().await;
        let pubtator = MockServer::start().await;
        let europepmc = MockServer::start().await;
        let pubmed = MockServer::start().await;
        let s2 = MockServer::start().await;
        let _pubtator_base = set_env_var("BIOMCP_PUBTATOR_BASE", Some(&pubtator.uri()));
        let _europepmc_base = set_env_var("BIOMCP_EUROPEPMC_BASE", Some(&europepmc.uri()));
        let _pubmed_base = set_env_var("BIOMCP_PUBMED_BASE", Some(&pubmed.uri()));
        let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&s2.uri()));
        let _s2_key = set_env_var("S2_API_KEY", None);

        Mock::given(method("GET"))
            .and(path("/graph/v1/paper/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "total": 0,
                "data": []
            })))
            .mount(&s2)
            .await;

        Mock::given(method("GET"))
            .and(query_param("page", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "results": [{
                    "_id": "pt-1",
                    "pmid": 22663011,
                    "title": "PubTator match",
                    "journal": "Cancer Cell",
                    "date": "2025-01-01",
                    "score": 42.0
                }],
                "count": 1,
                "total_pages": 1,
                "current": 1,
                "page_size": 25,
                "facets": {}
            })))
            .expect(1)
            .mount(&pubtator)
            .await;

        Mock::given(method("GET"))
            .and(query_param("page", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "results": [],
                "count": 1,
                "total_pages": 1,
                "current": 2,
                "page_size": 25,
                "facets": {}
            })))
            .expect(1)
            .mount(&pubtator)
            .await;

        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param(
                "query",
                "alternative microexon splicing metastasis AND NOT PUB_TYPE:\"retracted publication\"",
            ))
            .and(query_param("format", "json"))
            .and(query_param("page", "1"))
            .and(query_param("pageSize", "25"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "hitCount": 1,
                "resultList": {
                    "result": [{
                        "id": "EP-1",
                        "pmid": "22663012",
                        "title": "Europe PMC match",
                        "journalTitle": "Nature",
                        "firstPublicationDate": "2024-01-01",
                        "citedByCount": 25,
                        "pubType": "journal article"
                    }]
                }
            })))
            .expect(1)
            .mount(&europepmc)
            .await;

        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param("page", "2"))
            .and(query_param("format", "json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "hitCount": 1,
                "resultList": {
                    "result": []
                }
            })))
            .expect(1)
            .mount(&europepmc)
            .await;

        Mock::given(method("GET"))
            .and(path("/esearch.fcgi"))
            .and(query_param("db", "pubmed"))
            .and(query_param("retmode", "json"))
            .and(query_param("retstart", "0"))
            .and(query_param("retmax", "100"))
            .and(query_param(
                "term",
                "alternative microexon splicing metastasis NOT retracted publication[pt]",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "esearchresult": {
                    "count": "1",
                    "idlist": ["22663013"]
                }
            })))
            .expect(1)
            .mount(&pubmed)
            .await;

        Mock::given(method("GET"))
            .and(path("/esummary.fcgi"))
            .and(query_param("db", "pubmed"))
            .and(query_param("retmode", "json"))
            .and(query_param("id", "22663013"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "result": {
                    "uids": ["22663013"],
                    "22663013": {
                        "uid": "22663013",
                        "title": "PubMed visible match",
                        "sortpubdate": "2025/03/01 00:00",
                        "fulljournalname": "Science",
                        "source": "Science"
                    }
                }
            })))
            .expect(1)
            .mount(&pubmed)
            .await;

        Mock::given(method("GET"))
            .and(path("/esearch.fcgi"))
            .and(query_param("db", "pubmed"))
            .and(query_param("retmode", "json"))
            .and(query_param("retstart", "1"))
            .and(query_param("retmax", "100"))
            .and(query_param(
                "term",
                "alternative microexon splicing metastasis NOT retracted publication[pt]",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "esearchresult": {
                    "count": "1",
                    "idlist": []
                }
            })))
            .expect(1)
            .mount(&pubmed)
            .await;

        let page = search_page(
            &ArticleSearchFilters {
                keyword: Some("alternative microexon splicing metastasis".into()),
                exclude_retracted: true,
                ..empty_filters()
            },
            5,
            0,
            ArticleSourceFilter::All,
        )
        .await
        .expect("federated search should succeed");

        assert!(!page.results.is_empty());
        assert!(page.results.iter().any(|row| {
            row.source == ArticleSource::PubMed
                || row.matched_sources.contains(&ArticleSource::PubMed)
        }));
    }

    #[test]
    fn merge_federated_pages_preserves_known_retraction_status_from_later_duplicate() {
        let pubtator_page = SearchPage::offset(
            vec![row_with(
                "200",
                ArticleSource::PubTator,
                Some("2025-01-01"),
                Some(1),
                None,
            )],
            Some(1),
        );
        let europe_page = SearchPage::offset(
            vec![row_with(
                "200",
                ArticleSource::EuropePmc,
                Some("2025-01-01"),
                Some(10),
                Some(true),
            )],
            Some(1),
        );

        let merged = merge_federated_pages(
            Ok(pubtator_page),
            Ok(europe_page),
            None,
            Ok(Vec::new()),
            Ok(Vec::new()),
            10,
            0,
            &empty_filters(),
        )
        .expect("federated merge should succeed");

        assert_eq!(merged.results.len(), 1);
        assert_eq!(merged.results[0].source, ArticleSource::PubTator);
        assert_eq!(merged.results[0].is_retracted, Some(true));
    }

    #[test]
    fn article_search_result_serializes_unknown_retraction_as_null() {
        let row = row_with(
            "100",
            ArticleSource::PubTator,
            Some("2025-01-01"),
            Some(1),
            None,
        );

        let value = serde_json::to_value(&row).expect("search row should serialize");
        assert!(value.get("is_retracted").is_some());
        assert!(value["is_retracted"].is_null());
    }

    #[test]
    fn merge_article_candidates_dedups_transitively_across_identifiers() {
        let merged = merge_article_candidates(vec![
            ArticleSearchResult {
                pmid: "100".into(),
                pmcid: Some("PMC100".into()),
                doi: None,
                title: "Primary PMID row".into(),
                journal: Some("Journal".into()),
                date: Some("2025-01-01".into()),
                citation_count: None,
                influential_citation_count: None,
                source: ArticleSource::PubTator,
                score: Some(42.0),
                is_retracted: None,
                abstract_snippet: None,
                ranking: None,
                matched_sources: vec![ArticleSource::PubTator],
                normalized_title: "primary pmid row".into(),
                normalized_abstract: String::new(),
                publication_type: None,
                source_local_position: 3,
            },
            ArticleSearchResult {
                pmid: String::new(),
                pmcid: Some("PMC100".into()),
                doi: Some("10.1000/example".into()),
                title: "Europe metadata".into(),
                journal: Some("Journal".into()),
                date: Some("2025-01-01".into()),
                citation_count: Some(15),
                influential_citation_count: None,
                source: ArticleSource::EuropePmc,
                score: None,
                is_retracted: Some(false),
                abstract_snippet: Some("Europe abstract".into()),
                ranking: None,
                matched_sources: vec![ArticleSource::EuropePmc],
                normalized_title: "europe metadata".into(),
                normalized_abstract: "europe abstract".into(),
                publication_type: Some("Review".into()),
                source_local_position: 1,
            },
            ArticleSearchResult {
                pmid: String::new(),
                pmcid: None,
                doi: Some("10.1000/example".into()),
                title: "Semantic Scholar metadata".into(),
                journal: Some("Journal".into()),
                date: Some("2025-01-01".into()),
                citation_count: Some(99),
                influential_citation_count: Some(7),
                source: ArticleSource::SemanticScholar,
                score: None,
                is_retracted: None,
                abstract_snippet: Some("Semantic Scholar abstract".into()),
                ranking: None,
                matched_sources: vec![ArticleSource::SemanticScholar],
                normalized_title: "semantic scholar metadata".into(),
                normalized_abstract: "semantic scholar abstract".into(),
                publication_type: None,
                source_local_position: 2,
            },
        ]);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].row.source, ArticleSource::PubTator);
        assert_eq!(merged[0].row.pmid, "100");
        assert_eq!(merged[0].row.pmcid.as_deref(), Some("PMC100"));
        assert_eq!(merged[0].row.doi.as_deref(), Some("10.1000/example"));
        assert_eq!(
            merged[0].row.matched_sources,
            vec![
                ArticleSource::PubTator,
                ArticleSource::EuropePmc,
                ArticleSource::SemanticScholar,
            ]
        );
        assert_eq!(merged[0].row.citation_count, Some(15));
        assert_eq!(merged[0].row.influential_citation_count, Some(7));
        assert_eq!(
            merged[0].row.abstract_snippet.as_deref(),
            Some("Europe abstract")
        );
        assert_eq!(merged[0].row.is_retracted, Some(false));
        assert_eq!(merged[0].row.source_local_position, 1);
    }

    #[test]
    fn merge_article_candidates_keeps_min_source_local_position() {
        let mut europe = row("100", ArticleSource::EuropePmc);
        europe.source_local_position = 3;
        let mut pubmed = row("100", ArticleSource::PubMed);
        pubmed.source_local_position = 1;

        let merged = merge_article_candidates(vec![europe, pubmed]);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].row.source_local_position, 1);
        assert_eq!(
            merged[0].row.matched_sources,
            vec![ArticleSource::EuropePmc, ArticleSource::PubMed]
        );
    }

    #[test]
    fn pubmed_led_rescue_preserves_per_source_positions_through_merge() {
        let mut europe = row("100", ArticleSource::EuropePmc);
        europe.source_local_position = 4;
        let mut pubmed = row("100", ArticleSource::PubMed);
        pubmed.source_local_position = 0;
        let mut semantic = row("100", ArticleSource::SemanticScholar);
        semantic.source_local_position = 2;

        let merged = merge_article_candidates(vec![europe, pubmed, semantic]);

        assert_eq!(merged.len(), 1);
        assert_eq!(
            merged[0].source_positions,
            vec![
                ArticleSourcePosition {
                    source: ArticleSource::EuropePmc,
                    local_position: 4,
                },
                ArticleSourcePosition {
                    source: ArticleSource::PubMed,
                    local_position: 0,
                },
                ArticleSourcePosition {
                    source: ArticleSource::SemanticScholar,
                    local_position: 2,
                },
            ]
        );
    }

    #[test]
    fn keyword_tokenization_decomposes_multi_word_into_separate_anchors() {
        let mut filters = empty_filters();
        filters.keyword = Some("LB-100 HDAC inhibitor".into());

        assert_eq!(
            build_anchor_set(&filters),
            vec![
                "lb100".to_string(),
                "hdac".to_string(),
                "inhibitor".to_string()
            ]
        );
    }

    #[test]
    fn keyword_tokenization_dedups_structured_filter_overlap() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());
        filters.keyword = Some("BRAF melanoma".into());

        assert_eq!(
            build_anchor_set(&filters),
            vec!["braf".to_string(), "melanoma".to_string()]
        );
    }

    #[test]
    fn multi_concept_keyword_partial_match_scores_nonzero() {
        let mut filters = empty_filters();
        filters.keyword = Some("LB-100 HDAC inhibitor".into());

        let mut rows = vec![ArticleSearchResult {
            normalized_title: "lb100 sensitization and hdac activity".into(),
            ..row("100", ArticleSource::EuropePmc)
        }];

        rank_result_rows_by_directness(&mut rows, &filters);

        let ranking = rows[0].ranking.as_ref().expect("ranking should be present");
        assert_eq!(ranking.anchor_count, 3);
        assert_eq!(ranking.combined_anchor_hits, 2);
        assert_eq!(ranking.directness_tier, 1);
    }

    #[test]
    fn multi_concept_keyword_all_tokens_in_title_scores_tier3() {
        let mut filters = empty_filters();
        filters.keyword = Some("LB-100 HDAC inhibitor".into());

        let mut rows = vec![ArticleSearchResult {
            normalized_title: "lb100 hdac inhibitor activity".into(),
            ..row("100", ArticleSource::EuropePmc)
        }];

        rank_result_rows_by_directness(&mut rows, &filters);

        let ranking = rows[0].ranking.as_ref().expect("ranking should be present");
        assert_eq!(ranking.anchor_count, 3);
        assert_eq!(ranking.title_anchor_hits, 3);
        assert_eq!(ranking.directness_tier, 3);
    }

    #[test]
    fn compound_name_variants_match_symmetrically_in_ranking() {
        let mut filters = empty_filters();
        filters.keyword = Some("LB-100".into());

        let mut rows = vec![ArticleSearchResult {
            normalized_title: "lb100 sensitization response".into(),
            ..row("100", ArticleSource::EuropePmc)
        }];

        rank_result_rows_by_directness(&mut rows, &filters);

        let ranking = rows[0].ranking.as_ref().expect("ranking should be present");
        assert_eq!(ranking.anchor_count, 1);
        assert_eq!(ranking.title_anchor_hits, 1);
        assert_eq!(ranking.directness_tier, 3);
    }

    mod ranking_calibration {
        use super::*;

        fn calibration_row(
            pmid: &str,
            source: ArticleSource,
            title: &str,
            abstract_snippet: &str,
            source_local_position: usize,
        ) -> ArticleSearchResult {
            let mut row = row(pmid, source);
            row.title = title.to_string();
            row.normalized_title = crate::transform::article::normalize_article_search_text(title);
            row.abstract_snippet =
                (!abstract_snippet.is_empty()).then(|| abstract_snippet.to_string());
            row.normalized_abstract =
                crate::transform::article::normalize_article_search_text(abstract_snippet);
            row.matched_sources = vec![source];
            row.source_local_position = source_local_position;
            row
        }

        fn lb100_mesh_synonym_fixture() -> (ArticleSearchFilters, Vec<ArticleSearchResult>) {
            let mut filters = empty_filters();
            filters.keyword = Some("hepatic steatosis PP2A phosphatase inhibitor".into());
            filters.ranking.requested_mode = Some(ArticleRankingMode::Lexical);

            let pubmed_answer = calibration_row(
                "31832001",
                ArticleSource::PubMed,
                "LB100 ameliorates nonalcoholic fatty liver disease via the AMPK/Sirt1 pathway",
                "",
                0,
            );
            let literal_match_competitor = calibration_row(
                "99000001",
                ArticleSource::EuropePmc,
                "PP2A phosphatase inhibitor response in hepatic steatosis",
                "",
                1,
            );

            (filters, vec![literal_match_competitor, pubmed_answer])
        }

        fn lb100_anchor_count_fixture() -> (ArticleSearchFilters, Vec<ArticleSearchResult>) {
            let mut filters = empty_filters();
            filters.drug = Some("LB-100".into());
            filters.disease = Some("hepatic steatosis".into());
            filters.keyword = Some("LB-100 hepatic steatosis AMPK".into());
            filters.ranking.requested_mode = Some(ArticleRankingMode::Lexical);

            let pubmed_answer = calibration_row(
                "31832001",
                ArticleSource::PubMed,
                "LB100 ameliorates nonalcoholic fatty liver disease via the AMPK/Sirt1 pathway",
                "",
                0,
            );
            let literal_match_competitor = calibration_row(
                "99000002",
                ArticleSource::EuropePmc,
                "Dietary intervention for hepatic steatosis progression",
                "",
                1,
            );

            (filters, vec![literal_match_competitor, pubmed_answer])
        }

        fn row_by_pmid<'a>(rows: &'a [ArticleSearchResult], pmid: &str) -> &'a ArticleSearchResult {
            rows.iter()
                .find(|row| row.pmid == pmid)
                .unwrap_or_else(|| panic!("missing row for PMID {pmid}"))
        }

        fn row_position(rows: &[ArticleSearchResult], pmid: &str) -> usize {
            rows.iter()
                .position(|row| row.pmid == pmid)
                .unwrap_or_else(|| panic!("missing row for PMID {pmid}"))
        }

        fn worked_example_row(
            pmid: &str,
            source: ArticleSource,
            title: &str,
            abstract_snippet: &str,
            source_local_position: usize,
            citations: u64,
            score: Option<f64>,
        ) -> ArticleSearchResult {
            let mut row =
                calibration_row(pmid, source, title, abstract_snippet, source_local_position);
            row.citation_count = Some(citations);
            row.score = score;
            row
        }

        fn hybrid_worked_example_fixture() -> (ArticleSearchFilters, Vec<ArticleSearchResult>) {
            let mut filters = empty_filters();
            filters.keyword = Some("congenital absence ganglion cells".into());
            filters.ranking.requested_mode = Some(ArticleRankingMode::Hybrid);

            let paper_a = worked_example_row(
                "1001",
                ArticleSource::LitSense2,
                "Enteric neural crest migration in gut development",
                "",
                4,
                53,
                Some(0.95),
            );
            let paper_b = worked_example_row(
                "1002",
                ArticleSource::EuropePmc,
                "Congenital absence ganglion cells in rectal biopsy",
                "",
                0,
                0,
                None,
            );
            let paper_c = worked_example_row(
                "1003",
                ArticleSource::LitSense2,
                "Ganglion deficiency pathology cohort",
                "Congenital absence ganglion cells were confirmed across the cohort",
                2,
                231,
                Some(0.72),
            );
            let paper_d = worked_example_row(
                "1004",
                ArticleSource::EuropePmc,
                "Ganglion review note",
                "",
                7,
                10,
                None,
            );
            let paper_e = worked_example_row(
                "1005",
                ArticleSource::LitSense2,
                "Enteric neuropathy pathway atlas",
                "",
                11,
                6,
                Some(0.80),
            );

            (filters, vec![paper_a, paper_b, paper_c, paper_d, paper_e])
        }

        #[test]
        fn mesh_synonym_zero_overlap_pubmed_row_does_not_rescue_above_literal_competitor() {
            let (filters, candidates) = lb100_mesh_synonym_fixture();
            let page = finalize_article_candidates(candidates, 10, 0, None, &filters);
            let pubmed = row_by_pmid(&page.results, "31832001");
            let competitor = row_by_pmid(&page.results, "99000001");

            let pubmed_ranking = pubmed.ranking.as_ref().expect("ranking should be present");
            let competitor_ranking = competitor
                .ranking
                .as_ref()
                .expect("ranking should be present");
            assert_eq!(pubmed_ranking.directness_tier, 0);
            assert_eq!(pubmed_ranking.title_anchor_hits, 0);
            assert_eq!(pubmed_ranking.combined_anchor_hits, 0);
            assert!(
                competitor_ranking.directness_tier > pubmed_ranking.directness_tier,
                "the baseline lexical signal should still be weaker on the PubMed answer itself",
            );

            let pubmed_pos = row_position(&page.results, "31832001");
            let competitor_pos = row_position(&page.results, "99000001");
            assert!(
                pubmed_pos > competitor_pos,
                "a zero-overlap PubMed row must not rescue above the literal-match Europe PMC competitor",
            );
            assert!(!pubmed_ranking.pubmed_rescue);
            assert_eq!(pubmed_ranking.pubmed_rescue_kind, None);
            assert_eq!(pubmed_ranking.pubmed_source_position, None);
        }

        #[test]
        fn anchor_count_pubmed_rescue_surfaces_above_higher_title_hit_competitor() {
            let (filters, candidates) = lb100_anchor_count_fixture();
            let page = finalize_article_candidates(candidates, 10, 0, None, &filters);
            let pubmed = row_by_pmid(&page.results, "31832001");
            let competitor = row_by_pmid(&page.results, "99000002");

            let pubmed_ranking = pubmed.ranking.as_ref().expect("ranking should be present");
            let competitor_ranking = competitor
                .ranking
                .as_ref()
                .expect("ranking should be present");

            assert_eq!(pubmed_ranking.directness_tier, 1);
            assert_eq!(competitor_ranking.directness_tier, 1);
            assert_eq!(pubmed_ranking.title_anchor_hits, 2);
            assert_eq!(competitor_ranking.title_anchor_hits, 3);
            assert!(
                competitor_ranking.title_anchor_hits > pubmed_ranking.title_anchor_hits,
                "the baseline lexical signal should still favor the Europe PMC competitor itself",
            );

            let pubmed_pos = row_position(&page.results, "31832001");
            let competitor_pos = row_position(&page.results, "99000002");
            assert!(
                pubmed_pos < competitor_pos,
                "a top-ranked PubMed-only weak lexical row should rescue above the higher-title-hit Europe PMC competitor",
            );
            assert!(pubmed_ranking.pubmed_rescue);
            assert_eq!(
                pubmed_ranking.pubmed_rescue_kind,
                Some(ArticlePubMedRescueKind::Unique)
            );
            assert_eq!(pubmed_ranking.pubmed_source_position, Some(0));
        }

        #[test]
        fn zero_overlap_pubmed_unique_position_zero_is_not_rescued() {
            let mut filters = empty_filters();
            filters.keyword = Some("ncRNA promoter prediction tools".into());
            filters.ranking.requested_mode = Some(ArticleRankingMode::Lexical);

            let pubmed = calibration_row(
                "41721224",
                ArticleSource::PubMed,
                "Genome-wide survey and expression analysis of peptides containing tyrosine sulfation in human and animal proteins",
                "",
                0,
            );

            let page = finalize_article_candidates(vec![pubmed], 10, 0, None, &filters);
            let pubmed = row_by_pmid(&page.results, "41721224");
            let pubmed_ranking = pubmed.ranking.as_ref().expect("ranking should be present");

            assert_eq!(pubmed_ranking.combined_anchor_hits, 0);
            assert_eq!(pubmed_ranking.directness_tier, 0);
            assert!(!pubmed_ranking.pubmed_rescue);
            assert_eq!(pubmed_ranking.pubmed_rescue_kind, None);
            assert_eq!(pubmed_ranking.pubmed_source_position, None);
        }

        #[test]
        fn exactly_one_anchor_hit_pubmed_unique_position_zero_is_rescued() {
            let mut filters = empty_filters();
            filters.gene = Some("AMPK".into());
            filters.disease = Some("hepatic steatosis".into());
            filters.keyword = Some("PP2A inhibitor".into());
            filters.ranking.requested_mode = Some(ArticleRankingMode::Lexical);

            let pubmed = calibration_row(
                "99000003",
                ArticleSource::PubMed,
                "AMPK signaling in hepatocytes",
                "",
                0,
            );
            let competitor = calibration_row(
                "99000004",
                ArticleSource::EuropePmc,
                "PP2A inhibitor response in hepatic steatosis",
                "",
                1,
            );

            let page = finalize_article_candidates(vec![competitor, pubmed], 10, 0, None, &filters);
            let pubmed = row_by_pmid(&page.results, "99000003");
            let pubmed_ranking = pubmed.ranking.as_ref().expect("ranking should be present");

            assert_eq!(pubmed_ranking.combined_anchor_hits, 1);
            assert_eq!(pubmed_ranking.directness_tier, 1);
            assert!(pubmed_ranking.pubmed_rescue);
            assert_eq!(
                pubmed_ranking.pubmed_rescue_kind,
                Some(ArticlePubMedRescueKind::Unique)
            );
            assert_eq!(row_position(&page.results, "99000003"), 0);
            assert_eq!(row_position(&page.results, "99000004"), 1);
        }

        #[test]
        fn pubmed_led_row_rescues_when_pubmed_position_is_strictly_best() {
            let (filters, mut candidates) = lb100_anchor_count_fixture();
            let mut europe_duplicate = calibration_row(
                "31832001",
                ArticleSource::EuropePmc,
                "LB100 ameliorates nonalcoholic fatty liver disease via the AMPK/Sirt1 pathway",
                "",
                3,
            );
            europe_duplicate.pmcid = Some("PMC31832001".into());
            candidates.push(europe_duplicate);

            let page = finalize_article_candidates(candidates, 10, 0, None, &filters);
            let pubmed_led_pos = row_position(&page.results, "31832001");
            let competitor_pos = row_position(&page.results, "99000002");

            assert!(
                pubmed_led_pos < competitor_pos,
                "a merged row should rescue when PubMed found it first and the non-PubMed duplicate trails behind",
            );
            let pubmed_led = row_by_pmid(&page.results, "31832001");
            let pubmed_led_ranking = pubmed_led
                .ranking
                .as_ref()
                .expect("ranking should be present");
            assert!(pubmed_led_ranking.pubmed_rescue);
            assert_eq!(
                pubmed_led_ranking.pubmed_rescue_kind,
                Some(ArticlePubMedRescueKind::Led)
            );
            assert_eq!(pubmed_led_ranking.pubmed_source_position, Some(0));
        }

        #[test]
        fn shared_source_tie_does_not_count_as_pubmed_led() {
            let (filters, mut candidates) = lb100_anchor_count_fixture();
            let europe_tied_duplicate = calibration_row(
                "31832001",
                ArticleSource::EuropePmc,
                "LB100 ameliorates nonalcoholic fatty liver disease via the AMPK/Sirt1 pathway",
                "",
                0,
            );
            candidates.push(europe_tied_duplicate);

            let page = finalize_article_candidates(candidates, 10, 0, None, &filters);
            let pubmed_led_pos = row_position(&page.results, "31832001");
            let competitor_pos = row_position(&page.results, "99000002");

            assert!(
                pubmed_led_pos > competitor_pos,
                "a shared-source tie at position 0 must not count as PubMed-led rescue",
            );
            let pubmed_led = row_by_pmid(&page.results, "31832001");
            let pubmed_led_ranking = pubmed_led
                .ranking
                .as_ref()
                .expect("ranking should be present");
            assert!(!pubmed_led_ranking.pubmed_rescue);
            assert_eq!(pubmed_led_ranking.pubmed_rescue_kind, None);
            assert_eq!(pubmed_led_ranking.pubmed_source_position, None);
        }

        #[test]
        fn shared_source_row_with_better_non_pubmed_position_does_not_rescue() {
            let (filters, mut candidates) = lb100_anchor_count_fixture();
            let europe_leading_duplicate = calibration_row(
                "31832001",
                ArticleSource::EuropePmc,
                "LB100 ameliorates nonalcoholic fatty liver disease via the AMPK/Sirt1 pathway",
                "",
                0,
            );
            let mut pubmed_nonleading = calibration_row(
                "31832001",
                ArticleSource::PubMed,
                "LB100 ameliorates nonalcoholic fatty liver disease via the AMPK/Sirt1 pathway",
                "",
                1,
            );
            pubmed_nonleading.pmcid = Some("PMC31832001".into());

            candidates.retain(|row| row.pmid != "31832001");
            candidates.push(europe_leading_duplicate);
            candidates.push(pubmed_nonleading);

            let page = finalize_article_candidates(candidates, 10, 0, None, &filters);
            let merged_pos = row_position(&page.results, "31832001");
            let competitor_pos = row_position(&page.results, "99000002");

            assert!(
                merged_pos > competitor_pos,
                "a merged row where Europe PMC leads PubMed must not rescue",
            );
            let merged = row_by_pmid(&page.results, "31832001");
            let merged_ranking = merged.ranking.as_ref().expect("ranking should be present");
            assert!(!merged_ranking.pubmed_rescue);
            assert_eq!(merged_ranking.pubmed_rescue_kind, None);
            assert_eq!(merged_ranking.pubmed_source_position, None);
        }

        #[test]
        fn pubmed_nonfirst_position_does_not_rescue() {
            let (filters, mut candidates) = lb100_mesh_synonym_fixture();
            let pubmed = candidates
                .iter_mut()
                .find(|row| row.pmid == "31832001")
                .expect("PubMed fixture row should be present");
            pubmed.source_local_position = 1;

            let page = finalize_article_candidates(candidates, 10, 0, None, &filters);
            let pubmed_pos = row_position(&page.results, "31832001");
            let competitor_pos = row_position(&page.results, "99000001");

            assert!(
                pubmed_pos > competitor_pos,
                "PubMed rows beyond local position 0 must not rescue",
            );
            let pubmed = row_by_pmid(&page.results, "31832001");
            let pubmed_ranking = pubmed.ranking.as_ref().expect("ranking should be present");
            assert!(!pubmed_ranking.pubmed_rescue);
            assert_eq!(pubmed_ranking.pubmed_rescue_kind, None);
            assert_eq!(pubmed_ranking.pubmed_source_position, None);
        }

        #[test]
        fn rescue_metadata_records_kind_and_position() {
            let (filters, mut led_candidates) = lb100_anchor_count_fixture();
            led_candidates.push(calibration_row(
                "31832001",
                ArticleSource::EuropePmc,
                "LB100 ameliorates nonalcoholic fatty liver disease via the AMPK/Sirt1 pathway",
                "",
                3,
            ));
            let led_page = finalize_article_candidates(led_candidates, 10, 0, None, &filters);
            let led = row_by_pmid(&led_page.results, "31832001")
                .ranking
                .as_ref()
                .expect("ranking should be present");
            assert_eq!(led.pubmed_rescue_kind, Some(ArticlePubMedRescueKind::Led));
            assert_eq!(led.pubmed_source_position, Some(0));

            let (unique_filters, unique_candidates) = lb100_mesh_synonym_fixture();
            let unique_page =
                finalize_article_candidates(unique_candidates, 10, 0, None, &unique_filters);
            let unique = row_by_pmid(&unique_page.results, "31832001")
                .ranking
                .as_ref()
                .expect("ranking should be present");
            assert_eq!(unique.pubmed_rescue_kind, None);
            assert_eq!(unique.pubmed_source_position, None);
        }

        #[test]
        fn rescued_rows_still_use_lexical_and_citation_tiebreaks() {
            let mut filters = empty_filters();
            filters.gene = Some("BRAF".into());
            filters.keyword = Some("melanoma review".into());
            filters.ranking.requested_mode = Some(ArticleRankingMode::Lexical);

            let mut weak = calibration_row("100", ArticleSource::PubMed, "BRAF case report", "", 0);
            weak.citation_count = Some(1);

            let mut stronger = calibration_row(
                "200",
                ArticleSource::PubMed,
                "BRAF review of outcomes",
                "",
                0,
            );
            stronger.citation_count = Some(5);
            stronger.publication_type = Some("Review".into());

            let mut cited =
                calibration_row("300", ArticleSource::PubMed, "melanoma case report", "", 0);
            cited.citation_count = Some(50);

            let page =
                finalize_article_candidates(vec![weak, cited, stronger], 10, 0, None, &filters);

            assert_eq!(
                page.results
                    .iter()
                    .map(|row| row.pmid.as_str())
                    .collect::<Vec<_>>(),
                vec!["200", "300", "100"]
            );
        }

        #[test]
        fn hybrid_default_weights_orders_example_one() {
            let (filters, candidates) = hybrid_worked_example_fixture();
            let page = finalize_article_candidates(candidates, 10, 0, None, &filters);

            assert_eq!(
                page.results
                    .iter()
                    .map(|row| row.pmid.as_str())
                    .collect::<Vec<_>>(),
                vec!["1003", "1001", "1002", "1005", "1004"]
            );
        }

        #[test]
        fn lexical_mode_matches_current_ordering() {
            let (mut filters, candidates) = hybrid_worked_example_fixture();
            filters.ranking.requested_mode = Some(ArticleRankingMode::Lexical);
            let page = finalize_article_candidates(candidates, 10, 0, None, &filters);

            assert_eq!(
                page.results
                    .iter()
                    .map(|row| row.pmid.as_str())
                    .collect::<Vec<_>>(),
                vec!["1002", "1003", "1004", "1001", "1005"]
            );
        }

        #[test]
        fn semantic_mode_prefers_score_before_lexical_fallback() {
            let (mut filters, candidates) = hybrid_worked_example_fixture();
            filters.ranking.requested_mode = Some(ArticleRankingMode::Semantic);
            let page = finalize_article_candidates(candidates, 10, 0, None, &filters);

            assert_eq!(
                page.results
                    .iter()
                    .map(|row| row.pmid.as_str())
                    .collect::<Vec<_>>(),
                vec!["1001", "1005", "1003", "1002", "1004"]
            );
        }

        #[test]
        fn hybrid_entity_only_falls_back_without_nan() {
            let mut filters = empty_filters();
            filters.disease = Some("Hirschsprung disease".into());
            filters.ranking.requested_mode = Some(ArticleRankingMode::Hybrid);

            let rows = vec![
                worked_example_row(
                    "2001",
                    ArticleSource::EuropePmc,
                    "Hirschsprung disease review",
                    "",
                    0,
                    5,
                    None,
                ),
                worked_example_row(
                    "2002",
                    ArticleSource::EuropePmc,
                    "Enteric neuropathy mechanisms",
                    "Hirschsprung disease cases were reviewed in the cohort",
                    0,
                    100,
                    None,
                ),
                worked_example_row(
                    "2003",
                    ArticleSource::EuropePmc,
                    "Ganglion development note",
                    "",
                    0,
                    1000,
                    None,
                ),
            ];

            let page = finalize_article_candidates(rows, 10, 0, None, &filters);

            assert_eq!(
                page.results
                    .iter()
                    .map(|row| row.pmid.as_str())
                    .collect::<Vec<_>>(),
                vec!["2001", "2002", "2003"]
            );
            assert!(page.results.iter().all(|row| {
                let ranking = row.ranking.as_ref().expect("ranking should be present");
                ranking.semantic_score == Some(0.0)
                    && ranking
                        .composite_score
                        .is_some_and(|score| score.is_finite())
            }));
        }

        #[test]
        fn hybrid_custom_weights_shift_ordering() {
            let (mut filters, candidates) = hybrid_worked_example_fixture();
            filters.ranking = ArticleRankingOptions::from_inputs(
                Some("hybrid"),
                Some(0.1),
                Some(0.6),
                Some(0.2),
                Some(0.1),
            )
            .expect("options should parse");
            let page = finalize_article_candidates(candidates, 10, 0, None, &filters);

            assert_eq!(
                page.results
                    .iter()
                    .map(|row| row.pmid.as_str())
                    .collect::<Vec<_>>(),
                vec!["1003", "1002", "1004", "1001", "1005"]
            );
        }

        #[test]
        fn hybrid_scoring_is_zero_safe() {
            let mut filters = empty_filters();
            filters.keyword = Some("ganglion".into());
            filters.ranking.requested_mode = Some(ArticleRankingMode::Hybrid);

            let rows = vec![
                worked_example_row(
                    "3001",
                    ArticleSource::LitSense2,
                    "Ganglion atlas",
                    "",
                    0,
                    0,
                    Some(0.8),
                ),
                worked_example_row(
                    "3002",
                    ArticleSource::EuropePmc,
                    "Ganglion case note",
                    "",
                    0,
                    0,
                    None,
                ),
            ];

            let page = finalize_article_candidates(rows, 10, 0, None, &filters);
            assert!(page.results.iter().all(|row| {
                let ranking = row.ranking.as_ref().expect("ranking should be present");
                ranking.citation_score == Some(0.0)
                    && ranking.position_score == Some(0.0)
                    && ranking
                        .composite_score
                        .is_some_and(|score| score.is_finite())
            }));
        }

        #[test]
        fn hybrid_uses_litsense2_signal_for_semantic_score() {
            let mut filters = empty_filters();
            filters.keyword = Some("BRAF melanoma".into());
            filters.ranking.requested_mode = Some(ArticleRankingMode::Hybrid);

            let pubtator_only = worked_example_row(
                "4001",
                ArticleSource::PubTator,
                "BRAF melanoma resistance map",
                "",
                0,
                10,
                Some(285.0),
            );
            let litsense2_only = worked_example_row(
                "4002",
                ArticleSource::LitSense2,
                "BRAF melanoma pathway atlas",
                "",
                1,
                5,
                Some(0.85),
            );
            let pubtator_duplicate = worked_example_row(
                "4003",
                ArticleSource::PubTator,
                "Merged BRAF melanoma evidence",
                "",
                0,
                12,
                Some(285.0),
            );
            let litsense2_duplicate = worked_example_row(
                "4003",
                ArticleSource::LitSense2,
                "Merged BRAF melanoma evidence",
                "",
                2,
                12,
                Some(0.95),
            );

            let page = finalize_article_candidates(
                vec![
                    pubtator_only,
                    litsense2_only,
                    pubtator_duplicate,
                    litsense2_duplicate,
                ],
                10,
                0,
                None,
                &filters,
            );

            let pubtator = row_by_pmid(&page.results, "4001");
            let litsense2 = row_by_pmid(&page.results, "4002");
            let merged = row_by_pmid(&page.results, "4003");

            assert_eq!(
                pubtator
                    .ranking
                    .as_ref()
                    .expect("ranking should be present")
                    .semantic_score,
                Some(0.0)
            );
            assert_eq!(
                litsense2
                    .ranking
                    .as_ref()
                    .expect("ranking should be present")
                    .semantic_score,
                Some(0.85)
            );
            assert_eq!(merged.score, Some(285.0));
            assert_eq!(
                merged
                    .ranking
                    .as_ref()
                    .expect("ranking should be present")
                    .semantic_score,
                Some(0.95)
            );
        }

        #[test]
        fn semantic_mode_ignores_non_litsense2_raw_scores() {
            let mut filters = empty_filters();
            filters.keyword = Some("BRAF melanoma".into());
            filters.ranking.requested_mode = Some(ArticleRankingMode::Semantic);

            let pubtator_only = worked_example_row(
                "5001",
                ArticleSource::PubTator,
                "BRAF melanoma resistance map",
                "",
                0,
                10,
                Some(285.0),
            );
            let litsense2_only = worked_example_row(
                "5002",
                ArticleSource::LitSense2,
                "BRAF melanoma pathway atlas",
                "",
                1,
                5,
                Some(0.85),
            );

            let page = finalize_article_candidates(
                vec![pubtator_only, litsense2_only],
                10,
                0,
                None,
                &filters,
            );

            assert_eq!(
                page.results
                    .iter()
                    .map(|row| row.pmid.as_str())
                    .collect::<Vec<_>>(),
                vec!["5002", "5001"]
            );

            let litsense2 = row_by_pmid(&page.results, "5002");
            let pubtator = row_by_pmid(&page.results, "5001");

            assert_eq!(
                litsense2
                    .ranking
                    .as_ref()
                    .expect("ranking should be present")
                    .mode,
                Some(ArticleRankingMode::Semantic)
            );
            assert_eq!(
                litsense2
                    .ranking
                    .as_ref()
                    .expect("ranking should be present")
                    .semantic_score,
                Some(0.85)
            );
            assert_eq!(
                pubtator
                    .ranking
                    .as_ref()
                    .expect("ranking should be present")
                    .mode,
                Some(ArticleRankingMode::Semantic)
            );
            assert_eq!(
                pubtator
                    .ranking
                    .as_ref()
                    .expect("ranking should be present")
                    .semantic_score,
                Some(0.0)
            );
        }
    }

    #[test]
    fn directness_ranking_uses_full_title_and_token_boundaries() {
        let mut filters = empty_filters();
        filters.gene = Some("MET".into());
        filters.keyword = Some("ALL".into());

        let long_prefix =
            "This intentionally long prefix exists to push the anchors well past sixty bytes";
        let mut rows = vec![
            ArticleSearchResult {
                pmid: "100".into(),
                pmcid: None,
                doi: None,
                title: format!("{long_prefix} MET ALL response study"),
                journal: Some("Journal A".into()),
                date: Some("2025-01-01".into()),
                citation_count: Some(10),
                influential_citation_count: Some(1),
                source: ArticleSource::EuropePmc,
                score: None,
                is_retracted: Some(false),
                abstract_snippet: Some("Direct abstract".into()),
                ranking: None,
                matched_sources: vec![ArticleSource::EuropePmc],
                normalized_title: format!(
                    "{} met all response study",
                    long_prefix.to_ascii_lowercase()
                ),
                normalized_abstract: "direct abstract".into(),
                publication_type: None,
                source_local_position: 0,
            },
            ArticleSearchResult {
                pmid: "200".into(),
                pmcid: None,
                doi: None,
                title: "Meta-analysis of small molecule therapy".into(),
                journal: Some("Journal B".into()),
                date: Some("2025-01-01".into()),
                citation_count: Some(500),
                influential_citation_count: Some(50),
                source: ArticleSource::EuropePmc,
                score: None,
                is_retracted: Some(false),
                abstract_snippet: None,
                ranking: None,
                matched_sources: vec![ArticleSource::EuropePmc],
                normalized_title: "meta-analysis of small molecule therapy".into(),
                normalized_abstract: String::new(),
                publication_type: Some("Meta-Analysis".into()),
                source_local_position: 1,
            },
            ArticleSearchResult {
                pmid: "300".into(),
                pmcid: None,
                doi: None,
                title: "ALL biomarker response study".into(),
                journal: Some("Journal C".into()),
                date: Some("2025-01-01".into()),
                citation_count: Some(100),
                influential_citation_count: Some(5),
                source: ArticleSource::EuropePmc,
                score: None,
                is_retracted: Some(false),
                abstract_snippet: Some("MET is discussed in the abstract".into()),
                ranking: None,
                matched_sources: vec![ArticleSource::EuropePmc],
                normalized_title: "all biomarker response study".into(),
                normalized_abstract: "met is discussed in the abstract".into(),
                publication_type: None,
                source_local_position: 2,
            },
        ];

        rank_result_rows_by_directness(&mut rows, &filters);

        assert_eq!(rows[0].pmid, "100");
        assert_eq!(
            rows[0]
                .ranking
                .as_ref()
                .map(|ranking| ranking.directness_tier),
            Some(3)
        );
        assert_eq!(
            rows[1]
                .ranking
                .as_ref()
                .map(|ranking| ranking.directness_tier),
            Some(2)
        );
        assert_eq!(
            rows[2]
                .ranking
                .as_ref()
                .map(|ranking| ranking.directness_tier),
            Some(0)
        );
        assert_eq!(
            rows[2]
                .ranking
                .as_ref()
                .map(|ranking| ranking.combined_anchor_hits),
            Some(0)
        );
    }

    #[test]
    fn directness_ranking_prefers_cue_then_citation_then_source_local_position() {
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());
        filters.keyword = Some("melanoma".into());

        let mut rows = vec![
            ArticleSearchResult {
                pmid: "100".into(),
                pmcid: None,
                doi: None,
                title: "BRAF melanoma study".into(),
                journal: Some("Journal A".into()),
                date: Some("2025-01-01".into()),
                citation_count: Some(10),
                influential_citation_count: Some(1),
                source: ArticleSource::EuropePmc,
                score: None,
                is_retracted: Some(false),
                abstract_snippet: None,
                ranking: None,
                matched_sources: vec![ArticleSource::EuropePmc],
                normalized_title: "braf melanoma study".into(),
                normalized_abstract: String::new(),
                publication_type: None,
                source_local_position: 0,
            },
            ArticleSearchResult {
                pmid: "200".into(),
                pmcid: None,
                doi: None,
                title: "BRAF melanoma systematic review".into(),
                journal: Some("Journal B".into()),
                date: Some("2025-01-01".into()),
                citation_count: Some(5),
                influential_citation_count: Some(0),
                source: ArticleSource::EuropePmc,
                score: None,
                is_retracted: Some(false),
                abstract_snippet: None,
                ranking: None,
                matched_sources: vec![ArticleSource::EuropePmc],
                normalized_title: "braf melanoma systematic review".into(),
                normalized_abstract: String::new(),
                publication_type: Some("Review".into()),
                source_local_position: 1,
            },
            ArticleSearchResult {
                pmid: "300".into(),
                pmcid: None,
                doi: None,
                title: "BRAF melanoma clinical trial review".into(),
                journal: Some("Journal C".into()),
                date: Some("2025-01-01".into()),
                citation_count: Some(50),
                influential_citation_count: Some(7),
                source: ArticleSource::EuropePmc,
                score: None,
                is_retracted: Some(false),
                abstract_snippet: None,
                ranking: None,
                matched_sources: vec![ArticleSource::EuropePmc],
                normalized_title: "braf melanoma clinical trial review".into(),
                normalized_abstract: String::new(),
                publication_type: Some("Clinical Trial".into()),
                source_local_position: 2,
            },
        ];

        rank_result_rows_by_directness(&mut rows, &filters);

        let pmids: Vec<&str> = rows.iter().map(|row| row.pmid.as_str()).collect();
        assert_eq!(pmids, vec!["300", "200", "100"]);
        assert_eq!(
            rows[0]
                .ranking
                .as_ref()
                .map(|ranking| ranking.study_or_review_cue),
            Some(true)
        );
    }
}
