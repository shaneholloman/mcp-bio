use super::*;

#[cfg(test)]
mod tests;

pub fn variant_markdown(
    variant: &Variant,
    requested_sections: &[String],
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("variant.md.j2")?;
    let section_only = is_section_only_requested(requested_sections);
    let include_all = has_all_section(requested_sections);
    let requested = requested_section_names(requested_sections);
    let has_requested = |name: &str| requested.iter().any(|s| s.eq_ignore_ascii_case(name));
    let show_prediction_section = !section_only || include_all || has_requested("predict");
    let show_predictions_section = include_all || has_requested("predictions");
    let show_clinvar_section = !section_only || include_all || has_requested("clinvar");
    let show_population_section = !section_only || include_all || has_requested("population");
    let show_conservation_section = include_all || has_requested("conservation");
    let show_cosmic_section = include_all || has_requested("cosmic");
    let show_cgi_section = include_all || has_requested("cgi");
    let show_civic_section = include_all || has_requested("civic");
    let show_cbioportal_section = include_all || has_requested("cbioportal");
    let show_gwas_section = include_all || has_requested("gwas");
    let variant_label = if !variant.gene.trim().is_empty() && variant.hgvs_p.is_some() {
        format!(
            "{} {}",
            variant.gene.trim(),
            variant.hgvs_p.as_deref().unwrap_or_default().trim()
        )
    } else if !variant.gene.trim().is_empty() {
        variant.gene.trim().to_string()
    } else {
        variant.id.trim().to_string()
    };
    let prediction = variant.prediction.as_ref();
    let (expr_i, splice_i, chrom_i) = prediction
        .map(prediction_interpretations)
        .unwrap_or((None, None, None));
    let body = tmpl.render(context! {
        section_only => section_only,
        section_header => section_header(&variant_label, requested_sections),
        id => &variant.id,
        gene => &variant.gene,
        hgvs_p => &variant.hgvs_p,
        legacy_name => &variant.legacy_name,
        hgvs_c => &variant.hgvs_c,
        consequence => &variant.consequence,
        rsid => &variant.rsid,
        cosmic_id => &variant.cosmic_id,
        significance => &variant.significance,
        clinvar_id => &variant.clinvar_id,
        clinvar_review_status => &variant.clinvar_review_status,
        clinvar_review_stars => &variant.clinvar_review_stars,
        conditions => &variant.conditions,
        clinvar_conditions => &variant.clinvar_conditions,
        clinvar_condition_reports => &variant.clinvar_condition_reports,
        top_disease => &variant.top_disease,
        gnomad_af => &variant.gnomad_af,
        allele_frequency_percent => &variant.allele_frequency_percent,
        population_breakdown => &variant.population_breakdown,
        cadd_score => &variant.cadd_score,
        sift_pred => &variant.sift_pred,
        polyphen_pred => &variant.polyphen_pred,
        conservation => &variant.conservation,
        expanded_predictions => &variant.expanded_predictions,
        cosmic_context => &variant.cosmic_context,
        cgi_associations => &variant.cgi_associations,
        civic => &variant.civic,
        cancer_frequencies => &variant.cancer_frequencies,
        cancer_frequency_source => &variant.cancer_frequency_source,
        gwas => &variant.gwas,
        gwas_unavailable_reason => &variant.gwas_unavailable_reason,
        prediction => prediction,
        expression_interpretation => expr_i,
        splice_interpretation => splice_i,
        chromatin_interpretation => chrom_i,
        show_prediction_section => show_prediction_section,
        show_predictions_section => show_predictions_section,
        show_clinvar_section => show_clinvar_section,
        show_population_section => show_population_section,
        show_conservation_section => show_conservation_section,
        show_cosmic_section => show_cosmic_section,
        show_cgi_section => show_cgi_section,
        show_civic_section => show_civic_section,
        show_cbioportal_section => show_cbioportal_section,
        show_gwas_section => show_gwas_section,
        sections_block => format_sections_block("variant", &variant.id, sections_variant(variant, requested_sections)),
        related_block => format_related_block(related_variant(variant)),
    })?;
    Ok(append_evidence_urls(body, variant_evidence_urls(variant)))
}

