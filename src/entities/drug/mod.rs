//! Drug entity models and workflows exposed through the stable drug facade.

mod label;
mod metadata;
mod query;
mod targets;

pub use self::query::search_query_summary;

use std::collections::HashSet;
use std::future::Future;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::entities::SearchPage;
use crate::error::BioMcpError;
use crate::sources::civic::{CivicClient, CivicContext};
use crate::sources::ema::{EmaClient, EmaDrugIdentity, EmaSyncMode};
use crate::sources::mychem::{
    MYCHEM_FIELDS_GET, MYCHEM_FIELDS_SEARCH, MyChemClient, MyChemHit, MyChemNdcField,
    MyChemQueryResponse,
};
use crate::sources::openfda::OpenFdaClient;
use crate::sources::who_pq::{WhoPqClient, WhoPqIdentity, WhoPqSyncMode};
use crate::transform;

use self::label::{
    extract_inline_label, extract_interaction_text_from_label, extract_label_set_id,
    extract_label_warnings_text, extract_openfda_values_from_result,
};
use self::metadata::{
    apply_openfda_metadata, fetch_shortage_entries, fetch_top_adverse_events,
    map_drugsfda_approvals,
};
use self::query::{AtcExpansion, build_mychem_query, mechanism_atc_expansions};
use self::targets::{enrich_indications, enrich_targets};

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
