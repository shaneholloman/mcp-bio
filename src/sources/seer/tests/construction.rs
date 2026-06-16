//! Tier 2 — request construction. Pure: builds `RequestPlan`s and asserts the exact
//! method / path / query that would be sent. Nothing is sent.

use super::super::*;
use crate::sources::HttpMethod;

#[test]
fn site_catalog_plan_fetches_variable_formats() {
    let plan = SeerClient::site_catalog_plan();

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "get_var_formats.php");
    assert!(plan.query.is_empty());
}

#[test]
fn survival_plan_sets_site_and_required_filters() {
    let plan = SeerClient::survival_plan(97);

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "render_region_5.php");
    assert_eq!(plan.query_value("site"), Some("97"));
    assert_eq!(plan.query_value("data_type"), Some("4"));
    assert_eq!(plan.query_value("graph_type"), Some("1"));
    assert_eq!(plan.query_value("compareBy"), Some("sex"));
    assert_eq!(plan.query_value("relative_survival_interval"), Some("5"));
}
