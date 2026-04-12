use super::*;

#[cfg(test)]
mod tests;

pub fn adverse_event_markdown(
    event: &AdverseEvent,
    requested_sections: &[String],
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("adverse_event.md.j2")?;
    let section_only = is_section_only_requested(requested_sections);
    let parsed = crate::entities::adverse_event::parse_sections(requested_sections)?;
    let show_reactions_section = !section_only || parsed.include_reactions;
    let show_outcomes_section = !section_only || parsed.include_outcomes;
    let show_concomitant_section = !section_only || parsed.include_concomitant;
    let show_guidance_section = !section_only || parsed.include_guidance;
    let drug = quote_arg(&event.drug);
    let indication = event
        .indication
        .as_deref()
        .map(quote_arg)
        .unwrap_or_default();
    let body = tmpl.render(context! {
        section_only => section_only,
        section_header => section_header("Adverse Event", requested_sections),
        report_id => &event.report_id,
        drug => &event.drug,
        reactions => &event.reactions,
        outcomes => &event.outcomes,
        patient => &event.patient,
        concomitant_medications => &event.concomitant_medications,
        reporter_type => &event.reporter_type,
        reporter_country => &event.reporter_country,
        indication => &event.indication,
        guidance_indication => indication,
        guidance_drug => drug,
        show_reactions_section => show_reactions_section,
        show_outcomes_section => show_outcomes_section,
        show_concomitant_section => show_concomitant_section,
        show_guidance_section => show_guidance_section,
        serious => &event.serious,
        date => &event.date,
        sections_block => format_sections_block("adverse-event", &event.report_id, sections_adverse_event(event, requested_sections)),
        related_block => format_related_block(related_adverse_event(event)),
    })?;
    Ok(append_evidence_urls(
        body,
        adverse_event_evidence_urls(event),
    ))
}

pub fn adverse_event_search_markdown(
    query: &str,
    results: &[AdverseEventSearchResult],
    summary: &AdverseEventSearchSummary,
) -> Result<String, BioMcpError> {
    adverse_event_search_markdown_with_footer(query, results, summary, "")
}

pub fn adverse_event_search_markdown_with_footer(
    query: &str,
    results: &[AdverseEventSearchResult],
    summary: &AdverseEventSearchSummary,
    pagination_footer: &str,
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("adverse_event_search.md.j2")?;
    let body = tmpl.render(context! {
        query => query,
        count => results.len(),
        summary => summary,
        results => results,
        pagination_footer => pagination_footer,
    })?;
    Ok(with_pagination_footer(body, pagination_footer))
}

pub fn adverse_event_count_markdown(
    query: &str,
    count_field: &str,
    buckets: &[AdverseEventCountBucket],
) -> Result<String, BioMcpError> {
    let mut out = String::new();
    out.push_str("# Adverse Event Counts\n");
    out.push_str(&format!("\nQuery: {query}\n"));
    out.push_str(&format!("Count field: {count_field}\n\n"));
    out.push_str("| Value | Count |\n");
    out.push_str("|---|---|\n");
    if buckets.is_empty() {
        out.push_str("| - | 0 |\n");
    } else {
        for bucket in buckets {
            out.push_str(&format!("| {} | {} |\n", bucket.value, bucket.count));
        }
    }
    Ok(out)
}

pub fn device_event_markdown(event: &DeviceEvent) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("device_event.md.j2")?;
    let body = tmpl.render(context! {
        report_id => &event.report_id,
        report_number => &event.report_number,
        device => &event.device,
        manufacturer => &event.manufacturer,
        event_type => &event.event_type,
        date => &event.date,
        description => &event.description,
        related_block => format_related_block(related_device_event(event)),
    })?;
    Ok(append_evidence_urls(
        body,
        device_event_evidence_urls(event),
    ))
}

#[allow(dead_code)]
pub fn device_event_search_markdown(
    query: &str,
    results: &[DeviceEventSearchResult],
) -> Result<String, BioMcpError> {
    device_event_search_markdown_with_footer(query, results, "")
}

pub fn device_event_search_markdown_with_footer(
    query: &str,
    results: &[DeviceEventSearchResult],
    pagination_footer: &str,
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("device_event_search.md.j2")?;
    let body = tmpl.render(context! {
        query => query,
        count => results.len(),
        results => results,
        pagination_footer => pagination_footer,
    })?;
    Ok(with_pagination_footer(body, pagination_footer))
}

#[allow(dead_code)]
pub fn recall_search_markdown(
    query: &str,
    results: &[RecallSearchResult],
) -> Result<String, BioMcpError> {
    recall_search_markdown_with_footer(query, results, "")
}

pub fn recall_search_markdown_with_footer(
    query: &str,
    results: &[RecallSearchResult],
    pagination_footer: &str,
) -> Result<String, BioMcpError> {
    let tmpl = env()?.get_template("recall_search.md.j2")?;
    let body = tmpl.render(context! {
        query => query,
        count => results.len(),
        results => results,
        pagination_footer => pagination_footer,
    })?;
    Ok(with_pagination_footer(body, pagination_footer))
}
