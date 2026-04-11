//! Drug query builders and query-summary helpers.

use crate::error::BioMcpError;
use crate::sources::mychem::MyChemClient;

use super::DrugSearchFilters;

pub(super) fn build_mychem_query(filters: &DrugSearchFilters) -> Result<String, BioMcpError> {
    let mut terms: Vec<String> = Vec::new();

    if let Some(q) = filters
        .query
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        terms.push(MyChemClient::escape_query_value(q));
    }

    if let Some(target) = filters
        .target
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        // Prefer GtoPdb targets for consistent gene symbols.
        terms.push(format!(
            "gtopdb.interaction_targets.symbol:{}",
            MyChemClient::escape_query_value(target)
        ));
    }

    if let Some(ind) = filters
        .indication
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        if ind.chars().any(|c| c.is_whitespace()) {
            terms.push(format!(
                "drugcentral.drug_use.indication.concept_name:\"{}\"",
                MyChemClient::escape_query_value(ind)
            ));
        } else {
            terms.push(format!(
                "drugcentral.drug_use.indication.concept_name:*{}*",
                MyChemClient::escape_query_value(ind)
            ));
        }
    }

    if let Some(mechanism) = filters
        .mechanism
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let escaped = MyChemClient::escape_query_value(mechanism);
        let tokens = mechanism
            .split_whitespace()
            .map(MyChemClient::escape_query_value)
            .filter(|v| !v.is_empty())
            .collect::<Vec<_>>();

        let mut clauses = vec![
            format!("chembl.drug_mechanisms.action_type:\"{escaped}\""),
            format!("chembl.drug_mechanisms.mechanism_of_action:\"{escaped}\""),
            format!("ndc.pharm_classes:\"{escaped}\""),
        ];

        if !tokens.is_empty() {
            for field in [
                "chembl.drug_mechanisms.action_type",
                "chembl.drug_mechanisms.mechanism_of_action",
                "ndc.pharm_classes",
            ] {
                let all_tokens = tokens
                    .iter()
                    .map(|token| format!("{field}:*{token}*"))
                    .collect::<Vec<_>>()
                    .join(" AND ");
                clauses.push(format!("({all_tokens})"));
            }
        }

        for expansion in mechanism_atc_expansions(mechanism) {
            clauses.push(match expansion {
                AtcExpansion::Prefix(prefix) => {
                    format!("chembl.atc_classifications:{prefix}*")
                }
                AtcExpansion::Exact(code) => {
                    format!("chembl.atc_classifications:{code}")
                }
            });
        }

        terms.push(format!("({})", clauses.join(" OR ")));
    }

    if let Some(t) = filters
        .drug_type
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let t_norm = t.to_ascii_lowercase();
        let mapped = match t_norm.as_str() {
            "biologic" | "biologics" | "antibody" => Some("Antibody".to_string()),
            "small-molecule" | "small_molecule" | "small molecule" | "small" => {
                Some("Small molecule".to_string())
            }
            _ => None,
        };

        let value = mapped.unwrap_or_else(|| t.to_string());
        if value.chars().any(|c| c.is_whitespace()) {
            terms.push(format!(
                "chembl.molecule_type:\"{}\"",
                MyChemClient::escape_query_value(&value)
            ));
        } else {
            terms.push(format!(
                "chembl.molecule_type:{}",
                MyChemClient::escape_query_value(&value)
            ));
        }
    }

    if let Some(atc) = filters
        .atc
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        terms.push(format!(
            "chembl.atc_classifications:{}",
            MyChemClient::escape_query_value(atc)
        ));
    }

    if let Some(pharm_class) = filters
        .pharm_class
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let escaped = MyChemClient::escape_query_value(pharm_class);
        terms.push(format!(
            "(drugcentral.pharmacology_class:\"{escaped}\" OR ndc.pharm_classes:\"{escaped}\")"
        ));
    }

    if filters
        .interactions
        .as_deref()
        .map(str::trim)
        .is_some_and(|v| !v.is_empty())
    {
        return Err(BioMcpError::InvalidArgument(
            "Interaction-partner drug search is unavailable from the public data sources currently used by BioMCP.".into(),
        ));
    }

    if terms.is_empty() {
        return Err(BioMcpError::InvalidArgument(
            "At least one filter is required. Example: biomcp search drug -q pembrolizumab".into(),
        ));
    }

    Ok(terms.join(" AND "))
}

pub(super) fn mechanism_atc_expansions(mechanism: &str) -> Vec<AtcExpansion> {
    let normalized = mechanism.trim().to_ascii_lowercase();
    if normalized
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .any(|token| token == "purine")
    {
        return vec![
            AtcExpansion::Prefix("L01BB"),
            AtcExpansion::Exact("L01XX08"),
        ];
    }
    Vec::new()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AtcExpansion {
    Prefix(&'static str),
    Exact(&'static str),
}

fn normalize_query_summary(filters: &DrugSearchFilters) -> String {
    if !filters.has_structured_filters()
        && let Some(q) = filters
            .query
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
    {
        return q.to_string();
    }

    let mut parts: Vec<String> = Vec::new();
    if let Some(q) = filters
        .query
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        parts.push(q.to_string());
    }
    if let Some(v) = filters
        .target
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        parts.push(format!("target={v}"));
    }
    if let Some(v) = filters
        .indication
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        parts.push(format!("indication={v}"));
    }
    if let Some(v) = filters
        .mechanism
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        parts.push(format!("mechanism={v}"));
    }
    if let Some(v) = filters
        .drug_type
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        parts.push(format!("type={v}"));
    }
    if let Some(v) = filters
        .atc
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        parts.push(format!("atc={v}"));
    }
    if let Some(v) = filters
        .pharm_class
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        parts.push(format!("pharm_class={v}"));
    }
    if let Some(v) = filters
        .interactions
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        parts.push(format!("interactions={v}"));
    }

    parts.join(", ")
}

pub fn search_query_summary(filters: &DrugSearchFilters) -> String {
    normalize_query_summary(filters)
}

#[cfg(test)]
mod tests;
