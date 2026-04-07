use futures::{StreamExt, stream};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tracing::warn;

use crate::entities::SearchPage;
use crate::entities::disease::resolve_disease_hit_by_name;
use crate::error::BioMcpError;
use crate::sources::clinicaltrials::{
    ClinicalTrialsClient, CtGovLocation, CtGovSearchParams, CtGovStudy,
};
use crate::sources::mydisease::MyDiseaseClient;
use crate::sources::nci_cts::{
    NciCtsClient, NciDiseaseFilter, NciGeoFilter, NciSearchParams, NciStatusFilter,
};
use crate::transform;
use crate::utils::date::validate_since;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trial {
    pub nct_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub title: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub study_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age_range: Option<String>,
    #[serde(default)]
    pub conditions: Vec<String>,
    #[serde(default)]
    pub interventions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sponsor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enrollment: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eligibility_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locations: Option<Vec<TrialLocation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcomes: Option<TrialOutcomes>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arms: Option<Vec<TrialArm>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub references: Option<Vec<TrialReference>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialLocation {
    pub facility: String,
    pub city: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    pub country: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_phone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialOutcomes {
    #[serde(default)]
    pub primary: Vec<TrialOutcome>,
    #[serde(default)]
    pub secondary: Vec<TrialOutcome>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialOutcome {
    pub measure: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_frame: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialArm {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arm_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub interventions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialReference {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pmid: Option<String>,
    pub citation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialSearchResult {
    pub nct_id: String,
    pub title: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    #[serde(default)]
    pub conditions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sponsor: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct TrialSearchFilters {
    pub condition: Option<String>,
    pub intervention: Option<String>,
    pub facility: Option<String>,
    pub status: Option<String>,
    pub phase: Option<String>,
    pub study_type: Option<String>,
    pub age: Option<f32>,
    pub sex: Option<String>,
    pub sponsor: Option<String>,
    pub sponsor_type: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub mutation: Option<String>,
    pub criteria: Option<String>,
    pub biomarker: Option<String>,
    pub prior_therapies: Option<String>,
    pub progression_on: Option<String>,
    pub line_of_therapy: Option<String>,
    pub results_available: bool,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub distance: Option<u32>,
    pub source: TrialSource,
}

#[derive(Debug, Clone, Default, Copy)]
pub enum TrialSource {
    #[default]
    ClinicalTrialsGov,
    NciCts,
}

impl TrialSource {
    pub fn from_flag(value: &str) -> Result<Self, BioMcpError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "ctgov" | "clinicaltrials" | "clinicaltrials.gov" => Ok(Self::ClinicalTrialsGov),
            "nci" | "nci_cts" | "cts" => Ok(Self::NciCts),
            other => Err(BioMcpError::InvalidArgument(format!(
                "Unknown --source '{other}'. Expected 'ctgov' or 'nci'."
            ))),
        }
    }
}

const TRIAL_SECTION_ELIGIBILITY: &str = "eligibility";
const TRIAL_SECTION_LOCATIONS: &str = "locations";
const TRIAL_SECTION_OUTCOMES: &str = "outcomes";
const TRIAL_SECTION_ARMS: &str = "arms";
const TRIAL_SECTION_REFERENCES: &str = "references";
const TRIAL_SECTION_ALL: &str = "all";

pub const TRIAL_SECTION_NAMES: &[&str] = &[
    TRIAL_SECTION_ELIGIBILITY,
    TRIAL_SECTION_LOCATIONS,
    TRIAL_SECTION_OUTCOMES,
    TRIAL_SECTION_ARMS,
    TRIAL_SECTION_REFERENCES,
    TRIAL_SECTION_ALL,
];

const ELIGIBILITY_MAX_CHARS: usize = 12_000;
const FACILITY_GEO_VERIFY_CONCURRENCY: usize = 8;
const ELIGIBILITY_VERIFY_CONCURRENCY: usize = 8;
const CTGOV_MAX_PAGE_FETCHES: usize = 20;
const CTGOV_COUNT_PAGE_SIZE: usize = 1000;
const COUNT_TRAVERSAL_PAGE_CAP: usize = 50;

/// Describes the precision of a trial `--count-only` result.
#[derive(Debug, PartialEq)]
pub enum TrialCount {
    /// Exact post-filtered count.
    Exact(usize),
    /// Upstream CTGov total before client-side age post-filtering.
    Approximate(usize),
    /// Traversal cap was hit, so the exact total is unknown.
    Unknown,
}

#[derive(Debug, Clone, Copy, Default)]
struct TrialSections {
    include_eligibility: bool,
    include_locations: bool,
    include_outcomes: bool,
    include_arms: bool,
    include_references: bool,
}

fn parse_sections(sections: &[String]) -> Result<TrialSections, BioMcpError> {
    let mut out = TrialSections::default();
    let mut include_all = false;

    for raw in sections {
        let section = raw.trim().to_ascii_lowercase();
        if section.is_empty() {
            continue;
        }
        if section == "--json" || section == "-j" {
            continue;
        }
        match section.as_str() {
            TRIAL_SECTION_ELIGIBILITY => out.include_eligibility = true,
            TRIAL_SECTION_LOCATIONS => out.include_locations = true,
            TRIAL_SECTION_OUTCOMES => out.include_outcomes = true,
            TRIAL_SECTION_ARMS => out.include_arms = true,
            TRIAL_SECTION_REFERENCES => out.include_references = true,
            TRIAL_SECTION_ALL => include_all = true,
            _ => {
                return Err(BioMcpError::InvalidArgument(format!(
                    "Unknown section \"{section}\" for trial. Available: {}",
                    TRIAL_SECTION_NAMES.join(", ")
                )));
            }
        }
    }

    if include_all {
        out.include_eligibility = true;
        out.include_locations = true;
        out.include_outcomes = true;
        out.include_arms = true;
        out.include_references = true;
    }

    Ok(out)
}

fn essie_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if matches!(
            ch,
            '\\' | '\"'
                | '+'
                | '-'
                | '!'
                | '('
                | ')'
                | '{'
                | '}'
                | '['
                | ']'
                | '^'
                | '~'
                | '*'
                | '?'
                | ':'
                | '/'
                | '|'
        ) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

fn quote_essie_literal(value: &str) -> String {
    format!("\"{}\"", essie_escape(value))
}

fn boolean_operator_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(OR|AND|NOT)\b").expect("valid boolean operator regex"))
}

fn split_boolean_expression(value: &str) -> Option<(Vec<String>, Vec<String>)> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut terms = Vec::new();
    let mut operators: Vec<String> = Vec::new();
    let mut last = 0;
    for caps in boolean_operator_regex().captures_iter(trimmed) {
        let Some(matched) = caps.get(0) else {
            continue;
        };
        let op = caps
            .get(1)
            .map(|m| m.as_str().to_ascii_uppercase())
            .unwrap_or_else(|| "OR".to_string());
        let term = trimmed[last..matched.start()].trim();
        if term.is_empty() {
            // Consecutive operators (e.g., "AND NOT") — merge into previous operator.
            if let Some(prev) = operators.last_mut() {
                prev.push(' ');
                prev.push_str(&op);
            } else {
                // Leading operator (e.g., "NOT dMMR") — treat as prefix.
                operators.push(op);
            }
        } else {
            terms.push(term.to_string());
            operators.push(op);
        }
        last = matched.end();
    }

    let tail = trimmed[last..].trim();
    if tail.is_empty() {
        return None;
    }
    terms.push(tail.to_string());
    Some((terms, operators))
}

fn has_boolean_operators(value: &str) -> bool {
    split_boolean_expression(value).is_some_and(|(_, operators)| !operators.is_empty())
}

fn essie_escape_boolean_expression(value: &str) -> String {
    let trimmed = value.trim();
    let Some((terms, operators)) = split_boolean_expression(trimmed) else {
        return quote_essie_literal(trimmed);
    };
    if operators.is_empty() {
        return quote_essie_literal(&terms[0]);
    }

    let mut rendered = String::new();
    let has_leading_unary_operator = operators.len() == terms.len();
    for (idx, term) in terms.iter().enumerate() {
        if idx == 0 && has_leading_unary_operator {
            rendered.push_str(&operators[0]);
            rendered.push(' ');
        } else if idx > 0 {
            rendered.push(' ');
            let operator_idx = if has_leading_unary_operator {
                idx
            } else {
                idx - 1
            };
            rendered.push_str(&operators[operator_idx]);
            rendered.push(' ');
        }
        rendered.push_str(&quote_essie_literal(term));
    }
    rendered
}

fn has_essie_filters(filters: &TrialSearchFilters) -> bool {
    filters
        .prior_therapies
        .as_deref()
        .map(str::trim)
        .is_some_and(|v| !v.is_empty())
        || filters
            .progression_on
            .as_deref()
            .map(str::trim)
            .is_some_and(|v| !v.is_empty())
        || filters
            .line_of_therapy
            .as_deref()
            .map(str::trim)
            .is_some_and(|v| !v.is_empty())
}

fn line_of_therapy_patterns(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_uppercase().as_str() {
        "1L" => Some(
            "\"first line\" OR \"first-line\" OR \"1st line\" OR \"frontline\" OR \"treatment naive\" OR \"previously untreated\"",
        ),
        "2L" => Some(
            "\"second line\" OR \"second-line\" OR \"2nd line\" OR \"one prior line\" OR \"1 prior line\"",
        ),
        "3L+" => Some(
            "\"third line\" OR \"third-line\" OR \"3rd line\" OR \"≥2 prior\" OR \"at least 2 prior\" OR \"heavily pretreated\"",
        ),
        _ => None,
    }
}

fn build_essie_fragments(filters: &TrialSearchFilters) -> Result<Vec<String>, BioMcpError> {
    let mut fragments = Vec::new();

    if let Some(therapy) = filters
        .prior_therapies
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let therapy = essie_escape(therapy);
        fragments.push(format!(
            "AREA[EligibilityCriteria](\"{therapy}\" AND (prior OR previous OR received))"
        ));
    }

    if let Some(drug) = filters
        .progression_on
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let drug = essie_escape(drug);
        fragments.push(format!(
            "AREA[EligibilityCriteria](\"{drug}\" AND (progression OR resistant OR refractory))"
        ));
    }

    if let Some(line) = filters
        .line_of_therapy
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let patterns = line_of_therapy_patterns(line).ok_or_else(|| {
            BioMcpError::InvalidArgument(
                "Invalid --line-of-therapy value. Expected one of: 1L, 2L, 3L+".into(),
            )
        })?;
        fragments.push(format!("AREA[EligibilityCriteria]({patterns})"));
    }

    Ok(fragments)
}

fn normalize_enum_key(value: &str) -> String {
    let mut out = String::new();
    let mut prev_sep = false;
    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_uppercase());
            prev_sep = false;
            continue;
        }
        if matches!(ch, ' ' | ',' | '-' | '_') && !prev_sep {
            out.push('_');
            prev_sep = true;
        }
    }
    out.trim_matches('_').to_string()
}

fn invalid_status_error(raw: &str) -> BioMcpError {
    BioMcpError::InvalidArgument(format!(
        "Unrecognized --status value '{raw}'. Expected one of: \
NOT_YET_RECRUITING, RECRUITING, ENROLLING_BY_INVITATION, ACTIVE_NOT_RECRUITING, \
COMPLETED, SUSPENDED, TERMINATED, WITHDRAWN. Aliases: active, comma/space forms."
    ))
}

fn normalize_single_status(value: &str) -> Result<&'static str, BioMcpError> {
    let key = normalize_enum_key(value);
    match key.as_str() {
        "NOT_YET_RECRUITING" => Ok("NOT_YET_RECRUITING"),
        "RECRUITING" => Ok("RECRUITING"),
        "ENROLLING_BY_INVITATION" | "ENROLLING" => Ok("ENROLLING_BY_INVITATION"),
        "ACTIVE_NOT_RECRUITING" | "ACTIVE" => Ok("ACTIVE_NOT_RECRUITING"),
        "COMPLETED" | "COMPLETE" => Ok("COMPLETED"),
        "SUSPENDED" => Ok("SUSPENDED"),
        "TERMINATED" => Ok("TERMINATED"),
        "WITHDRAWN" => Ok("WITHDRAWN"),
        _ => Err(invalid_status_error(value)),
    }
}

fn normalize_status(value: &str) -> Result<String, BioMcpError> {
    let raw = value.trim();
    if raw.is_empty() {
        return Err(BioMcpError::InvalidArgument(
            "--status must not be empty".into(),
        ));
    }

    // Preserve existing single-value aliases, including
    // "active, not recruiting".
    if let Ok(single) = normalize_single_status(raw) {
        return Ok(single.to_string());
    }

    let parts = raw
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() < 2 {
        return Err(invalid_status_error(raw));
    }

    let mut normalized = Vec::with_capacity(parts.len());
    for part in parts {
        normalized.push(normalize_single_status(part)?.to_string());
    }
    Ok(normalized.join(","))
}

fn status_priority(value: &str) -> u8 {
    match normalize_enum_key(value).as_str() {
        "RECRUITING" => 0,
        "ACTIVE_NOT_RECRUITING" => 1,
        "ENROLLING_BY_INVITATION" => 2,
        "NOT_YET_RECRUITING" => 3,
        "COMPLETED" => 4,
        "UNKNOWN" => 5,
        "WITHDRAWN" => 6,
        "TERMINATED" => 7,
        "SUSPENDED" => 8,
        _ => 9,
    }
}

