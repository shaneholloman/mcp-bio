use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::OnceLock;
use std::time::Duration;

use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::entities::SearchPage;
use crate::error::BioMcpError;
use crate::sources::chembl::ChemblClient;
use crate::sources::civic::{CivicClient, CivicContext};
use crate::sources::ema::{EmaClient, EmaDrugIdentity, EmaSyncMode};
use crate::sources::mychem::{
    MYCHEM_FIELDS_GET, MYCHEM_FIELDS_SEARCH, MyChemClient, MyChemHit, MyChemNdcField,
    MyChemQueryResponse,
};
use crate::sources::openfda::{DrugsFdaResult, OpenFdaClient, OpenFdaResponse};
use crate::sources::opentargets::{OpenTargetsClient, OpenTargetsTarget};
use crate::sources::who_pq::{WhoPqClient, WhoPqIdentity, WhoPqSyncMode};
use crate::transform;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Drug {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drugbank_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chembl_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unii: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drug_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mechanism: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mechanisms: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_date_raw: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_date_display: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub brand_names: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variant_targets: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_family_name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub indications: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interactions: Vec<DrugInteraction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interaction_text: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pharm_classes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub top_adverse_events: Vec<String>,

    #[serde(skip)]
    pub faers_query: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<DrugLabel>,

    #[serde(skip)]
    pub label_set_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub shortage: Option<Vec<DrugShortageEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approvals: Option<Vec<DrugApproval>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub us_safety_warnings: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ema_regulatory: Option<Vec<EmaRegulatoryRow>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ema_safety: Option<EmaSafetyInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ema_shortage: Option<Vec<EmaShortageEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub who_prequalification: Option<Vec<WhoPrequalificationEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub civic: Option<CivicContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrugInteraction {
    pub drug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrugLabelIndication {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pivotal_trial: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrugLabel {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub indication_summary: Vec<DrugLabelIndication>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indications: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dosage: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrugShortageEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub company_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generic_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_info: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_posting_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrugApproval {
    pub application_number: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sponsor_name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub openfda_brand_names: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub openfda_generic_names: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub products: Vec<DrugApprovalProduct>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub submissions: Vec<DrugApprovalSubmission>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrugApprovalProduct {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brand_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dosage_form: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marketing_status: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_ingredients: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrugApprovalSubmission {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submission_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submission_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrugSearchResult {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drugbank_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drug_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mechanism: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhoPrequalificationEntry {
    pub who_reference_number: String,
    pub inn: String,
    pub presentation: String,
    pub dosage_form: String,
    pub product_type: String,
    pub therapeutic_area: String,
    pub applicant: String,
    pub listing_basis: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alternative_listing_basis: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prequalification_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhoPrequalificationSearchResult {
    pub inn: String,
    pub therapeutic_area: String,
    pub dosage_form: String,
    pub applicant: String,
    pub who_reference_number: String,
    pub listing_basis: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prequalification_date: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DrugRegion {
    #[default]
    Us,
    Eu,
    Who,
    All,
}

impl DrugRegion {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Us => "us",
            Self::Eu => "eu",
            Self::Who => "who",
            Self::All => "all",
        }
    }

    pub fn includes_us(self) -> bool {
        matches!(self, Self::Us | Self::All)
    }

    pub fn includes_eu(self) -> bool {
        matches!(self, Self::Eu | Self::All)
    }

    pub fn includes_who(self) -> bool {
        matches!(self, Self::Who | Self::All)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmaDrugSearchResult {
    pub name: String,
    pub active_substance: String,
    pub ema_product_number: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmaRegulatoryRow {
    pub medicine_name: String,
    pub active_substance: String,
    pub ema_product_number: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub holder: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent_activity: Vec<EmaRegulatoryActivity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmaRegulatoryActivity {
    pub first_published_date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EmaSafetyInfo {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dhpcs: Vec<EmaDhpcEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub referrals: Vec<EmaReferralEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub psusas: Vec<EmaPsusaEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmaDhpcEntry {
    pub medicine_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dhpc_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regulatory_outcome: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_published_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmaReferralEntry {
    pub referral_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_substance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub associated_medicines: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_referral: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referral_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub procedure_start_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prac_recommendation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmaPsusaEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_medicines: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_substance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub procedure_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regulatory_outcome: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_published_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmaShortageEntry {
    pub medicine_affected: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability_of_alternatives: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_published_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated_date: Option<String>,
}

#[derive(Debug, Clone)]
pub enum DrugSearchPageWithRegion {
    Us(SearchPage<DrugSearchResult>),
    Eu(SearchPage<EmaDrugSearchResult>),
    Who(SearchPage<WhoPrequalificationSearchResult>),
    All {
        us: SearchPage<DrugSearchResult>,
        eu: SearchPage<EmaDrugSearchResult>,
        who: SearchPage<WhoPrequalificationSearchResult>,
    },
}

#[derive(Debug, Clone, Default)]
pub struct DrugSearchFilters {
    pub query: Option<String>,
    pub target: Option<String>,
    pub indication: Option<String>,
    pub mechanism: Option<String>,
    pub drug_type: Option<String>,
    pub atc: Option<String>,
    pub pharm_class: Option<String>,
    pub interactions: Option<String>,
}

impl DrugSearchFilters {
    pub fn has_structured_filters(&self) -> bool {
        self.target.is_some()
            || self.indication.is_some()
            || self.mechanism.is_some()
            || self.drug_type.is_some()
            || self.atc.is_some()
            || self.pharm_class.is_some()
            || self.interactions.is_some()
    }
}

const DRUG_SECTION_LABEL: &str = "label";
const DRUG_SECTION_REGULATORY: &str = "regulatory";
const DRUG_SECTION_SAFETY: &str = "safety";
const DRUG_SECTION_SHORTAGE: &str = "shortage";
const DRUG_SECTION_TARGETS: &str = "targets";
const DRUG_SECTION_INDICATIONS: &str = "indications";
const DRUG_SECTION_INTERACTIONS: &str = "interactions";
const DRUG_SECTION_CIVIC: &str = "civic";
const DRUG_SECTION_APPROVALS: &str = "approvals";
const DRUG_SECTION_ALL: &str = "all";

pub const DRUG_SECTION_NAMES: &[&str] = &[
    DRUG_SECTION_LABEL,
    DRUG_SECTION_REGULATORY,
    DRUG_SECTION_SAFETY,
    DRUG_SECTION_SHORTAGE,
    DRUG_SECTION_TARGETS,
    DRUG_SECTION_INDICATIONS,
    DRUG_SECTION_INTERACTIONS,
    DRUG_SECTION_CIVIC,
    DRUG_SECTION_APPROVALS,
    DRUG_SECTION_ALL,
];

const OPTIONAL_SAFETY_TIMEOUT: Duration = Duration::from_secs(8);

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

fn build_mychem_query(filters: &DrugSearchFilters) -> Result<String, BioMcpError> {
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

pub async fn search(
    filters: &DrugSearchFilters,
    limit: usize,
) -> Result<Vec<DrugSearchResult>, BioMcpError> {
    Ok(search_page(filters, limit, 0).await?.results)
}

pub async fn search_page(
    filters: &DrugSearchFilters,
    limit: usize,
    offset: usize,
) -> Result<SearchPage<DrugSearchResult>, BioMcpError> {
    const MAX_SEARCH_LIMIT: usize = 50;
    if limit == 0 || limit > MAX_SEARCH_LIMIT {
        return Err(BioMcpError::InvalidArgument(format!(
            "--limit must be between 1 and {MAX_SEARCH_LIMIT}"
        )));
    }

    let q = build_mychem_query(filters)?;

    let client = MyChemClient::new()?;
    // Fetch extra hits to account for de-duplication by normalized name.
    let fetch_limit = if filters
        .mechanism
        .as_deref()
        .map(str::trim)
        .is_some_and(|v| !v.is_empty())
    {
        MAX_SEARCH_LIMIT
    } else {
        (limit.saturating_mul(2)).min(MAX_SEARCH_LIMIT)
    };
    let resp = client
        .query_with_fields(&q, fetch_limit, offset, MYCHEM_FIELDS_SEARCH)
        .await?;

    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<DrugSearchResult> = Vec::new();
    for hit in &resp.hits {
        let Some(mut r) = transform::drug::from_mychem_search_hit(hit) else {
            continue;
        };

        if let Some(requested_target) = filters
            .target
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            if !hit_mentions_target(hit, requested_target) {
                continue;
            }
            // Display the matched target explicitly so multi-target drugs are not misleading.
            r.target = Some(requested_target.to_ascii_uppercase());
        }

        if let Some(requested_mechanism) = filters
            .mechanism
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            && !hit_mentions_mechanism(hit, requested_mechanism)
        {
            continue;
        }

        // Normalize and de-duplicate by name.
        r.name = r.name.trim().to_ascii_lowercase();
        if r.name.is_empty() {
            continue;
        }
        if !seen.insert(r.name.clone()) {
            continue;
        }

        out.push(r);
        if out.len() >= limit {
            break;
        }
    }

    if should_attempt_openfda_fallback(&out, offset, filters)
        && let Some(query) = filters
            .query
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        && let Ok(client) = OpenFdaClient::new()
        && let Ok(Some(label_response)) = client.label_search(query).await
    {
        let rows = search_results_from_openfda_label_response(&label_response, query, limit);
        if !rows.is_empty() {
            let total = rows.len();
            return Ok(SearchPage::offset(rows, Some(total)));
        }
    }

    Ok(SearchPage::offset(out, Some(resp.total)))
}

fn should_attempt_openfda_fallback(
    out: &[DrugSearchResult],
    offset: usize,
    filters: &DrugSearchFilters,
) -> bool {
    out.is_empty() && offset == 0 && !filters.has_structured_filters()
}

fn hit_mentions_target(hit: &MyChemHit, target: &str) -> bool {
    let target = target.trim();
    if target.is_empty() {
        return false;
    }
    let target_upper = target.to_ascii_uppercase();

    if let Some(gtopdb) = hit.gtopdb.as_ref() {
        for row in &gtopdb.interaction_targets {
            if row
                .symbol
                .as_deref()
                .map(str::trim)
                .is_some_and(|s| s.eq_ignore_ascii_case(&target_upper))
            {
                return true;
            }
        }
    }

    if let Some(chembl) = hit.chembl.as_ref() {
        for row in &chembl.drug_mechanisms {
            if row
                .target_name
                .as_deref()
                .map(str::trim)
                .is_some_and(|s| s.eq_ignore_ascii_case(&target_upper))
            {
                return true;
            }
        }
    }

    false
}

fn text_matches_mechanism(candidate: &str, mechanism: &str, tokens: &[&str]) -> bool {
    let candidate = candidate.trim();
    if candidate.is_empty() {
        return false;
    }
    let candidate_lower = candidate.to_ascii_lowercase();
    if candidate_lower.contains(mechanism) {
        return true;
    }
    tokens.iter().all(|token| candidate_lower.contains(token))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AtcExpansion {
    Prefix(&'static str),
    Exact(&'static str),
}

fn mechanism_atc_expansions(mechanism: &str) -> Vec<AtcExpansion> {
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

fn hit_mentions_mechanism(hit: &MyChemHit, mechanism: &str) -> bool {
    let mechanism = mechanism.trim().to_ascii_lowercase();
    if mechanism.is_empty() {
        return false;
    }
    let tokens = mechanism
        .split_whitespace()
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>();
    let atc_expansions = mechanism_atc_expansions(&mechanism);

    if let Some(chembl) = hit.chembl.as_ref() {
        for row in &chembl.drug_mechanisms {
            if row
                .action_type
                .as_deref()
                .is_some_and(|action| text_matches_mechanism(action, &mechanism, &tokens))
                || row
                    .mechanism_of_action
                    .as_deref()
                    .is_some_and(|action| text_matches_mechanism(action, &mechanism, &tokens))
            {
                return true;
            }
        }

        if chembl
            .atc_classifications
            .clone()
            .into_vec()
            .iter()
            .any(|code| {
                atc_expansions.iter().any(|expansion| match expansion {
                    AtcExpansion::Prefix(prefix) => code.starts_with(prefix),
                    AtcExpansion::Exact(exact) => code == exact,
                })
            })
        {
            return true;
        }
    }

    if let Some(ndc) = hit.ndc.as_ref() {
        let matches_class = |value: &str| text_matches_mechanism(value, &mechanism, &tokens);
        match ndc {
            MyChemNdcField::One(v) => {
                if v.pharm_classes
                    .iter()
                    .filter_map(|cls| cls.as_str())
                    .any(matches_class)
                {
                    return true;
                }
            }
            MyChemNdcField::Many(rows) => {
                if rows.iter().any(|row| {
                    row.pharm_classes
                        .iter()
                        .filter_map(|cls| cls.as_str())
                        .any(matches_class)
                }) {
                    return true;
                }
            }
        }
    }

    false
}

#[derive(Debug, Clone, Copy, Default)]
struct DrugSections {
    include_label: bool,
    include_regulatory: bool,
    include_safety: bool,
    include_shortage: bool,
    include_targets: bool,
    include_indications: bool,
    include_interactions: bool,
    include_civic: bool,
    include_approvals: bool,
    requested_all: bool,
    requested_safety: bool,
    requested_shortage: bool,
}

fn parse_sections(sections: &[String]) -> Result<DrugSections, BioMcpError> {
    let mut out = DrugSections::default();
    let mut include_all = false;
    let mut any_section = false;

    for raw in sections {
        let section = raw.trim().to_ascii_lowercase();
        if section.is_empty() {
            continue;
        }
        if section == "--json" || section == "-j" {
            continue;
        }
        any_section = true;
        match section.as_str() {
            DRUG_SECTION_LABEL => {
                out.include_label = true;
            }
            DRUG_SECTION_REGULATORY => out.include_regulatory = true,
            DRUG_SECTION_SAFETY => {
                out.include_safety = true;
                out.requested_safety = true;
            }
            DRUG_SECTION_SHORTAGE => {
                out.include_shortage = true;
                out.requested_shortage = true;
            }
            DRUG_SECTION_TARGETS => out.include_targets = true,
            DRUG_SECTION_INDICATIONS => out.include_indications = true,
            DRUG_SECTION_INTERACTIONS => out.include_interactions = true,
            DRUG_SECTION_CIVIC => out.include_civic = true,
            DRUG_SECTION_APPROVALS => out.include_approvals = true,
            DRUG_SECTION_ALL => {
                include_all = true;
                out.requested_all = true;
            }
            _ => {
                return Err(BioMcpError::InvalidArgument(format!(
                    "Unknown section \"{section}\" for drug. Available: {}",
                    DRUG_SECTION_NAMES.join(", ")
                )));
            }
        }
    }

    if include_all {
        out.include_label = true;
        out.include_regulatory = true;
        out.include_safety = true;
        out.include_shortage = true;
        out.include_targets = true;
        out.include_indications = true;
        out.include_interactions = true;
        out.include_civic = true;
    } else if !any_section {
        out.include_targets = true;
    }

    Ok(out)
}

fn is_section_only_requested(sections: &[String]) -> bool {
    !sections
        .iter()
        .any(|section| section.trim().eq_ignore_ascii_case(DRUG_SECTION_ALL))
        && sections.iter().any(|section| !section.trim().is_empty())
}

fn normalize_date_yyyymmdd(value: Option<&str>) -> Option<String> {
    let v = value?.trim();
    if v.len() != 8 || !v.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(format!("{}-{}-{}", &v[0..4], &v[4..6], &v[6..8]))
}

fn label_text(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value?;
    let text = match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|v| v.as_str())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
        _ => String::new(),
    };
    let text = text.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

fn truncate_with_note(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let truncated = value.chars().take(max_chars).collect::<String>();
    let total = value.chars().count();
    format!("{truncated}\n\n(truncated, {total} chars total)")
}

fn label_subsection_boundary_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\(\s*1\.\d+\s*\)").expect("valid label subsection regex"))
}

fn label_numbered_subsection_heading_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\b1\.\d+\s+[A-Z]").expect("valid subsection heading regex"))
}

fn label_numbered_subsection_prefix_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"^\s*1\.\d+\s+").expect("valid subsection heading prefix regex")
    })
}

fn label_heading_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)^\s*1\s+indications and usage\b[:\s-]*").expect("valid heading regex")
    })
}

fn normalize_label_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn find_ascii_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    let needle = needle.trim();
    if needle.is_empty() {
        return None;
    }

    haystack
        .to_ascii_lowercase()
        .find(&needle.to_ascii_lowercase())
}

fn strip_label_intro_prefix(segment: &str) -> &str {
    let lower = segment.to_ascii_lowercase();
    for needle in ["indicated:", "indicated for:", "indicated for"] {
        if let Some(idx) = lower.rfind(needle) {
            return segment[idx + needle.len()..].trim();
        }
    }
    segment.trim()
}

fn strip_leading_label_indication_prefixes(mut segment: &str) -> &str {
    loop {
        let lower = segment.to_ascii_lowercase();
        let mut next = None;
        for needle in [
            "the treatment of ",
            "treatment of ",
            "adult patients with ",
            "adult patient with ",
            "adults with ",
            "pediatric patients with ",
            "pediatric patient with ",
            "patients with ",
            "patient with ",
            "children with ",
            "women with ",
            "people with ",
        ] {
            if lower.starts_with(needle) {
                next = Some(segment[needle.len()..].trim());
                break;
            }
        }
        match next {
            Some(trimmed) => segment = trimmed,
            None => return segment.trim(),
        }
    }
}

fn label_continuation_prefix(lower: &str) -> bool {
    [
        "for ",
        "as ",
        "in combination",
        "continued as ",
        "following ",
        "after ",
        "where ",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
}

fn label_patient_phrase_start<'a>(segment: &'a str, lower: &str) -> Option<&'a str> {
    [
        "adults with ",
        "adult patients with ",
        "adult patient with ",
        "pediatric patients with ",
        "pediatric patient with ",
        "patients with ",
        "patient with ",
        "children with ",
        "women with ",
        "people with ",
        "treatment of ",
    ]
    .iter()
    .find_map(|needle| lower.find(needle).map(|idx| &segment[idx + needle.len()..]))
}

fn label_candidate_cutoff(segment: &str) -> &str {
    let lower = segment.to_ascii_lowercase();
    let mut end = segment.len();
    for needle in [
        ",",
        ";",
        " in combination",
        " as a single agent",
        " as first-line",
        " as first line",
        " as adjuvant",
        " as determined",
        " for ",
        " in adult",
        " in pediatric",
        " in patients",
        " after ",
        " following ",
        " who ",
        " who:",
        " whose ",
        " where ",
    ] {
        if let Some(idx) = lower.find(needle) {
            end = end.min(idx);
        }
    }
    segment[..end].trim()
}

fn normalize_label_indication_name(segment: &str) -> Option<String> {
    let candidate = strip_leading_label_indication_prefixes(label_candidate_cutoff(segment))
        .trim_matches(|c: char| c.is_whitespace() || matches!(c, ':' | ';' | '.' | '-'))
        .trim();
    if candidate.is_empty() {
        return None;
    }
    let lower = candidate.to_ascii_lowercase();
    let has_disease_signal = [
        "cancer",
        "carcinoma",
        "melanoma",
        "lymphoma",
        "leukemia",
        "tumor",
        "tumours",
        "myeloma",
        "sarcoma",
        "nsclc",
        "hnscc",
        "rcc",
    ]
    .iter()
    .any(|needle| lower.contains(needle));
    if !has_disease_signal {
        return None;
    }
    Some(candidate.to_string())
}

fn extract_label_indication_name(segment: &str) -> Option<String> {
    let segment = strip_label_intro_prefix(segment);
    let lower = segment.to_ascii_lowercase();
    if lower.is_empty() {
        return None;
    }
    if !label_continuation_prefix(&lower)
        && let Some(candidate) = normalize_label_indication_name(segment)
    {
        return Some(candidate);
    }
    let patient_slice = label_patient_phrase_start(segment, &lower)?;
    normalize_label_indication_name(patient_slice)
}

fn label_drug_markers(label_response: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for key in ["brand_name", "generic_name"] {
        for value in extract_openfda_values(label_response, key) {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                continue;
            }
            let dedupe_key = trimmed.to_ascii_lowercase();
            if seen.insert(dedupe_key) {
                out.push(trimmed.to_string());
            }
        }
    }
    out
}

fn extract_label_numbered_subsection_name(
    section: &str,
    drug_markers: &[String],
) -> Option<String> {
    let section = label_numbered_subsection_prefix_regex()
        .replace(section, "")
        .into_owned();
    let section = section.trim();
    if section.is_empty() {
        return None;
    }

    let end = drug_markers
        .iter()
        .filter_map(|marker| find_ascii_case_insensitive(section, marker))
        .filter(|idx| *idx > 0)
        .min();
    let candidate = end.map(|idx| &section[..idx]).unwrap_or(section);
    let candidate = candidate
        .trim()
        .trim_matches(|c: char| c.is_whitespace() || matches!(c, ':' | ';' | '.' | '-' | '•'))
        .trim();
    if candidate.is_empty() {
        return None;
    }
    Some(candidate.to_string())
}

fn push_label_indication_summary_row(
    out: &mut Vec<DrugLabelIndication>,
    seen: &mut HashSet<String>,
    name: String,
    max_rows: usize,
) -> bool {
    let dedupe_key = name.to_ascii_lowercase();
    if !seen.insert(dedupe_key) {
        return false;
    }

    out.push(DrugLabelIndication {
        name,
        approval_date: None,
        pivotal_trial: None,
    });
    out.len() >= max_rows
}

fn extract_label_indication_summary(
    label_response: &serde_json::Value,
) -> Vec<DrugLabelIndication> {
    const MAX_SUMMARY_ROWS: usize = 20;

    let Some(indications_text) = label_response
        .get("results")
        .and_then(|v| v.as_array())
        .and_then(|v| v.first())
        .and_then(|top| label_text(top.get("indications_and_usage")))
    else {
        return Vec::new();
    };

    let normalized = normalize_label_whitespace(&indications_text);
    let stripped = label_heading_regex().replace(&normalized, "").into_owned();

    let mut out: Vec<DrugLabelIndication> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let subsection_starts = label_numbered_subsection_heading_regex()
        .find_iter(&stripped)
        .map(|m| m.start())
        .collect::<Vec<_>>();
    if !subsection_starts.is_empty() {
        let drug_markers = label_drug_markers(label_response);
        for (idx, start) in subsection_starts.iter().enumerate() {
            let end = subsection_starts
                .get(idx + 1)
                .copied()
                .unwrap_or(stripped.len());
            let section = stripped[*start..end].trim();
            let Some(name) = extract_label_numbered_subsection_name(section, &drug_markers)
                .or_else(|| extract_label_indication_name(section))
            else {
                continue;
            };
            if push_label_indication_summary_row(&mut out, &mut seen, name, MAX_SUMMARY_ROWS) {
                return out;
            }
        }
        if !out.is_empty() {
            return out;
        }
    }

    for segment in label_subsection_boundary_regex().split(&stripped) {
        let Some(name) = extract_label_indication_name(segment) else {
            continue;
        };
        if push_label_indication_summary_row(&mut out, &mut seen, name, MAX_SUMMARY_ROWS) {
            break;
        }
    }
    out
}

fn extract_inline_label(label_response: &serde_json::Value, raw_mode: bool) -> Option<DrugLabel> {
    const LABEL_MAX_CHARS: usize = 2000;

    let top = label_response
        .get("results")
        .and_then(|v| v.as_array())
        .and_then(|v| v.first())?;

    let indication_summary = extract_label_indication_summary(label_response);
    let raw_indications = label_text(top.get("indications_and_usage"))
        .map(|v| truncate_with_note(&normalize_label_whitespace(&v), LABEL_MAX_CHARS));
    let raw_warnings = label_text(top.get("warnings_and_cautions"))
        .map(|v| truncate_with_note(&normalize_label_whitespace(&v), LABEL_MAX_CHARS));
    let raw_dosage = label_text(top.get("dosage_and_administration"))
        .map(|v| truncate_with_note(&normalize_label_whitespace(&v), LABEL_MAX_CHARS));

    let indications = if raw_mode || indication_summary.is_empty() {
        raw_indications
    } else {
        None
    };
    let warnings = if raw_mode { raw_warnings } else { None };
    let dosage = if raw_mode { raw_dosage } else { None };

    if indication_summary.is_empty()
        && indications.is_none()
        && warnings.is_none()
        && dosage.is_none()
    {
        return None;
    }

    Some(DrugLabel {
        indication_summary,
        indications,
        warnings,
        dosage,
    })
}

fn extract_label_warnings_text(label_response: &serde_json::Value) -> Option<String> {
    label_response
        .get("results")
        .and_then(|v| v.as_array())
        .and_then(|v| v.first())
        .and_then(|top| label_text(top.get("warnings_and_cautions")))
}

fn extract_label_set_id(label_response: &serde_json::Value) -> Option<String> {
    let top = label_response
        .get("results")
        .and_then(|v| v.as_array())
        .and_then(|v| v.first())?;

    top.get("set_id")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            top.get("openfda")
                .and_then(|v| v.get("spl_set_id"))
                .and_then(|v| match v {
                    serde_json::Value::String(s) => Some(s.as_str()),
                    serde_json::Value::Array(items) => items.iter().find_map(|item| item.as_str()),
                    _ => None,
                })
        })
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

fn extract_interaction_text_from_label(label_response: &serde_json::Value) -> Option<String> {
    const LABEL_MAX_CHARS: usize = 2000;

    let top = label_response
        .get("results")
        .and_then(|v| v.as_array())
        .and_then(|v| v.first())?;

    label_text(top.get("drug_interactions")).map(|v| truncate_with_note(&v, LABEL_MAX_CHARS))
}

fn extract_openfda_values_from_result(result: &serde_json::Value, key: &str) -> Vec<String> {
    let Some(top) = result.get("openfda").and_then(|v| v.get(key)) else {
        return Vec::new();
    };

    match top {
        serde_json::Value::String(s) => {
            let s = s.trim();
            if s.is_empty() {
                Vec::new()
            } else {
                vec![s.to_string()]
            }
        }
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|v| v.as_str())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn extract_openfda_values(label_response: &serde_json::Value, key: &str) -> Vec<String> {
    let Some(results) = label_response.get("results").and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    let mut out: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for result in results {
        let values = extract_openfda_values_from_result(result, key);
        for value in values {
            let key = value.to_ascii_lowercase();
            if !seen.insert(key) {
                continue;
            }
            out.push(value);
        }
    }
    out
}

fn search_results_from_openfda_label_response(
    label_response: &serde_json::Value,
    query: &str,
    max_results: usize,
) -> Vec<DrugSearchResult> {
    let query = query.trim();
    if query.is_empty() || max_results == 0 {
        return Vec::new();
    }

    let Some(results) = label_response.get("results").and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    let mut exact_matches: Vec<DrugSearchResult> = Vec::new();
    let mut others: Vec<DrugSearchResult> = Vec::new();
    for result in results {
        let brand_names = extract_openfda_values_from_result(result, "brand_name");
        let generic_names = extract_openfda_values_from_result(result, "generic_name");
        let Some(name) = generic_names
            .first()
            .cloned()
            .or_else(|| brand_names.first().cloned())
        else {
            continue;
        };
        let name = name.trim().to_ascii_lowercase();
        if name.is_empty() {
            continue;
        }

        let row = DrugSearchResult {
            name,
            drugbank_id: None,
            drug_type: None,
            mechanism: None,
            target: None,
        };
        let is_exact_brand_match = brand_names
            .iter()
            .map(|value| value.trim())
            .any(|value| value.eq_ignore_ascii_case(query));
        if is_exact_brand_match {
            exact_matches.push(row);
        } else {
            others.push(row);
        }
    }

    let mut out: Vec<DrugSearchResult> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for row in exact_matches.into_iter().chain(others) {
        if !seen.insert(row.name.clone()) {
            continue;
        }
        out.push(row);
        if out.len() >= max_results {
            break;
        }
    }
    out
}

fn normalize_route(route: &str) -> String {
    let route = route.trim().to_ascii_lowercase();
    if route.is_empty() {
        return String::new();
    }
    if matches!(
        route.as_str(),
        "iv" | "intravenous" | "intravenous injection" | "intravenous infusion"
    ) {
        return "IV".to_string();
    }
    if matches!(route.as_str(), "subcutaneous" | "sub-cutaneous") {
        return "subcutaneous".to_string();
    }
    route
}

fn maybe_brand_alias(name: &str) -> Option<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() || !trimmed.contains(' ') {
        return None;
    }
    let first = trimmed.split_whitespace().next()?;
    if first.len() < 4 {
        return None;
    }
    if first
        .chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-')
    {
        return Some(first.to_string());
    }
    None
}

fn route_rank(route: &str) -> usize {
    if route == "IV" {
        0
    } else if route == "subcutaneous" {
        1
    } else if route == "oral" {
        2
    } else {
        3
    }
}

fn apply_openfda_metadata(drug: &mut Drug, label_response: &serde_json::Value) {
    let mut brand_names: Vec<String> = extract_openfda_values(label_response, "brand_name");
    brand_names.extend(
        brand_names
            .iter()
            .filter_map(|name| maybe_brand_alias(name))
            .collect::<Vec<_>>(),
    );
    brand_names.extend(drug.brand_names.clone());
    let mut seen: HashSet<String> = HashSet::new();
    let mut merged: Vec<String> = Vec::new();
    for name in brand_names {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            continue;
        }
        let key = trimmed.to_ascii_lowercase();
        if !seen.insert(key) {
            continue;
        }
        merged.push(trimmed.to_string());
        if merged.len() >= 5 {
            break;
        }
    }
    if !merged.is_empty() {
        drug.brand_names = merged;
    }

    let mut routes = extract_openfda_values(label_response, "route")
        .into_iter()
        .map(|v| normalize_route(&v))
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>();
    if let Some(existing) = drug.route.as_deref() {
        let normalized = normalize_route(existing);
        if !normalized.is_empty() {
            routes.push(normalized);
        }
    }
    routes.sort_by(|a, b| route_rank(a).cmp(&route_rank(b)).then_with(|| a.cmp(b)));
    routes.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    if !routes.is_empty() {
        drug.route = Some(routes.join(", "));
    }
}

fn trim_nonempty(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn dedupe_trimmed_casefold(values: impl IntoIterator<Item = String>, max: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for value in values {
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        let key = value.to_ascii_lowercase();
        if !seen.insert(key) {
            continue;
        }
        out.push(value.to_string());
        if out.len() >= max {
            break;
        }
    }
    out
}

fn map_drugsfda_approvals(resp: OpenFdaResponse<DrugsFdaResult>) -> Vec<DrugApproval> {
    let mut out: Vec<DrugApproval> = Vec::new();
    let mut seen_apps: HashSet<String> = HashSet::new();

    for row in resp.results {
        let Some(application_number) = row
            .application_number
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
        else {
            continue;
        };
        if !seen_apps.insert(application_number.to_ascii_lowercase()) {
            continue;
        }

        let sponsor_name = row
            .sponsor_name
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string);

        let (openfda_brand_names, openfda_generic_names) = row
            .openfda
            .map(|meta| {
                (
                    dedupe_trimmed_casefold(meta.brand_name, 10),
                    dedupe_trimmed_casefold(meta.generic_name, 10),
                )
            })
            .unwrap_or_default();

        let mut products: Vec<DrugApprovalProduct> = row
            .products
            .into_iter()
            .filter_map(|product| {
                let brand_name = trim_nonempty(product.brand_name);
                let dosage_form = trim_nonempty(product.dosage_form);
                let route = trim_nonempty(product.route).map(|v| normalize_route(&v));
                let marketing_status = trim_nonempty(product.marketing_status);
                let active_ingredients = dedupe_trimmed_casefold(
                    product.active_ingredients.into_iter().filter_map(|ai| {
                        let name = ai.name.as_deref().map(str::trim).filter(|v| !v.is_empty());
                        let strength = ai
                            .strength
                            .as_deref()
                            .map(str::trim)
                            .filter(|v| !v.is_empty());
                        match (name, strength) {
                            (Some(name), Some(strength)) => Some(format!("{name} ({strength})")),
                            (Some(name), None) => Some(name.to_string()),
                            _ => None,
                        }
                    }),
                    6,
                );

                if brand_name.is_none()
                    && dosage_form.is_none()
                    && route.is_none()
                    && marketing_status.is_none()
                    && active_ingredients.is_empty()
                {
                    return None;
                }

                Some(DrugApprovalProduct {
                    brand_name,
                    dosage_form,
                    route,
                    marketing_status,
                    active_ingredients,
                })
            })
            .collect();

        products.truncate(6);

        let mut submissions: Vec<DrugApprovalSubmission> = row
            .submissions
            .into_iter()
            .filter_map(|submission| {
                let submission_type = trim_nonempty(submission.submission_type);
                let submission_number = trim_nonempty(submission.submission_number);
                let status = trim_nonempty(submission.submission_status);
                let status_date =
                    normalize_date_yyyymmdd(submission.submission_status_date.as_deref());

                if submission_type.is_none()
                    && submission_number.is_none()
                    && status.is_none()
                    && status_date.is_none()
                {
                    return None;
                }

                Some(DrugApprovalSubmission {
                    submission_type,
                    submission_number,
                    status,
                    status_date,
                })
            })
            .collect();

        submissions.sort_by(|a, b| b.status_date.cmp(&a.status_date));
        submissions.truncate(8);

        out.push(DrugApproval {
            application_number,
            sponsor_name,
            openfda_brand_names,
            openfda_generic_names,
            products,
            submissions,
        });
        if out.len() >= 8 {
            break;
        }
    }

    out
}

async fn fetch_shortage_entries(drug_name: &str) -> Result<Vec<DrugShortageEntry>, BioMcpError> {
    let drug_name = drug_name.trim();
    if drug_name.is_empty() {
        return Ok(Vec::new());
    }

    let escaped = OpenFdaClient::escape_query_value(drug_name);
    let q = if drug_name.chars().any(|c| c.is_whitespace()) {
        format!(
            "generic_name:\"{escaped}\" OR openfda.generic_name:\"{escaped}\" OR openfda.brand_name:\"{escaped}\""
        )
    } else {
        format!(
            "generic_name:*{escaped}* OR openfda.generic_name:*{escaped}* OR openfda.brand_name:*{escaped}*"
        )
    };

    let client = OpenFdaClient::new()?;
    let resp = client.shortage_search(&q, 5, 0).await?;
    let Some(resp) = resp else {
        return Ok(Vec::new());
    };

    let out = resp
        .results
        .into_iter()
        .map(|r| DrugShortageEntry {
            status: r
                .status
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty()),
            availability: r
                .availability
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty()),
            company_name: r
                .company_name
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty()),
            generic_name: r
                .generic_name
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty()),
            related_info: r
                .related_info
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty()),
            update_date: normalize_date_yyyymmdd(r.update_date.as_deref()),
            initial_posting_date: normalize_date_yyyymmdd(r.initial_posting_date.as_deref()),
        })
        .collect::<Vec<_>>();

    Ok(out)
}

