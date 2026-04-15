//! CTGov trial search query, pagination, and count helpers.

use std::collections::{HashMap, HashSet};

use futures::future::join_all;

use crate::entities::SearchPage;
use crate::entities::drug::resolve_trial_aliases;
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
    intervention_query: Option<&str>,
    page_token: Option<String>,
    page_size: usize,
) -> CtGovSearchParams {
    CtGovSearchParams {
        condition: filters.condition.clone(),
        intervention: intervention_query.map(normalize_intervention_query),
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

#[derive(Debug)]
struct CtGovFilteredPage {
    total_count: Option<usize>,
    studies: Vec<CtGovStudy>,
    next_page_token: Option<String>,
    raw_study_count: usize,
}

#[derive(Debug, Clone)]
struct AliasWorkerState {
    alias_query: String,
    next_page_token: Option<String>,
    exhausted: bool,
    pages_fetched: usize,
}

fn raw_intervention_query(filters: &TrialSearchFilters) -> Option<&str> {
    filters
        .intervention
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

async fn resolve_ctgov_intervention_aliases(
    filters: &TrialSearchFilters,
) -> Result<Vec<String>, BioMcpError> {
    if !matches!(filters.source, TrialSource::ClinicalTrialsGov) || filters.no_alias_expand {
        return Ok(raw_intervention_query(filters)
            .map(|value| vec![value.to_string()])
            .unwrap_or_default());
    }

    let Some(intervention_query) = raw_intervention_query(filters) else {
        return Ok(Vec::new());
    };

    resolve_trial_aliases(intervention_query).await
}

fn alias_expansion_next_page_error() -> BioMcpError {
    BioMcpError::InvalidArgument(
        "--next-page is not supported when intervention alias expansion uses multiple queries; use --offset or --no-alias-expand"
            .into(),
    )
}

async fn fetch_ctgov_filtered_page(
    client: &ClinicalTrialsClient,
    filters: &TrialSearchFilters,
    context: &CtGovSearchContext,
    intervention_query: Option<&str>,
    page_token: Option<String>,
    page_size: usize,
) -> Result<CtGovFilteredPage, BioMcpError> {
    let resp = client
        .search(&build_ctgov_search_params(
            filters,
            context,
            intervention_query,
            page_token,
            page_size,
        ))
        .await?;

    let raw_study_count = resp.studies.len();
    let studies = if raw_study_count == 0 {
        Vec::new()
    } else {
        apply_ctgov_post_filters(client, filters, context, resp.studies).await
    };

    Ok(CtGovFilteredPage {
        total_count: resp.total_count.map(|value| value as usize),
        studies,
        next_page_token: resp.next_page_token,
        raw_study_count,
    })
}

async fn search_page_with_single_ctgov_intervention(
    client: &ClinicalTrialsClient,
    filters: &TrialSearchFilters,
    context: &CtGovSearchContext,
    intervention_query: Option<&str>,
    limit: usize,
    offset: usize,
    next_page: Option<String>,
) -> Result<SearchPage<TrialSearchResult>, BioMcpError> {
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
        let page = fetch_ctgov_filtered_page(
            client,
            filters,
            context,
            intervention_query,
            page_token.clone(),
            page_size,
        )
        .await?;

        if total.is_none() {
            total = page.total_count;
        }

        if page.raw_study_count == 0 {
            exhausted = true;
            break;
        }

        let next_page_token = page.next_page_token;
        let mut studies = page.studies;
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

async fn search_page_with_ctgov_alias_union(
    client: &ClinicalTrialsClient,
    filters: &TrialSearchFilters,
    context: &CtGovSearchContext,
    aliases: &[String],
    limit: usize,
    offset: usize,
) -> Result<SearchPage<TrialSearchResult>, BioMcpError> {
    let page_size = offset.saturating_add(limit).clamp(1, 100);
    let mut workers: Vec<AliasWorkerState> = aliases
        .iter()
        .cloned()
        .map(|alias_query| AliasWorkerState {
            alias_query,
            next_page_token: None,
            exhausted: false,
            pages_fetched: 0,
        })
        .collect();
    let mut merged_rows: Vec<TrialSearchResult> = Vec::new();
    let mut merged_index: HashMap<String, usize> = HashMap::new();
    let mut traversal_capped = false;

    loop {
        let active_indices: Vec<usize> = workers
            .iter()
            .enumerate()
            .filter_map(|(index, worker)| (!worker.exhausted).then_some(index))
            .collect();
        if active_indices.is_empty() {
            break;
        }

        let pages = join_all(active_indices.iter().map(|index| {
            let worker = &workers[*index];
            fetch_ctgov_filtered_page(
                client,
                filters,
                context,
                Some(worker.alias_query.as_str()),
                worker.next_page_token.clone(),
                page_size,
            )
        }))
        .await;

        for (index, page_result) in active_indices.into_iter().zip(pages) {
            let page = page_result?;
            let worker = &mut workers[index];
            worker.pages_fetched += 1;

            if page.raw_study_count == 0 {
                worker.exhausted = true;
                worker.next_page_token = page.next_page_token;
                continue;
            }

            for study in page.studies {
                let mut row = transform::trial::from_ctgov_hit(&study);
                if merged_index.contains_key(&row.nct_id) {
                    continue;
                }
                row.matched_intervention_label = Some(worker.alias_query.clone());
                merged_index.insert(row.nct_id.clone(), merged_rows.len());
                merged_rows.push(row);
            }

            worker.next_page_token = page.next_page_token;
            if worker.next_page_token.is_none() {
                worker.exhausted = true;
                continue;
            }
            if worker.pages_fetched >= CTGOV_MAX_PAGE_FETCHES {
                worker.exhausted = true;
                traversal_capped = true;
            }
        }

        if merged_rows.len() >= offset.saturating_add(limit) {
            break;
        }
    }

    if !context.has_explicit_status {
        sort_trials_by_status_priority(&mut merged_rows);
    }

    let total = if traversal_capped
        || workers
            .iter()
            .any(|worker| !worker.exhausted || worker.next_page_token.is_some())
    {
        None
    } else {
        Some(merged_rows.len())
    };

    let rows = merged_rows.into_iter().skip(offset).take(limit).collect();
    Ok(SearchPage::cursor(rows, total, None))
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
    let aliases = resolve_ctgov_intervention_aliases(filters).await?;

    if aliases.len() > 1 {
        if next_page
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
        {
            return Err(alias_expansion_next_page_error());
        }
        return search_page_with_ctgov_alias_union(
            client, filters, &context, &aliases, limit, offset,
        )
        .await;
    }

    search_page_with_single_ctgov_intervention(
        client,
        filters,
        &context,
        raw_intervention_query(filters),
        limit,
        offset,
        next_page,
    )
    .await
}

async fn count_all_with_ctgov_alias_union(
    client: &ClinicalTrialsClient,
    filters: &TrialSearchFilters,
    context: &CtGovSearchContext,
    aliases: &[String],
) -> Result<TrialCount, BioMcpError> {
    let mut workers: Vec<AliasWorkerState> = aliases
        .iter()
        .cloned()
        .map(|alias_query| AliasWorkerState {
            alias_query,
            next_page_token: None,
            exhausted: false,
            pages_fetched: 0,
        })
        .collect();
    let mut unique_nct_ids: HashSet<String> = HashSet::new();
    let mut fetched_pages = 0usize;

    loop {
        let active_indices: Vec<usize> = workers
            .iter()
            .enumerate()
            .filter_map(|(index, worker)| (!worker.exhausted).then_some(index))
            .collect();
        if active_indices.is_empty() {
            return Ok(TrialCount::Exact(unique_nct_ids.len()));
        }

        if fetched_pages.saturating_add(active_indices.len()) > COUNT_TRAVERSAL_PAGE_CAP {
            return Ok(TrialCount::Unknown);
        }

        let pages = join_all(active_indices.iter().map(|index| {
            let worker = &workers[*index];
            fetch_ctgov_filtered_page(
                client,
                filters,
                context,
                Some(worker.alias_query.as_str()),
                worker.next_page_token.clone(),
                CTGOV_COUNT_PAGE_SIZE,
            )
        }))
        .await;
        fetched_pages = fetched_pages.saturating_add(active_indices.len());

        for (index, page_result) in active_indices.into_iter().zip(pages) {
            let page = page_result?;
            let worker = &mut workers[index];
            worker.pages_fetched += 1;

            if page.raw_study_count == 0 {
                worker.exhausted = true;
                worker.next_page_token = page.next_page_token;
                continue;
            }

            for study in page.studies {
                let row = transform::trial::from_ctgov_hit(&study);
                unique_nct_ids.insert(row.nct_id);
            }

            worker.next_page_token = page.next_page_token;
            if worker.next_page_token.is_none() {
                worker.exhausted = true;
            }
        }
    }
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
    let aliases = resolve_ctgov_intervention_aliases(filters).await?;

    if aliases.len() > 1 {
        return count_all_with_ctgov_alias_union(client, filters, &context, &aliases).await;
    }

    if !context.uses_expensive_post_filters {
        let resp = client
            .search(&build_ctgov_search_params(
                filters,
                &context,
                raw_intervention_query(filters),
                None,
                1,
            ))
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
                raw_intervention_query(filters),
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
