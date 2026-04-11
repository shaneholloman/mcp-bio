#[allow(unused_imports)]
use super::super::test_support::*;
use super::*;

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