fn extract_top_adverse_events(resp: &crate::sources::openfda::OpenFdaCountResponse) -> Vec<String> {
    let mut ranked: Vec<(String, usize)> = resp
        .results
        .iter()
        .filter_map(|bucket| {
            let term = bucket.term.trim();
            if term.is_empty() {
                return None;
            }
            Some((term.to_string(), bucket.count))
        })
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked.truncate(3);
    ranked.into_iter().map(|(label, _)| label).collect()
}

fn faers_adverse_event_query(drug_name: &str) -> Option<String> {
    let drug_name = drug_name.trim();
    if drug_name.is_empty() {
        return None;
    }

    let escaped = OpenFdaClient::escape_query_value(drug_name);
    Some(format!(
        "(patient.drug.openfda.generic_name:\"{escaped}\" OR patient.drug.openfda.brand_name:\"{escaped}\" OR patient.drug.medicinalproduct:\"{escaped}\") AND patient.drug.drugcharacterization:1"
    ))
}

async fn fetch_top_adverse_events(
    drug_name: &str,
) -> Result<(Vec<String>, Option<String>), BioMcpError> {
    let Some(q) = faers_adverse_event_query(drug_name) else {
        return Ok((Vec::new(), None));
    };

    let client = OpenFdaClient::new()?;
    let resp = client
        .faers_count(&q, "patient.reaction.reactionmeddrapt.exact", 50)
        .await?;
    let Some(resp) = resp else {
        return Ok((Vec::new(), Some(q)));
    };
    Ok((extract_top_adverse_events(&resp), Some(q)))
}

