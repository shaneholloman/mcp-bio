#[test]
fn search_next_commands_diagnostic_prefers_top_accession_then_list() {
    let results = vec![
        DiagnosticSearchResult {
            accession: "GTR000000001.1".to_string(),
            name: "BRCA1 Hereditary Cancer Panel".to_string(),
            test_type: Some("molecular".to_string()),
            manufacturer_or_lab: Some("OncoPanel BRCA1".to_string()),
            genes: vec!["BRCA1".to_string()],
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

    let commands = search_next_commands_diagnostic(&results);
    assert_eq!(
        commands,
        vec![
            "biomcp get diagnostic GTR000000001.1".to_string(),
            "biomcp list diagnostic".to_string()
        ]
    );
}

#[test]
fn related_diagnostic_only_points_back_to_list_help() {
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
        genes: None,
        conditions: None,
        methods: None,
    };

    assert_eq!(
        related_diagnostic(&diagnostic),
        vec!["biomcp list diagnostic".to_string()]
    );
}
