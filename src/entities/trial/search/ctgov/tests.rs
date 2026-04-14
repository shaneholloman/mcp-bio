//! Tests for CTGov trial search helpers.

use super::super::super::test_support::*;
use super::super::{prepare_ctgov_search_context, validate_trial_search};
use super::*;

#[test]
fn ctgov_query_term_broadens_mutation_across_discovery_fields() {
    let filters = TrialSearchFilters {
        mutation: Some("dMMR OR MSI-H".into()),
        criteria: Some("mismatch repair deficient".into()),
        ..Default::default()
    };

    let query = ctgov_query_term(&filters, None)
        .expect("query term should build")
        .expect("query term should not be empty");
    assert!(query.contains(
        "(AREA[EligibilityCriteria](\"dMMR\" OR \"MSI\\-H\") OR \
AREA[BriefTitle](\"dMMR\" OR \"MSI\\-H\") OR \
AREA[OfficialTitle](\"dMMR\" OR \"MSI\\-H\") OR \
AREA[BriefSummary](\"dMMR\" OR \"MSI\\-H\") OR \
AREA[Keyword](\"dMMR\" OR \"MSI\\-H\"))"
    ));
    assert!(query.contains("AREA[EligibilityCriteria](\"mismatch repair deficient\")"));
}

#[test]
fn ctgov_query_term_broadens_simple_mutation_across_discovery_fields() {
    let filters = TrialSearchFilters {
        mutation: Some("G12D".into()),
        ..Default::default()
    };

    let query = ctgov_query_term(&filters, None)
        .expect("query term should build")
        .expect("query term should not be empty");
    assert!(query.contains(
        "(AREA[EligibilityCriteria](\"G12D\") OR AREA[BriefTitle](\"G12D\") OR \
AREA[OfficialTitle](\"G12D\") OR AREA[BriefSummary](\"G12D\") OR AREA[Keyword](\"G12D\"))"
    ));
}

#[test]
fn ctgov_query_term_joins_multi_phase_filters_with_and() {
    let filters = TrialSearchFilters {
        condition: Some("melanoma".into()),
        ..Default::default()
    };

    let query = ctgov_query_term(&filters, Some(&["PHASE1".into(), "PHASE2".into()]))
        .expect("query term should build")
        .expect("query term should not be empty");
    assert!(query.contains("(AREA[Phase]PHASE1 AND AREA[Phase]PHASE2)"));
}

#[test]
fn build_ctgov_search_params_maps_all_shared_fields() {
    let filters = TrialSearchFilters {
        condition: Some("melanoma".into()),
        intervention: Some("HRS 4642".into()),
        facility: Some("Mayo Clinic".into()),
        status: Some("active".into()),
        phase: Some("1/2".into()),
        study_type: Some("Interventional".into()),
        sex: Some("female".into()),
        sponsor: Some("Acme Oncology".into()),
        sponsor_type: Some("industry".into()),
        mutation: Some("MSI-H".into()),
        criteria: Some("mismatch repair deficient".into()),
        results_available: true,
        lat: Some(42.3601),
        lon: Some(-71.0589),
        distance: Some(25),
        ..Default::default()
    };
    let normalized = validate_trial_search(&filters).expect("filters should validate");
    let context =
        prepare_ctgov_search_context(&filters, &normalized).expect("context should build");

    let params = build_ctgov_search_params(&filters, &context, Some("cursor-1".into()), 37);

    assert_eq!(params.condition, filters.condition);
    assert_eq!(params.intervention.as_deref(), Some("HRS-4642"));
    assert_eq!(params.facility, context.facility);
    assert_eq!(params.status, context.normalized_status);
    assert_eq!(params.agg_filters, context.agg_filters);
    assert_eq!(params.query_term, context.query_term);
    assert!(params.count_total);
    assert_eq!(params.page_token.as_deref(), Some("cursor-1"));
    assert_eq!(params.page_size, 37);
    assert_eq!(params.lat, filters.lat);
    assert_eq!(params.lon, filters.lon);
    assert_eq!(params.distance_miles, filters.distance);
}