fn merge_unique_casefold(dst: &mut Vec<String>, values: impl IntoIterator<Item = String>) {
    let mut seen: HashSet<String> = dst.iter().map(|v| v.to_ascii_lowercase()).collect();
    for value in values {
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        let key = value.to_ascii_lowercase();
        if !seen.insert(key) {
            continue;
        }
        dst.push(value.to_string());
    }
}

async fn enrich_targets(drug: &mut Drug, civic_context: Option<&CivicContext>) {
    let mut chembl_rows = Vec::new();
    let mut opentargets_targets = Vec::new();
    if let Some(chembl_id) = drug.chembl_id.as_deref() {
        match ChemblClient::new() {
            Ok(client) => match client.drug_targets(chembl_id, 15).await {
                Ok(rows) => {
                    let targets = rows
                        .iter()
                        .filter(|row| !row.target.eq_ignore_ascii_case("Unknown target"))
                        .map(|row| row.target.clone())
                        .collect::<Vec<_>>();
                    merge_unique_casefold(&mut drug.targets, targets);

                    let mechanisms = rows
                        .iter()
                        .filter(|row| !row.target.eq_ignore_ascii_case("Unknown target"))
                        .map(|row| {
                            row.mechanism
                                .clone()
                                .unwrap_or_else(|| format!("{} of {}", row.action, row.target))
                        })
                        .collect::<Vec<_>>();
                    merge_unique_casefold(&mut drug.mechanisms, mechanisms);
                    chembl_rows = rows;
                }
                Err(err) => warn!("ChEMBL unavailable for drug targets section: {err}"),
            },
            Err(err) => warn!("ChEMBL client init failed: {err}"),
        }

        match OpenTargetsClient::new() {
            Ok(client) => match client.drug_sections(chembl_id, 15).await {
                Ok(sections) => {
                    let targets = sections
                        .targets
                        .iter()
                        .map(|t| t.approved_symbol.clone())
                        .collect::<Vec<_>>();
                    merge_unique_casefold(&mut drug.targets, targets);
                    opentargets_targets = sections.targets;
                }
                Err(err) => warn!("OpenTargets unavailable for drug targets section: {err}"),
            },
            Err(err) => warn!("OpenTargets client init failed: {err}"),
        }
    }

    drug.targets.truncate(12);
    drug.variant_targets = civic_context
        .map(|context| extract_variant_targets_from_civic(context, &drug.targets))
        .unwrap_or_default();
    drug.variant_targets.truncate(12);
    drug.target_family = None;
    drug.target_family_name = None;
    let inferred_target_family = strict_target_family_label(&drug.targets);
    let inferred_target_family_name = inferred_target_family
        .as_ref()
        .and_then(|_| derive_target_family_name(&drug.targets, &opentargets_targets));

    if drug.targets.len() >= 2
        && let Some(target_chembl_id) = family_target_chembl_id(&chembl_rows, &drug.targets)
    {
        match ChemblClient::new() {
            Ok(client) => match client.target_summary(&target_chembl_id).await {
                Ok(summary) if summary.target_type.eq_ignore_ascii_case("PROTEIN FAMILY") => {
                    let _family_pref_name = summary.pref_name.trim();
                    drug.target_family = inferred_target_family.clone();
                    drug.target_family_name = inferred_target_family_name.clone();
                }
                Ok(_) => {}
                Err(err) => {
                    warn!("ChEMBL unavailable for drug target family summary: {err}");
                    drug.target_family = inferred_target_family.clone();
                    drug.target_family_name = inferred_target_family_name.clone();
                }
            },
            Err(err) => {
                warn!("ChEMBL client init failed: {err}");
                drug.target_family = inferred_target_family.clone();
                drug.target_family_name = inferred_target_family_name.clone();
            }
        }
    }

    if !drug.mechanisms.is_empty() {
        drug.mechanism = drug.mechanisms.first().cloned();
    }
    drug.mechanisms.truncate(6);
}

