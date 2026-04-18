use crate::error::BioMcpError;
use crate::sources::gtr::{GtrClient, GtrIndex, GtrRecord, GtrSyncMode};

use super::{
    DIAGNOSTIC_SECTION_CONDITIONS, DIAGNOSTIC_SECTION_GENES, DIAGNOSTIC_SECTION_METHODS,
    DIAGNOSTIC_SECTION_NAMES, DIAGNOSTIC_SOURCE, Diagnostic, optional_text,
    preferred_diagnostic_name,
};

#[derive(Debug, Clone, Copy, Default)]
struct DiagnosticSections {
    include_genes: bool,
    include_conditions: bool,
    include_methods: bool,
}

fn parse_sections(sections: &[String]) -> Result<DiagnosticSections, BioMcpError> {
    let mut out = DiagnosticSections::default();
    let mut include_all = false;

    for raw in sections {
        let section = raw.trim().to_ascii_lowercase();
        if section.is_empty() || section == "--json" || section == "-j" {
            continue;
        }

        match section.as_str() {
            DIAGNOSTIC_SECTION_GENES => out.include_genes = true,
            DIAGNOSTIC_SECTION_CONDITIONS => out.include_conditions = true,
            DIAGNOSTIC_SECTION_METHODS => out.include_methods = true,
            "all" => include_all = true,
            _ => {
                return Err(BioMcpError::InvalidArgument(format!(
                    "Unknown section \"{section}\" for diagnostic. Available: {}",
                    DIAGNOSTIC_SECTION_NAMES.join(", ")
                )));
            }
        }
    }

    if include_all {
        out.include_genes = true;
        out.include_conditions = true;
        out.include_methods = true;
    }

    Ok(out)
}

fn diagnostic_from_record(
    record: &GtrRecord,
    index: &GtrIndex,
    sections: DiagnosticSections,
) -> Diagnostic {
    Diagnostic {
        source: DIAGNOSTIC_SOURCE.to_string(),
        source_id: record.accession.clone(),
        accession: record.accession.clone(),
        name: preferred_diagnostic_name(record),
        test_type: optional_text(&record.test_type),
        manufacturer: optional_text(&record.manufacturer_test_name),
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

pub async fn get(accession: &str, sections: &[String]) -> Result<Diagnostic, BioMcpError> {
    let accession = accession.trim();
    if accession.is_empty() {
        return Err(BioMcpError::InvalidArgument(
            "Diagnostic accession is required. Example: biomcp get diagnostic GTR000000001.1"
                .into(),
        ));
    }

    let sections = parse_sections(sections)?;
    let client = GtrClient::ready(GtrSyncMode::Auto).await?;
    let index = client.load_index()?;
    let record = index.record(accession).ok_or_else(|| BioMcpError::NotFound {
        entity: "diagnostic".to_string(),
        id: accession.to_string(),
        suggestion:
            "Try searching with a filter such as `biomcp search diagnostic --gene BRCA1` or inspect `biomcp list diagnostic`.".to_string(),
    })?;

    Ok(diagnostic_from_record(record, &index, sections))
}
