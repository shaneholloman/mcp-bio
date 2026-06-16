//! Tier 2 — request construction. Pure: builds `RequestPlan`s and asserts the exact
//! method / path that would be sent. Nothing is sent.

use super::super::*;
use crate::sources::HttpMethod;

#[test]
fn by_gene_plan_uses_encoded_path_and_no_body() {
    let plan = CancerHotspotsClient::by_gene_plan(" ALK FUSION ");

    assert_eq!(plan.method, HttpMethod::Get);
    assert_eq!(plan.path, "api/hotspots/single/byGene/ALK%20FUSION");
    assert!(plan.query.is_empty());
    assert!(plan.headers.is_empty());
}

#[test]
fn encode_path_segment_preserves_safe_characters_and_escapes_others() {
    assert_eq!(encode_path_segment("BRAF"), "BRAF");
    assert_eq!(encode_path_segment("A/B C"), "A%2FB%20C");
    assert_eq!(encode_path_segment("ALK~fusion-1.2"), "ALK~fusion-1.2");
}
