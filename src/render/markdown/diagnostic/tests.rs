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
        target_marker: None,
        regulatory_version: None,
        prequalification_year: None,
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
    assert!(markdown.contains("Source: NCBI Genetic Testing Registry"));
    assert!(markdown.contains("Method Categories: Molecular genetics"));
    assert!(markdown.contains("## Genes"));
    assert!(markdown.contains("No genes listed in NCBI Genetic Testing Registry."));
    assert!(markdown.contains("## Conditions"));
    assert!(markdown.contains("Breast cancer"));
    assert!(markdown.contains("## Methods"));
    assert!(markdown.contains("No methods listed in NCBI Genetic Testing Registry."));
    assert!(markdown.contains("biomcp list diagnostic"));
}

#[test]
fn diagnostic_search_markdown_shows_source_column_and_detail_hint() {
    let results = vec![
        DiagnosticSearchResult {
            source: "gtr".to_string(),
            accession: "GTR000000001.1".to_string(),
            name: "BRCA1 Hereditary Cancer Panel".to_string(),
            test_type: Some("molecular".to_string()),
            manufacturer_or_lab: Some("OncoPanel BRCA1".to_string()),
            genes: vec!["BRCA1".to_string(), "BARD1".to_string()],
            conditions: vec!["Breast cancer".to_string()],
        },
        DiagnosticSearchResult {
            source: "who-ivd".to_string(),
            accession: "ITPW02232- TC40".to_string(),
            name: "ONE STEP Anti-HIV (1&2) Test".to_string(),
            test_type: Some("Immunochromatographic (lateral flow)".to_string()),
            manufacturer_or_lab: Some("InTec Products, Inc.".to_string()),
            genes: vec![],
            conditions: vec!["HIV".to_string()],
        },
    ];

    let markdown = diagnostic_search_markdown("gene=BRCA1", &results, Some(results.len()))
        .expect("rendered markdown");

    assert!(markdown.contains("# Diagnostic tests: gene=BRCA1"));
    assert!(markdown.contains("|Accession|Name|Type|Manufacturer / Lab|Source|Genes|Conditions|"));
    assert!(markdown.contains("|GTR000000001.1|BRCA1 Hereditary Cancer Panel|molecular|OncoPanel BRCA1|NCBI Genetic Testing Registry|BRCA1, BARD1|Breast cancer|"));
    assert!(markdown.contains("|ITPW02232- TC40|ONE STEP Anti-HIV (1&2) Test|Immunochromatographic (lateral flow)|InTec Products, Inc.|WHO Prequalified IVD|-|HIV|"));
    assert!(markdown.contains("Use `biomcp get diagnostic GTR000000001.1` for details."));
}

#[test]
fn diagnostic_markdown_renders_who_summary_fields_and_supported_sections_only() {
    let diagnostic = Diagnostic {
        source: "who-ivd".to_string(),
        source_id: "ITPW02232- TC40".to_string(),
        accession: "ITPW02232- TC40".to_string(),
        name: "ONE STEP Anti-HIV (1&2) Test".to_string(),
        test_type: Some("Immunochromatographic (lateral flow)".to_string()),
        manufacturer: Some("InTec Products, Inc.".to_string()),
        target_marker: Some("HIV".to_string()),
        regulatory_version: Some("Rest-of-World".to_string()),
        prequalification_year: Some("2019".to_string()),
        laboratory: None,
        institution: None,
        country: None,
        clia_number: None,
        state_licenses: None,
        current_status: None,
        public_status: None,
        method_categories: vec![],
        genes: None,
        conditions: Some(vec!["HIV".to_string()]),
        methods: None,
    };

    let markdown =
        diagnostic_markdown(&diagnostic, &["all".to_string()]).expect("rendered markdown");

    assert!(markdown.contains("Source: WHO Prequalified IVD"));
    assert!(markdown.contains("Assay Format: Immunochromatographic (lateral flow)"));
    assert!(markdown.contains("Target / Marker: HIV"));
    assert!(markdown.contains("Regulatory Version: Rest-of-World"));
    assert!(markdown.contains("Prequalification Year: 2019"));
    assert!(markdown.contains("## Conditions"));
    assert!(markdown.contains("HIV"));
    assert!(!markdown.contains("## Genes"));
    assert!(!markdown.contains("## Methods"));
    assert!(markdown.contains("biomcp list diagnostic"));
}
