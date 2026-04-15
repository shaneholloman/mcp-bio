fn article_search_result(pmid: &str) -> ArticleSearchResult {
    ArticleSearchResult {
        pmid: pmid.to_string(),
        pmcid: None,
        doi: None,
        title: "Entity-aware article".to_string(),
        journal: Some("Journal".to_string()),
        date: Some("2025-01-01".to_string()),
        citation_count: Some(12),
        influential_citation_count: Some(4),
        source: ArticleSource::EuropePmc,
        matched_sources: vec![ArticleSource::EuropePmc],
        score: None,
        is_retracted: Some(false),
        abstract_snippet: Some("Abstract".to_string()),
        ranking: None,
        normalized_title: "entity-aware article".to_string(),
        normalized_abstract: "abstract".to_string(),
        publication_type: None,
        source_local_position: 0,
    }
}

fn article_filters(
    keyword: Option<&str>,
    gene: Option<&str>,
    drug: Option<&str>,
) -> crate::entities::article::ArticleSearchFilters {
    crate::entities::article::ArticleSearchFilters {
        gene: gene.map(str::to_string),
        gene_anchored: false,
        disease: None,
        drug: drug.map(str::to_string),
        author: None,
        keyword: keyword.map(str::to_string),
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

#[test]
fn article_search_related_results_include_primary_article_and_gene_pivots() {
    let related = related_article_search_results(
        &[article_search_result("22663011")],
        &article_filters(Some("SRY Sox9 miRNA"), None, None),
    );

    assert_eq!(related[0], "biomcp get article 22663011");
    assert_eq!(related[1], "biomcp get gene SRY");
    assert_eq!(related[2], "biomcp search article -g SRY -k \"Sox9 miRNA\"");
}

#[test]
fn article_search_related_results_detect_drug_without_dna_false_positive() {
    let related = related_article_search_results(
        &[article_search_result("22663011")],
        &article_filters(Some("psoralen photobinding DNA"), None, None),
    );

    assert!(related.contains(&"biomcp get drug psoralen".to_string()));
    assert!(!related.contains(&"biomcp get gene DNA".to_string()));
}

#[test]
fn article_search_related_results_skip_redundant_typed_hints() {
    let with_gene_filter = related_article_search_results(
        &[article_search_result("22663011")],
        &article_filters(Some("SRY Sox9 miRNA"), Some("BRAF"), None),
    );
    assert!(!with_gene_filter.contains(&"biomcp get gene SRY".to_string()));
    assert!(!with_gene_filter.contains(&"biomcp search article -g SRY -k \"Sox9 miRNA\"".to_string()));

    let with_drug_filter = related_article_search_results(
        &[article_search_result("22663011")],
        &article_filters(Some("psoralen photobinding DNA"), None, Some("psoralen")),
    );
    assert!(!with_drug_filter.contains(&"biomcp get drug psoralen".to_string()));
}
