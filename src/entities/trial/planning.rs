//! Rare-disease trial planning request/plan boundary.
//!
//! The first implementation uses a deliberately small curated seed for the
//! Phelan-McDermid / SHANK3 contract. Future ontology-backed expansion can add
//! sources behind this module without changing the request or plan shape.

use crate::error::BioMcpError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TrialPlanningMode {
    Search,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RareDiseaseTrialRequest {
    pub(crate) raw_query: Option<String>,
    pub(crate) condition: Option<String>,
    pub(crate) gene: Option<String>,
    pub(crate) sponsor: Option<String>,
    pub(crate) strict_condition: bool,
    pub(crate) mode: TrialPlanningMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct RareDiseaseTrialPlan {
    pub(crate) primary_condition_labels: Vec<ConditionLabel>,
    pub(crate) gene_labels: Vec<GeneLabel>,
    pub(crate) expanded_condition_labels: Vec<ConditionExpansion>,
    pub(crate) query_terms: Vec<TrialQueryTerm>,
    pub(crate) warnings: Vec<PlanningWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConditionLabel {
    pub(crate) label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GeneLabel {
    pub(crate) symbol: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConditionExpansion {
    pub(crate) label: String,
    pub(crate) source: String,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TrialQueryTerm {
    pub(crate) term: String,
    pub(crate) field: TrialQueryField,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TrialQueryField {
    Condition,
    Biomarker,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlanningWarning {
    pub(crate) term: String,
    pub(crate) reason: String,
}

pub(crate) fn plan_rare_disease_trials(
    request: RareDiseaseTrialRequest,
) -> Result<RareDiseaseTrialPlan, BioMcpError> {
    let mut plan = RareDiseaseTrialPlan::default();

    if let Some(condition) = request.condition.as_deref() {
        plan.primary_condition_labels.push(ConditionLabel {
            label: condition.to_string(),
        });
        plan.query_terms.push(TrialQueryTerm {
            term: condition.to_string(),
            field: TrialQueryField::Condition,
        });

        if !request.strict_condition && is_phelan_mcdermid(condition) {
            plan.expanded_condition_labels.push(ConditionExpansion {
                label: "22q13 deletion syndrome".to_string(),
                source: "curated rare-disease trial seed".to_string(),
                reason: "bounded synonym for Phelan-McDermid syndrome planning".to_string(),
            });
        }
    }

    if let Some(gene) = request.gene.as_deref() {
        if gene.eq_ignore_ascii_case("SHANK3") && request.condition.is_none() {
            plan.primary_condition_labels.push(ConditionLabel {
                label: "Phelan-McDermid syndrome".to_string(),
            });
            plan.expanded_condition_labels.push(ConditionExpansion {
                label: "22q13 deletion syndrome".to_string(),
                source: "curated rare-disease trial seed".to_string(),
                reason: "bounded SHANK3-associated condition for trial planning".to_string(),
            });
        }
        plan.gene_labels.push(GeneLabel {
            symbol: gene.to_string(),
        });
        plan.query_terms.push(TrialQueryTerm {
            term: gene.to_string(),
            field: TrialQueryField::Biomarker,
        });
    }

    if let Some(raw_query) = request.raw_query.as_deref() {
        add_noisy_term_warnings(raw_query, &mut plan.warnings);
    }

    Ok(plan)
}

fn is_phelan_mcdermid(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    normalized.contains("phelan") && normalized.contains("mcdermid")
}

fn add_noisy_term_warnings(raw_query: &str, warnings: &mut Vec<PlanningWarning>) {
    let lower = raw_query.to_ascii_lowercase();
    if lower.contains("autism") {
        warnings.push(PlanningWarning {
            term: "autism".to_string(),
            reason: "broad phenotype omitted from bounded rare-disease trial expansion".to_string(),
        });
    }
    for term in ["SHANK1", "SHANK2"] {
        if lower.contains(&term.to_ascii_lowercase()) {
            warnings.push(PlanningWarning {
                term: term.to_string(),
                reason: "unrelated SHANK-family term omitted from SHANK3 trial planning"
                    .to_string(),
            });
        }
    }
}