fn normalize_variant_target_label(profile_name: &str, gene_symbol: &str) -> Option<String> {
    let profile_name = profile_name.trim();
    let gene_symbol = gene_symbol.trim();
    if profile_name.is_empty() || gene_symbol.is_empty() {
        return None;
    }
    let profile_lower = profile_name.to_ascii_lowercase();
    let gene_lower = gene_symbol.to_ascii_lowercase();
    if profile_lower == gene_lower || !profile_lower.starts_with(&gene_lower) {
        return None;
    }
    let remainder = profile_name.get(gene_symbol.len()..)?.trim();
    if remainder.is_empty() {
        return None;
    }
    let remainder_flat = remainder.replace(' ', "");
    if gene_symbol.eq_ignore_ascii_case("EGFR") && remainder_flat.eq_ignore_ascii_case("VIII") {
        return Some("EGFRvIII".to_string());
    }
    Some(profile_name.to_string())
}

fn extract_variant_targets_from_civic(
    civic: &CivicContext,
    generic_targets: &[String],
) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for profile in civic
        .evidence_items
        .iter()
        .map(|row| row.molecular_profile.as_str())
        .chain(
            civic
                .assertions
                .iter()
                .map(|row| row.molecular_profile.as_str()),
        )
    {
        for target in generic_targets {
            let Some(normalized) = normalize_variant_target_label(profile, target) else {
                continue;
            };
            let key = normalized.to_ascii_lowercase();
            if seen.insert(key) {
                out.push(normalized);
            }
            break;
        }
    }
    out
}

fn family_target_chembl_id(
    chembl_rows: &[crate::sources::chembl::ChemblTarget],
    displayed_targets: &[String],
) -> Option<String> {
    let displayed = displayed_targets
        .iter()
        .map(|target| target.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    let target_ids = chembl_rows
        .iter()
        .filter(|row| {
            displayed.contains(&row.target.to_ascii_lowercase())
                || row
                    .mechanism
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|value| !value.is_empty())
        })
        .filter_map(|row| row.target_chembl_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<HashSet<_>>();
    if target_ids.len() == 1 {
        target_ids.into_iter().next()
    } else {
        None
    }
}

fn strict_target_family_label(targets: &[String]) -> Option<String> {
    let distinct = targets
        .iter()
        .map(|target| target.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    if distinct.len() < 2 {
        return None;
    }

    let prefix_len = common_prefix_len_casefold(targets)?;
    if prefix_len < 2 {
        return None;
    }

    let prefix = &targets[0][..prefix_len];
    let all_numeric_suffixes = targets.iter().all(|target| {
        let Some(suffix) = target.get(prefix_len..) else {
            return false;
        };
        !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit())
    });
    if all_numeric_suffixes {
        Some(prefix.to_string())
    } else {
        None
    }
}

fn derive_target_family_name(
    displayed_targets: &[String],
    opentargets_targets: &[OpenTargetsTarget],
) -> Option<String> {
    let names_by_symbol = opentargets_targets
        .iter()
        .filter_map(|target| {
            let approved_name = target.approved_name.as_deref()?.trim();
            if approved_name.is_empty() {
                return None;
            }
            Some((
                target.approved_symbol.to_ascii_lowercase(),
                approved_name.to_string(),
            ))
        })
        .collect::<HashMap<_, _>>();
    let names = displayed_targets
        .iter()
        .map(|target| names_by_symbol.get(&target.to_ascii_lowercase()).cloned())
        .collect::<Option<Vec<_>>>()?;
    common_name_stem(&names)
}

fn common_name_stem(names: &[String]) -> Option<String> {
    if names.len() < 2 {
        return None;
    }

    let prefix_len = common_prefix_len_casefold(names)?;
    if prefix_len < 6 {
        return None;
    }

    let prefix = &names[0][..prefix_len];
    let boundary_aligned = prefix
        .chars()
        .last()
        .is_some_and(|ch| !ch.is_alphanumeric());
    let mut candidate = prefix.trim_end();
    if !boundary_aligned {
        candidate = trim_to_word_boundary(candidate);
    }
    candidate = candidate.trim_end_matches(|ch: char| ch.is_whitespace() || ",;:-/".contains(ch));
    if candidate.len() < 6 || !candidate.chars().any(|ch| ch.is_alphabetic()) {
        None
    } else {
        Some(candidate.to_string())
    }
}

fn trim_to_word_boundary(value: &str) -> &str {
    let mut end = value.len();
    while end > 0 {
        let Some(ch) = value[..end].chars().last() else {
            break;
        };
        if !ch.is_alphanumeric() && ch != ')' && ch != ']' {
            break;
        }
        end -= ch.len_utf8();
    }
    value[..end].trim_end()
}

fn common_prefix_len_casefold(values: &[String]) -> Option<usize> {
    let first = values.first()?;
    let mut prefix_len = first.len();
    for value in &values[1..] {
        let common_len = first
            .char_indices()
            .zip(value.chars())
            .take_while(|((_, left), right)| left.eq_ignore_ascii_case(right))
            .map(|((idx, left), _)| idx + left.len_utf8())
            .last()
            .unwrap_or(0);
        prefix_len = prefix_len.min(common_len);
    }
    Some(prefix_len)
}

async fn enrich_indications(drug: &mut Drug) {
    let Some(chembl_id) = drug.chembl_id.as_deref() else {
        return;
    };

    match OpenTargetsClient::new() {
        Ok(client) => match client.drug_sections(chembl_id, 15).await {
            Ok(sections) => {
                let indications = sections
                    .indications
                    .into_iter()
                    .map(|i| {
                        match i
                            .max_clinical_stage
                            .as_deref()
                            .and_then(format_opentargets_clinical_stage)
                        {
                            Some(stage) => format!("{} ({stage})", i.disease_name),
                            None => i.disease_name,
                        }
                    })
                    .collect::<Vec<_>>();
                merge_unique_casefold(&mut drug.indications, indications);
            }
            Err(err) => warn!("OpenTargets unavailable for drug indications section: {err}"),
        },
        Err(err) => warn!("OpenTargets client init failed: {err}"),
    }

    drug.indications.truncate(12);
}

fn format_opentargets_clinical_stage(stage: &str) -> Option<String> {
    let normalized = stage.trim();
    if normalized.is_empty() {
        return None;
    }

    let normalized = normalized.to_ascii_uppercase();
    let label = match normalized.as_str() {
        "UNKNOWN" => return None,
        "APPROVAL" => "Approved".to_string(),
        "EARLY_PHASE_1" => "Early Phase 1".to_string(),
        "PHASE_1" => "Phase 1".to_string(),
        "PHASE_2" => "Phase 2".to_string(),
        "PHASE_3" => "Phase 3".to_string(),
        "PHASE_4" => "Phase 4".to_string(),
        "PHASE_1_2" => "Phase 1/2".to_string(),
        "PHASE_2_3" => "Phase 2/3".to_string(),
        other => other
            .replace('_', " ")
            .split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                let Some(first) = chars.next() else {
                    return String::new();
                };
                let mut out = String::new();
                out.extend(first.to_uppercase());
                out.push_str(&chars.as_str().to_ascii_lowercase());
                out
            })
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
    };

    (!label.is_empty()).then_some(label)
}

async fn fetch_civic_therapy_context(name: &str) -> Option<CivicContext> {
    let name = name.trim();
    if name.is_empty() {
        return Some(CivicContext::default());
    }

    let civic_fut = async {
        let client = CivicClient::new()?;
        client.by_therapy(name, 10).await
    };

    match tokio::time::timeout(OPTIONAL_SAFETY_TIMEOUT, civic_fut).await {
        Ok(Ok(context)) => Some(context),
        Ok(Err(err)) => {
            warn!(drug = %name, "CIViC unavailable for drug section: {err}");
            None
        }
        Err(_) => {
            warn!(
                drug = %name,
                timeout_secs = OPTIONAL_SAFETY_TIMEOUT.as_secs(),
                "CIViC drug section timed out"
            );
            None
        }
    }
}

async fn add_approvals_section(drug: &mut Drug) {
    let name = drug.name.trim();
    if name.is_empty() {
        drug.approvals = Some(Vec::new());
        return;
    }

    let escaped = OpenFdaClient::escape_query_value(name);
    let query = if name.chars().any(|c| c.is_whitespace()) {
        format!(
            "openfda.generic_name:\"{escaped}\" OR openfda.brand_name:\"{escaped}\" OR products.brand_name:\"{escaped}\""
        )
    } else {
        format!(
            "openfda.generic_name:*{escaped}* OR openfda.brand_name:*{escaped}* OR products.brand_name:*{escaped}*"
        )
    };

    let approvals_fut = async {
        let client = OpenFdaClient::new()?;
        client.drugsfda_search(&query, 8, 0).await
    };

    match tokio::time::timeout(OPTIONAL_SAFETY_TIMEOUT, approvals_fut).await {
        Ok(Ok(resp)) => {
            let approvals = resp.map(map_drugsfda_approvals).unwrap_or_default();
            drug.approvals = Some(approvals);
        }
        Ok(Err(err)) => {
            warn!(drug = %drug.name, "OpenFDA Drugs@FDA unavailable: {err}");
            drug.approvals = Some(Vec::new());
        }
        Err(_) => {
            warn!(
                drug = %drug.name,
                timeout_secs = OPTIONAL_SAFETY_TIMEOUT.as_secs(),
                "OpenFDA Drugs@FDA section timed out"
            );
            drug.approvals = Some(Vec::new());
        }
    }
}

struct ResolvedDrugBase {
    drug: Drug,
    label_response: Option<serde_json::Value>,
}

