//! Diagnostic markdown renderers.

use super::*;
use serde::Serialize;
use std::fmt::Write as _;

#[cfg(test)]
mod tests;

#[derive(Debug, Serialize)]
struct DiagnosticSearchRow<'a> {
    accession: &'a str,
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    test_type: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    manufacturer_or_lab: Option<&'a str>,
    source: &'a str,
    source_label: String,
    genes: &'a [String],
    conditions: &'a [String],
}

pub fn diagnostic_markdown(
    diagnostic: &Diagnostic,
    requested_sections: &[String],
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("diagnostic.md.j2")?;
    let include_all = has_all_section(requested_sections);
    let requested = requested_section_names(requested_sections);
    let supported =
        crate::entities::diagnostic::supported_diagnostic_sections_for_source(&diagnostic.source);
    let has_requested = |name: &str| {
        requested
            .iter()
            .any(|section| section.eq_ignore_ascii_case(name))
    };
    let supports = |name: &str| {
        supported
            .iter()
            .any(|section| section.eq_ignore_ascii_case(name))
    };
    let show_genes_section = supports("genes") && (include_all || has_requested("genes"));
    let show_conditions_section =
        supports("conditions") && (include_all || has_requested("conditions"));
    let show_methods_section = supports("methods") && (include_all || has_requested("methods"));
    let show_regulatory_section = supports("regulatory") && has_requested("regulatory");
    let regulatory_block = if show_regulatory_section {
        render_regulatory_block(diagnostic.regulatory.as_deref())
    } else {
        String::new()
    };

    tmpl.render(context! {
        accession => &diagnostic.accession,
        source_label => crate::entities::diagnostic::diagnostic_source_label(&diagnostic.source),
        source_id => &diagnostic.source_id,
        name => &diagnostic.name,
        test_type_label => if diagnostic.source.eq_ignore_ascii_case(crate::entities::diagnostic::DIAGNOSTIC_SOURCE_WHO_IVD) { "Assay Format" } else { "Type" },
        test_type => &diagnostic.test_type,
        manufacturer => &diagnostic.manufacturer,
        target_marker => &diagnostic.target_marker,
        regulatory_version => &diagnostic.regulatory_version,
        prequalification_year => &diagnostic.prequalification_year,
        laboratory => &diagnostic.laboratory,
        institution => &diagnostic.institution,
        country => &diagnostic.country,
        clia_number => &diagnostic.clia_number,
        state_licenses => &diagnostic.state_licenses,
        current_status => &diagnostic.current_status,
        public_status => &diagnostic.public_status,
        method_categories => &diagnostic.method_categories,
        genes => &diagnostic.genes,
        conditions => &diagnostic.conditions,
        methods => &diagnostic.methods,
        show_genes_section => show_genes_section,
        show_conditions_section => show_conditions_section,
        show_methods_section => show_methods_section,
        regulatory_block => regulatory_block,
        sections_block => format_sections_block(
            "diagnostic",
            &diagnostic.accession,
            sections_diagnostic(diagnostic, requested_sections),
        ),
        related_block => format_related_block(related_diagnostic(diagnostic)),
    })
    .map_err(Into::into)
}

fn render_regulatory_block(rows: Option<&[DiagnosticRegulatoryRecord]>) -> String {
    let Some(rows) = rows else {
        return String::new();
    };

    let mut out = String::from("## Regulatory (FDA Device)\n\n");
    if rows.is_empty() {
        out.push_str("No FDA device 510(k) or PMA records matched this diagnostic.\n\n");
        return out;
    }

    out.push_str(
        "| Type | Number | Name | Applicant | Decision Date | Decision | Product Code | Supplements |\n",
    );
    out.push_str("| --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for row in rows {
        let supplements = row
            .supplement_count
            .filter(|count| *count > 0)
            .map(|count| count.to_string())
            .unwrap_or_else(|| "-".to_string());
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} |",
            markdown_cell(&row.submission_type),
            markdown_cell(&row.number),
            markdown_cell(&row.display_name),
            markdown_cell(row.applicant.as_deref().unwrap_or_default()),
            markdown_cell(row.decision_date.as_deref().unwrap_or_default()),
            markdown_cell(row.decision_description.as_deref().unwrap_or_default()),
            markdown_cell(row.product_code.as_deref().unwrap_or_default()),
            markdown_cell(&supplements),
        );
    }
    out.push('\n');
    out
}

#[allow(dead_code)]
pub fn diagnostic_search_markdown(
    query: &str,
    results: &[DiagnosticSearchResult],
    total: Option<usize>,
) -> Result<String, BioMcpError> {
    diagnostic_search_markdown_with_footer(query, results, total, "")
}

pub fn diagnostic_search_markdown_with_footer(
    query: &str,
    results: &[DiagnosticSearchResult],
    total: Option<usize>,
    pagination_footer: &str,
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("diagnostic_search.md.j2")?;
    let rendered_results = results
        .iter()
        .map(|result| DiagnosticSearchRow {
            accession: &result.accession,
            name: &result.name,
            test_type: result.test_type.as_deref(),
            manufacturer_or_lab: result.manufacturer_or_lab.as_deref(),
            source: &result.source,
            source_label: crate::entities::diagnostic::diagnostic_source_label(&result.source)
                .to_string(),
            genes: &result.genes,
            conditions: &result.conditions,
        })
        .collect::<Vec<_>>();
    let top_accession = results
        .first()
        .map(|result| result.accession.as_str())
        .unwrap_or("");
    let body = tmpl.render(context! {
        query => query,
        count => results.len(),
        total => total,
        top_accession => crate::render::markdown::quote_arg(top_accession),
        results => &rendered_results,
        pagination_footer => pagination_footer,
    })?;
    Ok(with_pagination_footer(body, pagination_footer))
}
