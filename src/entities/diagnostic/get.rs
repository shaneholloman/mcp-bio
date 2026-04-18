use crate::error::BioMcpError;
use crate::sources::gtr::{GtrClient, GtrIndex, GtrRecord, GtrSyncMode};
use crate::sources::who_ivd::{WhoIvdClient, WhoIvdRecord, WhoIvdSyncMode};

use super::{
    DIAGNOSTIC_SECTION_CONDITIONS, DIAGNOSTIC_SECTION_GENES, DIAGNOSTIC_SECTION_METHODS,
    DIAGNOSTIC_SECTION_NAMES, DIAGNOSTIC_SOURCE_GTR, DIAGNOSTIC_SOURCE_WHO_IVD, Diagnostic,
    diagnostic_source_label, looks_like_gtr_accession, optional_text, preferred_diagnostic_name,
    supported_diagnostic_sections_for_source,
};

#[derive(Debug, Clone, Copy, Default)]
struct DiagnosticSections {
    include_genes: bool,
    include_conditions: bool,
    include_methods: bool,
    include_all: bool,
}

fn parse_sections(sections: &[String]) -> Result<DiagnosticSections, BioMcpError> {
    let mut out = DiagnosticSections::default();

    for raw in sections {
        let section = raw.trim().to_ascii_lowercase();
        if section.is_empty() || section == "--json" || section == "-j" {
            continue;
        }

        match section.as_str() {
            DIAGNOSTIC_SECTION_GENES => out.include_genes = true,
            DIAGNOSTIC_SECTION_CONDITIONS => out.include_conditions = true,
            DIAGNOSTIC_SECTION_METHODS => out.include_methods = true,
            "all" => out.include_all = true,
            _ => {
                return Err(BioMcpError::InvalidArgument(format!(
                    "Unknown section \"{section}\" for diagnostic. Available: {}",
                    DIAGNOSTIC_SECTION_NAMES.join(", ")
                )));
            }
        }
    }

    Ok(out)
}

fn quote_identifier(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return String::new();
    }
    if value.chars().any(|c| c.is_whitespace()) {
        format!("\"{}\"", value.replace('\"', "\\\""))
    } else {
        value.to_string()
    }
}

fn unsupported_diagnostic_section_error(
    section: &str,
    source: &str,
    accession: &str,
) -> BioMcpError {
    let quoted = quote_identifier(accession);
    BioMcpError::InvalidArgument(format!(
        "Section `{section}` is not available for {} diagnostic records. Try `biomcp get diagnostic {quoted} conditions`",
        diagnostic_source_label(source)
    ))
}

fn resolve_sections_for_source(
    source: &str,
    accession: &str,
    raw_sections: &[String],
) -> Result<DiagnosticSections, BioMcpError> {
    let mut resolved = parse_sections(raw_sections)?;
    let supported = supported_diagnostic_sections_for_source(source);

    for raw in raw_sections {
        let section = raw.trim();
        if section.is_empty()
            || section.eq_ignore_ascii_case("--json")
            || section.eq_ignore_ascii_case("-j")
            || section.eq_ignore_ascii_case("all")
        {
            continue;
        }
        if !supported
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(section))
        {
            return Err(unsupported_diagnostic_section_error(
                section, source, accession,
            ));
        }
    }

    if resolved.include_all {
        resolved.include_genes = supported
            .iter()
            .any(|section| section.eq_ignore_ascii_case(DIAGNOSTIC_SECTION_GENES));
        resolved.include_conditions = supported
            .iter()
            .any(|section| section.eq_ignore_ascii_case(DIAGNOSTIC_SECTION_CONDITIONS));
        resolved.include_methods = supported
            .iter()
            .any(|section| section.eq_ignore_ascii_case(DIAGNOSTIC_SECTION_METHODS));
    }

    Ok(resolved)
}

