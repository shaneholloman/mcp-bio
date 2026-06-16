//! Tier 2 - request construction. Pure: builds VAERS XML form requests and
//! asserts path, form fields, aggregate group, vaccine code, and user-agent.

use super::super::*;
use crate::error::BioMcpError;
use crate::sources::{HttpMethod, RequestBody};

#[test]
fn request_template_tracks_captured_fixture() {
    assert_eq!(REQUEST_TEMPLATE, super::REACTIONS_REQUEST_FIXTURE);
}

#[test]
fn aggregate_request_plan_posts_form_encoded_reaction_xml() {
    let plan = aggregate_request_plan(VaersAggregateKind::Reactions, "MMR").expect("plan");

    assert_eq!(plan.method, HttpMethod::Post);
    assert_eq!(plan.path, VAERS_REQUEST_PATH);
    let RequestBody::Form(form) = &plan.body else {
        panic!("expected form body, got {:?}", plan.body);
    };
    assert_eq!(
        form.iter()
            .find(|(key, _)| key == "accept_datause_restrictions")
            .map(|(_, value)| value.as_str()),
        Some("true")
    );
    let xml = form
        .iter()
        .find(|(key, _)| key == "request_xml")
        .map(|(_, value)| value.as_str())
        .expect("request_xml form field");
    assert_eq!(
        super::parameter_map(xml),
        super::parameter_map(super::REACTIONS_REQUEST_FIXTURE)
    );
}

#[test]
fn build_request_xml_matches_serious_and_age_fixture_parameters() {
    let serious = build_request_xml(VaersAggregateKind::Seriousness, "MMR").expect("request");
    assert_eq!(
        super::parameter_map(&serious),
        super::parameter_map(super::SERIOUS_REQUEST_FIXTURE)
    );

    let age = build_request_xml(VaersAggregateKind::Age, "MMR").expect("request");
    assert_eq!(
        super::parameter_map(&age),
        super::parameter_map(super::AGE_REQUEST_FIXTURE)
    );
}

#[test]
fn build_request_xml_escapes_and_validates_vaccine_code() {
    let built = build_request_xml(VaersAggregateKind::Reactions, "A&B").expect("request");
    assert!(built.contains("A&amp;B"));

    let err = build_request_xml(VaersAggregateKind::Reactions, " ").unwrap_err();
    assert!(matches!(err, BioMcpError::InvalidArgument(_)));
}

#[test]
fn vaers_client_uses_cdc_wonder_compatible_user_agent_constant() {
    assert_eq!(CDC_WONDER_COMPATIBLE_USER_AGENT, "Wget/1.21.4");
    assert!(!CDC_WONDER_COMPATIBLE_USER_AGENT.contains("biomcp-cli/"));
}
