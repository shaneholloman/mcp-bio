use super::*;
#[allow(unused_imports)]
use crate::entities::article::candidates::finalize_article_candidates;

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
    row.abstract_snippet = (!abstract_snippet.is_empty()).then(|| abstract_snippet.to_string());
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
    let mut row = calibration_row(pmid, source, title, abstract_snippet, source_local_position);
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

mod hybrid;
mod rescue;
