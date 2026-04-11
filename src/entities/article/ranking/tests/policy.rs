use super::*;

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
