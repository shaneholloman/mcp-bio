//! Shared article test helpers used by sidecar test modules.

#[allow(unused_imports)]
pub(super) use super::{
    ARTICLE_BATCH_MAX_IDS, AnnotationCount, Article, ArticleAnnotations, ArticleBatchEntitySummary,
    ArticleBatchItem, ArticlePubMedRescueKind, ArticleRankingMode, ArticleRankingOptions,
    ArticleSearchFilters, ArticleSearchResult, ArticleSemanticScholar, ArticleSemanticScholarPdf,
    ArticleSort, ArticleSource, ArticleSourceFilter,
};
#[allow(unused_imports)]
pub(super) use crate::entities::SearchPage;
#[allow(unused_imports)]
pub(super) use crate::error::BioMcpError;
#[allow(unused_imports)]
pub(super) use crate::sources::europepmc::EuropePmcSort;

pub(super) async fn lock_env() -> tokio::sync::MutexGuard<'static, ()> {
    crate::test_support::env_lock().lock().await
}

pub(super) struct EnvVarGuard {
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

pub(super) fn set_env_var(name: &'static str, value: Option<&str>) -> EnvVarGuard {
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

pub(super) fn sample_jats_article_xml(title: &str, body: &str) -> String {
    format!(
        "<article><front><article-meta><title-group><article-title>{title}</article-title></title-group><abstract><p>Abstract text.</p></abstract></article-meta></front><body><p>{body}</p></body></article>"
    )
}

pub(super) fn sample_pmc_articleset_xml(title: &str, body: &str) -> String {
    format!(
        "<?xml version=\"1.0\"?><!DOCTYPE pmc-articleset><pmc-articleset>{}</pmc-articleset>",
        sample_jats_article_xml(title, body)
    )
}

pub(super) fn empty_filters() -> ArticleSearchFilters {
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

pub(super) fn row(pmid: &str, source: ArticleSource) -> ArticleSearchResult {
    row_with(pmid, source, Some("2025-01-01"), Some(1), Some(false))
}

pub(super) fn row_with(
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
