use super::*;

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