fn sort_trials_by_status_priority(rows: &mut [TrialSearchResult]) {
    rows.sort_by(|a, b| {
        status_priority(&a.status)
            .cmp(&status_priority(&b.status))
            .then_with(|| a.nct_id.cmp(&b.nct_id))
    });
}

fn invalid_phase_error(raw: &str) -> BioMcpError {
    BioMcpError::InvalidArgument(format!(
        "Unrecognized --phase value '{raw}'. Expected one of: NA, EARLY_PHASE1, PHASE1, PHASE2, PHASE3, PHASE4. \
Aliases: 1-4, 1/2, early_phase1, early1, n/a."
    ))
}

fn invalid_sex_error(raw: &str) -> BioMcpError {
    BioMcpError::InvalidArgument(format!(
        "Unrecognized --sex value '{raw}'. Expected one of: female, male, all."
    ))
}

fn normalize_sex(value: &str) -> Result<Option<&'static str>, BioMcpError> {
    let raw = value.trim();
    if raw.is_empty() {
        return Err(BioMcpError::InvalidArgument(
            "--sex must not be empty".into(),
        ));
    }
    match normalize_enum_key(raw).as_str() {
        "FEMALE" | "F" => Ok(Some("f")),
        "MALE" | "M" => Ok(Some("m")),
        "ALL" | "ANY" | "BOTH" => Ok(None),
        _ => Err(invalid_sex_error(raw)),
    }
}

fn invalid_sponsor_type_error(raw: &str) -> BioMcpError {
    BioMcpError::InvalidArgument(format!(
        "Unrecognized --sponsor-type value '{raw}'. Expected one of: nih, industry, fed, other."
    ))
}

fn normalize_sponsor_type(value: &str) -> Result<&'static str, BioMcpError> {
    let raw = value.trim();
    if raw.is_empty() {
        return Err(BioMcpError::InvalidArgument(
            "--sponsor-type must not be empty".into(),
        ));
    }
    match normalize_enum_key(raw).as_str() {
        "NIH" => Ok("nih"),
        "INDUSTRY" => Ok("industry"),
        "FED" | "FEDERAL" => Ok("fed"),
        "OTHER" => Ok("other"),
        _ => Err(invalid_sponsor_type_error(raw)),
    }
}

fn normalize_phase(value: &str) -> Result<Vec<String>, BioMcpError> {
    let v = value.trim();
    if v.is_empty() {
        return Err(BioMcpError::InvalidArgument(
            "--phase must not be empty".into(),
        ));
    }

    let compact = v
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect::<String>()
        .to_ascii_uppercase();
    if compact == "1/2" {
        return Ok(vec!["PHASE1".to_string(), "PHASE2".to_string()]);
    }
    if matches!(compact.as_str(), "EARLY_PHASE1" | "EARLYPHASE1" | "EARLY1") {
        return Ok(vec!["EARLY_PHASE1".to_string()]);
    }
    if matches!(compact.as_str(), "NA" | "N/A") {
        return Ok(vec!["NA".to_string()]);
    }
    if compact.chars().all(|c| c.is_ascii_digit()) {
        return match compact.as_str() {
            "1" => Ok(vec!["PHASE1".to_string()]),
            "2" => Ok(vec!["PHASE2".to_string()]),
            "3" => Ok(vec!["PHASE3".to_string()]),
            "4" => Ok(vec!["PHASE4".to_string()]),
            _ => Err(invalid_phase_error(v)),
        };
    }

    let key = normalize_enum_key(v);
    match key.as_str() {
        "PHASE1" => Ok(vec!["PHASE1".to_string()]),
        "PHASE2" => Ok(vec!["PHASE2".to_string()]),
        "PHASE3" => Ok(vec!["PHASE3".to_string()]),
        "PHASE4" => Ok(vec!["PHASE4".to_string()]),
        "EARLY_PHASE1" | "EARLY1" => Ok(vec!["EARLY_PHASE1".to_string()]),
        "NA" => Ok(vec!["NA".to_string()]),
        _ => Err(invalid_phase_error(v)),
    }
}

fn normalized_status_filter(filters: &TrialSearchFilters) -> Result<Option<String>, BioMcpError> {
    filters
        .status
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(normalize_status)
        .transpose()
}

fn normalized_phase_filter(
    filters: &TrialSearchFilters,
) -> Result<Option<Vec<String>>, BioMcpError> {
    filters
        .phase
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(normalize_phase)
        .transpose()
}

fn normalize_intervention_query(value: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"([A-Za-z]{2,})\s+(\d{2,})").expect("valid regex"));
    re.replace_all(value.trim(), "$1-$2").into_owned()
}

fn normalized_facility_filter(filters: &TrialSearchFilters) -> Option<String> {
    filters
        .facility
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

async fn resolve_nci_disease_filter(
    condition: Option<&str>,
) -> Result<Option<NciDiseaseFilter>, BioMcpError> {
    let Some(condition) = condition.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    let client = MyDiseaseClient::new()?;
    match resolve_disease_hit_by_name(&client, condition).await {
        Ok(hit) => {
            let mut disease = transform::disease::from_mydisease_hit(hit);
            if let Some(nci_id) = disease
                .xrefs
                .remove("NCI")
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
            {
                Ok(Some(NciDiseaseFilter::ConceptId(nci_id)))
            } else {
                Ok(Some(NciDiseaseFilter::Keyword(condition.to_string())))
            }
        }
        Err(BioMcpError::NotFound { .. }) => {
            Ok(Some(NciDiseaseFilter::Keyword(condition.to_string())))
        }
        Err(err) => {
            warn!(
                condition,
                error = %err,
                "NCI disease grounding failed, falling back to keyword"
            );
            Ok(Some(NciDiseaseFilter::Keyword(condition.to_string())))
        }
    }
}

fn nci_status_filter(value: Option<&str>) -> Result<Option<NciStatusFilter>, BioMcpError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    if value.contains(',') {
        return Err(BioMcpError::InvalidArgument(
            "--status accepts one mapped status at a time for --source nci; comma-separated status lists are not supported".into(),
        ));
    }

    let filter = match value {
        "RECRUITING" => NciStatusFilter::SiteRecruitmentStatus("ACTIVE".into()),
        "NOT_YET_RECRUITING" => NciStatusFilter::CurrentTrialStatus("Approved".into()),
        "ENROLLING_BY_INVITATION" => {
            NciStatusFilter::CurrentTrialStatus("Enrolling by Invitation".into())
        }
        "ACTIVE_NOT_RECRUITING" => {
            NciStatusFilter::SiteRecruitmentStatus("CLOSED_TO_ACCRUAL".into())
        }
        "COMPLETED" => NciStatusFilter::CurrentTrialStatus("Complete".into()),
        "SUSPENDED" => NciStatusFilter::CurrentTrialStatus("Temporarily Closed to Accrual".into()),
        "TERMINATED" => NciStatusFilter::CurrentTrialStatus("INACTIVE".into()),
        "WITHDRAWN" => NciStatusFilter::CurrentTrialStatus("Withdrawn".into()),
        other => {
            return Err(BioMcpError::InvalidArgument(format!(
                "--status {other} is not supported for --source nci"
            )));
        }
    };

    Ok(Some(filter))
}

fn nci_phase_filters(value: Option<&[String]>) -> Result<Vec<String>, BioMcpError> {
    let Some(phases) = value else {
        return Ok(Vec::new());
    };
    if phases == ["PHASE1", "PHASE2"] {
        return Ok(vec!["I_II".to_string()]);
    }

    phases
        .iter()
        .map(|phase| match phase.as_str() {
            "PHASE1" => Ok("I".to_string()),
            "PHASE2" => Ok("II".to_string()),
            "PHASE3" => Ok("III".to_string()),
            "PHASE4" => Ok("IV".to_string()),
            "NA" => Ok("NA".to_string()),
            "EARLY_PHASE1" => Err(BioMcpError::InvalidArgument(
                "--phase early_phase1 is not supported for --source nci".into(),
            )),
            other => Err(BioMcpError::InvalidArgument(format!(
                "--phase {other} is not supported for --source nci"
            ))),
        })
        .collect()
}

fn nci_geo_filter(filters: &TrialSearchFilters) -> Option<NciGeoFilter> {
    let (Some(lat), Some(lon), Some(distance_miles)) = (filters.lat, filters.lon, filters.distance)
    else {
        return None;
    };
    Some(NciGeoFilter {
        lat,
        lon,
        distance_miles,
    })
}

fn normalize_facility_text(value: &str) -> Option<String> {
    let normalized = value
        .split_whitespace()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    (!normalized.is_empty()).then_some(normalized)
}

fn haversine_miles(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const EARTH_RADIUS_MILES: f64 = 3958.7613;
    let to_rad = |deg: f64| deg.to_radians();
    let d_lat = to_rad(lat2 - lat1);
    let d_lon = to_rad(lon2 - lon1);
    let lat1_rad = to_rad(lat1);
    let lat2_rad = to_rad(lat2);

    let a =
        (d_lat / 2.0).sin().powi(2) + lat1_rad.cos() * lat2_rad.cos() * (d_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    EARTH_RADIUS_MILES * c
}

fn location_matches_facility_geo(
    location: &CtGovLocation,
    facility_needle: &str,
    origin_lat: f64,
    origin_lon: f64,
    max_distance_miles: u32,
) -> bool {
    let Some(location_facility) = location
        .facility
        .as_deref()
        .and_then(normalize_facility_text)
    else {
        return false;
    };
    if !location_facility.contains(facility_needle) {
        return false;
    }
    let Some(geo) = location.geo_point.as_ref() else {
        return false;
    };
    let (Some(lat), Some(lon)) = (geo.lat, geo.lon) else {
        return false;
    };

    haversine_miles(origin_lat, origin_lon, lat, lon) <= max_distance_miles as f64
}

fn ctgov_nct_id(study: &CtGovStudy) -> Option<String> {
    study
        .protocol_section
        .as_ref()
        .and_then(|section| section.identification_module.as_ref())
        .and_then(|id| id.nct_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn trial_matches_facility_geo(
    study: &CtGovStudy,
    facility_needle: &str,
    origin_lat: f64,
    origin_lon: f64,
    max_distance_miles: u32,
) -> bool {
    study
        .protocol_section
        .as_ref()
        .and_then(|section| section.contacts_locations_module.as_ref())
        .map(|module| {
            module.locations.iter().any(|location| {
                location_matches_facility_geo(
                    location,
                    facility_needle,
                    origin_lat,
                    origin_lon,
                    max_distance_miles,
                )
            })
        })
        .unwrap_or(false)
}

fn exclusion_criteria_header_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?mi)^\s*(?:Key\s+)?Exclusion\s+Criteria\s*:?\s*$")
            .expect("exclusion criteria header regex is valid")
    })
}

fn split_eligibility_sections(text: &str) -> (String, String) {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return (String::new(), String::new());
    }

    let Some(header) = exclusion_criteria_header_re().find(trimmed) else {
        return (trimmed.to_ascii_lowercase(), String::new());
    };

    let inclusion = trimmed[..header.start()].trim().to_ascii_lowercase();
    let exclusion = trimmed[header.end()..].trim().to_ascii_lowercase();
    (inclusion, exclusion)
}

fn contains_keyword_tokens(section_text: &str, keyword: &str) -> bool {
    if section_text.is_empty() {
        return false;
    }

    let token_pattern = keyword
        .split_whitespace()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(regex::escape)
        .collect::<Vec<String>>();

    if token_pattern.is_empty() {
        return false;
    }

    token_pattern.iter().all(|token| {
        let pattern = build_token_pattern(token);
        Regex::new(&pattern)
            .map(|regex| regex.is_match(section_text))
            .unwrap_or(false)
    })
}

fn build_token_pattern(escaped_token: &str) -> String {
    let start = if escaped_token
        .chars()
        .next()
        .is_some_and(|c| c.is_alphanumeric() || c == '_')
    {
        r"\b"
    } else {
        r"(^|[^\w])"
    };
    let end = if escaped_token
        .chars()
        .last()
        .is_some_and(|c| c.is_alphanumeric() || c == '_')
    {
        r"\b"
    } else {
        r"($|[^\w])"
    };
    format!("{start}{escaped_token}{end}")
}

fn contains_exclusion_language(text: &str) -> bool {
    [
        "exclude",
        "excluded",
        "exclusion",
        "ineligible",
        "ineligibility",
        "not eligible",
        "not allowed",
        "not permitted",
        "must not",
        "must have no",
        "no prior",
        "no previous",
        "not have received",
        "not have previously",
        "not received",
        "not previously received",
        "have not received",
        "should not have",
        "cannot have",
    ]
    .iter()
    .any(|cue| text.contains(cue))
}

fn keyword_has_positive_inclusion_context(inclusion_text: &str, keyword: &str) -> bool {
    inclusion_text
        .split(['\n', '.', ';'])
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .filter(|segment| contains_keyword_tokens(segment, keyword))
        .any(|segment| !contains_exclusion_language(segment))
}

fn keyword_has_negative_inclusion_context(inclusion_text: &str, keyword: &str) -> bool {
    inclusion_text
        .split(['\n', '.', ';'])
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .filter(|segment| contains_keyword_tokens(segment, keyword))
        .any(contains_exclusion_language)
}

