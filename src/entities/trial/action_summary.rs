use std::collections::HashSet;

use serde::Serialize;

use crate::error::BioMcpError;

use super::{
    Trial, TrialContact, TrialEligibility, TrialLocation, TrialSearchFilters, TrialSource, get,
    search_page,
};

#[derive(Debug, Clone, Default)]
pub struct ActionSummaryHints {
    pub facility: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub distance: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrialActionSummary {
    pub results: Vec<TrialActionSummaryItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrialActionSummaryItem {
    pub nct_id: String,
    pub title: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trial_type: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub access_caveats: Vec<TrialAccessCaveat>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ranked_sites: Vec<RankedTrialSite>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contacts: Vec<TrialContact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eligibility: Option<TrialEligibility>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub eligibility_snippets: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RankedTrialSite {
    pub facility: String,
    pub city: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    pub country: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    pub match_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_facility: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrialAccessCaveat {
    pub kind: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

pub async fn action_summary(
    filters: &TrialSearchFilters,
    hints: ActionSummaryHints,
    limit: usize,
    offset: usize,
) -> Result<TrialActionSummary, BioMcpError> {
    let mut candidate_filters = filters.clone();
    candidate_filters.facility = None;
    candidate_filters.lat = None;
    candidate_filters.lon = None;
    candidate_filters.distance = None;
    candidate_filters.source = TrialSource::ClinicalTrialsGov;

    let page = search_page(&candidate_filters, limit, offset, None).await?;
    let mut seen = HashSet::new();
    let mut items = Vec::new();
    let detail_sections = [
        "eligibility".to_string(),
        "contacts".to_string(),
        "locations".to_string(),
        "arms".to_string(),
        "outcomes".to_string(),
    ];

    for hit in page.results {
        if !seen.insert(hit.nct_id.to_ascii_uppercase()) {
            continue;
        }
        let trial = get(
            &hit.nct_id,
            &detail_sections,
            TrialSource::ClinicalTrialsGov,
        )
        .await?;
        items.push(summarize_trial(trial, &hints));
    }

    Ok(TrialActionSummary { results: items })
}

fn summarize_trial(trial: Trial, hints: &ActionSummaryHints) -> TrialActionSummaryItem {
    let trial_type = classify_trial_type(&trial);
    let access_caveats = classify_access_caveats(&trial);
    let ranked_sites = rank_sites(trial.locations.as_deref().unwrap_or(&[]), hints);
    let eligibility_snippets = eligibility_snippets(trial.eligibility_text.as_deref());

    TrialActionSummaryItem {
        nct_id: trial.nct_id,
        title: trial.title,
        status: trial.status,
        trial_type,
        access_caveats,
        ranked_sites,
        contacts: trial.contacts.unwrap_or_default(),
        eligibility: trial.eligibility,
        eligibility_snippets,
    }
}

fn haystack(trial: &Trial) -> String {
    let mut parts = vec![trial.title.as_str()];
    if let Some(summary) = trial.summary.as_deref() {
        parts.push(summary);
    }
    if let Some(text) = trial.eligibility_text.as_deref() {
        parts.push(text);
    }
    if let Some(arms) = trial.arms.as_deref() {
        for arm in arms {
            parts.push(&arm.label);
            if let Some(description) = arm.description.as_deref() {
                parts.push(description);
            }
        }
    }
    for intervention in &trial.intervention_details {
        parts.push(&intervention.name);
        if let Some(description) = intervention.description.as_deref() {
            parts.push(description);
        }
    }
    parts.join("\n").to_ascii_lowercase()
}

fn classify_trial_type(trial: &Trial) -> Option<String> {
    haystack(trial)
        .contains("open-label extension")
        .then(|| "open_label_extension".to_string())
}

fn classify_access_caveats(trial: &Trial) -> Vec<TrialAccessCaveat> {
    let text = haystack(trial);
    if text.contains("antecedent") && (text.contains("completed") || text.contains("prior")) {
        vec![TrialAccessCaveat {
            kind: "antecedent_study_required".to_string(),
            label: "Antecedent study required".to_string(),
            evidence: trial
                .eligibility_text
                .clone()
                .or_else(|| trial.summary.clone()),
        }]
    } else {
        Vec::new()
    }
}

fn rank_sites(locations: &[TrialLocation], hints: &ActionSummaryHints) -> Vec<RankedTrialSite> {
    let requested = hints
        .facility
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let requested_lower = requested.map(str::to_ascii_lowercase);
    let has_facility_match = requested_lower.as_deref().is_some_and(|needle| {
        locations
            .iter()
            .any(|location| location.facility.to_ascii_lowercase().contains(needle))
    });

    let mut sites = locations
        .iter()
        .map(|location| {
            let facility_matches = requested_lower
                .as_deref()
                .is_some_and(|needle| location.facility.to_ascii_lowercase().contains(needle));
            let match_status = if requested.is_some() && !has_facility_match {
                "no_listed_facility_match"
            } else if facility_matches {
                "listed_facility_match"
            } else if hints.lat.is_some() || hints.lon.is_some() || hints.distance.is_some() {
                "listed_site_coordinates_not_ranked"
            } else {
                "listed_site"
            };
            RankedTrialSite {
                facility: location.facility.clone(),
                city: location.city.clone(),
                state: location.state.clone(),
                country: location.country.clone(),
                status: location.status.clone(),
                match_status: match_status.to_string(),
                requested_facility: requested.map(str::to_string),
            }
        })
        .collect::<Vec<_>>();

    sites.sort_by_key(|site| match site.match_status.as_str() {
        "listed_facility_match" => 0,
        _ => 1,
    });
    sites
}

fn eligibility_snippets(text: Option<&str>) -> Vec<String> {
    text.into_iter()
        .flat_map(|value| value.split('.'))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .take(3)
        .map(str::to_string)
        .collect()
}

pub fn trial_type_label(value: &str) -> &str {
    match value {
        "open_label_extension" => "Open-label extension",
        _ => value,
    }
}