async fn resolve_drug_base(
    name: &str,
    fetch_label_response: bool,
    label_required: bool,
) -> Result<ResolvedDrugBase, BioMcpError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(BioMcpError::InvalidArgument(
            "Drug name is required. Example: biomcp get drug pembrolizumab".into(),
        ));
    }
    if name.len() > 256 {
        return Err(BioMcpError::InvalidArgument(
            "Drug name is too long.".into(),
        ));
    }

    let original_not_found = || BioMcpError::NotFound {
        entity: "drug".into(),
        id: name.to_string(),
        suggestion: format!("Try searching: biomcp search drug -q \"{name}\""),
    };

    let mut lookup_name = name.to_string();
    let mut resp = direct_drug_lookup(name).await?;

    if resp.hits.is_empty() {
        let fallback_filters = DrugSearchFilters {
            query: Some(name.to_string()),
            ..Default::default()
        };
        let fallback_name = search_page(&fallback_filters, 2, 0)
            .await
            .ok()
            .and_then(|page| {
                if page.results.len() != 1 {
                    return None;
                }
                let candidate = page.results[0].name.trim();
                if candidate.is_empty() || candidate.eq_ignore_ascii_case(name) {
                    None
                } else {
                    Some(candidate.to_string())
                }
            });

        if let Some(candidate) = fallback_name {
            if let Ok(fallback_resp) = direct_drug_lookup(&candidate).await
                && !fallback_resp.hits.is_empty()
            {
                lookup_name = candidate;
                resp = fallback_resp;
            } else {
                return Err(original_not_found());
            }
        } else {
            return Err(original_not_found());
        }
    }

    let selected = transform::drug::select_hits_for_name(&resp.hits, &lookup_name);
    let mut drug = transform::drug::merge_mychem_hits(&selected, &lookup_name);

    let mut label_response_opt: Option<serde_json::Value> = None;
    if fetch_label_response {
        match OpenFdaClient::new() {
            Ok(client) => match client.label_search(&drug.name).await {
                Ok(v) => label_response_opt = v,
                Err(err) => {
                    if label_required {
                        return Err(err);
                    }
                }
            },
            Err(err) => {
                if label_required {
                    return Err(err);
                }
            }
        }
    }

    if let Some(label_response) = label_response_opt.as_ref() {
        apply_openfda_metadata(&mut drug, label_response);
        drug.label_set_id = extract_label_set_id(label_response);
    }

    Ok(ResolvedDrugBase {
        drug,
        label_response: label_response_opt,
    })
}

async fn direct_drug_lookup(query: &str) -> Result<MyChemQueryResponse, BioMcpError> {
    MyChemClient::new()?
        .query_with_fields(query, 25, 0, MYCHEM_FIELDS_GET)
        .await
}

async fn try_resolve_drug_identity(name: &str) -> Option<Drug> {
    match resolve_drug_base(name, false, false).await {
        Ok(resolved) => Some(resolved.drug),
        Err(err) => {
            warn!(query = %name, "Drug identity resolution unavailable for EMA alias expansion: {err}");
            None
        }
    }
}

async fn populate_common_sections(
    drug: &mut Drug,
    label_response: Option<&serde_json::Value>,
    section_flags: &DrugSections,
    raw_label: bool,
) {
    let civic_context = if section_flags.include_targets || section_flags.include_civic {
        fetch_civic_therapy_context(&drug.name).await
    } else {
        None
    };

    drug.label = if section_flags.include_label {
        label_response.and_then(|response| extract_inline_label(response, raw_label))
    } else {
        None
    };

    if section_flags.include_interactions {
        drug.interaction_text = label_response.and_then(extract_interaction_text_from_label);
    } else {
        drug.interactions.clear();
        drug.interaction_text = None;
    }

    if section_flags.include_targets {
        enrich_targets(drug, civic_context.as_ref()).await;
    } else {
        drug.variant_targets.clear();
    }

    if section_flags.include_indications {
        enrich_indications(drug).await;
    }

    if section_flags.include_civic {
        drug.civic = Some(civic_context.unwrap_or_default());
    } else {
        drug.civic = None;
    }
}

async fn populate_top_adverse_event_preview(drug: &mut Drug) {
    match tokio::time::timeout(
        OPTIONAL_SAFETY_TIMEOUT,
        fetch_top_adverse_events(&drug.name),
    )
    .await
    {
        Ok(Ok((events, faers_query))) => {
            drug.top_adverse_events = events;
            drug.faers_query = faers_query;
        }
        Ok(Err(err)) => {
            warn!(
                drug = %drug.name,
                "OpenFDA adverse-event preview unavailable: {err}"
            );
        }
        Err(_) => {
            warn!(
                drug = %drug.name,
                timeout_secs = OPTIONAL_SAFETY_TIMEOUT.as_secs(),
                "OpenFDA adverse-event preview timed out"
            );
        }
    }
}

async fn populate_us_regional_sections(
    drug: &mut Drug,
    label_response: Option<&serde_json::Value>,
    section_flags: &DrugSections,
) -> Result<(), BioMcpError> {
    if section_flags.include_shortage {
        drug.shortage = Some(fetch_shortage_entries(&drug.name).await?);
    } else {
        drug.shortage = None;
    }

    if section_flags.include_regulatory || section_flags.include_approvals {
        add_approvals_section(drug).await;
    } else {
        drug.approvals = None;
    }

    drug.us_safety_warnings = if section_flags.include_safety {
        label_response.and_then(extract_label_warnings_text)
    } else {
        None
    };

    Ok(())
}

fn build_ema_identity(requested_name: &str, drug: &Drug) -> EmaDrugIdentity {
    EmaDrugIdentity::with_aliases(requested_name, Some(&drug.name), &drug.brand_names)
}

fn build_who_identity(requested_name: &str, drug: &Drug) -> WhoPqIdentity {
    WhoPqIdentity::with_aliases(requested_name, Some(&drug.name), &drug.brand_names)
}

async fn populate_ema_sections(
    drug: &mut Drug,
    requested_name: &str,
    section_flags: &DrugSections,
) -> Result<(), BioMcpError> {
    if !section_flags.include_regulatory
        && !section_flags.include_safety
        && !section_flags.include_shortage
    {
        drug.ema_regulatory = None;
        drug.ema_safety = None;
        drug.ema_shortage = None;
        return Ok(());
    }

    let client = EmaClient::ready(EmaSyncMode::Auto).await?;
    let identity = build_ema_identity(requested_name, drug);
    let anchor = client.resolve_anchor(&identity)?;

    drug.ema_regulatory = if section_flags.include_regulatory {
        Some(client.regulatory(&anchor)?)
    } else {
        None
    };
    drug.ema_safety = if section_flags.include_safety {
        Some(client.safety(&anchor)?)
    } else {
        None
    };
    drug.ema_shortage = if section_flags.include_shortage {
        Some(client.shortages(&anchor)?)
    } else {
        None
    };

    Ok(())
}

async fn populate_who_sections(
    drug: &mut Drug,
    requested_name: &str,
    section_flags: &DrugSections,
) -> Result<(), BioMcpError> {
    if !section_flags.include_regulatory {
        drug.who_prequalification = None;
        return Ok(());
    }

    let client = WhoPqClient::ready(WhoPqSyncMode::Auto).await?;
    let identity = build_who_identity(requested_name, drug);
    drug.who_prequalification = Some(client.regulatory(&identity)?);
    Ok(())
}

fn validate_region_usage(
    section_flags: &DrugSections,
    region: DrugRegion,
    region_explicit: bool,
) -> Result<(), BioMcpError> {
    if !region_explicit {
        return Ok(());
    }

    if section_flags.include_approvals {
        return Err(BioMcpError::InvalidArgument(
            "--region is not supported with approvals. Use regulatory for the regional regulatory view.".into(),
        ));
    }

    if !(section_flags.include_regulatory
        || section_flags.include_safety
        || section_flags.include_shortage)
    {
        return Err(BioMcpError::InvalidArgument(
            "--region can only be used with regulatory, safety, shortage, or all.".into(),
        ));
    }

    if matches!(region, DrugRegion::Who)
        && (section_flags.requested_safety || section_flags.requested_shortage)
        && !section_flags.requested_all
    {
        return Err(BioMcpError::InvalidArgument(
            "WHO regional data currently supports regulatory only; use --region us|eu for safety or shortage, or use --region who with regulatory/all.".into(),
        ));
    }

    Ok(())
}

fn validate_raw_usage(section_flags: &DrugSections, raw_label: bool) -> Result<(), BioMcpError> {
    if raw_label && !section_flags.include_label {
        return Err(BioMcpError::InvalidArgument(
            "--raw can only be used with label or all.".into(),
        ));
    }
    Ok(())
}

pub async fn get_with_region(
    name: &str,
    sections: &[String],
    region: DrugRegion,
    region_explicit: bool,
    raw_label: bool,
) -> Result<Drug, BioMcpError> {
    let section_flags = parse_sections(sections)?;
    validate_region_usage(&section_flags, region, region_explicit)?;
    validate_raw_usage(&section_flags, raw_label)?;

    let section_only = is_section_only_requested(sections);
    let fetch_label_response = !section_only
        || section_flags.include_label
        || section_flags.include_interactions
        || (region.includes_us() && section_flags.include_safety);

    let mut resolved =
        resolve_drug_base(name, fetch_label_response, section_flags.include_label).await?;
    populate_common_sections(
        &mut resolved.drug,
        resolved.label_response.as_ref(),
        &section_flags,
        raw_label,
    )
    .await;

    if region.includes_us() && (!section_only || section_flags.include_safety) {
        populate_top_adverse_event_preview(&mut resolved.drug).await;
    } else {
        resolved.drug.top_adverse_events.clear();
        resolved.drug.faers_query = None;
    }

    if region.includes_us() {
        populate_us_regional_sections(
            &mut resolved.drug,
            resolved.label_response.as_ref(),
            &section_flags,
        )
        .await?;
    } else {
        resolved.drug.shortage = None;
        resolved.drug.approvals = None;
        resolved.drug.us_safety_warnings = None;
    }

    if region.includes_eu() {
        populate_ema_sections(&mut resolved.drug, name, &section_flags).await?;
    } else {
        resolved.drug.ema_regulatory = None;
        resolved.drug.ema_safety = None;
        resolved.drug.ema_shortage = None;
    }

    if region.includes_who() {
        populate_who_sections(&mut resolved.drug, name, &section_flags).await?;
    } else {
        resolved.drug.who_prequalification = None;
    }

    Ok(resolved.drug)
}

pub async fn search_name_query_with_region(
    query: &str,
    limit: usize,
    offset: usize,
    region: DrugRegion,
) -> Result<DrugSearchPageWithRegion, BioMcpError> {
    let query = query.trim();
    if query.is_empty() {
        return Err(BioMcpError::InvalidArgument(
            "At least one filter is required. Example: biomcp search drug -q pembrolizumab".into(),
        ));
    }

    const MAX_SEARCH_LIMIT: usize = 50;
    if limit == 0 || limit > MAX_SEARCH_LIMIT {
        return Err(BioMcpError::InvalidArgument(format!(
            "--limit must be between 1 and {MAX_SEARCH_LIMIT}"
        )));
    }

    let filters = DrugSearchFilters {
        query: Some(query.to_string()),
        ..Default::default()
    };

    let resolved_identity = try_resolve_drug_identity(query).await;
    let eu_identity = match resolved_identity.as_ref() {
        Some(drug) => build_ema_identity(query, drug),
        None => EmaDrugIdentity::new(query),
    };
    let who_identity = match resolved_identity.as_ref() {
        Some(drug) => build_who_identity(query, drug),
        None => WhoPqIdentity::new(query),
    };

    let eu_client = if region.includes_eu() {
        Some(EmaClient::ready(EmaSyncMode::Auto).await?)
    } else {
        None
    };
    let who_client = if region.includes_who() {
        Some(WhoPqClient::ready(WhoPqSyncMode::Auto).await?)
    } else {
        None
    };

    match region {
        DrugRegion::Us => Ok(DrugSearchPageWithRegion::Us(
            search_page(&filters, limit, offset).await?,
        )),
        DrugRegion::Eu => Ok(DrugSearchPageWithRegion::Eu(
            eu_client
                .as_ref()
                .expect("EU client should exist for EU region")
                .search_medicines(&eu_identity, limit, offset)?,
        )),
        DrugRegion::Who => Ok(DrugSearchPageWithRegion::Who(
            who_client
                .as_ref()
                .expect("WHO client should exist for WHO region")
                .search(&who_identity, limit, offset)?,
        )),
        DrugRegion::All => Ok(DrugSearchPageWithRegion::All {
            us: search_page(&filters, limit, offset).await?,
            eu: eu_client
                .as_ref()
                .expect("EU client should exist for all region")
                .search_medicines(&eu_identity, limit, offset)?,
            who: who_client
                .as_ref()
                .expect("WHO client should exist for all region")
                .search(&who_identity, limit, offset)?,
        }),
    }
}

