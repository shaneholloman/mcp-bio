//! Tier 3 - response parsing and local result shaping. Pure: feeds saved JSON
//! values into decode helpers and validates output. No network.

use super::super::*;

#[test]
fn decode_domains_response_maps_rows_and_skips_blank_accessions() {
    let resp: InterProResponse = serde_json::from_value(serde_json::json!({
        "results": [
            {"metadata": {"accession": "IPR000719", "name": "Protein kinase", "type": "domain"}},
            {"metadata": {"accession": " ", "name": "skip", "type": "domain"}},
            {"metadata": null}
        ]
    }))
    .unwrap();

    let rows = InterProClient::decode_domains_response(resp, 3);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].accession, "IPR000719");
    assert_eq!(rows[0].name.as_deref(), Some("Protein kinase"));
    assert_eq!(rows[0].domain_type.as_deref(), Some("domain"));
}
