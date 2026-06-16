//! Tier 2 — request construction. Pure: builds the GraphQL `RequestPlan` and
//! asserts the exact method / path / JSON body that would be sent. Nothing is sent.

use super::super::*;
use crate::error::BioMcpError;
use crate::sources::{HttpMethod, RequestBody};

#[test]
fn molecular_profile_context_plan_sets_graphql_body_and_limit() {
    let plan = CivicClient::context_plan(CivicFilter::MolecularProfile(" BRAF V600E "), 100)
        .expect("context plan");

    assert_eq!(plan.method, HttpMethod::Post);
    assert_eq!(plan.path, "graphql");
    let RequestBody::Json(body) = &plan.body else {
        panic!("expected JSON body, got {:?}", plan.body);
    };
    assert!(body["query"].as_str().unwrap().contains("CivicContext"));
    assert_eq!(body["variables"]["molecularProfileName"], "BRAF V600E");
    assert_eq!(body["variables"]["first"], 25);
}

#[test]
fn therapy_and_disease_context_plans_set_their_variables() {
    let therapy =
        CivicClient::context_plan(CivicFilter::Therapy("vemurafenib"), 5).expect("therapy plan");
    let RequestBody::Json(body) = &therapy.body else {
        panic!("expected JSON body, got {:?}", therapy.body);
    };
    assert_eq!(body["variables"]["therapyName"], "vemurafenib");
    assert_eq!(body["variables"]["first"], 5);

    let disease =
        CivicClient::context_plan(CivicFilter::Disease("melanoma"), 0).expect("disease plan");
    let RequestBody::Json(body) = &disease.body else {
        panic!("expected JSON body, got {:?}", disease.body);
    };
    assert_eq!(body["variables"]["diseaseName"], "melanoma");
    assert_eq!(body["variables"]["first"], 1);
}

#[test]
fn required_query_value_rejects_empty() {
    let err = required_query_value("therapy name", "   ").unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
}