fn eligibility_keyword_in_inclusion(
    inclusion_text: &str,
    exclusion_text: &str,
    keyword: &str,
) -> bool {
    let keyword = keyword.trim().to_ascii_lowercase();
    if keyword.is_empty() {
        return true;
    }

    let inclusion_has_keyword = contains_keyword_tokens(inclusion_text, &keyword);

    // When exclusion section exists, use full logic
    if !exclusion_text.is_empty() {
        if inclusion_has_keyword && keyword_has_positive_inclusion_context(inclusion_text, &keyword)
        {
            return true;
        }
        if contains_keyword_tokens(exclusion_text, &keyword) {
            return false;
        }
        if inclusion_has_keyword {
            return false;
        }
        return true;
    }

    // No exclusion section — check inclusion text for negative context
    if !inclusion_has_keyword {
        return true; // keyword not mentioned at all, fail open
    }
    // Keyword is in inclusion text — reject if ANY sentence has exclusion language.
    // This is stricter than the with-exclusion-section path because without a
    // dedicated exclusion section, negative context like "must not have received X"
    // is embedded among protocol details that mention X in neutral/positive ways.
    !keyword_has_negative_inclusion_context(inclusion_text, &keyword)
}

fn collect_eligibility_keywords(filters: &TrialSearchFilters) -> Vec<String> {
    // Note: biomarker and mutation are intentionally excluded — they search
    // broader discovery fields rather than relying on EligibilityCriteria
    // alone, so post-filtering eligibility text would incorrectly narrow
    // relevant trials back down to inclusion-oriented mentions.
    let mut keywords = Vec::new();

    if let Some(criteria) = filters
        .criteria
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        && !has_boolean_operators(criteria)
    {
        keywords.push(criteria.to_string());
    }

    if let Some(prior_therapies) = filters
        .prior_therapies
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        keywords.push(prior_therapies.to_string());
    }

    if let Some(progression_on) = filters
        .progression_on
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        keywords.push(progression_on.to_string());
    }

    keywords
}

async fn verify_facility_geo(
    client: &ClinicalTrialsClient,
    studies: Vec<CtGovStudy>,
    facility_filter: &str,
    origin_lat: f64,
    origin_lon: f64,
    max_distance_miles: u32,
) -> Vec<CtGovStudy> {
    let Some(facility_needle) = normalize_facility_text(facility_filter) else {
        return studies;
    };

    let location_section = vec![TRIAL_SECTION_LOCATIONS.to_string()];
    let mut verification_stream = stream::iter(studies.into_iter().map(|study| {
        let nct_id = ctgov_nct_id(&study);
        let sections = location_section.clone();
        let facility_needle = facility_needle.clone();
        async move {
            let Some(nct_id) = nct_id else {
                return Some(study);
            };
            match client.get(&nct_id, &sections).await {
                Ok(details) => trial_matches_facility_geo(
                    &details,
                    &facility_needle,
                    origin_lat,
                    origin_lon,
                    max_distance_miles,
                )
                .then_some(study),
                Err(e) => {
                    warn!(nct_id, error = %e, "facility-geo detail fetch failed, keeping study");
                    Some(study)
                }
            }
        }
    }))
    .buffered(FACILITY_GEO_VERIFY_CONCURRENCY);

    let mut verified = Vec::new();
    while let Some(maybe_study) = verification_stream.next().await {
        if let Some(study) = maybe_study {
            verified.push(study);
        }
    }
    verified
}

async fn verify_eligibility_criteria(
    client: &ClinicalTrialsClient,
    studies: Vec<CtGovStudy>,
    keywords: &[String],
) -> Vec<CtGovStudy> {
    if keywords.is_empty() {
        return studies;
    }

    let eligibility_section = vec![TRIAL_SECTION_ELIGIBILITY.to_string()];
    let keywords = keywords.to_vec();
    let mut verification_stream = stream::iter(studies.into_iter().map(|study| {
        let nct_id = ctgov_nct_id(&study);
        let sections = eligibility_section.clone();
        let keywords = keywords.clone();
        async move {
            let Some(nct_id) = nct_id else {
                return Some(study);
            };
            match client.get(&nct_id, &sections).await {
                Ok(details) => {
                    let Some(criteria) = details
                        .protocol_section
                        .as_ref()
                        .and_then(|section| section.eligibility_module.as_ref())
                        .and_then(|module| module.eligibility_criteria.as_deref())
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                    else {
                        warn!(
                            nct_id,
                            "missing eligibility criteria in detail fetch, keeping study"
                        );
                        return Some(study);
                    };

                    let (inclusion, exclusion) = split_eligibility_sections(criteria);
                    keywords
                        .iter()
                        .all(|keyword| {
                            eligibility_keyword_in_inclusion(&inclusion, &exclusion, keyword)
                        })
                        .then_some(study)
                }
                Err(e) => {
                    warn!(nct_id, error = %e, "eligibility detail fetch failed, keeping study");
                    Some(study)
                }
            }
        }
    }))
    .buffered(ELIGIBILITY_VERIFY_CONCURRENCY);

    let mut verified = Vec::new();
    while let Some(maybe_study) = verification_stream.next().await {
        if let Some(study) = maybe_study {
            verified.push(study);
        }
    }
    verified
}

fn parse_age_years(value: &str) -> Option<f32> {
    let mut parts = value.split_whitespace();
    let amount = parts.next()?.parse::<f32>().ok()?;
    let unit = parts.next().map(|token| {
        token
            .trim_matches(|c: char| !c.is_ascii_alphabetic())
            .to_ascii_lowercase()
    });

    match unit.as_deref() {
        None | Some("year") | Some("years") => Some(amount),
        Some("month") | Some("months") => Some(amount / 12.0),
        Some("week") | Some("weeks") => Some(amount / 52.0),
        Some("day") | Some("days") => Some(amount / 365.0),
        _ => None,
    }
}

fn verify_age_eligibility(studies: Vec<CtGovStudy>, age: f32) -> Vec<CtGovStudy> {
    studies
        .into_iter()
        .filter(|study| {
            let module = study
                .protocol_section
                .as_ref()
                .and_then(|s| s.eligibility_module.as_ref());
            let min_ok = module
                .and_then(|m| m.minimum_age.as_deref())
                .and_then(parse_age_years)
                .is_none_or(|min| age >= min);
            let max_ok = module
                .and_then(|m| m.maximum_age.as_deref())
                .and_then(parse_age_years)
                .is_none_or(|max| age <= max);
            min_ok && max_ok
        })
        .collect()
}

fn ctgov_agg_filters(filters: &TrialSearchFilters) -> Result<Option<String>, BioMcpError> {
    let mut facets: Vec<String> = Vec::new();

    if let Some(sex) = filters
        .sex
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        && let Some(code) = normalize_sex(sex)?
    {
        facets.push(format!("sex:{code}"));
    }

    if let Some(sponsor_type) = filters
        .sponsor_type
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        facets.push(format!(
            "funderType:{}",
            normalize_sponsor_type(sponsor_type)?
        ));
    }

    if facets.is_empty() {
        Ok(None)
    } else {
        Ok(Some(facets.join(",")))
    }
}

fn validate_location(filters: &TrialSearchFilters) -> Result<(), BioMcpError> {
    let has_lat = filters.lat.is_some();
    let has_lon = filters.lon.is_some();
    let has_distance = filters.distance.is_some();

    if has_distance && (!has_lat || !has_lon) {
        return Err(BioMcpError::InvalidArgument(
            "--distance requires both --lat and --lon".into(),
        ));
    }
    if (has_lat || has_lon) && !has_distance {
        return Err(BioMcpError::InvalidArgument(
            "--lat/--lon requires --distance".into(),
        ));
    }
    if has_lat != has_lon {
        return Err(BioMcpError::InvalidArgument(
            "--lat and --lon must be provided together".into(),
        ));
    }
    Ok(())
}

fn truncate_inline_text(value: &str, max_chars: usize) -> String {
    let count = value.chars().count();
    if count <= max_chars {
        return value.to_string();
    }
    let truncated = value.chars().take(max_chars).collect::<String>();
    format!("{truncated}\n\n(truncated, {count} chars total)")
}

fn looks_like_nct_id(value: &str) -> bool {
    let v = value.trim().as_bytes();
    if v.len() != 11 {
        return false;
    }
    if &v[0..3] != b"NCT" {
        return false;
    }
    v[3..].iter().all(|b| b.is_ascii_digit())
}

fn normalize_nct_id(value: &str) -> String {
    let trimmed = value.trim();
    if let Some(prefix) = trimmed.get(..3)
        && prefix.eq_ignore_ascii_case("NCT")
    {
        return format!("NCT{}", &trimmed[3..]);
    }
    trimmed.to_string()
}

fn ctgov_query_term(
    filters: &TrialSearchFilters,
    normalized_phase: Option<&[String]>,
) -> Result<Option<String>, BioMcpError> {
    let mut terms: Vec<String> = Vec::new();

    if let Some(phases) = normalized_phase {
        if phases.len() == 1 {
            terms.push(format!("AREA[Phase]{}", phases[0]));
        } else if !phases.is_empty() {
            let inner = phases
                .iter()
                .map(|phase| format!("AREA[Phase]{phase}"))
                .collect::<Vec<_>>()
                .join(" AND ");
            terms.push(format!("({inner})"));
        }
    }
    if let Some(sponsor) = filters
        .sponsor
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let sponsor = essie_escape(sponsor);
        terms.push(format!("AREA[LeadSponsorName]\"{sponsor}\""));
    }
    if let Some(mutation) = filters
        .mutation
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let mutation = essie_escape_boolean_expression(mutation);
        // Search eligibility criteria, titles, summary, and keywords so
        // mutation-centric discovery does not miss trials that omit the
        // term from eligibility text but do mention it elsewhere.
        terms.push(format!(
            "(AREA[EligibilityCriteria]({mutation}) OR AREA[BriefTitle]({mutation}) \
             OR AREA[OfficialTitle]({mutation}) OR AREA[BriefSummary]({mutation}) \
             OR AREA[Keyword]({mutation}))"
        ));
    }
    if let Some(criteria) = filters
        .criteria
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let criteria = essie_escape_boolean_expression(criteria);
        terms.push(format!("AREA[EligibilityCriteria]({criteria})"));
    }
    if let Some(biomarker) = filters
        .biomarker
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        // Search curated structured fields (Keyword, InterventionName, Condition)
        // rather than free-text EligibilityCriteria. Gene symbols like "EGFR" in
        // eligibility text produce excessive false positives (e.g. diabetes trials
        // that mention EGFR in exclusion criteria). The curated fields are
        // author-maintained and far more precise for biomarker/gene queries.
        let biomarker = essie_escape(biomarker);
        terms.push(format!(
            "(AREA[Keyword]\"{biomarker}\" OR AREA[InterventionName]\"{biomarker}\" OR AREA[Condition]\"{biomarker}\")"
        ));
    }
    if let Some(study_type) = filters
        .study_type
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let study_type = essie_escape(study_type);
        terms.push(format!("AREA[StudyType]\"{study_type}\""));
    }
    terms.extend(build_essie_fragments(filters)?);
    if let Some(date_from) = filters
        .date_from
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let date_from = validate_since(date_from)?;
        let date_to = filters
            .date_to
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(validate_since)
            .transpose()?;
        if let Some(date_to) = date_to.as_deref() {
            if date_from.as_str() > date_to {
                return Err(BioMcpError::InvalidArgument(
                    "--date-from must be <= --date-to".into(),
                ));
            }
            terms.push(format!(
                "AREA[LastUpdatePostDate]RANGE[{date_from},{date_to}]"
            ));
        } else {
            terms.push(format!("AREA[LastUpdatePostDate]RANGE[{date_from},MAX]"));
        }
    } else if let Some(date_to) = filters
        .date_to
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let date_to = validate_since(date_to)?;
        terms.push(format!("AREA[LastUpdatePostDate]RANGE[MIN,{date_to}]"));
    }
    if filters.results_available {
        terms.push("AREA[ResultsFirstPostDate]RANGE[MIN,MAX]".to_string());
    }
    if terms.is_empty() {
        Ok(None)
    } else {
        Ok(Some(terms.join(" AND ")))
    }
}

