//! Diagnostic markdown renderers.

use super::*;

#[cfg(test)]
mod tests;

pub fn diagnostic_markdown(
    diagnostic: &Diagnostic,
    requested_sections: &[String],
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("diagnostic.md.j2")?;
    let include_all = has_all_section(requested_sections);
    let requested = requested_section_names(requested_sections);
    let has_requested = |name: &str| {
        requested
            .iter()
            .any(|section| section.eq_ignore_ascii_case(name))
    };
    let show_genes_section = include_all || has_requested("genes");
    let show_conditions_section = include_all || has_requested("conditions");
    let show_methods_section = include_all || has_requested("methods");

    tmpl.render(context! {
        accession => &diagnostic.accession,
        source_id => &diagnostic.source_id,
        name => &diagnostic.name,
        test_type => &diagnostic.test_type,
        manufacturer => &diagnostic.manufacturer,
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
        sections_block => format_sections_block(
            "diagnostic",
            &diagnostic.accession,
            sections_diagnostic(diagnostic, requested_sections),
        ),
        related_block => format_related_block(related_diagnostic(diagnostic)),
    })
    .map_err(Into::into)
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
    let top_accession = results
        .first()
        .map(|result| result.accession.as_str())
        .unwrap_or("");
    let body = tmpl.render(context! {
        query => query,
        count => results.len(),
        total => total,
        top_accession => top_accession,
        results => results,
        pagination_footer => pagination_footer,
    })?;
    Ok(with_pagination_footer(body, pagination_footer))
}
