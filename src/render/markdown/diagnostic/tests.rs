use super::*;

#[test]
fn diagnostic_markdown_renders_requested_sections_and_truthful_empty_states() {
    let diagnostic = Diagnostic {
        source: "gtr".to_string(),
        source_id: "GTR000000001.1".to_string(),
        accession: "GTR000000001.1".to_string(),
        name: "BRCA1 Hereditary Cancer Panel".to_string(),
        test_type: Some("molecular".to_string()),
        manufacturer: Some("OncoPanel BRCA1".to_string()),
        laboratory: Some("GenomOncology Lab".to_string()),
        institution: Some("GenomOncology Institute".to_string()),
        country: Some("USA".to_string()),
        clia_number: Some("12D3456789".to_string()),
        state_licenses: Some("NY|CA".to_string()),
        current_status: Some("Current".to_string()),
        public_status: Some("Public".to_string()),
        method_categories: vec!["Molecular genetics".to_string()],
        genes: Some(vec![]),
        conditions: Some(vec!["Breast cancer".to_string()]),
        methods: Some(vec![]),
    };

    let markdown = diagnostic_markdown(
        &diagnostic,
        &[
            "genes".to_string(),
            "conditions".to_string(),
            "methods".to_string(),
        ],
    )
    .expect("rendered markdown");

    assert!(markdown.contains("# Diagnostic: GTR000000001.1"));
    assert!(markdown.contains("Method Categories: Molecular genetics"));
    assert!(markdown.contains("## Genes"));
    assert!(markdown.contains("No genes listed in GTR."));
    assert!(markdown.contains("## Conditions"));
    assert!(markdown.contains("Breast cancer"));
    assert!(markdown.contains("## Methods"));
    assert!(markdown.contains("No methods listed in GTR."));
    assert!(markdown.contains("biomcp list diagnostic"));
}

#[test]
fn diagnostic_search_markdown_shows_table_and_detail_hint() {
    let results = vec![
        DiagnosticSearchResult {
            accession: "GTR000000001.1".to_string(),
            name: "BRCA1 Hereditary Cancer Panel".to_string(),
            test_type: Some("molecular".to_string()),
            manufacturer_or_lab: Some("OncoPanel BRCA1".to_string()),
            genes: vec!["BRCA1".to_string(), "BARD1".to_string()],
            conditions: vec!["Breast cancer".to_string()],
        },
        DiagnosticSearchResult {
            accession: "GTR000000002.1".to_string(),
            name: "EGFR Melanoma Molecular Assay".to_string(),
            test_type: Some("molecular".to_string()),
            manufacturer_or_lab: Some("Precision Diagnostics".to_string()),
            genes: vec!["EGFR".to_string()],
            conditions: vec!["Cutaneous melanoma".to_string()],
        },
    ];

    let markdown = diagnostic_search_markdown("gene=BRCA1", &results, Some(results.len()))
        .expect("rendered markdown");

    assert!(markdown.contains("# Diagnostic tests: gene=BRCA1"));
    assert!(markdown.contains("|Accession|Name|Type|Manufacturer / Lab|Genes|Conditions|"));
    assert!(markdown.contains("|GTR000000001.1|BRCA1 Hereditary Cancer Panel|molecular|OncoPanel BRCA1|BRCA1, BARD1|Breast cancer|"));
    assert!(markdown.contains("Use `biomcp get diagnostic GTR000000001.1` for details."));
}