#[test]
fn build_ctgov_search_params_preserves_none_values_without_defaults() {
    let filters = TrialSearchFilters {
        condition: Some("melanoma".into()),
        ..Default::default()
    };
    let normalized = validate_trial_search(&filters).expect("filters should validate");
    let context =
        prepare_ctgov_search_context(&filters, &normalized).expect("context should build");

    let params = build_ctgov_search_params(&filters, &context, None, 10);

    assert_eq!(params.condition, Some("melanoma".into()));
    assert_eq!(params.intervention, None);
    assert_eq!(params.facility, None);
    assert_eq!(params.status, None);
    assert_eq!(params.agg_filters, None);
    assert_eq!(params.query_term, None);
    assert!(params.count_total);
    assert_eq!(params.page_token, None);
    assert_eq!(params.page_size, 10);
    assert_eq!(params.lat, None);
    assert_eq!(params.lon, None);
    assert_eq!(params.distance_miles, None);
}

#[test]
fn build_ctgov_search_params_keeps_search_and_count_call_shapes_aligned() {
    let filters = TrialSearchFilters {
        condition: Some("melanoma".into()),
        intervention: Some("HRS 4642".into()),
        facility: Some("Dana-Farber Cancer Institute".into()),
        status: Some("recruiting".into()),
        phase: Some("2".into()),
        sex: Some("all".into()),
        sponsor_type: Some("nih".into()),
        mutation: Some("BRAF V600E".into()),
        criteria: Some("prior anti-braf therapy".into()),
        lat: Some(42.3355),
        lon: Some(-71.1041),
        distance: Some(15),
        ..Default::default()
    };
    let normalized = validate_trial_search(&filters).expect("filters should validate");
    let context =
        prepare_ctgov_search_context(&filters, &normalized).expect("context should build");

    let search_page_params =
        build_ctgov_search_params(&filters, &context, Some("page-1".into()), 25);
    let fast_count_params = build_ctgov_search_params(&filters, &context, None, 1);
    let slow_count_params = build_ctgov_search_params(
        &filters,
        &context,
        Some("page-2".into()),
        CTGOV_COUNT_PAGE_SIZE,
    );

    assert_eq!(search_page_params.condition, fast_count_params.condition);
    assert_eq!(search_page_params.condition, slow_count_params.condition);
    assert_eq!(
        search_page_params.intervention,
        fast_count_params.intervention
    );
    assert_eq!(
        search_page_params.intervention,
        slow_count_params.intervention
    );
    assert_eq!(search_page_params.facility, fast_count_params.facility);
    assert_eq!(search_page_params.facility, slow_count_params.facility);
    assert_eq!(search_page_params.status, fast_count_params.status);
    assert_eq!(search_page_params.status, slow_count_params.status);
    assert_eq!(
        search_page_params.agg_filters,
        fast_count_params.agg_filters
    );
    assert_eq!(
        search_page_params.agg_filters,
        slow_count_params.agg_filters
    );
    assert_eq!(search_page_params.query_term, fast_count_params.query_term);
    assert_eq!(search_page_params.query_term, slow_count_params.query_term);
    assert_eq!(
        search_page_params.count_total,
        fast_count_params.count_total
    );
    assert_eq!(
        search_page_params.count_total,
        slow_count_params.count_total
    );
    assert_eq!(search_page_params.lat, fast_count_params.lat);
    assert_eq!(search_page_params.lat, slow_count_params.lat);
    assert_eq!(search_page_params.lon, fast_count_params.lon);
    assert_eq!(search_page_params.lon, slow_count_params.lon);
    assert_eq!(
        search_page_params.distance_miles,
        fast_count_params.distance_miles
    );
    assert_eq!(
        search_page_params.distance_miles,
        slow_count_params.distance_miles
    );

    assert_eq!(search_page_params.page_token.as_deref(), Some("page-1"));
    assert_eq!(search_page_params.page_size, 25);
    assert_eq!(fast_count_params.page_token, None);
    assert_eq!(fast_count_params.page_size, 1);
    assert_eq!(slow_count_params.page_token.as_deref(), Some("page-2"));
    assert_eq!(slow_count_params.page_size, CTGOV_COUNT_PAGE_SIZE);
}

