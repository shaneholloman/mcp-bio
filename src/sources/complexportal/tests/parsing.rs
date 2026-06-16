//! Tier 3 - response parsing and local result shaping. Pure: feeds JSON bytes
//! into decode helpers and validates output. No network.

use reqwest::StatusCode;
use reqwest::header::HeaderValue;

use super::super::*;

#[test]
fn map_complexes_filters_false_positives_and_shapes_participants() {
    let response: ComplexPortalSearchResponse = serde_json::from_value(serde_json::json!({
        "elements": [
            {
                "complexAC": "CPX-1",
                "complexName": "BRAF complex",
                "description": "  RAF signaling complex  ",
                "predictedComplex": false,
                "interactors": [
                    {
                        "identifier": "P15056",
                        "name": "BRAF",
                        "stochiometry": " minValue: 1, maxValue: 1 ",
                        "interactorType": "protein"
                    },
                    {
                        "identifier": "Q02750",
                        "name": "MAP2K1",
                        "stochiometry": "",
                        "interactorType": "protein"
                    },
                    {
                        "identifier": "CHEBI:1234",
                        "name": "ATP",
                        "stochiometry": "minValue: 1, maxValue: 1",
                        "interactorType": "small molecule"
                    }
                ]
            },
            {
                "complexAC": "CPX-2",
                "complexName": "Description-only mention",
                "description": "Mentions P15056 but does not contain it as a participant",
                "predictedComplex": true,
                "interactors": [
                    {
                        "identifier": "Q9Y243",
                        "name": "AKT3",
                        "stochiometry": "minValue: 1, maxValue: 1",
                        "interactorType": "protein"
                    }
                ]
            }
        ]
    }))
    .unwrap();

    let rows = ComplexPortalClient::map_complexes(response, "P15056", 10);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].accession, "CPX-1");
    assert_eq!(rows[0].name, "BRAF complex");
    assert_eq!(
        rows[0].description.as_deref(),
        Some("RAF signaling complex")
    );
    assert_eq!(rows[0].participants.len(), 2);
    assert_eq!(rows[0].participants[0].accession, "P15056");
    assert_eq!(rows[0].participants[0].name, "BRAF");
    assert_eq!(
        rows[0].participants[0].stoichiometry.as_deref(),
        Some("minValue: 1, maxValue: 1")
    );
    assert_eq!(rows[0].participants[1].accession, "Q02750");
    assert_eq!(rows[0].participants[1].stoichiometry, None);
}

#[test]
fn decode_json_response_maps_empty_results() {
    let content_type = HeaderValue::from_static("application/json");
    let response: ComplexPortalSearchResponse = ComplexPortalClient::decode_json_response(
        StatusCode::OK,
        Some(&content_type),
        br#"{"elements":[]}"#,
    )
    .unwrap();

    let rows = ComplexPortalClient::map_complexes(response, "Q9Y243", 10);
    assert!(rows.is_empty());
}