fn has_any_query(filters: &TrialSearchFilters) -> bool {
    filters
        .condition
        .as_deref()
        .map(str::trim)
        .is_some_and(|v| !v.is_empty())
        || filters
            .intervention
            .as_deref()
            .map(str::trim)
            .is_some_and(|v| !v.is_empty())
        || filters
            .facility
            .as_deref()
            .map(str::trim)
            .is_some_and(|v| !v.is_empty())
        || filters
            .mutation
            .as_deref()
            .map(str::trim)
            .is_some_and(|v| !v.is_empty())
        || filters
            .criteria
            .as_deref()
            .map(str::trim)
            .is_some_and(|v| !v.is_empty())
        || filters
            .biomarker
            .as_deref()
            .map(str::trim)
            .is_some_and(|v| !v.is_empty())
        || filters
            .prior_therapies
            .as_deref()
            .map(str::trim)
            .is_some_and(|v| !v.is_empty())
        || filters
            .progression_on
            .as_deref()
            .map(str::trim)
            .is_some_and(|v| !v.is_empty())
        || filters
            .line_of_therapy
            .as_deref()
            .map(str::trim)
            .is_some_and(|v| !v.is_empty())
        || filters
            .sponsor
            .as_deref()
            .map(str::trim)
            .is_some_and(|v| !v.is_empty())
        || filters
            .status
            .as_deref()
            .map(str::trim)
            .is_some_and(|v| !v.is_empty())
        || filters
            .phase
            .as_deref()
            .map(str::trim)
            .is_some_and(|v| !v.is_empty())
        || filters
            .study_type
            .as_deref()
            .map(str::trim)
            .is_some_and(|v| !v.is_empty())
        || filters.age.is_some()
        || filters
            .sex
            .as_deref()
            .map(str::trim)
            .is_some_and(|v| !v.is_empty())
        || filters
            .sponsor_type
            .as_deref()
            .map(str::trim)
            .is_some_and(|v| !v.is_empty())
        || filters
            .date_from
            .as_deref()
            .map(str::trim)
            .is_some_and(|v| !v.is_empty())
        || filters
            .date_to
            .as_deref()
            .map(str::trim)
            .is_some_and(|v| !v.is_empty())
        || filters.results_available
        || filters.distance.is_some()
}

pub async fn search(
    filters: &TrialSearchFilters,
    limit: usize,
    offset: usize,
) -> Result<(Vec<TrialSearchResult>, Option<u32>), BioMcpError> {
    let page = search_page(filters, limit, offset, None).await?;
    Ok((page.results, page.total.map(|v| v as u32)))
}

struct NormalizedTrialSearch {
    normalized_status: Option<String>,
    normalized_phase: Option<Vec<String>>,
}

struct CtGovSearchContext {
    normalized_status: Option<String>,
    query_term: Option<String>,
    facility: Option<String>,
    agg_filters: Option<String>,
    eligibility_keywords: Vec<String>,
    facility_geo_verification: Option<(String, f64, f64, u32)>,
    uses_expensive_post_filters: bool,
    has_explicit_status: bool,
}

fn validate_search_page_args(
    limit: usize,
    offset: usize,
    next_page: Option<&str>,
) -> Result<(), BioMcpError> {
    const MAX_SEARCH_LIMIT: usize = 50;
    if limit == 0 || limit > MAX_SEARCH_LIMIT {
        return Err(BioMcpError::InvalidArgument(format!(
            "--limit must be between 1 and {MAX_SEARCH_LIMIT}"
        )));
    }
    if next_page
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
        && offset > 0
    {
        return Err(BioMcpError::InvalidArgument(
            "--next-page cannot be used together with --offset".into(),
        ));
    }
    Ok(())
}

fn validate_trial_search(
    filters: &TrialSearchFilters,
) -> Result<NormalizedTrialSearch, BioMcpError> {
    if !has_any_query(filters) {
        return Err(BioMcpError::InvalidArgument(
            "At least one filter is required. Example: biomcp search trial -c melanoma".into(),
        ));
    }

    let normalized_status = normalized_status_filter(filters)?;
    let normalized_phase = normalized_phase_filter(filters)?;
    validate_location(filters)?;

    if matches!(filters.source, TrialSource::NciCts)
        && normalized_status
            .as_deref()
            .is_some_and(|value| value.contains(','))
    {
        return Err(BioMcpError::InvalidArgument(
            "--status accepts one mapped status at a time for --source nci; comma-separated status lists are not supported".into(),
        ));
    }
    if matches!(filters.source, TrialSource::NciCts)
        && normalized_phase
            .as_ref()
            .is_some_and(|phases| phases.iter().any(|phase| phase == "EARLY_PHASE1"))
    {
        return Err(BioMcpError::InvalidArgument(
            "--phase early_phase1 is not supported for --source nci".into(),
        ));
    }

    if matches!(filters.source, TrialSource::NciCts) && has_essie_filters(filters) {
        return Err(BioMcpError::InvalidArgument(
            "--prior-therapies, --progression-on, and --line-of-therapy are only supported for --source ctgov".into(),
        ));
    }
    if matches!(filters.source, TrialSource::NciCts) && filters.results_available {
        return Err(BioMcpError::InvalidArgument(
            "--results-available is only supported for --source ctgov".into(),
        ));
    }
    if matches!(filters.source, TrialSource::NciCts) && filters.age.is_some() {
        return Err(BioMcpError::InvalidArgument(
            "--age is only supported for --source ctgov".into(),
        ));
    }
    if matches!(filters.source, TrialSource::NciCts)
        && filters
            .sex
            .as_deref()
            .map(str::trim)
            .is_some_and(|v| !v.is_empty())
    {
        return Err(BioMcpError::InvalidArgument(
            "--sex is only supported for --source ctgov".into(),
        ));
    }
    if matches!(filters.source, TrialSource::NciCts)
        && filters
            .sponsor_type
            .as_deref()
            .map(str::trim)
            .is_some_and(|v| !v.is_empty())
    {
        return Err(BioMcpError::InvalidArgument(
            "--sponsor-type is only supported for --source ctgov".into(),
        ));
    }

    Ok(NormalizedTrialSearch {
        normalized_status,
        normalized_phase,
    })
}

fn prepare_ctgov_search_context(
    filters: &TrialSearchFilters,
    normalized: &NormalizedTrialSearch,
) -> Result<CtGovSearchContext, BioMcpError> {
    let query_term = ctgov_query_term(filters, normalized.normalized_phase.as_deref())?;
    let facility = normalized_facility_filter(filters);
    let eligibility_keywords = collect_eligibility_keywords(filters);
    let agg_filters = ctgov_agg_filters(filters)?;
    let has_explicit_status = filters
        .status
        .as_deref()
        .map(str::trim)
        .is_some_and(|v| !v.is_empty());
    let facility_geo_verification = facility
        .as_deref()
        .zip(filters.lat)
        .zip(filters.lon)
        .zip(filters.distance)
        .map(|(((facility_name, lat), lon), distance)| {
            (facility_name.to_string(), lat, lon, distance)
        });
    let uses_expensive_post_filters =
        facility_geo_verification.is_some() || !eligibility_keywords.is_empty();

    Ok(CtGovSearchContext {
        normalized_status: normalized.normalized_status.clone(),
        query_term,
        facility,
        agg_filters,
        eligibility_keywords,
        facility_geo_verification,
        uses_expensive_post_filters,
        has_explicit_status,
    })
}

fn build_ctgov_search_params(
    filters: &TrialSearchFilters,
    context: &CtGovSearchContext,
    page_token: Option<String>,
    page_size: usize,
) -> CtGovSearchParams {
    CtGovSearchParams {
        condition: filters.condition.clone(),
        intervention: filters
            .intervention
            .as_deref()
            .map(normalize_intervention_query),
        facility: context.facility.clone(),
        status: context.normalized_status.clone(),
        agg_filters: context.agg_filters.clone(),
        query_term: context.query_term.clone(),
        count_total: true,
        page_token,
        page_size,
        lat: filters.lat,
        lon: filters.lon,
        distance_miles: filters.distance,
    }
}

async fn apply_ctgov_post_filters(
    client: &ClinicalTrialsClient,
    filters: &TrialSearchFilters,
    context: &CtGovSearchContext,
    mut studies: Vec<CtGovStudy>,
) -> Vec<CtGovStudy> {
    if let Some((facility_name, lat, lon, distance)) = context.facility_geo_verification.as_ref() {
        studies = verify_facility_geo(client, studies, facility_name, *lat, *lon, *distance).await;
    }
    if !context.eligibility_keywords.is_empty() {
        studies = verify_eligibility_criteria(client, studies, &context.eligibility_keywords).await;
    }
    if let Some(age) = filters.age {
        studies = verify_age_eligibility(studies, age);
    }
    studies
}

async fn search_page_with_ctgov_client(
    client: &ClinicalTrialsClient,
    filters: &TrialSearchFilters,
    limit: usize,
    offset: usize,
    next_page: Option<String>,
) -> Result<SearchPage<TrialSearchResult>, BioMcpError> {
    if !matches!(filters.source, TrialSource::ClinicalTrialsGov) {
        return Err(BioMcpError::InvalidArgument(
            "internal ctgov search helper requires --source ctgov".into(),
        ));
    }

    validate_search_page_args(limit, offset, next_page.as_deref())?;
    let normalized = validate_trial_search(filters)?;
    let context = prepare_ctgov_search_context(filters, &normalized)?;

    let page_size = limit.clamp(1, 100);
    let mut rows: Vec<TrialSearchResult> = Vec::new();
    let mut total: Option<usize> = None;
    let mut verified_total: usize = 0;
    let mut exhausted = false;
    let mut page_token = next_page
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let mut remaining_skip = offset;

    for _ in 0..CTGOV_MAX_PAGE_FETCHES {
        let resp = client
            .search(&build_ctgov_search_params(
                filters,
                &context,
                page_token.clone(),
                page_size,
            ))
            .await?;

        if total.is_none() {
            total = resp.total_count.map(|v| v as usize);
        }

        let next_page_token = resp.next_page_token;
        let mut studies = resp.studies;
        if studies.is_empty() {
            exhausted = true;
            break;
        }

        studies = apply_ctgov_post_filters(client, filters, &context, studies).await;
        if context.uses_expensive_post_filters {
            verified_total = verified_total.saturating_add(studies.len());
        }

        let page_started_with_skip = remaining_skip;
        let rows_before_page = rows.len();
        let page_study_count = studies.len();
        let mut page_consumed = 0;
        for study in studies.drain(..) {
            page_consumed += 1;
            if remaining_skip > 0 {
                remaining_skip -= 1;
                continue;
            }
            if rows.len() < limit {
                rows.push(transform::trial::from_ctgov_hit(&study));
            }
            if rows.len() >= limit {
                break;
            }
        }

        if rows.len() >= limit {
            if page_consumed >= page_study_count {
                page_token = next_page_token.clone();
            } else {
                page_token = None;
            }
            if next_page_token.is_none() {
                exhausted = true;
            }
            break;
        }

        // Once offset is satisfied inside a fetched page, stop at that page boundary so the
        // returned cursor remains CTGov's upstream next-page token for the unconsumed remainder.
        if page_started_with_skip > 0
            && remaining_skip == 0
            && rows.len() > rows_before_page
            && page_consumed >= page_study_count
        {
            page_token = next_page_token;
            if page_token.is_none() {
                exhausted = true;
            }
            break;
        }

        page_token = next_page_token;
        if page_token.is_none() {
            exhausted = true;
            break;
        }
    }

    if !context.has_explicit_status {
        sort_trials_by_status_priority(&mut rows);
    }

    rows.truncate(limit);
    let returned_total = if context.uses_expensive_post_filters {
        exhausted.then_some(verified_total)
    } else {
        total.or_else(|| Some(offset.saturating_add(rows.len())))
    };

    Ok(SearchPage::cursor(rows, returned_total, page_token))
}

async fn count_all_with_ctgov_client(
    client: &ClinicalTrialsClient,
    filters: &TrialSearchFilters,
) -> Result<TrialCount, BioMcpError> {
    if !matches!(filters.source, TrialSource::ClinicalTrialsGov) {
        return Err(BioMcpError::InvalidArgument(
            "internal ctgov count helper requires --source ctgov".into(),
        ));
    }

    let normalized = validate_trial_search(filters)?;
    let context = prepare_ctgov_search_context(filters, &normalized)?;

    if !context.uses_expensive_post_filters {
        let resp = client
            .search(&build_ctgov_search_params(filters, &context, None, 1))
            .await?;
        let total = resp.total_count.unwrap_or(0) as usize;
        return Ok(if filters.age.is_some() {
            TrialCount::Approximate(total)
        } else {
            TrialCount::Exact(total)
        });
    }

    let mut verified_total = 0usize;
    let mut page_token: Option<String> = None;
    let mut page_count = 0usize;

    loop {
        if page_count >= COUNT_TRAVERSAL_PAGE_CAP {
            return Ok(TrialCount::Unknown);
        }

        let resp = client
            .search(&build_ctgov_search_params(
                filters,
                &context,
                page_token.clone(),
                CTGOV_COUNT_PAGE_SIZE,
            ))
            .await?;
        page_count += 1;

        let next_page_token = resp.next_page_token;
        let studies = apply_ctgov_post_filters(client, filters, &context, resp.studies).await;
        verified_total = verified_total.saturating_add(studies.len());

        if next_page_token.is_none() {
            break;
        }
        page_token = next_page_token;
    }

    Ok(TrialCount::Exact(verified_total))
}

pub async fn count_all(filters: &TrialSearchFilters) -> Result<TrialCount, BioMcpError> {
    match filters.source {
        TrialSource::ClinicalTrialsGov => {
            let client = ClinicalTrialsClient::new()?;
            count_all_with_ctgov_client(&client, filters).await
        }
        TrialSource::NciCts => {
            let page = search_page(filters, 1, 0, None).await?;
            Ok(TrialCount::Exact(page.total.unwrap_or(page.results.len())))
        }
    }
}