fn prediction_interpretations(
    pred: &VariantPrediction,
) -> (
    Option<&'static str>,
    Option<&'static str>,
    Option<&'static str>,
) {
    let expr = pred.expression_lfc.map(|v| {
        if v > 0.2 {
            "Increased expression"
        } else if v < -0.2 {
            "Decreased expression"
        } else {
            "Minimal change"
        }
    });

    let splice = pred.splice_score.map(|v| {
        if v.abs() > 0.5 {
            "Higher splice impact"
        } else {
            "Low splice impact"
        }
    });

    let chrom = pred.chromatin_score.map(|v| {
        if v.abs() > 0.5 {
            "Altered accessibility"
        } else {
            "Low chromatin impact"
        }
    });

    (expr, splice, chrom)
}

#[allow(dead_code)]
pub fn variant_search_markdown(
    query: &str,
    results: &[VariantSearchResult],
) -> Result<String, BioMcpError> {
    variant_search_markdown_with_footer(query, results, "")
}

pub fn variant_search_markdown_with_footer(
    query: &str,
    results: &[VariantSearchResult],
    pagination_footer: &str,
) -> Result<String, BioMcpError> {
    variant_search_markdown_with_context(query, results, pagination_footer, None, None)
}

pub fn variant_search_markdown_with_context(
    query: &str,
    results: &[VariantSearchResult],
    pagination_footer: &str,
    gene_filter: Option<&str>,
    condition_filter: Option<&str>,
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("variant_search.md.j2")?;
    let body = tmpl.render(context! {
        query => query,
        count => results.len(),
        results => results,
        related_block => format_related_block(related_variant_search_results(
            results,
            gene_filter,
            condition_filter,
        )),
        pagination_footer => pagination_footer,
    })?;
    Ok(with_pagination_footer(body, pagination_footer))
}

#[allow(dead_code)]
pub fn phenotype_search_markdown(
    query: &str,
    results: &[PhenotypeSearchResult],
) -> Result<String, BioMcpError> {
    phenotype_search_markdown_with_footer(query, results, "")
}

pub fn phenotype_search_markdown_with_footer(
    query: &str,
    results: &[PhenotypeSearchResult],
    pagination_footer: &str,
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("phenotype_search.md.j2")?;
    let body = tmpl.render(context! {
        query => query,
        count => results.len(),
        results => results,
        related_block => format_related_block(related_phenotype_search_results(results)),
        pagination_footer => pagination_footer,
    })?;
    Ok(with_pagination_footer(body, pagination_footer))
}

#[allow(dead_code)]
pub fn gwas_search_markdown(
    query: &str,
    results: &[VariantGwasAssociation],
) -> Result<String, BioMcpError> {
    gwas_search_markdown_with_footer(query, results, "")
}

pub fn gwas_search_markdown_with_footer(
    query: &str,
    results: &[VariantGwasAssociation],
    pagination_footer: &str,
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("gwas_search.md.j2")?;
    let body = tmpl.render(context! {
        query => query,
        count => results.len(),
        results => results,
        pagination_footer => pagination_footer,
    })?;
    Ok(with_pagination_footer(body, pagination_footer))
}

pub fn variant_oncokb_markdown(result: &VariantOncoKbResult) -> String {
    let mut out = String::new();
    out.push_str("# OncoKB\n\n");
    out.push_str(&format!("Gene: {}\n", result.gene.trim()));
    out.push_str(&format!("Alteration: {}\n", result.alteration.trim()));
    if let Some(level) = result
        .level
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        out.push_str(&format!("Level: {level}\n"));
    }
    if let Some(oncogenic) = result
        .oncogenic
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        out.push_str(&format!("Oncogenic: {oncogenic}\n"));
    }
    if let Some(effect) = result
        .effect
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        out.push_str(&format!("Effect: {effect}\n"));
    }
    out.push('\n');

    if result.therapies.is_empty() {
        out.push_str("No therapy implications returned by OncoKB.\n");
    } else {
        out.push_str("## Therapies\n\n");
        out.push_str("| Drug | Level | Cancer Type | Note |\n");
        out.push_str("|------|-------|-------------|------|\n");
        for row in &result.therapies {
            let drugs = if row.drugs.is_empty() {
                "unspecified".to_string()
            } else {
                row.drugs.join(" + ")
            };
            let cancer = row.cancer_type.as_deref().unwrap_or("-");
            let note = row.note.as_deref().unwrap_or("-");
            out.push_str(&format!(
                "| {drugs} | {} | {cancer} | {note} |\n",
                row.level
            ));
        }
    }

    if !result.gene.trim().is_empty() && !result.alteration.trim().is_empty() {
        out.push_str(&format!(
            "\n[OncoKB](https://www.oncokb.org/gene/{}/{})\n",
            result.gene.trim(),
            result.alteration.trim()
        ));
    }

    out
}