#[tokio::test]
async fn age_filter_uses_native_total_semantics_across_limits() {
    let server = MockServer::start().await;
    let client = ClinicalTrialsClient::new_for_test(server.uri()).expect("client");

    Mock::given(method("GET"))
        .and(path("/studies"))
        .and(query_param("query.cond", "melanoma"))
        .and(query_param("filter.overallStatus", "RECRUITING"))
        .and(query_param("countTotal", "true"))
        .and(query_param("pageSize", "10"))
        .and(query_param_is_missing("pageToken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "studies": studies_with_age_matches(100, 60, "10"),
            "nextPageToken": "p2",
            "totalCount": 200
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/studies"))
        .and(query_param("query.cond", "melanoma"))
        .and(query_param("filter.overallStatus", "RECRUITING"))
        .and(query_param("countTotal", "true"))
        .and(query_param("pageSize", "20"))
        .and(query_param_is_missing("pageToken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "studies": studies_with_age_matches(100, 60, "20"),
            "nextPageToken": "p2",
            "totalCount": 200
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/studies"))
        .and(query_param("query.cond", "melanoma"))
        .and(query_param("filter.overallStatus", "RECRUITING"))
        .and(query_param("countTotal", "true"))
        .and(query_param("pageSize", "50"))
        .and(query_param_is_missing("pageToken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "studies": studies_with_age_matches(100, 60, "50"),
            "nextPageToken": "p2",
            "totalCount": 200
        })))
        .mount(&server)
        .await;

    let filters = age_filtered_ctgov_filters();

    assert_eq!(
        search_page_with_ctgov_client(&client, &filters, 10, 0, None)
            .await
            .expect("page")
            .total,
        Some(200)
    );
    assert_eq!(
        search_page_with_ctgov_client(&client, &filters, 20, 0, None)
            .await
            .expect("page")
            .total,
        Some(200)
    );
    assert_eq!(
        search_page_with_ctgov_client(&client, &filters, 50, 0, None)
            .await
            .expect("page")
            .total,
        Some(200)
    );
}

#[tokio::test]
async fn ctgov_cursor_preserves_next_page_token_after_offset_full_page_consumption() {
    let server = MockServer::start().await;
    let client = ClinicalTrialsClient::new_for_test(server.uri()).expect("client");

    Mock::given(method("GET"))
        .and(path("/studies"))
        .and(query_param("query.cond", "melanoma"))
        .and(query_param("filter.overallStatus", "RECRUITING"))
        .and(query_param("countTotal", "true"))
        .and(query_param("pageSize", "3"))
        .and(query_param_is_missing("pageToken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "studies": studies_with_age_matches(3, 3, "21"),
            "nextPageToken": "p2",
            "totalCount": 10
        })))
        .expect(1)
        .mount(&server)
        .await;

    let page = search_page_with_ctgov_client(&client, &age_filtered_ctgov_filters(), 3, 1, None)
        .await
        .expect("page");

    assert_eq!(page.results.len(), 2);
    assert_eq!(page.next_page_token, Some("p2".into()));
}

