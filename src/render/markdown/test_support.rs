//! Shared markdown test helpers used by sidecar test modules.

use crate::entities::article::{ArticleRankingOptions, ArticleSearchFilters, ArticleSort};

pub(super) fn article_filters_for_test(sort: ArticleSort) -> ArticleSearchFilters {
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
