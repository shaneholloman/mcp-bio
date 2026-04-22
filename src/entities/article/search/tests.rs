#[allow(unused_imports)]
use super::super::test_support::*;
use super::*;
#[allow(unused_imports)]
use wiremock::matchers::{body_string_contains, header, method, path, query_param};
#[allow(unused_imports)]
use wiremock::{Mock, MockServer, ResponseTemplate};

mod finalizer;
mod integration;
mod merge;

#[test]
fn validate_search_page_request_rejects_invalid_inputs_before_backend_io() {
    let filters = empty_filters();
    let err = validate_search_page_request(&filters, 5, ArticleSourceFilter::All)
        .expect_err("queryless article search should fail prevalidation");
    assert!(err.to_string().contains("At least one filter is required"));

    let mut filters = empty_filters();
    filters.keyword = Some("BRAF".into());
    let err = validate_search_page_request(&filters, 0, ArticleSourceFilter::All)
        .expect_err("invalid limit should fail prevalidation");
    assert!(err.to_string().contains("--limit must be between 1 and 50"));
}
