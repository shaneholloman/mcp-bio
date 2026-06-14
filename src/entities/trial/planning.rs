//! Rare-disease trial planning request/plan boundary.
//!
//! Ticket 414 lands the contract first. The implementation step fills this pure
//! planner with bounded rare-disease expansion behavior without calling source
//! clients.

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
    _request: RareDiseaseTrialRequest,
) -> Result<RareDiseaseTrialPlan, BioMcpError> {
    Ok(RareDiseaseTrialPlan::default())
}
