//! Request construction tests. Pure: build request plans and inspect them.
//! No network.

use crate::sources::HttpMethod;

use super::*;

#[test]
fn search_plan_uses_expected_query_contract() {
    let plan = MedlinePlusClient::search_plan(" chest pain ", 3)
        .unwrap()
        .unwrap();

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "ws/query");
    assert_eq!(plan.query_value("db"), Some("healthTopics"));
    assert_eq!(plan.query_value("term"), Some("chest pain"));
    assert_eq!(plan.query_value("retmax"), Some("3"));
}

#[test]
fn search_plan_uses_retmax_parameter() {
    let plan = MedlinePlusClient::search_plan("chest pain", 5)
        .unwrap()
        .unwrap();

    assert_eq!(plan.query_value("retmax"), Some("5"));
}

#[test]
fn search_plan_accepts_max_retmax() {
    let plan = MedlinePlusClient::search_plan("chest pain", MEDLINEPLUS_MAX_RETMAX)
        .unwrap()
        .unwrap();

    assert_eq!(plan.query_value("retmax"), Some("50"));
}

#[test]
fn search_plan_returns_none_for_empty_query() {
    let plan = MedlinePlusClient::search_plan("   ", 3).unwrap();

    assert_eq!(plan, None);
}

#[test]
fn search_plan_rejects_invalid_retmax_bounds() {
    for retmax in [0, MEDLINEPLUS_MAX_RETMAX + 1] {
        let err = MedlinePlusClient::search_plan("chest pain", retmax)
            .expect_err("invalid retmax should fail before request");
        assert!(
            matches!(&err, BioMcpError::InvalidArgument(message) if message.contains("MedlinePlus retmax must be between 1 and 50")),
            "unexpected error for retmax {retmax}: {err}"
        );
    }
}
