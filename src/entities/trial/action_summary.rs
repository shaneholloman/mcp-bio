use std::collections::HashSet;

use serde::Serialize;

use crate::error::BioMcpError;

use super::{
    Trial, TrialContact, TrialEligibility, TrialLocation, TrialSearchFilters, TrialSource, get,
    search::haversine_miles, search_page,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance_miles: Option<f64>,
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
    validate_geo_hints(&hints)?;

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
            let distance_miles = location_distance_miles(location, hints);
            let match_status = if requested.is_some() && !has_facility_match {
                "no_listed_facility_match"
            } else if facility_matches {
                "listed_facility_match"
            } else if hints.lat.is_some() {
                if distance_miles.is_some() {
                    "listed_geo_ranked"
                } else {
                    "listed_site_missing_coordinates"
                }
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
                distance_miles,
            }
        })
        .collect::<Vec<_>>();

    sites.sort_by(|a, b| {
        site_rank(a)
            .cmp(&site_rank(b))
            .then_with(|| match (a.distance_miles, b.distance_miles) {
                (Some(left), Some(right)) => left.total_cmp(&right),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            })
    });
    sites
}

fn site_rank(site: &RankedTrialSite) -> u8 {
    match site.match_status.as_str() {
        "listed_facility_match" => 0,
        "listed_geo_ranked" => 1,
        _ => 2,
    }
}

fn validate_geo_hints(hints: &ActionSummaryHints) -> Result<(), BioMcpError> {
    let has_lat = hints.lat.is_some();
    let has_lon = hints.lon.is_some();
    let has_distance = hints.distance.is_some();
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

fn location_distance_miles(location: &TrialLocation, hints: &ActionSummaryHints) -> Option<f64> {
    let (Some(origin_lat), Some(origin_lon), Some(site_lat), Some(site_lon)) =
        (hints.lat, hints.lon, location.latitude, location.longitude)
    else {
        return None;
    };
    Some(haversine_miles(origin_lat, origin_lon, site_lat, site_lon))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn location(facility: &str, lat: Option<f64>, lon: Option<f64>) -> TrialLocation {
        TrialLocation {
            facility: facility.to_string(),
            city: facility.to_string(),
            state: None,
            country: "United States".to_string(),
            status: Some("RECRUITING".to_string()),
            contact_name: None,
            contact_role: None,
            contact_phone: None,
            contact_email: None,
            latitude: lat,
            longitude: lon,
        }
    }

    #[test]
    fn geo_hints_rank_listed_sites_by_distance() {
        let sites = rank_sites(
            &[
                location("Far Site", Some(41.8781), Some(-87.6298)),
                location("Near Site", Some(42.2808), Some(-83.7430)),
            ],
            &ActionSummaryHints {
                lat: Some(42.2808),
                lon: Some(-83.7430),
                distance: Some(300),
                ..ActionSummaryHints::default()
            },
        );

        assert_eq!(sites[0].facility, "Near Site");
        assert_eq!(sites[0].match_status, "listed_geo_ranked");
        assert!(sites[0].distance_miles.unwrap_or(f64::INFINITY) < 1.0);
    }

    #[test]
    fn geo_hints_mark_missing_coordinates() {
        let sites = rank_sites(
            &[location("Unmapped Site", None, None)],
            &ActionSummaryHints {
                lat: Some(42.2808),
                lon: Some(-83.7430),
                distance: Some(300),
                ..ActionSummaryHints::default()
            },
        );

        assert_eq!(sites[0].match_status, "listed_site_missing_coordinates");
        assert!(sites[0].distance_miles.is_none());
    }

    #[test]
    fn action_summary_geo_hints_keep_existing_validation_rules() {
        let err = validate_geo_hints(&ActionSummaryHints {
            lat: Some(42.2808),
            ..ActionSummaryHints::default()
        })
        .expect_err("lat without lon/distance should be rejected");

        assert!(err.to_string().contains("--lat/--lon requires --distance"));
    }
}