#[tokio::test]
async fn age_filter_total_returns_native_total_when_exhausted() {
    let server = MockServer::start().await;
    let client = ClinicalTrialsClient::new_for_test(server.uri()).expect("client");

    let page_one = studies_with_age_matches(10, 7, "31");
    let page_two = studies_with_age_matches(10, 5, "32");

    Mock::given(method("GET"))
        .and(path("/studies"))
        .and(query_param("query.cond", "melanoma"))
        .and(query_param("filter.overallStatus", "RECRUITING"))
        .and(query_param("countTotal", "true"))
        .and(query_param("pageSize", "10"))
        .and(query_param_is_missing("pageToken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "studies": page_one,
            "nextPageToken": "p2",
            "totalCount": 20
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/studies"))
        .and(query_param("query.cond", "melanoma"))
        .and(query_param("filter.overallStatus", "RECRUITING"))
        .and(query_param("countTotal", "true"))
        .and(query_param("pageSize", "10"))
        .and(query_param("pageToken", "p2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "studies": page_two,
            "nextPageToken": null,
            "totalCount": 20
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/studies"))
        .and(query_param("query.cond", "melanoma"))
        .and(query_param("filter.overallStatus", "RECRUITING"))
        .and(query_param("countTotal", "true"))
        .and(query_param("pageSize", "50"))
        .and(query_param_is_missing("pageToken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "studies": studies_with_age_matches(10, 7, "41"),
            "nextPageToken": "p2",
            "totalCount": 20
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/studies"))
        .and(query_param("query.cond", "melanoma"))
        .and(query_param("filter.overallStatus", "RECRUITING"))
        .and(query_param("countTotal", "true"))
        .and(query_param("pageSize", "50"))
        .and(query_param("pageToken", "p2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "studies": studies_with_age_matches(10, 5, "42"),
            "nextPageToken": null,
            "totalCount": 20
        })))
        .expect(1)
        .mount(&server)
        .await;

    let filters = age_filtered_ctgov_filters();

    assert_eq!(
        search_page_with_ctgov_client(&client, &filters, 10, 0, None)
            .await
            .expect("page")
            .total,
        Some(20)
    );
    assert_eq!(
        search_page_with_ctgov_client(&client, &filters, 50, 0, None)
            .await
            .expect("page")
            .total,
        Some(20)
    );
}

#[tokio::test]
async fn count_all_returns_approximate_for_age_only_filters() {
    let server = MockServer::start().await;
    let client = ClinicalTrialsClient::new_for_test(server.uri()).expect("client");

    Mock::given(method("GET"))
        .and(path("/studies"))
        .and(query_param("query.cond", "melanoma"))
        .and(query_param("filter.overallStatus", "RECRUITING"))
        .and(query_param("countTotal", "true"))
        .and(query_param("pageSize", "1"))
        .and(query_param_is_missing("pageToken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "studies": [],
            "nextPageToken": null,
            "totalCount": 250
        })))
        .expect(1)
        .mount(&server)
        .await;

    let filters = age_filtered_ctgov_filters();
    assert_eq!(
        count_all_with_ctgov_client(&client, &filters)
            .await
            .expect("count"),
        TrialCount::Approximate(250)
    );
}

#[tokio::test]
async fn count_all_returns_exact_for_no_post_filters() {
    let server = MockServer::start().await;
    let client = ClinicalTrialsClient::new_for_test(server.uri()).expect("client");

    Mock::given(method("GET"))
        .and(path("/studies"))
        .and(query_param("query.cond", "melanoma"))
        .and(query_param("filter.overallStatus", "RECRUITING"))
        .and(query_param("countTotal", "true"))
        .and(query_param("pageSize", "1"))
        .and(query_param_is_missing("pageToken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "studies": [],
            "nextPageToken": null,
            "totalCount": 494
        })))
        .expect(1)
        .mount(&server)
        .await;

    let filters = TrialSearchFilters {
        condition: Some("melanoma".into()),
        status: Some("recruiting".into()),
        ..Default::default()
    };

    assert_eq!(
        count_all_with_ctgov_client(&client, &filters)
            .await
            .expect("count"),
        TrialCount::Exact(494)
    );
}

#[tokio::test]
async fn count_all_returns_unknown_when_expensive_post_filter_hits_page_cap() {
    let server = MockServer::start().await;
    let client = ClinicalTrialsClient::new_for_test(server.uri()).expect("client");

    Mock::given(method("GET"))
        .and(path("/studies"))
        .and(query_param("query.cond", "melanoma"))
        .and(query_param("countTotal", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "studies": [ctgov_search_study_fixture("NCT99999999", "18 Years", "75 Years")],
            "nextPageToken": "still-more",
            "totalCount": 60000
        })))
        .expect(50)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/studies/NCT99999999"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(ctgov_eligibility_detail_fixture(
                "NCT99999999",
                "Inclusion Criteria:\nMust have mismatch repair deficient disease",
            )),
        )
        .expect(50)
        .mount(&server)
        .await;

    let filters = TrialSearchFilters {
        condition: Some("melanoma".into()),
        criteria: Some("mismatch repair deficient".into()),
        ..Default::default()
    };

    assert_eq!(
        count_all_with_ctgov_client(&client, &filters)
            .await
            .expect("count"),
        TrialCount::Unknown
    );
}
