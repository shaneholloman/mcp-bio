//! Tier 2 — request construction. Pure: builds `RequestPlan`s and asserts the exact
//! method / path / JSON body that would be sent. Nothing is sent.

use super::super::*;
use crate::error::BioMcpError;
use crate::sources::{HttpMethod, RequestBody};
use serde_json::json;
use time::Month;

fn test_date(year: i32, month: Month, day: u8) -> Date {
    Date::from_calendar_date(year, month, day).expect("valid test date")
}

#[test]
fn funding_plan_builds_approved_post_body() {
    let (plan, query, fiscal_years) =
        NihReporterClient::funding_plan(" Marfan syndrome ", test_date(2026, Month::April, 11))
            .unwrap();

    assert_eq!(plan.method, HttpMethod::Post);
    assert_eq!(plan.path, NIH_REPORTER_PATH);
    assert_eq!(query, "Marfan syndrome");
    assert_eq!(fiscal_years, vec![2022, 2023, 2024, 2025, 2026]);

    let RequestBody::Json(body) = &plan.body else {
        panic!("expected JSON body, got {:?}", plan.body);
    };
    assert!(body["criteria"].get("project_terms").is_none());
    assert_eq!(body["criteria"]["advanced_text_search"]["operator"], "and");
    assert_eq!(
        body["criteria"]["advanced_text_search"]["search_field"],
        NIH_REPORTER_SEARCH_FIELDS
    );
    assert_eq!(
        body["criteria"]["advanced_text_search"]["search_text"],
        "\"Marfan syndrome\""
    );
    assert_eq!(
        body["criteria"]["fiscal_years"],
        json!([2022, 2023, 2024, 2025, 2026])
    );
    assert_eq!(body["include_fields"], json!(NIH_REPORTER_INCLUDE_FIELDS));
    assert_eq!(body["offset"], 0);
    assert_eq!(body["limit"], NIH_REPORTER_MAX_RESULTS);
    assert_eq!(body["sort_field"], "award_amount");
    assert_eq!(body["sort_order"], "desc");
}

#[test]
fn funding_plan_rejects_empty_query() {
    let err =
        NihReporterClient::funding_plan("   ", test_date(2026, Month::April, 11)).unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
    assert!(err.to_string().contains("query is required"));
}

#[test]
fn exact_phrase_search_text_escapes_quotes_and_backslashes() {
    assert_eq!(
        exact_phrase_search_text("BCR\\ABL \"fusion\""),
        "\"BCR\\\\ABL \\\"fusion\\\"\""
    );
}

#[test]
fn recent_nih_fiscal_years_roll_over_on_october_boundary() {
    assert_eq!(
        recent_nih_fiscal_years(test_date(2026, Month::September, 30)),
        vec![2022, 2023, 2024, 2025, 2026]
    );
    assert_eq!(
        recent_nih_fiscal_years(test_date(2026, Month::October, 1)),
        vec![2023, 2024, 2025, 2026, 2027]
    );
}
