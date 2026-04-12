//! OpenFDA fallback and label-row search coverage.

use super::*;

#[test]
fn search_results_from_openfda_label_response_prefers_exact_brand_match() {
    let response = serde_json::json!({
        "results": [
            {
                "openfda": {
                    "brand_name": ["KEYTRUDA QLEX"],
                    "generic_name": ["Pembrolizumab and berahyaluronidase alfa-pmph"]
                }
            },
            {
                "openfda": {
                    "brand_name": ["Keytruda"],
                    "generic_name": ["Pembrolizumab"]
                }
            }
        ]
    });

    let rows = search_results_from_openfda_label_response(&response, " Keytruda ", 5);
    let names = rows.into_iter().map(|row| row.name).collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "pembrolizumab".to_string(),
            "pembrolizumab and berahyaluronidase alfa-pmph".to_string()
        ]
    );
}

#[test]
fn search_results_from_openfda_label_response_returns_remaining_unique_generics() {
    let response = serde_json::json!({
        "results": [
            {
                "openfda": {
                    "brand_name": ["Keytruda"],
                    "generic_name": ["Pembrolizumab"]
                }
            },
            {
                "openfda": {
                    "brand_name": ["KEYTRUDA QLEX"],
                    "generic_name": ["Pembrolizumab and berahyaluronidase alfa-pmph"]
                }
            },
            {
                "openfda": {
                    "brand_name": ["Keytruda refill"],
                    "generic_name": ["Pembrolizumab"]
                }
            }
        ]
    });

    let rows = search_results_from_openfda_label_response(&response, "Keytruda", 5);
    let names = rows.into_iter().map(|row| row.name).collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "pembrolizumab".to_string(),
            "pembrolizumab and berahyaluronidase alfa-pmph".to_string()
        ]
    );
}

#[test]
fn search_results_from_openfda_label_response_respects_limit() {
    let response = serde_json::json!({
        "results": [
            {
                "openfda": {
                    "brand_name": ["Keytruda"],
                    "generic_name": ["Pembrolizumab"]
                }
            },
            {
                "openfda": {
                    "brand_name": ["KEYTRUDA QLEX"],
                    "generic_name": ["Pembrolizumab and berahyaluronidase alfa-pmph"]
                }
            }
        ]
    });

    let rows = search_results_from_openfda_label_response(&response, "Keytruda", 1);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "pembrolizumab");
}

#[test]
fn openfda_label_fallback_is_first_page_only() {
    let name_filters = DrugSearchFilters {
        query: Some("Keytruda".into()),
        ..Default::default()
    };
    let structured_filters = DrugSearchFilters {
        target: Some("EGFR".into()),
        ..Default::default()
    };
    let dummy = DrugSearchResult {
        name: "pembrolizumab".into(),
        drugbank_id: None,
        drug_type: None,
        mechanism: None,
        target: None,
    };

    assert!(should_attempt_openfda_fallback(&[], 0, &name_filters));
    assert!(!should_attempt_openfda_fallback(&[], 10, &name_filters));
    assert!(!should_attempt_openfda_fallback(
        &[],
        0,
        &structured_filters
    ));
    assert!(!should_attempt_openfda_fallback(&[dummy], 0, &name_filters));
}
