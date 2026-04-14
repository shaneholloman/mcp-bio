//! CTGov trial search query, pagination, and count helpers.

use crate::entities::SearchPage;
use crate::error::BioMcpError;
use crate::sources::clinicaltrials::{ClinicalTrialsClient, CtGovSearchParams, CtGovStudy};
use crate::transform;
use crate::utils::date::validate_since;

use super::super::{TrialCount, TrialSearchFilters, TrialSearchResult, TrialSource};
use super::{
    CtGovSearchContext, build_essie_fragments, essie_escape, essie_escape_boolean_expression,
    normalize_intervention_query, normalize_sex, normalize_sponsor_type,
    prepare_ctgov_search_context, sort_trials_by_status_priority, validate_search_page_args,
    validate_trial_search, verify_age_eligibility, verify_eligibility_criteria,
    verify_facility_geo,
};

pub(super) const CTGOV_COUNT_PAGE_SIZE: usize = 1000;
const CTGOV_MAX_PAGE_FETCHES: usize = 20;
const COUNT_TRAVERSAL_PAGE_CAP: usize = 50;

pub(super) fn ctgov_agg_filters(
    filters: &TrialSearchFilters,
) -> Result<Option<String>, BioMcpError> {
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

pub(super) fn validate_location(filters: &TrialSearchFilters) -> Result<(), BioMcpError> {
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

pub(super) fn ctgov_query_term(
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

pub(super) async fn search_page_with_ctgov_client(
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

pub(super) async fn count_all_with_ctgov_client(
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

#[cfg(test)]
mod tests;
