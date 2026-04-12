//! Shared funding render rows and formatters for markdown renderers.

use super::*;

#[derive(serde::Serialize)]
pub(super) struct FundingGrantRenderRow {
    project_title: String,
    pi_name: String,
    organization: String,
    fiscal_year: String,
    award_amount: String,
}

pub(super) fn format_funding_amount(amount: u64) -> String {
    let digits = amount.to_string();
    let mut out = String::new();
    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    let grouped = out.chars().rev().collect::<String>();
    format!("${grouped}")
}

pub(super) fn funding_year_window(section: &NihReporterFundingSection) -> String {
    match (section.fiscal_years.first(), section.fiscal_years.last()) {
        (Some(start), Some(end)) if start == end => format!("FY{start}"),
        (Some(start), Some(end)) => format!("FY{start}-FY{end}"),
        _ => "recent NIH fiscal years".to_string(),
    }
}

pub(super) fn funding_summary_line(section: Option<&NihReporterFundingSection>) -> Option<String> {
    let section = section.filter(|section| !section.grants.is_empty())?;
    Some(format!(
        "Showing top {} unique grants from {} matching NIH project-year records across {}.",
        section.grants.len(),
        section.matching_project_years,
        funding_year_window(section)
    ))
}

pub(super) fn funding_project_cell(grant: &NihReporterGrant) -> String {
    let title = markdown_cell(&grant.project_title);
    if let Some(url) = grant
        .project_detail_url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
    {
        format!("[{title}]({url})")
    } else {
        title
    }
}

pub(super) fn funding_rows(
    section: Option<&NihReporterFundingSection>,
) -> Vec<FundingGrantRenderRow> {
    let Some(section) = section else {
        return Vec::new();
    };

    section
        .grants
        .iter()
        .map(|grant| FundingGrantRenderRow {
            project_title: funding_project_cell(grant),
            pi_name: grant
                .pi_name
                .as_deref()
                .map(markdown_cell)
                .unwrap_or_else(|| "-".to_string()),
            organization: grant
                .organization
                .as_deref()
                .map(markdown_cell)
                .unwrap_or_else(|| "-".to_string()),
            fiscal_year: grant.fiscal_year.to_string(),
            award_amount: format_funding_amount(grant.award_amount),
        })
        .collect()
}