async fn search_structured_who_page(
    filters: &DrugSearchFilters,
    limit: usize,
    offset: usize,
) -> Result<SearchPage<WhoPrequalificationSearchResult>, BioMcpError> {
    let who_rows = WhoPqClient::ready(WhoPqSyncMode::Auto).await?.read_rows()?;
    search_structured_who_page_with(
        filters,
        limit,
        offset,
        |filters, page_limit, page_offset| {
            let filters = filters.clone();
            async move { search_page(&filters, page_limit, page_offset).await }
        },
        |name| crate::sources::who_pq::filter_regulatory_rows(&who_rows, &WhoPqIdentity::new(name)),
    )
    .await
}

async fn search_structured_who_page_with<F, Fut, M>(
    filters: &DrugSearchFilters,
    limit: usize,
    offset: usize,
    mut fetch_page: F,
    mut regulatory_rows: M,
) -> Result<SearchPage<WhoPrequalificationSearchResult>, BioMcpError>
where
    F: FnMut(&DrugSearchFilters, usize, usize) -> Fut,
    Fut: Future<Output = Result<SearchPage<DrugSearchResult>, BioMcpError>>,
    M: FnMut(&str) -> Vec<WhoPrequalificationEntry>,
{
    const MAX_SEARCH_LIMIT: usize = 50;
    if limit == 0 || limit > MAX_SEARCH_LIMIT {
        return Err(BioMcpError::InvalidArgument(format!(
            "--limit must be between 1 and {MAX_SEARCH_LIMIT}"
        )));
    }

    let mut expanded = Vec::new();
    let mut seen_refs = HashSet::new();
    let mut mychem_offset = 0usize;
    let page_size = MAX_SEARCH_LIMIT;

    loop {
        let page = fetch_page(filters, page_size, mychem_offset).await?;
        for candidate in &page.results {
            for row in regulatory_rows(&candidate.name) {
                if seen_refs.insert(row.who_reference_number.clone()) {
                    expanded.push(WhoPrequalificationSearchResult {
                        inn: row.inn,
                        therapeutic_area: row.therapeutic_area,
                        dosage_form: row.dosage_form,
                        applicant: row.applicant,
                        who_reference_number: row.who_reference_number,
                        listing_basis: row.listing_basis,
                        prequalification_date: row.prequalification_date,
                    });
                }
            }
        }

        let exhausted = page.results.is_empty()
            || page
                .total
                .is_some_and(|total| mychem_offset + page_size >= total);
        if exhausted {
            let total = expanded.len();
            let results = expanded.into_iter().skip(offset).take(limit).collect();
            return Ok(SearchPage::offset(results, Some(total)));
        }

        if expanded.len() > offset + limit {
            let results = expanded.into_iter().skip(offset).take(limit).collect();
            return Ok(SearchPage::offset(results, None));
        }

        mychem_offset += page_size;
    }
}

pub async fn search_page_with_region(
    filters: &DrugSearchFilters,
    limit: usize,
    offset: usize,
    region: DrugRegion,
) -> Result<DrugSearchPageWithRegion, BioMcpError> {
    if filters.has_structured_filters() {
        return match region {
            DrugRegion::Us => Ok(DrugSearchPageWithRegion::Us(
                search_page(filters, limit, offset).await?,
            )),
            DrugRegion::Who => Ok(DrugSearchPageWithRegion::Who(
                search_structured_who_page(filters, limit, offset).await?,
            )),
            DrugRegion::Eu | DrugRegion::All => Err(BioMcpError::InvalidArgument(
                "EMA and all-region search currently support name/alias lookups only; use --region us for structured MyChem filters or --region who to filter structured U.S. hits through WHO prequalification.".into(),
            )),
        };
    }

    search_name_query_with_region(
        filters.query.as_deref().unwrap_or_default(),
        limit,
        offset,
        region,
    )
    .await
}

pub async fn get(name: &str, sections: &[String]) -> Result<Drug, BioMcpError> {
    get_with_region(name, sections, DrugRegion::Us, false, false).await
}