pub async fn search_page(
    filters: &TrialSearchFilters,
    limit: usize,
    offset: usize,
    next_page: Option<String>,
) -> Result<SearchPage<TrialSearchResult>, BioMcpError> {
    match filters.source {
        TrialSource::ClinicalTrialsGov => {
            let client = ClinicalTrialsClient::new()?;
            search_page_with_ctgov_client(&client, filters, limit, offset, next_page).await
        }
        TrialSource::NciCts => {
            validate_search_page_args(limit, offset, next_page.as_deref())?;
            let normalized = validate_trial_search(filters)?;

            if filters.date_from.is_some() || filters.date_to.is_some() {
                return Err(BioMcpError::InvalidArgument(
                    "--date-from/--date-to is only supported for --source ctgov".into(),
                ));
            }
            if next_page
                .as_deref()
                .map(str::trim)
                .is_some_and(|value| !value.is_empty())
            {
                return Err(BioMcpError::InvalidArgument(
                    "--next-page is only supported for --source ctgov".into(),
                ));
            }
            let client = NciCtsClient::new()?;

            let params = NciSearchParams {
                disease: resolve_nci_disease_filter(filters.condition.as_deref()).await?,
                interventions: filters.intervention.clone(),
                sites_org_name: normalized_facility_filter(filters),
                status: nci_status_filter(normalized.normalized_status.as_deref())?,
                phases: nci_phase_filters(normalized.normalized_phase.as_deref())?,
                geo: nci_geo_filter(filters),
                biomarkers: filters
                    .biomarker
                    .clone()
                    .or_else(|| filters.mutation.clone())
                    .or_else(|| filters.criteria.clone()),
                size: limit,
                from: offset,
            };

            let resp = client.search(&params).await?;
            Ok(SearchPage::offset(
                resp.hits()
                    .iter()
                    .map(transform::trial::from_nci_hit)
                    .collect(),
                resp.total,
            ))
        }
    }
}

