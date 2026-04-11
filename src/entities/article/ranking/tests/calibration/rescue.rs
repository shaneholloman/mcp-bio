use super::*;

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
    let unique_page = finalize_article_candidates(unique_candidates, 10, 0, None, &unique_filters);
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

    let mut cited = calibration_row("300", ArticleSource::PubMed, "melanoma case report", "", 0);
    cited.citation_count = Some(50);

    let page = finalize_article_candidates(vec![weak, cited, stronger], 10, 0, None, &filters);

    assert_eq!(
        page.results
            .iter()
            .map(|row| row.pmid.as_str())
            .collect::<Vec<_>>(),
        vec!["200", "300", "100"]
    );
}