pub fn search_query_summary(filters: &DrugSearchFilters) -> String {
    normalize_query_summary(filters)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_mychem_query_requires_at_least_one_filter() {
        let filters = DrugSearchFilters::default();
        let err = build_mychem_query(&filters).unwrap_err();
        assert!(matches!(err, BioMcpError::InvalidArgument(_)));
    }

    #[test]
    fn build_mychem_query_includes_target_and_mechanism_filters() {
        let filters = DrugSearchFilters {
            query: Some("pembrolizumab".into()),
            target: Some("BRAF".into()),
            indication: None,
            mechanism: Some("inhibitor".into()),
            drug_type: Some("small molecule".into()),
            atc: None,
            pharm_class: None,
            interactions: None,
        };
        let q = build_mychem_query(&filters).unwrap();
        assert!(q.contains("pembrolizumab"));
        assert!(q.contains("gtopdb.interaction_targets.symbol:BRAF"));
        assert!(q.contains("chembl.drug_mechanisms.action_type:*inhibitor*"));
        assert!(q.contains("ndc.pharm_classes"));
        assert!(q.contains("chembl.molecule_type:\"Small molecule\""));
    }

    #[test]
    fn build_mychem_query_includes_mechanism_of_action_field() {
        let filters = DrugSearchFilters {
            mechanism: Some("adenosine deaminase inhibitor".into()),
            ..Default::default()
        };

        let q = build_mychem_query(&filters).unwrap();
        assert!(q.contains("chembl.drug_mechanisms.mechanism_of_action"));
        assert!(
            q.contains(
                "chembl.drug_mechanisms.mechanism_of_action:*adenosine* AND chembl.drug_mechanisms.mechanism_of_action:*deaminase*"
            )
        );
    }

    #[test]
    fn build_mychem_query_expands_purine_to_atc_codes() {
        let filters = DrugSearchFilters {
            mechanism: Some("purine analog".into()),
            ..Default::default()
        };

        let q = build_mychem_query(&filters).unwrap();
        assert!(q.contains("chembl.atc_classifications:L01BB*"));
        assert!(q.contains("chembl.atc_classifications:L01XX08"));
    }

    #[test]
    fn build_mychem_query_keeps_atc_filter_exact() {
        let filters = DrugSearchFilters {
            atc: Some("L01BB".into()),
            ..Default::default()
        };

        let q = build_mychem_query(&filters).unwrap();
        assert!(q.contains("chembl.atc_classifications:L01BB"));
        assert!(!q.contains("chembl.atc_classifications:L01BB*"));
    }

    #[test]
    fn build_mychem_query_escapes_free_text_query() {
        let filters = DrugSearchFilters {
            query: Some("EGFR:inhibitor (3rd-gen)".into()),
            target: None,
            indication: None,
            mechanism: None,
            drug_type: None,
            atc: None,
            pharm_class: None,
            interactions: None,
        };

        let q = build_mychem_query(&filters).unwrap();
        assert!(q.contains(r"EGFR\:inhibitor"));
        assert!(q.contains(r"\(3rd\-gen\)"));
    }

    #[test]
    fn drug_search_filters_detect_structured_filters() {
        let plain_name = DrugSearchFilters {
            query: Some("Keytruda".into()),
            ..Default::default()
        };
        assert!(!plain_name.has_structured_filters());

        let structured = DrugSearchFilters {
            target: Some("EGFR".into()),
            ..Default::default()
        };
        assert!(structured.has_structured_filters());
    }

    #[test]
    fn search_results_from_openfda_label_response_prefers_exact_brand_match() {
        let response = serde_json::json!({
            "results": [
                {
                    "openfda": {
                        "brand_name": ["KEYTRUDA QLEX"],
                        "generic_name": ["Pembrolizumab and berahyaluronidase alfa-pmph"]
                    }
                },
                {
                    "openfda": {
                        "brand_name": ["Keytruda"],
                        "generic_name": ["Pembrolizumab"]
                    }
                }
            ]
        });

        let rows = search_results_from_openfda_label_response(&response, " Keytruda ", 5);
        let names = rows.into_iter().map(|row| row.name).collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "pembrolizumab".to_string(),
                "pembrolizumab and berahyaluronidase alfa-pmph".to_string()
            ]
        );
    }

    #[test]
    fn search_results_from_openfda_label_response_returns_remaining_unique_generics() {
        let response = serde_json::json!({
            "results": [
                {
                    "openfda": {
                        "brand_name": ["Keytruda"],
                        "generic_name": ["Pembrolizumab"]
                    }
                },
                {
                    "openfda": {
                        "brand_name": ["KEYTRUDA QLEX"],
                        "generic_name": ["Pembrolizumab and berahyaluronidase alfa-pmph"]
                    }
                },
                {
                    "openfda": {
                        "brand_name": ["Keytruda refill"],
                        "generic_name": ["Pembrolizumab"]
                    }
                }
            ]
        });

        let rows = search_results_from_openfda_label_response(&response, "Keytruda", 5);
        let names = rows.into_iter().map(|row| row.name).collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "pembrolizumab".to_string(),
                "pembrolizumab and berahyaluronidase alfa-pmph".to_string()
            ]
        );
    }

    #[test]
    fn search_results_from_openfda_label_response_respects_limit() {
        let response = serde_json::json!({
            "results": [
                {
                    "openfda": {
                        "brand_name": ["Keytruda"],
                        "generic_name": ["Pembrolizumab"]
                    }
                },
                {
                    "openfda": {
                        "brand_name": ["KEYTRUDA QLEX"],
                        "generic_name": ["Pembrolizumab and berahyaluronidase alfa-pmph"]
                    }
                }
            ]
        });

        let rows = search_results_from_openfda_label_response(&response, "Keytruda", 1);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "pembrolizumab");
    }

    #[test]
    fn extract_interaction_text_from_label_uses_openfda_drug_interactions() {
        let response = serde_json::json!({
            "results": [{
                "drug_interactions": [
                    "DRUG INTERACTIONS",
                    "Warfarin has documented interactions with aspirin."
                ]
            }]
        });

        let text = extract_interaction_text_from_label(&response).expect("interaction text");
        assert!(text.contains("DRUG INTERACTIONS"));
        assert!(text.contains("Warfarin has documented interactions with aspirin."));
    }

    #[test]
    fn extract_interaction_text_from_label_returns_none_when_missing() {
        let response = serde_json::json!({
            "results": [{
                "warnings_and_cautions": ["No interaction section present"]
            }]
        });

        assert_eq!(extract_interaction_text_from_label(&response), None);
    }

    #[test]
    fn extract_inline_label_summary_mode_preserves_subtype_wording() {
        let response = serde_json::json!({
            "results": [{
                "indications_and_usage": [
                    "1 INDICATIONS AND USAGE",
                    "(1.1) KEYTRUDA, in combination with chemotherapy, is indicated for the treatment of patients with high-risk early-stage triple-negative breast cancer.",
                    "(1.2) KEYTRUDA, as a single agent, is indicated for the treatment of adult patients with renal cell carcinoma."
                ],
                "warnings_and_cautions": ["Warnings"],
                "dosage_and_administration": ["Dosage"]
            }]
        });

        let label = extract_inline_label(&response, false).expect("summary label");
        assert_eq!(
            label
                .indication_summary
                .iter()
                .map(|row| row.name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "high-risk early-stage triple-negative breast cancer",
                "renal cell carcinoma"
            ]
        );
        assert!(label.warnings.is_none());
        assert!(label.dosage.is_none());
        assert!(label.indications.is_none());
    }

    #[test]
    fn extract_inline_label_summary_mode_trims_patient_eligibility_qualifiers() {
        let response = serde_json::json!({
            "results": [{
                "indications_and_usage": [
                    "1 INDICATIONS AND USAGE",
                    "(1.1) KEYTRUDA is indicated for the treatment of patients with locally advanced or metastatic urothelial carcinoma who are not eligible for cisplatin-containing chemotherapy.",
                    "(1.2) KEYTRUDA is indicated for the treatment of adults with locally advanced unresectable or metastatic HER2-negative gastric or gastroesophageal junction (GEJ) adenocarcinoma whose tumors express PD-L1 (CPS ≥1) as determined by an FDA-authorized test."
                ]
            }]
        });

        let label = extract_inline_label(&response, false).expect("summary label");
        assert_eq!(
            label
                .indication_summary
                .iter()
                .map(|row| row.name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "locally advanced or metastatic urothelial carcinoma",
                "locally advanced unresectable or metastatic HER2-negative gastric or gastroesophageal junction (GEJ) adenocarcinoma"
            ]
        );
    }

    #[test]
    fn extract_inline_label_summary_mode_uses_numbered_subsection_titles() {
        let response = serde_json::json!({
            "results": [{
                "indications_and_usage": [
                    "1 INDICATIONS AND USAGE • THALOMID in combination with dexamethasone is indicated for the treatment of patients with newly diagnosed multiple myeloma (MM). ( 1.1 ) • THALOMID is indicated for the acute treatment of the cutaneous manifestations of moderate to severe erythema nodosum leprosum (ENL). THALOMID is not indicated as monotherapy for such ENL treatment in the presence of moderate to severe neuritis. THALOMID is also indicated as maintenance therapy for prevention and suppression of the cutaneous manifestations of ENL recurrence. ( 1.2 ) 1.1 Multiple Myeloma THALOMID in combination with dexamethasone is indicated for the treatment of patients with newly diagnosed multiple myeloma (MM) [see Clinical Studies (14.1) ] . 1.2 Erythema Nodosum Leprosum THALOMID is indicated for the acute treatment of the cutaneous manifestations of moderate to severe erythema nodosum leprosum (ENL). THALOMID is not indicated as monotherapy for such ENL treatment in the presence of moderate to severe neuritis. THALOMID is also indicated as maintenance therapy for prevention and suppression of the cutaneous manifestations of ENL recurrence [see Clinical Studies (14.2) ]."
                ],
                "openfda": {
                    "brand_name": ["THALOMID"],
                    "generic_name": ["thalidomide"]
                }
            }]
        });

        let label = extract_inline_label(&response, false).expect("summary label");
        assert_eq!(
            label
                .indication_summary
                .iter()
                .map(|row| row.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Multiple Myeloma", "Erythema Nodosum Leprosum"]
        );
    }

    #[test]
    fn extract_inline_label_summary_mode_falls_back_to_raw_indications_when_no_rows() {
        let response = serde_json::json!({
            "results": [{
                "indications_and_usage": [
                    "1 INDICATIONS AND USAGE",
                    "Use with diet and exercise to improve glycemic control."
                ],
                "warnings_and_cautions": ["Warnings"],
                "dosage_and_administration": ["Dosage"]
            }]
        });

        let label = extract_inline_label(&response, false).expect("fallback label");
        assert!(label.indication_summary.is_empty());
        assert!(label.indications.as_deref().is_some());
        assert!(label.warnings.is_none());
        assert!(label.dosage.is_none());
    }

    #[test]
    fn extract_inline_label_raw_mode_preserves_truncated_raw_subsections() {
        let response = serde_json::json!({
            "results": [{
                "indications_and_usage": [
                    "1 INDICATIONS AND USAGE",
                    "(1.1) KEYTRUDA, in combination with chemotherapy, is indicated for the treatment of patients with high-risk early-stage triple-negative breast cancer."
                ],
                "warnings_and_cautions": ["Warnings"],
                "dosage_and_administration": ["Dosage"]
            }]
        });

        let label = extract_inline_label(&response, true).expect("raw label");
        assert!(!label.indication_summary.is_empty());
        assert!(label.indications.as_deref().is_some());
        assert!(label.warnings.as_deref().is_some());
        assert!(label.dosage.as_deref().is_some());
    }

    #[test]
    fn build_mychem_query_rejects_public_interaction_filter() {
        let filters = DrugSearchFilters {
            query: None,
            target: None,
            indication: None,
            mechanism: None,
            drug_type: None,
            atc: None,
            pharm_class: None,
            interactions: Some("warfarin".into()),
        };

        let err = build_mychem_query(&filters).unwrap_err();
        assert!(matches!(err, BioMcpError::InvalidArgument(_)));
        assert!(err.to_string().contains(
            "Interaction-partner drug search is unavailable from the public data sources"
        ));
    }

    #[test]
    fn mechanism_match_uses_mechanism_fields_not_drug_name() {
        let hit: MyChemHit = serde_json::from_value(serde_json::json!({
            "_id": "x",
            "_score": 1.0,
            "drugbank": {"name": "alpha.1-proteinase inhibitor human"},
            "chembl": {
                "drug_mechanisms": [{"action_type": "protease inhibitor", "target_name": "ELANE"}]
            }
        }))
        .expect("valid hit");

        assert!(!hit_mentions_mechanism(&hit, "kinase inhibitor"));
        assert!(hit_mentions_mechanism(&hit, "protease inhibitor"));
    }

    #[test]
    fn hit_mentions_mechanism_matches_atc_purine_hits() {
        let hit: MyChemHit = serde_json::from_value(serde_json::json!({
            "_id": "x",
            "_score": 1.0,
            "chembl": {
                "atc_classifications": ["L01BB07"],
                "drug_mechanisms": []
            }
        }))
        .expect("valid hit");

        assert!(hit_mentions_mechanism(&hit, "purine"));
        assert!(hit_mentions_mechanism(&hit, "purine analog"));
    }

    #[test]
    fn hit_mentions_mechanism_matches_mechanism_of_action_text() {
        let hit: MyChemHit = serde_json::from_value(serde_json::json!({
            "_id": "x",
            "_score": 1.0,
            "chembl": {
                "drug_mechanisms": [{
                    "mechanism_of_action": "Adenosine deaminase inhibitor"
                }]
            }
        }))
        .expect("valid hit");

        assert!(hit_mentions_mechanism(
            &hit,
            "adenosine deaminase inhibitor"
        ));
        assert!(hit_mentions_mechanism(&hit, "deaminase inhibitor"));
    }

    #[test]
    fn mechanism_atc_expansions_returns_purine_mapping() {
        assert_eq!(
            mechanism_atc_expansions("purine analog"),
            vec![
                AtcExpansion::Prefix("L01BB"),
                AtcExpansion::Exact("L01XX08")
            ]
        );
        assert!(mechanism_atc_expansions("kinase inhibitor").is_empty());
    }

    #[test]
    fn parse_sections_supports_all_and_rejects_unknown() {
        let flags = parse_sections(&["all".to_string()]).unwrap();
        assert!(flags.include_label);
        assert!(flags.include_regulatory);
        assert!(flags.include_safety);
        assert!(flags.include_shortage);
        assert!(flags.include_targets);
        assert!(flags.include_indications);
        assert!(flags.include_interactions);
        assert!(flags.include_civic);
        assert!(!flags.include_approvals);

        let err = parse_sections(&["bad".to_string()]).unwrap_err();
        assert!(matches!(err, BioMcpError::InvalidArgument(_)));
    }

    #[test]
    fn parse_sections_all_with_explicit_label_keeps_label() {
        let flags = parse_sections(&["all".to_string(), "label".to_string()]).unwrap();
        assert!(flags.include_label);
    }

    #[test]
    fn parse_sections_default_card_includes_targets_enrichment() {
        let flags = parse_sections(&[]).unwrap();
        assert!(flags.include_targets);
    }

    #[test]
    fn validate_region_usage_rejects_approvals_with_explicit_region() {
        let flags = parse_sections(&["approvals".to_string()]).unwrap();
        let err = validate_region_usage(&flags, DrugRegion::Us, true).unwrap_err();
        assert!(matches!(err, BioMcpError::InvalidArgument(_)));
        assert!(err.to_string().contains("approvals"));
    }

    #[test]
    fn validate_region_usage_rejects_explicit_region_without_regional_sections() {
        let flags = parse_sections(&["targets".to_string()]).unwrap();
        let err = validate_region_usage(&flags, DrugRegion::Us, true).unwrap_err();
        assert!(matches!(err, BioMcpError::InvalidArgument(_)));
        assert!(err.to_string().contains("--region can only be used"));
    }

    #[test]
    fn validate_region_usage_rejects_who_safety_only_requests() {
        let flags = parse_sections(&["safety".to_string()]).unwrap();
        let err = validate_region_usage(&flags, DrugRegion::Who, true).unwrap_err();
        assert!(matches!(err, BioMcpError::InvalidArgument(_)));
        assert!(
            err.to_string()
                .contains("WHO regional data currently supports regulatory only")
        );
    }

    #[test]
    fn validate_region_usage_rejects_who_shortage_only_requests() {
        let flags = parse_sections(&["shortage".to_string()]).unwrap();
        let err = validate_region_usage(&flags, DrugRegion::Who, true).unwrap_err();
        assert!(matches!(err, BioMcpError::InvalidArgument(_)));
        assert!(
            err.to_string()
                .contains("WHO regional data currently supports regulatory only")
        );
    }

    #[test]
    fn validate_region_usage_allows_who_all_requests() {
        let flags = parse_sections(&["all".to_string()]).unwrap();
        validate_region_usage(&flags, DrugRegion::Who, true).expect("who all should be valid");
    }

    fn mychem_row(name: &str) -> DrugSearchResult {
        DrugSearchResult {
            name: name.to_string(),
            drugbank_id: None,
            drug_type: None,
            mechanism: None,
            target: None,
        }
    }

    fn who_row(reference: &str, inn: &str) -> WhoPrequalificationEntry {
        WhoPrequalificationEntry {
            who_reference_number: reference.to_string(),
            inn: inn.to_string(),
            presentation: format!("{inn} Tablet 100mg"),
            dosage_form: "Tablet".to_string(),
            product_type: "Finished Pharmaceutical Product".to_string(),
            therapeutic_area: "Malaria".to_string(),
            applicant: "Example Applicant".to_string(),
            listing_basis: "Prequalification - Abridged".to_string(),
            alternative_listing_basis: None,
            prequalification_date: Some("2024-01-01".to_string()),
        }
    }

    #[tokio::test]
    async fn structured_who_search_stops_after_one_extra_match_and_reports_unknown_total() {
        let filters = DrugSearchFilters {
            indication: Some("malaria".into()),
            ..Default::default()
        };
        let fetch_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let fetch_count_for_closure = fetch_count.clone();

        let page = search_structured_who_page_with(
            &filters,
            2,
            0,
            move |_, _, page_offset| {
                let fetch_count = fetch_count_for_closure.clone();
                async move {
                    fetch_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    match page_offset {
                        0 => Ok(SearchPage::offset(
                            vec![mychem_row("candidate-a"), mychem_row("candidate-b")],
                            Some(100),
                        )),
                        _ => Ok(SearchPage::offset(Vec::new(), Some(100))),
                    }
                }
            },
            |name| match name {
                "candidate-a" => vec![who_row("W1", "Artemether"), who_row("W2", "Lumefantrine")],
                "candidate-b" => vec![who_row("W3", "Artesunate")],
                _ => Vec::new(),
            },
        )
        .await
        .expect("structured WHO search");

        assert_eq!(fetch_count.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(page.total, None);
        assert_eq!(page.results.len(), 2);
        assert_eq!(page.results[0].who_reference_number, "W1");
        assert_eq!(page.results[1].who_reference_number, "W2");
    }

    #[tokio::test]
    async fn structured_who_search_reports_exact_total_when_mychem_is_exhausted() {
        let filters = DrugSearchFilters {
            indication: Some("malaria".into()),
            ..Default::default()
        };

        let page = search_structured_who_page_with(
            &filters,
            5,
            0,
            |_, _, page_offset| async move {
                match page_offset {
                    0 => Ok(SearchPage::offset(vec![mychem_row("candidate-a")], Some(1))),
                    _ => Ok(SearchPage::offset(Vec::new(), Some(1))),
                }
            },
            |name| match name {
                "candidate-a" => vec![who_row("W1", "Artemether/Lumefantrine")],
                _ => Vec::new(),
            },
        )
        .await
        .expect("structured WHO search");

        assert_eq!(page.total, Some(1));
        assert_eq!(page.results.len(), 1);
        assert_eq!(page.results[0].who_reference_number, "W1");
    }

    #[test]
    fn format_opentargets_clinical_stage_maps_known_stages() {
        assert_eq!(
            format_opentargets_clinical_stage("APPROVAL").as_deref(),
            Some("Approved")
        );
        assert_eq!(
            format_opentargets_clinical_stage("PHASE_3").as_deref(),
            Some("Phase 3")
        );
        assert_eq!(
            format_opentargets_clinical_stage("PHASE_1_2").as_deref(),
            Some("Phase 1/2")
        );
        assert_eq!(
            format_opentargets_clinical_stage("PHASE_2_3").as_deref(),
            Some("Phase 2/3")
        );
        assert_eq!(
            format_opentargets_clinical_stage("EARLY_PHASE_1").as_deref(),
            Some("Early Phase 1")
        );
    }

    #[test]
    fn format_opentargets_clinical_stage_suppresses_unknown_and_blank() {
        assert_eq!(format_opentargets_clinical_stage("UNKNOWN"), None);
        assert_eq!(format_opentargets_clinical_stage("   "), None);
    }

    #[test]
    fn format_opentargets_clinical_stage_falls_back_for_future_values() {
        assert_eq!(
            format_opentargets_clinical_stage("PRECLINICAL").as_deref(),
            Some("Preclinical")
        );
    }

    #[test]
    fn map_drugsfda_approvals_extracts_key_fields() {
        let resp: OpenFdaResponse<DrugsFdaResult> = serde_json::from_value(serde_json::json!({
            "meta": {"results": {"skip": 0, "limit": 1, "total": 1}},
            "results": [{
                "application_number": "NDA021304",
                "sponsor_name": "Example Pharma",
                "openfda": {
                    "brand_name": ["DrugX"],
                    "generic_name": ["drugx"]
                },
                "products": [{
                    "brand_name": "DrugX",
                    "dosage_form": "TABLET",
                    "route": "ORAL",
                    "marketing_status": "Prescription",
                    "active_ingredients": [{"name": "drugx", "strength": "10 mg"}]
                }],
                "submissions": [{
                    "submission_type": "ORIG",
                    "submission_number": "1",
                    "submission_status": "AP",
                    "submission_status_date": "20120101"
                }]
            }]
        }))
        .expect("response should parse");

        let rows = map_drugsfda_approvals(resp);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].application_number, "NDA021304");
        assert_eq!(rows[0].openfda_brand_names, vec!["DrugX"]);
        assert_eq!(
            rows[0].products[0].active_ingredients,
            vec!["drugx (10 mg)"]
        );
        assert_eq!(
            rows[0].submissions[0].status_date.as_deref(),
            Some("2012-01-01")
        );
    }

    #[test]
    fn extract_label_set_id_prefers_top_level_set_id() {
        let response = serde_json::json!({
            "results": [{
                "set_id": "abc-123",
                "openfda": {
                    "spl_set_id": ["fallback-456"]
                }
            }]
        });

        assert_eq!(extract_label_set_id(&response).as_deref(), Some("abc-123"));
    }

    #[test]
    fn extract_label_set_id_falls_back_to_spl_set_id() {
        let response = serde_json::json!({
            "results": [{
                "openfda": {
                    "spl_set_id": ["fallback-456"]
                }
            }]
        });

        assert_eq!(
            extract_label_set_id(&response).as_deref(),
            Some("fallback-456")
        );
    }

    #[test]
    fn extract_top_adverse_events_ranks_by_frequency() {
        let resp: crate::sources::openfda::OpenFdaCountResponse =
            serde_json::from_value(serde_json::json!({
                "meta": {},
                "results": [
                    {"term": "Rash", "count": 2},
                    {"term": "Nausea", "count": 1},
                    {"term": "Fatigue", "count": 2}
                ]
            }))
            .expect("valid openfda response");

        let out = extract_top_adverse_events(&resp);
        assert_eq!(out, vec!["Fatigue", "Rash", "Nausea"]);
    }

    #[test]
    fn openfda_label_fallback_is_first_page_only() {
        let name_filters = DrugSearchFilters {
            query: Some("Keytruda".into()),
            ..Default::default()
        };
        let structured_filters = DrugSearchFilters {
            target: Some("EGFR".into()),
            ..Default::default()
        };
        let dummy = DrugSearchResult {
            name: "pembrolizumab".into(),
            drugbank_id: None,
            drug_type: None,
            mechanism: None,
            target: None,
        };

        // Fallback fires only when MyChem returned nothing, on page 1, without structured filters.
        assert!(should_attempt_openfda_fallback(&[], 0, &name_filters));

        // Page 2+ must not trigger fallback even with an empty MyChem result set.
        assert!(!should_attempt_openfda_fallback(&[], 10, &name_filters));

        // Structured-filter searches must not fall back to OpenFDA label rescue.
        assert!(!should_attempt_openfda_fallback(
            &[],
            0,
            &structured_filters
        ));

        // When MyChem already returned rows, no fallback regardless of offset.
        assert!(!should_attempt_openfda_fallback(&[dummy], 0, &name_filters));
    }

    #[test]
    fn strict_target_family_label_accepts_numeric_suffix_family() {
        let targets = vec![
            "PARP1".to_string(),
            "PARP2".to_string(),
            "PARP3".to_string(),
        ];
        assert_eq!(
            strict_target_family_label(&targets).as_deref(),
            Some("PARP")
        );
    }

    #[test]
    fn strict_target_family_label_handles_embedded_digits() {
        let targets = vec!["CYP2C9".to_string(), "CYP2C19".to_string()];
        assert_eq!(
            strict_target_family_label(&targets).as_deref(),
            Some("CYP2C")
        );
    }

    #[test]
    fn strict_target_family_label_rejects_mixed_targets() {
        let targets = vec!["ABL1".to_string(), "KIT".to_string(), "PDGFRB".to_string()];
        assert!(strict_target_family_label(&targets).is_none());
    }

    #[test]
    fn strict_target_family_label_rejects_single_target() {
        let targets = vec!["PDCD1".to_string()];
        assert!(strict_target_family_label(&targets).is_none());
    }

    #[test]
    fn family_target_chembl_id_requires_single_matching_target_id() {
        let rows = vec![
            crate::sources::chembl::ChemblTarget {
                target: "PARP1".to_string(),
                action: "INHIBITOR".to_string(),
                mechanism: None,
                target_chembl_id: Some("CHEMBL3390820".to_string()),
            },
            crate::sources::chembl::ChemblTarget {
                target: "PARP2".to_string(),
                action: "INHIBITOR".to_string(),
                mechanism: None,
                target_chembl_id: Some("CHEMBL3390820".to_string()),
            },
        ];
        let displayed_targets = vec!["PARP1".to_string(), "PARP2".to_string()];
        assert_eq!(
            family_target_chembl_id(&rows, &displayed_targets).as_deref(),
            Some("CHEMBL3390820")
        );
    }

    #[test]
    fn family_target_chembl_id_rejects_multiple_matching_target_ids() {
        let rows = vec![
            crate::sources::chembl::ChemblTarget {
                target: "PARP1".to_string(),
                action: "INHIBITOR".to_string(),
                mechanism: None,
                target_chembl_id: Some("CHEMBL3390820".to_string()),
            },
            crate::sources::chembl::ChemblTarget {
                target: "PARP2".to_string(),
                action: "INHIBITOR".to_string(),
                mechanism: None,
                target_chembl_id: Some("CHEMBL1234".to_string()),
            },
        ];
        let displayed_targets = vec!["PARP1".to_string(), "PARP2".to_string()];
        assert!(family_target_chembl_id(&rows, &displayed_targets).is_none());
    }

    #[test]
    fn family_target_chembl_id_rejects_missing_matching_target_id() {
        let rows = vec![crate::sources::chembl::ChemblTarget {
            target: "PARP1".to_string(),
            action: "INHIBITOR".to_string(),
            mechanism: None,
            target_chembl_id: None,
        }];
        let displayed_targets = vec!["PARP1".to_string(), "PARP2".to_string()];
        assert!(family_target_chembl_id(&rows, &displayed_targets).is_none());
    }

    #[test]
    fn normalize_variant_target_label_rejects_exact_gene_match() {
        assert_eq!(normalize_variant_target_label("EGFR", "EGFR"), None);
    }

    #[test]
    fn normalize_variant_target_label_keeps_spaced_protein_change() {
        assert_eq!(
            normalize_variant_target_label("BRAF V600E", "BRAF").as_deref(),
            Some("BRAF V600E")
        );
    }

    #[test]
    fn normalize_variant_target_label_normalizes_egfr_roman_suffix() {
        assert_eq!(
            normalize_variant_target_label("EGFR VIII", "EGFR").as_deref(),
            Some("EGFRvIII")
        );
        assert_eq!(
            normalize_variant_target_label("EGFRVIII", "EGFR").as_deref(),
            Some("EGFRvIII")
        );
    }

    #[test]
    fn extract_variant_targets_from_civic_deduplicates_and_filters_by_generic_target() {
        let civic = CivicContext {
            evidence_total_count: 2,
            assertion_total_count: 2,
            evidence_items: vec![
                crate::sources::civic::CivicEvidenceItem {
                    id: 1,
                    name: "EID1".to_string(),
                    molecular_profile: "EGFR VIII".to_string(),
                    evidence_type: "PREDICTIVE".to_string(),
                    evidence_level: "A".to_string(),
                    significance: "SENSITIVITYRESPONSE".to_string(),
                    disease: None,
                    therapies: vec!["rindopepimut".to_string()],
                    status: "ACCEPTED".to_string(),
                    citation: None,
                    source_type: None,
                    publication_year: None,
                },
                crate::sources::civic::CivicEvidenceItem {
                    id: 2,
                    name: "EID2".to_string(),
                    molecular_profile: "PDGFRA D842V".to_string(),
                    evidence_type: "PREDICTIVE".to_string(),
                    evidence_level: "A".to_string(),
                    significance: "SENSITIVITYRESPONSE".to_string(),
                    disease: None,
                    therapies: vec!["rindopepimut".to_string()],
                    status: "ACCEPTED".to_string(),
                    citation: None,
                    source_type: None,
                    publication_year: None,
                },
            ],
            assertions: vec![
                crate::sources::civic::CivicAssertion {
                    id: 3,
                    name: "AID3".to_string(),
                    molecular_profile: "EGFRVIII".to_string(),
                    assertion_type: "PREDICTIVE".to_string(),
                    assertion_direction: "SUPPORTS".to_string(),
                    amp_level: None,
                    significance: "SENSITIVITYRESPONSE".to_string(),
                    disease: None,
                    therapies: vec!["rindopepimut".to_string()],
                    status: "ACCEPTED".to_string(),
                    summary: None,
                    approvals_count: 0,
                },
                crate::sources::civic::CivicAssertion {
                    id: 4,
                    name: "AID4".to_string(),
                    molecular_profile: "EGFR".to_string(),
                    assertion_type: "PREDICTIVE".to_string(),
                    assertion_direction: "SUPPORTS".to_string(),
                    amp_level: None,
                    significance: "SENSITIVITYRESPONSE".to_string(),
                    disease: None,
                    therapies: vec!["rindopepimut".to_string()],
                    status: "ACCEPTED".to_string(),
                    summary: None,
                    approvals_count: 0,
                },
            ],
        };

        assert_eq!(
            extract_variant_targets_from_civic(&civic, &["EGFR".to_string()]),
            vec!["EGFRvIII".to_string()]
        );
    }

    #[test]
    fn family_target_chembl_id_accepts_mechanism_only_family_row() {
        let rows = vec![crate::sources::chembl::ChemblTarget {
            target: "Unknown target".to_string(),
            action: "INHIBITOR".to_string(),
            mechanism: Some("PARP 1, 2 and 3 inhibitor".to_string()),
            target_chembl_id: Some("CHEMBL3390820".to_string()),
        }];
        let displayed_targets = vec![
            "PARP1".to_string(),
            "PARP2".to_string(),
            "PARP3".to_string(),
        ];
        assert_eq!(
            family_target_chembl_id(&rows, &displayed_targets).as_deref(),
            Some("CHEMBL3390820")
        );
    }

    #[test]
    fn derive_target_family_name_requires_complete_member_names() {
        let displayed_targets = vec!["PARP1".to_string(), "PARP2".to_string()];
        let opentargets_targets = vec![
            OpenTargetsTarget {
                approved_symbol: "PARP1".to_string(),
                approved_name: Some("poly(ADP-ribose) polymerase 1".to_string()),
            },
            OpenTargetsTarget {
                approved_symbol: "PARP2".to_string(),
                approved_name: None,
            },
        ];
        assert!(derive_target_family_name(&displayed_targets, &opentargets_targets).is_none());
    }

    #[test]
    fn derive_target_family_name_trims_numeric_member_suffix() {
        let displayed_targets = vec!["PARP1".to_string(), "PARP2".to_string()];
        let opentargets_targets = vec![
            OpenTargetsTarget {
                approved_symbol: "PARP1".to_string(),
                approved_name: Some("poly(ADP-ribose) polymerase 1".to_string()),
            },
            OpenTargetsTarget {
                approved_symbol: "PARP2".to_string(),
                approved_name: Some("poly(ADP-ribose) polymerase 2".to_string()),
            },
        ];
        assert_eq!(
            derive_target_family_name(&displayed_targets, &opentargets_targets).as_deref(),
            Some("poly(ADP-ribose) polymerase")
        );
    }

    #[test]
    fn derive_target_family_name_handles_non_ascii_without_panicking() {
        let displayed_targets = vec!["GENE1".to_string(), "GENE2".to_string()];
        let opentargets_targets = vec![
            OpenTargetsTarget {
                approved_symbol: "GENE1".to_string(),
                approved_name: Some("électron receptor 1".to_string()),
            },
            OpenTargetsTarget {
                approved_symbol: "GENE2".to_string(),
                approved_name: Some("èlectron receptor 2".to_string()),
            },
        ];
        assert!(derive_target_family_name(&displayed_targets, &opentargets_targets).is_none());
    }
}