pub async fn get(
    nct_id: &str,
    sections: &[String],
    source: TrialSource,
) -> Result<Trial, BioMcpError> {
    let nct_id = normalize_nct_id(nct_id);
    let nct_id = nct_id.trim();
    if nct_id.is_empty() {
        return Err(BioMcpError::InvalidArgument(
            "NCT ID is required. Example: biomcp get trial NCT02576665".into(),
        ));
    }
    if nct_id.len() > 64 {
        return Err(BioMcpError::InvalidArgument("NCT ID is too long.".into()));
    }
    if !looks_like_nct_id(nct_id) {
        return Err(BioMcpError::InvalidArgument(format!(
            "Expected an NCT ID like NCT02576665 (got '{nct_id}')"
        )));
    }

    let section_flags = parse_sections(sections)?;

    match source {
        TrialSource::ClinicalTrialsGov => {
            let client = ClinicalTrialsClient::new()?;
            let study = client.get(nct_id, sections).await?;
            let mut trial = transform::trial::from_ctgov_study(&study);
            trial.source = Some("ClinicalTrials.gov".into());

            if section_flags.include_eligibility {
                let criteria = study
                    .protocol_section
                    .as_ref()
                    .and_then(|p| p.eligibility_module.as_ref())
                    .and_then(|m| m.eligibility_criteria.as_deref())
                    .map(str::trim)
                    .filter(|s| !s.is_empty());

                if let Some(criteria) = criteria {
                    trial.eligibility_text =
                        Some(truncate_inline_text(criteria, ELIGIBILITY_MAX_CHARS));
                }
            }
            if section_flags.include_references && trial.references.is_none() {
                trial.references = Some(Vec::new());
            }

            Ok(trial)
        }
        TrialSource::NciCts => {
            let client = NciCtsClient::new()?;
            let resp = client.get(nct_id).await?;
            let mut trial = transform::trial::from_nci_trial(&resp);
            trial.source = Some("NCI CTS".into());

            if section_flags.include_eligibility {
                // Best-effort: look for eligibility in common fields.
                let criteria = resp
                    .get("eligibility")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty());
                if let Some(criteria) = criteria {
                    trial.eligibility_text =
                        Some(truncate_inline_text(criteria, ELIGIBILITY_MAX_CHARS));
                } else {
                    warn!(nct_id, "NCI CTS eligibility criteria not found in response");
                }
            }
            if section_flags.include_references && trial.references.is_none() {
                trial.references = Some(Vec::new());
            }

            Ok(trial)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::sync::MutexGuard;
    use wiremock::matchers::{method, path, query_param, query_param_is_missing};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn env_lock() -> MutexGuard<'static, ()> {
        crate::test_support::env_lock().lock().await
    }

    struct EnvVarGuard {
        name: &'static str,
        previous: Option<String>,
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            // Safety: tests serialize environment mutation with `env_lock()`.
            unsafe {
                match &self.previous {
                    Some(value) => std::env::set_var(self.name, value),
                    None => std::env::remove_var(self.name),
                }
            }
        }
    }

    fn set_env_var(name: &'static str, value: Option<&str>) -> EnvVarGuard {
        let previous = std::env::var(name).ok();
        // Safety: tests serialize environment mutation with `env_lock()`.
        unsafe {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
        EnvVarGuard { name, previous }
    }

    fn nci_search_response(nct_id: &str) -> serde_json::Value {
        json!({
            "data": [{
                "nct_id": nct_id,
                "brief_title": "Fixture NCI Trial",
                "current_trial_status": "ACTIVE",
                "phase": "II",
                "diseases": ["Melanoma"]
            }],
            "total": 1
        })
    }

    fn mydisease_query_response(hit: serde_json::Value) -> serde_json::Value {
        json!({
            "total": 1,
            "hits": [hit]
        })
    }

    async fn mount_keyword_nci_search(nci: &MockServer, keyword: &str, nct_id: &str) {
        Mock::given(method("GET"))
            .and(path("/trials"))
            .and(query_param("keyword", keyword))
            .and(query_param_is_missing("diseases.nci_thesaurus_concept_id"))
            .and(query_param("size", "1"))
            .and(query_param("from", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(nci_search_response(nct_id)))
            .expect(1)
            .mount(nci)
            .await;
    }

    fn ctgov_study_fixture(locations: serde_json::Value) -> CtGovStudy {
        serde_json::from_value(json!({
            "protocolSection": {
                "identificationModule": {
                    "nctId": "NCT00000001",
                    "briefTitle": "Fixture Trial",
                    "overallStatus": "RECRUITING"
                },
                "contactsLocationsModule": {
                    "locations": locations
                }
            }
        }))
        .expect("valid CtGovStudy fixture")
    }

    #[test]
    fn split_eligibility_sections_detects_exclusion_header() {
        let text = "Inclusion Criteria:\nMust have MSI-H disease\n\nExclusion Criteria:\nNo active CNS mets";
        let (inclusion, exclusion) = split_eligibility_sections(text);
        assert!(inclusion.contains("must have msi-h disease"));
        assert!(exclusion.contains("no active cns mets"));
    }

    #[test]
    fn split_eligibility_sections_supports_key_exclusion_header() {
        let text =
            "Inclusion:\nBRAF V600E mutation\n\nKey Exclusion Criteria:\nPrior anti-braf therapy";
        let (inclusion, exclusion) = split_eligibility_sections(text);
        assert!(inclusion.contains("braf v600e mutation"));
        assert!(exclusion.contains("prior anti-braf therapy"));
    }

    #[test]
    fn split_eligibility_sections_without_exclusion_keeps_all_in_inclusion() {
        let text = "Inclusion Criteria:\nPathogenic EGFR mutation";
        let (inclusion, exclusion) = split_eligibility_sections(text);
        assert!(inclusion.contains("pathogenic egfr mutation"));
        assert!(exclusion.is_empty());
    }

    #[test]
    fn eligibility_keyword_in_inclusion_keeps_when_inclusion_matches() {
        assert!(eligibility_keyword_in_inclusion(
            "must have msi-h disease",
            "no untreated brain metastases",
            "MSI-H"
        ));
    }

    #[test]
    fn eligibility_keyword_in_inclusion_discards_exclusion_only_match() {
        assert!(!eligibility_keyword_in_inclusion(
            "must have metastatic colorectal cancer",
            "exclusion includes msi-h tumors",
            "MSI-H"
        ));
    }

    #[test]
    fn eligibility_keyword_in_inclusion_keeps_when_in_both_sections() {
        assert!(eligibility_keyword_in_inclusion(
            "inclusion requires braf v600e mutation",
            "exclude prior braf v600e inhibitor exposure",
            "BRAF V600E"
        ));
    }

    #[test]
    fn eligibility_keyword_in_inclusion_discards_negated_inclusion_sentence() {
        assert!(!eligibility_keyword_in_inclusion(
            "patients whose tumors are msi-h are excluded",
            "exclude active infection",
            "MSI-H"
        ));
    }

    #[test]
    fn eligibility_keyword_in_inclusion_fails_open_when_keyword_missing() {
        assert!(eligibility_keyword_in_inclusion(
            "include untreated metastatic disease",
            "exclude uncontrolled infection",
            "MSI-H"
        ));
    }

    #[test]
    fn eligibility_keyword_in_inclusion_fails_open_without_exclusion_section() {
        assert!(eligibility_keyword_in_inclusion(
            "patients with msi-h disease",
            "",
            "MSI-H"
        ));
    }

    #[test]
    fn eligibility_keyword_in_inclusion_rejects_negated_without_exclusion_section() {
        assert!(!eligibility_keyword_in_inclusion(
            "participants must not have previously received osimertinib",
            "",
            "osimertinib"
        ));
    }

    #[test]
    fn eligibility_keyword_in_inclusion_rejects_no_prior_without_exclusion_section() {
        assert!(!eligibility_keyword_in_inclusion(
            "no prior osimertinib therapy allowed",
            "",
            "osimertinib"
        ));
    }

    #[test]
    fn eligibility_keyword_in_inclusion_rejects_mixed_context_without_exclusion_section() {
        // Simulates trials like NCT03191149 where osimertinib is the study drug
        // (appears in many neutral sentences) but one sentence excludes prior use.
        assert!(!eligibility_keyword_in_inclusion(
            "participants must not have previously received osimertinib. \
             inability to swallow osimertinib tablets. \
             duration before restarting osimertinib is advised",
            "",
            "osimertinib"
        ));
    }

    #[test]
    fn parse_age_years_handles_standard_formats() {
        assert_eq!(parse_age_years("18 Years"), Some(18.0));
        assert_eq!(parse_age_years("75 Years"), Some(75.0));
        assert_eq!(parse_age_years("18"), Some(18.0));
        assert_eq!(parse_age_years("6 Months"), Some(0.5));
        assert!(
            (parse_age_years("2 Weeks").expect("weeks should parse") - (2.0 / 52.0)).abs()
                < f32::EPSILON
        );
        assert!(
            (parse_age_years("30 Days").expect("days should parse") - (30.0 / 365.0)).abs()
                < f32::EPSILON
        );
        assert_eq!(parse_age_years("N/A"), None);
        assert_eq!(parse_age_years(""), None);
    }

    #[test]
    fn verify_age_eligibility_handles_sub_year_minimum_age() {
        let study: CtGovStudy = serde_json::from_value(ctgov_search_study_fixture(
            "NCT00000001",
            "6 Months",
            "75 Years",
        ))
        .expect("study fixture should deserialize");

        assert!(verify_age_eligibility(vec![study.clone()], 0.0).is_empty());
        assert_eq!(verify_age_eligibility(vec![study], 0.5).len(), 1);
    }

    #[test]
    fn verify_age_eligibility_handles_sub_year_maximum_age() {
        let study: CtGovStudy =
            serde_json::from_value(ctgov_search_study_fixture("NCT00000002", "N/A", "6 Months"))
                .expect("study fixture should deserialize");

        assert_eq!(verify_age_eligibility(vec![study.clone()], 0.5).len(), 1);
        assert!(verify_age_eligibility(vec![study], 1.0).is_empty());
    }

    #[test]
    fn collect_eligibility_keywords_includes_supported_filters() {
        // biomarker is intentionally excluded — it now searches curated
        // structured fields rather than EligibilityCriteria free text.
        let filters = TrialSearchFilters {
            mutation: Some("MSI-H".into()),
            criteria: Some("mismatch repair deficient".into()),
            biomarker: Some("TMB-high".into()),
            prior_therapies: Some("osimertinib".into()),
            progression_on: Some("pembrolizumab".into()),
            ..Default::default()
        };

        assert_eq!(
            collect_eligibility_keywords(&filters),
            vec!["mismatch repair deficient", "osimertinib", "pembrolizumab"]
        );
    }

    #[test]
    fn collect_eligibility_keywords_omits_blank_values() {
        let filters = TrialSearchFilters {
            mutation: Some("   ".into()),
            criteria: Some("".into()),
            biomarker: Some(" MSI-H ".into()),
            prior_therapies: None,
            progression_on: Some("".into()),
            ..Default::default()
        };

        // biomarker excluded from eligibility keywords; mutation is blank
        assert_eq!(collect_eligibility_keywords(&filters), Vec::<String>::new());
    }

    #[test]
    fn collect_eligibility_keywords_skips_boolean_expressions() {
        let filters = TrialSearchFilters {
            mutation: Some("dMMR OR MSI-H".into()),
            criteria: Some("prior platinum AND ECOG 0-1".into()),
            prior_therapies: Some("pembrolizumab".into()),
            ..Default::default()
        };

        assert_eq!(
            collect_eligibility_keywords(&filters),
            vec!["pembrolizumab"]
        );
    }

    #[test]
    fn essie_escape_boolean_expression_preserves_or_operators() {
        assert_eq!(
            essie_escape_boolean_expression("dMMR OR MSI-H"),
            "\"dMMR\" OR \"MSI\\-H\""
        );
    }

    #[test]
    fn essie_escape_boolean_expression_handles_leading_not() {
        assert_eq!(
            essie_escape_boolean_expression("NOT MSI-H"),
            "NOT \"MSI\\-H\""
        );
    }

    #[test]
    fn essie_escape_boolean_expression_handles_and_not() {
        assert_eq!(
            essie_escape_boolean_expression("dMMR AND NOT MSI-H"),
            "\"dMMR\" AND NOT \"MSI\\-H\""
        );
    }

    #[test]
    fn ctgov_query_term_broadens_mutation_across_discovery_fields() {
        let filters = TrialSearchFilters {
            mutation: Some("dMMR OR MSI-H".into()),
            criteria: Some("mismatch repair deficient".into()),
            ..Default::default()
        };

        let query = ctgov_query_term(&filters, None)
            .expect("query term should build")
            .expect("query term should not be empty");
        assert!(query.contains(
            "(AREA[EligibilityCriteria](\"dMMR\" OR \"MSI\\-H\") OR \
AREA[BriefTitle](\"dMMR\" OR \"MSI\\-H\") OR \
AREA[OfficialTitle](\"dMMR\" OR \"MSI\\-H\") OR \
AREA[BriefSummary](\"dMMR\" OR \"MSI\\-H\") OR \
AREA[Keyword](\"dMMR\" OR \"MSI\\-H\"))"
        ));
        assert!(query.contains("AREA[EligibilityCriteria](\"mismatch repair deficient\")"));
    }

    #[test]
    fn ctgov_query_term_broadens_simple_mutation_across_discovery_fields() {
        let filters = TrialSearchFilters {
            mutation: Some("G12D".into()),
            ..Default::default()
        };

        let query = ctgov_query_term(&filters, None)
            .expect("query term should build")
            .expect("query term should not be empty");
        assert!(query.contains(
            "(AREA[EligibilityCriteria](\"G12D\") OR AREA[BriefTitle](\"G12D\") OR \
AREA[OfficialTitle](\"G12D\") OR AREA[BriefSummary](\"G12D\") OR AREA[Keyword](\"G12D\"))"
        ));
    }

    #[test]
    fn contains_keyword_tokens_matches_plus_suffix_token() {
        assert!(contains_keyword_tokens(
            "HER2+ positive breast cancer",
            "HER2+"
        ));
    }

    #[test]
    fn contains_keyword_tokens_does_not_match_without_plus_suffix() {
        assert!(!contains_keyword_tokens("her2 amplification", "HER2+"));
    }

    #[test]
    fn contains_keyword_tokens_matches_slash_separated_plus_tokens() {
        assert!(contains_keyword_tokens("ER+/PR+ breast cancer", "ER+"));
    }

    #[test]
    fn contains_keyword_tokens_matches_hyphenated_token() {
        assert!(contains_keyword_tokens("PD-L1 expression >=1%", "PD-L1"));
    }

    #[test]
    fn contains_keyword_tokens_matches_hyphenated_plus_token() {
        assert!(contains_keyword_tokens("PD-L1+ expression >=1%", "PD-L1+"));
    }

    #[test]
    fn contains_keyword_tokens_rejects_hyphenated_token_without_plus_suffix() {
        assert!(!contains_keyword_tokens("PD-L1 positive", "PD-L1+"));
    }

    #[test]
    fn contains_keyword_tokens_matches_word_token() {
        assert!(contains_keyword_tokens("BRAF V600E mutation", "BRAF"));
    }

    #[test]
    fn contains_keyword_tokens_rejects_substring_word_match() {
        assert!(!contains_keyword_tokens("abraf", "BRAF"));
    }

    #[test]
    fn status_priority_prefers_recruiting_over_completed() {
        assert!(status_priority("RECRUITING") < status_priority("COMPLETED"));
        assert!(status_priority("ACTIVE_NOT_RECRUITING") < status_priority("UNKNOWN"));
    }

    #[test]
    fn line_of_therapy_patterns_accepts_supported_values() {
        assert!(line_of_therapy_patterns("1L").is_some());
        assert!(line_of_therapy_patterns("2L").is_some());
        assert!(line_of_therapy_patterns("3L+").is_some());
        assert!(line_of_therapy_patterns("2l").is_some());
    }

    #[test]
    fn line_of_therapy_patterns_rejects_invalid_values() {
        assert!(line_of_therapy_patterns("4L").is_none());
        assert!(line_of_therapy_patterns("frontline").is_none());
    }

    #[test]
    fn normalize_phase_accepts_aliases() {
        assert_eq!(normalize_phase("1").unwrap(), vec!["PHASE1".to_string()]);
        assert_eq!(
            normalize_phase("PHASE2").unwrap(),
            vec!["PHASE2".to_string()]
        );
        assert_eq!(
            normalize_phase("1/2").unwrap(),
            vec!["PHASE1".to_string(), "PHASE2".to_string()]
        );
        assert_eq!(
            normalize_phase("early_phase1").unwrap(),
            vec!["EARLY_PHASE1".to_string()]
        );
        assert_eq!(
            normalize_phase("early1").unwrap(),
            vec!["EARLY_PHASE1".to_string()]
        );
        assert_eq!(normalize_phase("n/a").unwrap(), vec!["NA".to_string()]);
    }

    #[test]
    fn normalize_phase_rejects_invalid_value() {
        let err = normalize_phase("5").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Unrecognized --phase value"));
        assert!(msg.contains("EARLY_PHASE1"));
    }

    #[test]
    fn ctgov_query_term_joins_multi_phase_filters_with_and() {
        let filters = TrialSearchFilters {
            condition: Some("melanoma".into()),
            ..Default::default()
        };

        let query = ctgov_query_term(&filters, Some(&["PHASE1".into(), "PHASE2".into()]))
            .expect("query term should build")
            .expect("query term should not be empty");
        assert!(query.contains("(AREA[Phase]PHASE1 AND AREA[Phase]PHASE2)"));
    }

    #[test]
    fn normalize_intervention_query_canonicalizes_confirmed_drug_code_pattern() {
        assert_eq!(normalize_intervention_query("HRS 4642"), "HRS-4642");
    }

    #[test]
    fn normalize_intervention_query_preserves_generic_multiword_names() {
        assert_eq!(
            normalize_intervention_query("pembrolizumab"),
            "pembrolizumab"
        );
        assert_eq!(
            normalize_intervention_query("immune checkpoint inhibitor"),
            "immune checkpoint inhibitor"
        );
    }

    #[test]
    fn build_ctgov_search_params_maps_all_shared_fields() {
        let filters = TrialSearchFilters {
            condition: Some("melanoma".into()),
            intervention: Some("HRS 4642".into()),
            facility: Some("Mayo Clinic".into()),
            status: Some("active".into()),
            phase: Some("1/2".into()),
            study_type: Some("Interventional".into()),
            sex: Some("female".into()),
            sponsor: Some("Acme Oncology".into()),
            sponsor_type: Some("industry".into()),
            mutation: Some("MSI-H".into()),
            criteria: Some("mismatch repair deficient".into()),
            results_available: true,
            lat: Some(42.3601),
            lon: Some(-71.0589),
            distance: Some(25),
            ..Default::default()
        };
        let normalized = validate_trial_search(&filters).expect("filters should validate");
        let context =
            prepare_ctgov_search_context(&filters, &normalized).expect("context should build");

        let params = build_ctgov_search_params(&filters, &context, Some("cursor-1".into()), 37);

        assert_eq!(params.condition, filters.condition);
        assert_eq!(params.intervention.as_deref(), Some("HRS-4642"));
        assert_eq!(params.facility, context.facility);
        assert_eq!(params.status, context.normalized_status);
        assert_eq!(params.agg_filters, context.agg_filters);
        assert_eq!(params.query_term, context.query_term);
        assert!(params.count_total);
        assert_eq!(params.page_token.as_deref(), Some("cursor-1"));
        assert_eq!(params.page_size, 37);
        assert_eq!(params.lat, filters.lat);
        assert_eq!(params.lon, filters.lon);
        assert_eq!(params.distance_miles, filters.distance);
    }

    #[test]
    fn build_ctgov_search_params_preserves_none_values_without_defaults() {
        let filters = TrialSearchFilters {
            condition: Some("melanoma".into()),
            ..Default::default()
        };
        let normalized = validate_trial_search(&filters).expect("filters should validate");
        let context =
            prepare_ctgov_search_context(&filters, &normalized).expect("context should build");

        let params = build_ctgov_search_params(&filters, &context, None, 10);

        assert_eq!(params.condition, Some("melanoma".into()));
        assert_eq!(params.intervention, None);
        assert_eq!(params.facility, None);
        assert_eq!(params.status, None);
        assert_eq!(params.agg_filters, None);
        assert_eq!(params.query_term, None);
        assert!(params.count_total);
        assert_eq!(params.page_token, None);
        assert_eq!(params.page_size, 10);
        assert_eq!(params.lat, None);
        assert_eq!(params.lon, None);
        assert_eq!(params.distance_miles, None);
    }

    #[test]
    fn build_ctgov_search_params_keeps_search_and_count_call_shapes_aligned() {
        let filters = TrialSearchFilters {
            condition: Some("melanoma".into()),
            intervention: Some("HRS 4642".into()),
            facility: Some("Dana-Farber Cancer Institute".into()),
            status: Some("recruiting".into()),
            phase: Some("2".into()),
            sex: Some("all".into()),
            sponsor_type: Some("nih".into()),
            mutation: Some("BRAF V600E".into()),
            criteria: Some("prior anti-braf therapy".into()),
            lat: Some(42.3355),
            lon: Some(-71.1041),
            distance: Some(15),
            ..Default::default()
        };
        let normalized = validate_trial_search(&filters).expect("filters should validate");
        let context =
            prepare_ctgov_search_context(&filters, &normalized).expect("context should build");

        let search_page_params =
            build_ctgov_search_params(&filters, &context, Some("page-1".into()), 25);
        let fast_count_params = build_ctgov_search_params(&filters, &context, None, 1);
        let slow_count_params = build_ctgov_search_params(
            &filters,
            &context,
            Some("page-2".into()),
            CTGOV_COUNT_PAGE_SIZE,
        );

        assert_eq!(search_page_params.condition, fast_count_params.condition);
        assert_eq!(search_page_params.condition, slow_count_params.condition);
        assert_eq!(
            search_page_params.intervention,
            fast_count_params.intervention
        );
        assert_eq!(
            search_page_params.intervention,
            slow_count_params.intervention
        );
        assert_eq!(search_page_params.facility, fast_count_params.facility);
        assert_eq!(search_page_params.facility, slow_count_params.facility);
        assert_eq!(search_page_params.status, fast_count_params.status);
        assert_eq!(search_page_params.status, slow_count_params.status);
        assert_eq!(
            search_page_params.agg_filters,
            fast_count_params.agg_filters
        );
        assert_eq!(
            search_page_params.agg_filters,
            slow_count_params.agg_filters
        );
        assert_eq!(search_page_params.query_term, fast_count_params.query_term);
        assert_eq!(search_page_params.query_term, slow_count_params.query_term);
        assert_eq!(
            search_page_params.count_total,
            fast_count_params.count_total
        );
        assert_eq!(
            search_page_params.count_total,
            slow_count_params.count_total
        );
        assert_eq!(search_page_params.lat, fast_count_params.lat);
        assert_eq!(search_page_params.lat, slow_count_params.lat);
        assert_eq!(search_page_params.lon, fast_count_params.lon);
        assert_eq!(search_page_params.lon, slow_count_params.lon);
        assert_eq!(
            search_page_params.distance_miles,
            fast_count_params.distance_miles
        );
        assert_eq!(
            search_page_params.distance_miles,
            slow_count_params.distance_miles
        );

        assert_eq!(search_page_params.page_token.as_deref(), Some("page-1"));
        assert_eq!(search_page_params.page_size, 25);
        assert_eq!(fast_count_params.page_token, None);
        assert_eq!(fast_count_params.page_size, 1);
        assert_eq!(slow_count_params.page_token.as_deref(), Some("page-2"));
        assert_eq!(slow_count_params.page_size, CTGOV_COUNT_PAGE_SIZE);
    }

    #[test]
    fn normalize_status_accepts_ctgov_wording_and_aliases() {
        assert_eq!(
            normalize_status("active, not recruiting").unwrap(),
            "ACTIVE_NOT_RECRUITING"
        );
        assert_eq!(normalize_status("active").unwrap(), "ACTIVE_NOT_RECRUITING");
        assert_eq!(normalize_status("recruiting").unwrap(), "RECRUITING");
        assert_eq!(
            normalize_status("enrolling_by_invitation").unwrap(),
            "ENROLLING_BY_INVITATION"
        );
    }

    #[test]
    fn normalize_status_accepts_comma_separated_values() {
        assert_eq!(
            normalize_status("RECRUITING,ACTIVE_NOT_RECRUITING").unwrap(),
            "RECRUITING,ACTIVE_NOT_RECRUITING"
        );
        assert_eq!(
            normalize_status("recruiting,active").unwrap(),
            "RECRUITING,ACTIVE_NOT_RECRUITING"
        );
    }

    #[test]
    fn normalize_status_rejects_invalid_value() {
        let err = normalize_status("bogus").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Unrecognized --status value"));
        assert!(msg.contains("ENROLLING_BY_INVITATION"));
    }

    #[test]
    fn normalize_status_rejects_comma_list_with_invalid_value() {
        let err = normalize_status("bogus,recruiting").unwrap_err();
        assert!(err.to_string().contains("Unrecognized --status value"));
    }

    #[test]
    fn normalize_sex_accepts_supported_values() {
        assert_eq!(normalize_sex("female").unwrap(), Some("f"));
        assert_eq!(normalize_sex("male").unwrap(), Some("m"));
        assert_eq!(normalize_sex("all").unwrap(), None);
        assert_eq!(normalize_sex("F").unwrap(), Some("f"));
        assert_eq!(normalize_sex("M").unwrap(), Some("m"));
    }

    #[test]
    fn normalize_sponsor_type_accepts_supported_values() {
        assert_eq!(normalize_sponsor_type("nih").unwrap(), "nih");
        assert_eq!(normalize_sponsor_type("industry").unwrap(), "industry");
        assert_eq!(normalize_sponsor_type("fed").unwrap(), "fed");
        assert_eq!(normalize_sponsor_type("federal").unwrap(), "fed");
        assert_eq!(normalize_sponsor_type("other").unwrap(), "other");
    }

    #[test]
    fn normalize_sex_rejects_invalid_value() {
        let err = normalize_sex("unknown").unwrap_err();
        assert!(err.to_string().contains("Unrecognized --sex value"));
    }

    #[test]
    fn normalize_sponsor_type_rejects_invalid_value() {
        let err = normalize_sponsor_type("charity").unwrap_err();
        assert!(
            err.to_string()
                .contains("Unrecognized --sponsor-type value")
        );
    }

    #[test]
    fn normalize_nct_id_uppercases_prefix() {
        assert_eq!(normalize_nct_id("nct06162221"), "NCT06162221");
        assert_eq!(normalize_nct_id("NCT06162221"), "NCT06162221");
    }

    #[tokio::test]
    async fn get_rejects_non_nct_id_with_format_hint() {
        let err = get("WRONG", &[], TrialSource::ClinicalTrialsGov)
            .await
            .expect_err("invalid trial id should fail before API call");

        match err {
            BioMcpError::InvalidArgument(message) => {
                assert!(message.contains("Expected an NCT ID like NCT02576665"));
                assert!(message.contains("got 'WRONG'"));
            }
            other => panic!("expected InvalidArgument, got: {other}"),
        }
    }

    #[tokio::test]
    async fn nci_search_page_prefers_grounded_disease_concept_id() {
        let _lock = env_lock().await;
        let mydisease = MockServer::start().await;
        let nci = MockServer::start().await;
        let mydisease_base = format!("{}/v1", mydisease.uri());
        let _mydisease_base = set_env_var("BIOMCP_MYDISEASE_BASE", Some(&mydisease_base));
        let _nci_base = set_env_var("BIOMCP_NCI_CTS_BASE", Some(&nci.uri()));
        let _nci_key = set_env_var("NCI_API_KEY", Some("test-key"));

        Mock::given(method("GET"))
            .and(path("/v1/query"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(mydisease_query_response(json!({
                    "_id": "MONDO:0005105",
                    "mondo": {
                        "name": "Melanoma",
                        "xrefs": {
                            "ncit": ["C3224"]
                        }
                    }
                }))),
            )
            .mount(&mydisease)
            .await;

        Mock::given(method("GET"))
            .and(path("/trials"))
            .and(query_param("diseases.nci_thesaurus_concept_id", "C3224"))
            .and(query_param_is_missing("keyword"))
            .and(query_param("size", "1"))
            .and(query_param("from", "0"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(nci_search_response("NCT00000001")),
            )
            .expect(1)
            .mount(&nci)
            .await;

        let filters = TrialSearchFilters {
            source: TrialSource::NciCts,
            condition: Some("melanoma".into()),
            ..Default::default()
        };

        let page = search_page(&filters, 1, 0, None)
            .await
            .expect("grounded NCI search should succeed");
        assert_eq!(page.results.len(), 1);
        assert_eq!(page.results[0].nct_id, "NCT00000001");
    }

    #[tokio::test]
    async fn nci_search_page_falls_back_to_keyword_when_grounding_is_unavailable() {
        let _lock = env_lock().await;
        let mydisease = MockServer::start().await;
        let nci = MockServer::start().await;
        let mydisease_base = format!("{}/v1", mydisease.uri());
        let _mydisease_base = set_env_var("BIOMCP_MYDISEASE_BASE", Some(&mydisease_base));
        let _nci_base = set_env_var("BIOMCP_NCI_CTS_BASE", Some(&nci.uri()));
        let _nci_key = set_env_var("NCI_API_KEY", Some("test-key"));

        Mock::given(method("GET"))
            .and(path("/v1/query"))
            .respond_with(ResponseTemplate::new(500).set_body_string("resolver unavailable"))
            .mount(&mydisease)
            .await;

        mount_keyword_nci_search(&nci, "melanoma", "NCT00000002").await;

        let filters = TrialSearchFilters {
            source: TrialSource::NciCts,
            condition: Some("melanoma".into()),
            ..Default::default()
        };

        let page = search_page(&filters, 1, 0, None)
            .await
            .expect("keyword fallback should keep NCI search available");
        assert_eq!(page.results.len(), 1);
        assert_eq!(page.results[0].nct_id, "NCT00000002");
    }

    #[tokio::test]
    async fn nci_search_page_falls_back_to_keyword_when_best_hit_lacks_nci_xref() {
        let _lock = env_lock().await;
        let mydisease = MockServer::start().await;
        let nci = MockServer::start().await;
        let mydisease_base = format!("{}/v1", mydisease.uri());
        let _mydisease_base = set_env_var("BIOMCP_MYDISEASE_BASE", Some(&mydisease_base));
        let _nci_base = set_env_var("BIOMCP_NCI_CTS_BASE", Some(&nci.uri()));
        let _nci_key = set_env_var("NCI_API_KEY", Some("test-key"));

        Mock::given(method("GET"))
            .and(path("/v1/query"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(mydisease_query_response(json!({
                    "_id": "MONDO:0005105",
                    "mondo": {
                        "name": "Melanoma"
                    }
                }))),
            )
            .mount(&mydisease)
            .await;

        mount_keyword_nci_search(&nci, "melanoma", "NCT00000008").await;

        let filters = TrialSearchFilters {
            source: TrialSource::NciCts,
            condition: Some("melanoma".into()),
            ..Default::default()
        };

        let page = search_page(&filters, 1, 0, None)
            .await
            .expect("missing NCI xrefs should fall back to keyword search");
        assert_eq!(page.results.len(), 1);
        assert_eq!(page.results[0].nct_id, "NCT00000008");
    }

    #[tokio::test]
    async fn nci_search_page_falls_back_to_keyword_when_grounding_returns_no_hit() {
        let _lock = env_lock().await;
        let mydisease = MockServer::start().await;
        let nci = MockServer::start().await;
        let mydisease_base = format!("{}/v1", mydisease.uri());
        let _mydisease_base = set_env_var("BIOMCP_MYDISEASE_BASE", Some(&mydisease_base));
        let _nci_base = set_env_var("BIOMCP_NCI_CTS_BASE", Some(&nci.uri()));
        let _nci_key = set_env_var("NCI_API_KEY", Some("test-key"));

        Mock::given(method("GET"))
            .and(path("/v1/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "total": 0,
                "hits": []
            })))
            .mount(&mydisease)
            .await;

        mount_keyword_nci_search(&nci, "melanoma", "NCT00000009").await;

        let filters = TrialSearchFilters {
            source: TrialSource::NciCts,
            condition: Some("melanoma".into()),
            ..Default::default()
        };

        let page = search_page(&filters, 1, 0, None)
            .await
            .expect("missing MyDisease hits should fall back to keyword search");
        assert_eq!(page.results.len(), 1);
        assert_eq!(page.results[0].nct_id, "NCT00000009");
    }

    #[tokio::test]
    async fn nci_status_mapping_uses_documented_single_value_filters() {
        let _lock = env_lock().await;
        let nci = MockServer::start().await;
        let _nci_base = set_env_var("BIOMCP_NCI_CTS_BASE", Some(&nci.uri()));
        let _nci_key = set_env_var("NCI_API_KEY", Some("test-key"));

        let cases = [
            (
                "recruiting",
                "sites.recruitment_status",
                "ACTIVE",
                "NCT00000003",
            ),
            (
                "not yet recruiting",
                "current_trial_status",
                "Approved",
                "NCT00000004",
            ),
            (
                "enrolling by invitation",
                "current_trial_status",
                "Enrolling by Invitation",
                "NCT00000005",
            ),
            (
                "active, not recruiting",
                "sites.recruitment_status",
                "CLOSED_TO_ACCRUAL",
                "NCT00000006",
            ),
            (
                "completed",
                "current_trial_status",
                "Complete",
                "NCT00000007",
            ),
            (
                "suspended",
                "current_trial_status",
                "Temporarily Closed to Accrual",
                "NCT00000010",
            ),
            (
                "terminated",
                "current_trial_status",
                "INACTIVE",
                "NCT00000011",
            ),
            (
                "withdrawn",
                "current_trial_status",
                "Withdrawn",
                "NCT00000012",
            ),
        ];

        for &(_, query_key, query_value, nct_id) in &cases {
            match query_key {
                "current_trial_status" => {
                    Mock::given(method("GET"))
                        .and(path("/trials"))
                        .and(query_param("current_trial_status", query_value))
                        .and(query_param_is_missing("sites.recruitment_status"))
                        .and(query_param("size", "1"))
                        .and(query_param("from", "0"))
                        .respond_with(
                            ResponseTemplate::new(200).set_body_json(nci_search_response(nct_id)),
                        )
                        .expect(1)
                        .mount(&nci)
                        .await;
                }
                "sites.recruitment_status" => {
                    Mock::given(method("GET"))
                        .and(path("/trials"))
                        .and(query_param("sites.recruitment_status", query_value))
                        .and(query_param_is_missing("current_trial_status"))
                        .and(query_param("size", "1"))
                        .and(query_param("from", "0"))
                        .respond_with(
                            ResponseTemplate::new(200).set_body_json(nci_search_response(nct_id)),
                        )
                        .expect(1)
                        .mount(&nci)
                        .await;
                }
                other => panic!("unexpected query key: {other}"),
            }
        }

        for &(input, _, _, nct_id) in &cases {
            let filters = TrialSearchFilters {
                source: TrialSource::NciCts,
                status: Some(input.into()),
                ..Default::default()
            };

            let page = search_page(&filters, 1, 0, None)
                .await
                .unwrap_or_else(|_| panic!("{input} should map to a documented NCI status"));
            assert_eq!(page.results.len(), 1);
            assert_eq!(page.results[0].nct_id, nct_id);
        }
    }

    #[tokio::test]
    async fn nci_source_rejects_status_lists() {
        let _lock = env_lock().await;
        let nci = MockServer::start().await;
        let _nci_base = set_env_var("BIOMCP_NCI_CTS_BASE", Some(&nci.uri()));
        let _nci_key = set_env_var("NCI_API_KEY", Some("test-key"));

        Mock::given(method("GET"))
            .and(path("/trials"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(nci_search_response("NCT00000005")),
            )
            .mount(&nci)
            .await;

        let filters = TrialSearchFilters {
            source: TrialSource::NciCts,
            status: Some("recruiting,completed".into()),
            ..Default::default()
        };

        let err = search(&filters, 1, 0)
            .await
            .expect_err("NCI should reject comma-separated status lists");
        assert!(err.to_string().contains("one mapped status at a time"));
        assert!(err.to_string().contains("--source nci"));
    }

    #[tokio::test]
    async fn nci_phase_mapping_uses_i_ii_for_combined_phase() {
        let _lock = env_lock().await;
        let nci = MockServer::start().await;
        let _nci_base = set_env_var("BIOMCP_NCI_CTS_BASE", Some(&nci.uri()));
        let _nci_key = set_env_var("NCI_API_KEY", Some("test-key"));

        let cases = [
            ("1", "I", "NCT00000016"),
            ("2", "II", "NCT00000013"),
            ("3", "III", "NCT00000017"),
            ("4", "IV", "NCT00000018"),
            ("na", "NA", "NCT00000014"),
            ("1/2", "I_II", "NCT00000015"),
        ];

        for &(_, expected_phase, nct_id) in &cases {
            Mock::given(method("GET"))
                .and(path("/trials"))
                .and(query_param("phase", expected_phase))
                .and(query_param("size", "1"))
                .and(query_param("from", "0"))
                .respond_with(ResponseTemplate::new(200).set_body_json(nci_search_response(nct_id)))
                .expect(1)
                .mount(&nci)
                .await;
        }

        for &(input_phase, _, nct_id) in &cases {
            let filters = TrialSearchFilters {
                source: TrialSource::NciCts,
                phase: Some(input_phase.into()),
                ..Default::default()
            };

            let page = search_page(&filters, 1, 0, None)
                .await
                .unwrap_or_else(|_| panic!("{input_phase} should map to a documented NCI phase"));
            assert_eq!(page.results.len(), 1);
            assert_eq!(page.results[0].nct_id, nct_id);
        }
    }

    #[tokio::test]
    async fn nci_source_rejects_early_phase1() {
        let _lock = env_lock().await;
        let nci = MockServer::start().await;
        let _nci_base = set_env_var("BIOMCP_NCI_CTS_BASE", Some(&nci.uri()));
        let _nci_key = set_env_var("NCI_API_KEY", Some("test-key"));

        Mock::given(method("GET"))
            .and(path("/trials"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(nci_search_response("NCT00000007")),
            )
            .mount(&nci)
            .await;

        let filters = TrialSearchFilters {
            source: TrialSource::NciCts,
            phase: Some("early_phase1".into()),
            ..Default::default()
        };

        let err = search(&filters, 1, 0)
            .await
            .expect_err("NCI should reject early_phase1");
        assert!(err.to_string().contains("early_phase1"));
        assert!(err.to_string().contains("--source nci"));
    }

    #[tokio::test]
    async fn nci_source_rejects_essie_filters() {
        let filters = TrialSearchFilters {
            source: TrialSource::NciCts,
            prior_therapies: Some("platinum".into()),
            ..Default::default()
        };

        let err = search(&filters, 10, 0).await.expect_err("should fail");
        assert!(
            format!("{err}").contains("--prior-therapies, --progression-on, and --line-of-therapy"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn nci_source_rejects_age_filter() {
        let filters = TrialSearchFilters {
            source: TrialSource::NciCts,
            condition: Some("melanoma".into()),
            age: Some(67.0),
            ..Default::default()
        };

        let err = search(&filters, 10, 0).await.expect_err("should fail");
        assert!(
            format!("{err}").contains("--age is only supported for --source ctgov"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn nci_source_rejects_sex_filter() {
        let filters = TrialSearchFilters {
            source: TrialSource::NciCts,
            condition: Some("melanoma".into()),
            sex: Some("female".into()),
            ..Default::default()
        };

        let err = search(&filters, 10, 0).await.expect_err("should fail");
        assert!(
            format!("{err}").contains("--sex is only supported for --source ctgov"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn nci_source_rejects_sponsor_type_filter() {
        let filters = TrialSearchFilters {
            source: TrialSource::NciCts,
            condition: Some("melanoma".into()),
            sponsor_type: Some("nih".into()),
            ..Default::default()
        };

        let err = search(&filters, 10, 0).await.expect_err("should fail");
        assert!(
            format!("{err}").contains("--sponsor-type is only supported for --source ctgov"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn facility_geo_discards_mixed_site_false_positive() {
        let study = ctgov_study_fixture(json!([
            {
                "facility": "University Hospitals Cleveland Medical Center",
                "city": "Cleveland",
                "country": "United States",
                "geoPoint": { "lat": 40.7128, "lon": -74.0060 }
            },
            {
                "facility": "Cleveland Clinic Taussig Cancer Center",
                "city": "Cleveland",
                "country": "United States",
                "geoPoint": { "lat": 41.4993, "lon": -81.6944 }
            }
        ]));

        assert!(!trial_matches_facility_geo(
            &study,
            "university hospitals",
            41.4993,
            -81.6944,
            50
        ));
    }

    #[test]
    fn facility_geo_keeps_same_site_match() {
        let study = ctgov_study_fixture(json!([
            {
                "facility": "University Hospitals Cleveland Medical Center",
                "city": "Cleveland",
                "country": "United States",
                "geoPoint": { "lat": 41.5031, "lon": -81.6208 }
            }
        ]));

        assert!(trial_matches_facility_geo(
            &study,
            "university hospitals",
            41.4993,
            -81.6944,
            50
        ));
    }

    fn ctgov_search_study_fixture(nct_id: &str, min_age: &str, max_age: &str) -> serde_json::Value {
        json!({
            "protocolSection": {
                "identificationModule": {
                    "nctId": nct_id,
                    "briefTitle": format!("Trial {nct_id}")
                },
                "statusModule": {
                    "overallStatus": "RECRUITING"
                },
                "eligibilityModule": {
                    "minimumAge": min_age,
                    "maximumAge": max_age
                }
            }
        })
    }

    fn ctgov_eligibility_detail_fixture(nct_id: &str, criteria: &str) -> serde_json::Value {
        json!({
            "protocolSection": {
                "identificationModule": {
                    "nctId": nct_id
                },
                "eligibilityModule": {
                    "eligibilityCriteria": criteria
                }
            }
        })
    }

    fn age_filtered_ctgov_filters() -> TrialSearchFilters {
        TrialSearchFilters {
            condition: Some("melanoma".into()),
            status: Some("recruiting".into()),
            age: Some(51.0),
            ..Default::default()
        }
    }

    fn studies_with_age_matches(
        total: usize,
        eligible: usize,
        prefix: &str,
    ) -> Vec<serde_json::Value> {
        (0..total)
            .map(|index| {
                let nct_id = format!("NCT{prefix}{index:07}");
                if index < eligible {
                    ctgov_search_study_fixture(&nct_id, "18 Years", "75 Years")
                } else {
                    ctgov_search_study_fixture(&nct_id, "18 Years", "50 Years")
                }
            })
            .collect()
    }

    #[tokio::test]
    async fn age_filter_uses_native_total_semantics_across_limits() {
        let server = MockServer::start().await;
        let client = ClinicalTrialsClient::new_for_test(server.uri()).expect("client");

        Mock::given(method("GET"))
            .and(path("/studies"))
            .and(query_param("query.cond", "melanoma"))
            .and(query_param("filter.overallStatus", "RECRUITING"))
            .and(query_param("countTotal", "true"))
            .and(query_param("pageSize", "10"))
            .and(query_param_is_missing("pageToken"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "studies": studies_with_age_matches(100, 60, "10"),
                "nextPageToken": "p2",
                "totalCount": 200
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/studies"))
            .and(query_param("query.cond", "melanoma"))
            .and(query_param("filter.overallStatus", "RECRUITING"))
            .and(query_param("countTotal", "true"))
            .and(query_param("pageSize", "20"))
            .and(query_param_is_missing("pageToken"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "studies": studies_with_age_matches(100, 60, "20"),
                "nextPageToken": "p2",
                "totalCount": 200
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/studies"))
            .and(query_param("query.cond", "melanoma"))
            .and(query_param("filter.overallStatus", "RECRUITING"))
            .and(query_param("countTotal", "true"))
            .and(query_param("pageSize", "50"))
            .and(query_param_is_missing("pageToken"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "studies": studies_with_age_matches(100, 60, "50"),
                "nextPageToken": "p2",
                "totalCount": 200
            })))
            .mount(&server)
            .await;

        let filters = age_filtered_ctgov_filters();

        assert_eq!(
            search_page_with_ctgov_client(&client, &filters, 10, 0, None)
                .await
                .expect("page")
                .total,
            Some(200)
        );
        assert_eq!(
            search_page_with_ctgov_client(&client, &filters, 20, 0, None)
                .await
                .expect("page")
                .total,
            Some(200)
        );
        assert_eq!(
            search_page_with_ctgov_client(&client, &filters, 50, 0, None)
                .await
                .expect("page")
                .total,
            Some(200)
        );
    }

    #[tokio::test]
    async fn ctgov_cursor_preserves_next_page_token_after_offset_full_page_consumption() {
        let server = MockServer::start().await;
        let client = ClinicalTrialsClient::new_for_test(server.uri()).expect("client");

        Mock::given(method("GET"))
            .and(path("/studies"))
            .and(query_param("query.cond", "melanoma"))
            .and(query_param("filter.overallStatus", "RECRUITING"))
            .and(query_param("countTotal", "true"))
            .and(query_param("pageSize", "3"))
            .and(query_param_is_missing("pageToken"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "studies": studies_with_age_matches(3, 3, "21"),
                "nextPageToken": "p2",
                "totalCount": 10
            })))
            .expect(1)
            .mount(&server)
            .await;

        let page =
            search_page_with_ctgov_client(&client, &age_filtered_ctgov_filters(), 3, 1, None)
                .await
                .expect("page");

        assert_eq!(page.results.len(), 2);
        assert_eq!(page.next_page_token, Some("p2".into()));
    }

    #[tokio::test]
    async fn age_filter_total_returns_native_total_when_exhausted() {
        let server = MockServer::start().await;
        let client = ClinicalTrialsClient::new_for_test(server.uri()).expect("client");

        let page_one = studies_with_age_matches(10, 7, "31");
        let page_two = studies_with_age_matches(10, 5, "32");

        Mock::given(method("GET"))
            .and(path("/studies"))
            .and(query_param("query.cond", "melanoma"))
            .and(query_param("filter.overallStatus", "RECRUITING"))
            .and(query_param("countTotal", "true"))
            .and(query_param("pageSize", "10"))
            .and(query_param_is_missing("pageToken"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "studies": page_one,
                "nextPageToken": "p2",
                "totalCount": 20
            })))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/studies"))
            .and(query_param("query.cond", "melanoma"))
            .and(query_param("filter.overallStatus", "RECRUITING"))
            .and(query_param("countTotal", "true"))
            .and(query_param("pageSize", "10"))
            .and(query_param("pageToken", "p2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "studies": page_two,
                "nextPageToken": null,
                "totalCount": 20
            })))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/studies"))
            .and(query_param("query.cond", "melanoma"))
            .and(query_param("filter.overallStatus", "RECRUITING"))
            .and(query_param("countTotal", "true"))
            .and(query_param("pageSize", "50"))
            .and(query_param_is_missing("pageToken"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "studies": studies_with_age_matches(10, 7, "41"),
                "nextPageToken": "p2",
                "totalCount": 20
            })))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/studies"))
            .and(query_param("query.cond", "melanoma"))
            .and(query_param("filter.overallStatus", "RECRUITING"))
            .and(query_param("countTotal", "true"))
            .and(query_param("pageSize", "50"))
            .and(query_param("pageToken", "p2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "studies": studies_with_age_matches(10, 5, "42"),
                "nextPageToken": null,
                "totalCount": 20
            })))
            .expect(1)
            .mount(&server)
            .await;

        let filters = age_filtered_ctgov_filters();

        assert_eq!(
            search_page_with_ctgov_client(&client, &filters, 10, 0, None)
                .await
                .expect("page")
                .total,
            Some(20)
        );
        assert_eq!(
            search_page_with_ctgov_client(&client, &filters, 50, 0, None)
                .await
                .expect("page")
                .total,
            Some(20)
        );
    }

    #[tokio::test]
    async fn count_all_returns_approximate_for_age_only_filters() {
        let server = MockServer::start().await;
        let client = ClinicalTrialsClient::new_for_test(server.uri()).expect("client");

        Mock::given(method("GET"))
            .and(path("/studies"))
            .and(query_param("query.cond", "melanoma"))
            .and(query_param("filter.overallStatus", "RECRUITING"))
            .and(query_param("countTotal", "true"))
            .and(query_param("pageSize", "1"))
            .and(query_param_is_missing("pageToken"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "studies": [],
                "nextPageToken": null,
                "totalCount": 250
            })))
            .expect(1)
            .mount(&server)
            .await;

        let filters = age_filtered_ctgov_filters();
        assert_eq!(
            count_all_with_ctgov_client(&client, &filters)
                .await
                .expect("count"),
            TrialCount::Approximate(250)
        );
    }

    #[tokio::test]
    async fn count_all_returns_exact_for_no_post_filters() {
        let server = MockServer::start().await;
        let client = ClinicalTrialsClient::new_for_test(server.uri()).expect("client");

        Mock::given(method("GET"))
            .and(path("/studies"))
            .and(query_param("query.cond", "melanoma"))
            .and(query_param("filter.overallStatus", "RECRUITING"))
            .and(query_param("countTotal", "true"))
            .and(query_param("pageSize", "1"))
            .and(query_param_is_missing("pageToken"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "studies": [],
                "nextPageToken": null,
                "totalCount": 494
            })))
            .expect(1)
            .mount(&server)
            .await;

        let filters = TrialSearchFilters {
            condition: Some("melanoma".into()),
            status: Some("recruiting".into()),
            ..Default::default()
        };

        assert_eq!(
            count_all_with_ctgov_client(&client, &filters)
                .await
                .expect("count"),
            TrialCount::Exact(494)
        );
    }

    #[tokio::test]
    async fn count_all_returns_unknown_when_expensive_post_filter_hits_page_cap() {
        let server = MockServer::start().await;
        let client = ClinicalTrialsClient::new_for_test(server.uri()).expect("client");

        Mock::given(method("GET"))
            .and(path("/studies"))
            .and(query_param("query.cond", "melanoma"))
            .and(query_param("countTotal", "true"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "studies": [ctgov_search_study_fixture("NCT99999999", "18 Years", "75 Years")],
                "nextPageToken": "still-more",
                "totalCount": 60000
            })))
            .expect(50)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/studies/NCT99999999"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                ctgov_eligibility_detail_fixture(
                    "NCT99999999",
                    "Inclusion Criteria:\nMust have mismatch repair deficient disease",
                ),
            ))
            .expect(50)
            .mount(&server)
            .await;

        let filters = TrialSearchFilters {
            condition: Some("melanoma".into()),
            criteria: Some("mismatch repair deficient".into()),
            ..Default::default()
        };

        assert_eq!(
            count_all_with_ctgov_client(&client, &filters)
                .await
                .expect("count"),
            TrialCount::Unknown
        );
    }
}
