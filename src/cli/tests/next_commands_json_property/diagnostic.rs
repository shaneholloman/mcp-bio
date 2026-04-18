use super::*;
use crate::entities::diagnostic::{Diagnostic, DiagnosticSearchResult};

#[test]
fn diagnostic_json_next_commands_parse() {
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
        genes: Some(vec!["BRCA1".to_string(), "BARD1".to_string()]),
        conditions: None,
        methods: None,
    };
    let requested_sections = ["genes".to_string()];
    let next_commands =
        crate::render::markdown::diagnostic_next_commands(&diagnostic, &requested_sections);

    assert!(
        !next_commands.contains(&"biomcp get diagnostic GTR000000001.1 genes".to_string()),
        "requested section should not be suggested again: {next_commands:?}"
    );
    assert!(next_commands.contains(&"biomcp get diagnostic GTR000000001.1 conditions".to_string()));
    assert!(next_commands.contains(&"biomcp get diagnostic GTR000000001.1 methods".to_string()));
    assert!(next_commands.contains(&"biomcp list diagnostic".to_string()));

    assert_entity_json_next_commands(
        "diagnostic",
        &diagnostic,
        crate::render::markdown::diagnostic_evidence_urls(&diagnostic),
        next_commands,
        crate::render::provenance::diagnostic_section_sources(&diagnostic),
    );
}

#[test]
fn diagnostic_search_json_next_commands_parse() {
    let results = vec![DiagnosticSearchResult {
        accession: "GTR000000001.1".to_string(),
        name: "BRCA1 Hereditary Cancer Panel".to_string(),
        test_type: Some("molecular".to_string()),
        manufacturer_or_lab: Some("OncoPanel BRCA1".to_string()),
        genes: vec!["BRCA1".to_string()],
        conditions: vec!["Breast cancer".to_string()],
    }];
    let pagination = crate::cli::PaginationMeta::offset(0, 10, results.len(), Some(results.len()));
    let json = crate::cli::search_json_with_meta(
        results.clone(),
        pagination,
        crate::render::markdown::search_next_commands_diagnostic(&results),
    )
    .expect("diagnostic search json");
    let commands = collect_next_commands(&json);

    assert_eq!(
        commands,
        vec![
            "biomcp get diagnostic GTR000000001.1".to_string(),
            "biomcp list diagnostic".to_string()
        ]
    );
    assert_json_next_commands_parse("diagnostic-search", &json);
}