fn diagnostic_from_record(
    record: &GtrRecord,
    index: &GtrIndex,
    sections: DiagnosticSections,
) -> Diagnostic {
    Diagnostic {
        source: DIAGNOSTIC_SOURCE_GTR.to_string(),
        source_id: record.accession.clone(),
        accession: record.accession.clone(),
        name: preferred_diagnostic_name(record),
        test_type: optional_text(&record.test_type),
        manufacturer: optional_text(&record.manufacturer_test_name),
        target_marker: None,
        regulatory_version: None,
        prequalification_year: None,
        laboratory: optional_text(&record.name_of_laboratory),
        institution: optional_text(&record.name_of_institution),
        country: optional_text(&record.facility_country),
        clia_number: optional_text(&record.clia_number),
        state_licenses: optional_text(&record.state_licenses),
        current_status: optional_text(&record.test_curr_stat),
        public_status: optional_text(&record.test_pub_stat),
        method_categories: record.method_categories.clone(),
        genes: sections
            .include_genes
            .then(|| index.merged_genes(&record.accession)),
        conditions: sections
            .include_conditions
            .then(|| index.conditions(&record.accession)),
        methods: sections.include_methods.then(|| record.methods.clone()),
    }
}

fn diagnostic_from_who_ivd_record(
    record: &WhoIvdRecord,
    sections: DiagnosticSections,
) -> Diagnostic {
    Diagnostic {
        source: DIAGNOSTIC_SOURCE_WHO_IVD.to_string(),
        source_id: record.product_code.clone(),
        accession: record.product_code.clone(),
        name: optional_text(&record.product_name).unwrap_or_else(|| record.product_code.clone()),
        test_type: optional_text(&record.assay_format),
        manufacturer: optional_text(&record.manufacturer_name),
        target_marker: optional_text(&record.target_marker),
        regulatory_version: optional_text(&record.regulatory_version),
        prequalification_year: optional_text(&record.prequalification_year),
        laboratory: None,
        institution: None,
        country: None,
        clia_number: None,
        state_licenses: None,
        current_status: None,
        public_status: None,
        method_categories: Vec::new(),
        genes: None,
        conditions: sections
            .include_conditions
            .then(|| optional_text(&record.target_marker).into_iter().collect()),
        methods: None,
    }
}

pub async fn get(accession: &str, sections: &[String]) -> Result<Diagnostic, BioMcpError> {
    let accession = accession.trim();
    if accession.is_empty() {
        return Err(BioMcpError::InvalidArgument(
            "Diagnostic accession or WHO IVD product code is required. Example: biomcp get diagnostic GTR000000001.1".into(),
        ));
    }

    if looks_like_gtr_accession(accession) {
        let client = GtrClient::ready(GtrSyncMode::Auto).await?;
        let index = client.load_index()?;
        let record = index.record(accession).ok_or_else(|| BioMcpError::NotFound {
            entity: "diagnostic".to_string(),
            id: accession.to_string(),
            suggestion:
                "Try searching with a filter such as `biomcp search diagnostic --gene BRCA1` or inspect `biomcp list diagnostic`.".to_string(),
        })?;
        let sections = resolve_sections_for_source(DIAGNOSTIC_SOURCE_GTR, accession, sections)?;
        return Ok(diagnostic_from_record(record, &index, sections));
    }

    let client = WhoIvdClient::ready(WhoIvdSyncMode::Auto).await?;
    let record = client.get(accession)?.ok_or_else(|| BioMcpError::NotFound {
        entity: "diagnostic".to_string(),
        id: accession.to_string(),
        suggestion:
            "Try searching with a filter such as `biomcp search diagnostic --disease HIV --source who-ivd` or inspect `biomcp list diagnostic`.".to_string(),
    })?;
    let sections = resolve_sections_for_source(DIAGNOSTIC_SOURCE_WHO_IVD, accession, sections)?;
    Ok(diagnostic_from_who_ivd_record(&record, sections))
}
