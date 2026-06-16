mod construction;
mod parsing;

fn search_response_json() -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "results": [{
            "primaryAccession": "P15056",
            "uniProtkbId": "BRAF_HUMAN",
            "proteinDescription": {
                "recommendedName": {
                    "fullName": {"value": "Serine/threonine-protein kinase B-raf"}
                }
            },
            "genes": [{"geneName": {"value": "BRAF"}}]
        }]
    }))
    .expect("fixture JSON should serialize")
}
