//! Article entity models and workflows exposed through the stable article facade.

mod backends;
mod batch;
mod candidates;
mod detail;
mod enrichment;
mod filters;
mod graph;
mod planner;
mod query;
mod ranking;
mod search;
#[cfg(test)]
mod test_support;

#[cfg(test)]
use self::backends::{
    search_litsense2_candidates, search_pubmed_page, search_semantic_scholar_candidates,
};
pub use self::batch::get_batch_compact;
#[cfg(test)]
use self::batch::{article_batch_item_from_article, merge_semantic_scholar_compact_rows};
#[cfg(test)]
use self::candidates::{
    ArticleSourcePosition, article_candidate_from_row, article_source_priority,
    finalize_article_candidates, merge_article_candidates,
};
pub use self::detail::get;
#[cfg(test)]
use self::detail::{
    ArticleIdType, fulltext_cache_key, is_doi, is_pubtator_lag_error, parse_article_id,
    parse_pmcid, parse_pmid, parse_sections,
};
#[cfg(test)]
use self::enrichment::enrich_article_search_rows_with_semantic_scholar;
#[cfg(test)]
use self::filters::{matches_optional_date_filter, normalize_article_type};
#[cfg(test)]
use self::filters::{matches_result_filters, normalized_date_bounds, parse_row_date};
#[cfg(test)]
use self::graph::semantic_scholar_lookup_id;
pub use self::graph::{citations, recommendations, references};
#[allow(unused_imports)]
pub(crate) use self::planner::{
    ArticleSearchDebugSummary, article_type_limitation_note, litsense2_search_enabled,
    semantic_scholar_search_enabled, summarize_debug_plan,
};
#[cfg(test)]
use self::planner::{BackendPlan, plan_backends};
#[cfg(test)]
use self::query::{
    build_free_text_article_query, build_pubmed_esearch_params, build_pubmed_search_term,
    build_search_query, europepmc_keyword, pubtator_sort,
};
#[cfg(test)]
use self::ranking::validate_article_ranking_options;
#[allow(unused_imports)]
pub(crate) use self::ranking::{article_effective_ranking_mode, article_relevance_ranking_policy};
#[cfg(test)]
use self::ranking::{build_anchor_set, rank_articles_by_directness, resolve_article_ranking};
#[cfg(test)]
use self::search::merge_federated_pages;
pub use self::search::{search, search_page};

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[cfg(test)]
use crate::entities::SearchPage;
use crate::error::BioMcpError;
use crate::sources::europepmc::EuropePmcSort;

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
const FULLTEXT_CACHE_VERSION: &str = "jats-v2";
const PUBMED_RESCUE_POSITION_MAX: usize = 0;
const INVALID_ARTICLE_ID_MSG: &str = "\
Unsupported identifier format. BioMCP resolves PMID (digits only, e.g., 22663011), \
PMCID (starts with PMC, e.g., PMC9984800), and DOI (starts with 10., \
e.g., 10.1056/NEJMoa1203421). publisher PIIs (e.g., S1535610826000103) are not \
indexed by PubMed or Europe PMC and cannot be resolved.";
pub const ARTICLE_RELEVANCE_RANKING_POLICY: &str = "calibrated PubMed rescue + lexical directness (top-ranked weak PubMed unique/led rows with at least one anchor hit > title coverage > title+abstract coverage > study/review cue > citation support > source-local position)";
pub const ARTICLE_SEMANTIC_RANKING_POLICY: &str =
    "semantic relevance (semantic score first, lexical directness fallback)";

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

    fn sample_jats_article_xml(title: &str, body: &str) -> String {
        format!(
            "<article><front><article-meta><title-group><article-title>{title}</article-title></title-group><abstract><p>Abstract text.</p></abstract></article-meta></front><body><p>{body}</p></body></article>"
        )
    }

    fn sample_pmc_articleset_xml(title: &str, body: &str) -> String {
        format!(
            "<?xml version=\"1.0\"?><!DOCTYPE pmc-articleset><pmc-articleset>{}</pmc-articleset>",
            sample_jats_article_xml(title, body)
        )
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
        assert!(key.starts_with("article-fulltext-jats-v2:"));
        assert!(key.ends_with("22663011"));
    }

    #[tokio::test]
    async fn get_fulltext_prefers_europepmc_before_ncbi_efetch() {
        let _guard = lock_env().await;
        let pubtator = MockServer::start().await;
        let europepmc = MockServer::start().await;
        let efetch = MockServer::start().await;
        let pmc_oa = MockServer::start().await;
        let s2 = MockServer::start().await;
        let _pubtator_base = set_env_var("BIOMCP_PUBTATOR_BASE", Some(&pubtator.uri()));
        let _europepmc_base = set_env_var("BIOMCP_EUROPEPMC_BASE", Some(&europepmc.uri()));
        let _efetch_base = set_env_var("BIOMCP_PUBMED_BASE", Some(&efetch.uri()));
        let _pmc_oa_base = set_env_var("BIOMCP_PMC_OA_BASE", Some(&pmc_oa.uri()));
        let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&s2.uri()));
        let _s2_key = set_env_var("S2_API_KEY", None);

        Mock::given(method("GET"))
            .and(path("/publications/export/biocjson"))
            .and(query_param("pmids", "22663011"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "PubTator3": [{
                    "pmid": 22663011,
                    "pmcid": "PMC123456",
                    "passages": [
                        {"infons": {"type": "title"}, "text": "Europe full text winner"},
                        {"infons": {"type": "abstract"}, "text": "Abstract text."}
                    ]
                }]
            })))
            .expect(1)
            .mount(&pubtator)
            .await;

        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param("query", "EXT_ID:22663011 AND SRC:MED"))
            .and(query_param("format", "json"))
            .and(query_param("page", "1"))
            .and(query_param("pageSize", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "hitCount": 1,
                "resultList": {
                    "result": [{
                        "id": "22663011",
                        "pmid": "22663011",
                        "pmcid": "PMC123456",
                        "title": "Europe full text winner",
                        "journalTitle": "Journal One",
                        "firstPublicationDate": "2025-01-01"
                    }]
                }
            })))
            .expect(1)
            .mount(&europepmc)
            .await;

        Mock::given(method("GET"))
            .and(path("/PMC123456/fullTextXML"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(sample_jats_article_xml(
                    "Europe full text winner",
                    "Europe PMC body text.",
                )),
            )
            .expect(1)
            .mount(&europepmc)
            .await;

        Mock::given(method("GET"))
            .and(path("/efetch.fcgi"))
            .and(query_param("db", "pmc"))
            .and(query_param("id", "123456"))
            .and(query_param("rettype", "xml"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(sample_pmc_articleset_xml(
                    "efetch should not run",
                    "efetch should not run.",
                )),
            )
            .expect(0)
            .mount(&efetch)
            .await;

        Mock::given(method("GET"))
            .and(path("/"))
            .and(query_param("id", "PMC123456"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&pmc_oa)
            .await;

        Mock::given(method("GET"))
            .and(path("/graph/v1/paper/PMID:22663011"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "paperId": "paper-1",
                "title": "Europe full text winner"
            })))
            .expect(1)
            .mount(&s2)
            .await;

        let article = get("22663011", &["fulltext".to_string()])
            .await
            .expect("fulltext request should succeed");

        assert!(article.full_text_note.is_none());
        let path = article.full_text_path.expect("full text path");
        let metadata = std::fs::metadata(path).expect("saved full text metadata");
        assert!(metadata.len() > 0);
    }

    #[tokio::test]
    async fn get_fulltext_falls_back_to_ncbi_efetch_before_pmc_oa() {
        let _guard = lock_env().await;
        let pubtator = MockServer::start().await;
        let europepmc = MockServer::start().await;
        let efetch = MockServer::start().await;
        let pmc_oa = MockServer::start().await;
        let s2 = MockServer::start().await;
        let _pubtator_base = set_env_var("BIOMCP_PUBTATOR_BASE", Some(&pubtator.uri()));
        let _europepmc_base = set_env_var("BIOMCP_EUROPEPMC_BASE", Some(&europepmc.uri()));
        let _efetch_base = set_env_var("BIOMCP_PUBMED_BASE", Some(&efetch.uri()));
        let _pmc_oa_base = set_env_var("BIOMCP_PMC_OA_BASE", Some(&pmc_oa.uri()));
        let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&s2.uri()));
        let _s2_key = set_env_var("S2_API_KEY", None);

        Mock::given(method("GET"))
            .and(path("/publications/export/biocjson"))
            .and(query_param("pmids", "22663012"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "PubTator3": [{
                    "pmid": 22663012,
                    "pmcid": "PMC123457",
                    "passages": [
                        {"infons": {"type": "title"}, "text": "efetch fallback winner"},
                        {"infons": {"type": "abstract"}, "text": "Abstract text."}
                    ]
                }]
            })))
            .expect(1)
            .mount(&pubtator)
            .await;

        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param("query", "EXT_ID:22663012 AND SRC:MED"))
            .and(query_param("format", "json"))
            .and(query_param("page", "1"))
            .and(query_param("pageSize", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "hitCount": 1,
                "resultList": {
                    "result": [{
                        "id": "22663012",
                        "pmid": "22663012",
                        "pmcid": "PMC123457",
                        "title": "efetch fallback winner",
                        "journalTitle": "Journal One",
                        "firstPublicationDate": "2025-01-01"
                    }]
                }
            })))
            .expect(1)
            .mount(&europepmc)
            .await;

        Mock::given(method("GET"))
            .and(path("/PMC123457/fullTextXML"))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&europepmc)
            .await;

        Mock::given(method("GET"))
            .and(path("/efetch.fcgi"))
            .and(query_param("db", "pmc"))
            .and(query_param("id", "123457"))
            .and(query_param("rettype", "xml"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(sample_pmc_articleset_xml(
                    "efetch fallback winner",
                    "NCBI efetch body text.",
                )),
            )
            .expect(1)
            .mount(&efetch)
            .await;

        Mock::given(method("GET"))
            .and(path("/"))
            .and(query_param("id", "PMC123457"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&pmc_oa)
            .await;

        Mock::given(method("GET"))
            .and(path("/graph/v1/paper/PMID:22663012"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "paperId": "paper-1",
                "title": "efetch fallback winner"
            })))
            .expect(1)
            .mount(&s2)
            .await;

        let article = get("22663012", &["fulltext".to_string()])
            .await
            .expect("fulltext request should succeed");

        assert!(article.full_text_note.is_none());
        let path = article.full_text_path.expect("full text path");
        let metadata = std::fs::metadata(path).expect("saved full text metadata");
        assert!(metadata.len() > 0);
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
