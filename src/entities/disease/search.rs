//! Disease search, phenotype search, and search-only match helpers.

use super::*;

use super::associations::normalize_hpo_id;
use super::resolution::{rerank_disease_search_hits, resolver_queries};

pub(super) const MAX_DISEASE_SEARCH_LIMIT: usize = 50;

fn inheritance_matches(hit: &crate::sources::mydisease::MyDiseaseHit, expected: &str) -> bool {
    let needle = expected.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return true;
    }
    hit.hpo
        .as_ref()
        .map(|hpo| {
            hpo.inheritance.iter().any(|row| {
                row.hpo_name
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|v| v.to_ascii_lowercase().contains(&needle))
                    || row
                        .hpo_id
                        .as_deref()
                        .map(str::trim)
                        .is_some_and(|v| v.to_ascii_lowercase().contains(&needle))
            })
        })
        .unwrap_or(false)
}

fn phenotype_matches(hit: &crate::sources::mydisease::MyDiseaseHit, expected: &str) -> bool {
    let needle = expected.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return true;
    }
    hit.hpo
        .as_ref()
        .map(|hpo| {
            hpo.phenotype_related_to_disease.iter().any(|row| {
                row.hpo_id
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|v| v.to_ascii_lowercase().contains(&needle))
            })
        })
        .unwrap_or(false)
}

fn onset_matches(hit: &crate::sources::mydisease::MyDiseaseHit, expected: &str) -> bool {
    let needle = expected.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return true;
    }
    hit.hpo
        .as_ref()
        .map(|hpo| {
            hpo.clinical_course.iter().any(|row| {
                row.hpo_name
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|v| v.to_ascii_lowercase().contains(&needle))
            })
        })
        .unwrap_or(false)
}

#[allow(dead_code)]
pub async fn search(
    filters: &DiseaseSearchFilters,
    limit: usize,
) -> Result<Vec<DiseaseSearchResult>, BioMcpError> {
    Ok(search_page(filters, limit, 0).await?.results)
}

pub async fn search_page(
    filters: &DiseaseSearchFilters,
    limit: usize,
    offset: usize,
) -> Result<SearchPage<DiseaseSearchResult>, BioMcpError> {
    if limit == 0 || limit > MAX_DISEASE_SEARCH_LIMIT {
        return Err(BioMcpError::InvalidArgument(format!(
            "--limit must be between 1 and {MAX_DISEASE_SEARCH_LIMIT}"
        )));
    }

    let query = filters
        .query
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| {
            BioMcpError::InvalidArgument(
                "Query is required. Example: biomcp search disease -q melanoma".into(),
            )
        })?;

    let inheritance = filters
        .inheritance
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let phenotype = filters
        .phenotype
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let onset = filters
        .onset
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());

    let client = MyDiseaseClient::new()?;
    let needed = limit.saturating_add(offset).max(limit);
    let fetch_size = if needed >= 50 {
        needed
    } else {
        (needed.saturating_mul(5)).clamp(needed, 50)
    };
    let prefer_doid = filters
        .source
        .as_deref()
        .map(str::trim)
        .is_some_and(|s| s.eq_ignore_ascii_case("doid"));

    let mut merged_total = 0usize;
    let mut query_hits = Vec::new();
    for (query_idx, resolved_query) in resolver_queries(query).into_iter().enumerate() {
        let resp = client
            .query(
                &resolved_query,
                fetch_size,
                0,
                filters.source.as_deref(),
                inheritance,
                phenotype,
                onset,
            )
            .await?;
        merged_total = merged_total.max(resp.total);
        let hits = resp
            .hits
            .into_iter()
            .filter(|hit| {
                inheritance.is_none_or(|value| inheritance_matches(hit, value))
                    && phenotype.is_none_or(|value| phenotype_matches(hit, value))
                    && onset.is_none_or(|value| onset_matches(hit, value))
            })
            .collect::<Vec<_>>();
        query_hits.push((query_idx, hits));
    }

    let ranked_hits = rerank_disease_search_hits(query, query_hits);
    let total = Some(merged_total.max(ranked_hits.len()));
    let results = ranked_hits
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|hit| {
            let mut row = transform::disease::from_mydisease_search_hit(&hit);
            if prefer_doid && let Some(doid) = transform::disease::doid_from_mydisease_hit(&hit) {
                row.id = doid;
            }
            row
        })
        .collect::<Vec<_>>();

    Ok(SearchPage::offset(results, total))
}

pub fn search_query_summary(filters: &DiseaseSearchFilters) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(v) = filters
        .query
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        parts.push(v.to_string());
    }
    if let Some(v) = filters
        .source
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        parts.push(format!("source={v}"));
    }
    if let Some(v) = filters
        .inheritance
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        parts.push(format!("inheritance={v}"));
    }
    if let Some(v) = filters
        .phenotype
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        parts.push(format!("phenotype={v}"));
    }
    if let Some(v) = filters
        .onset
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        parts.push(format!("onset={v}"));
    }
    parts.join(", ")
}

const PHENOTYPE_QUERY_EXAMPLES: &str = "Examples: biomcp search phenotype \"HP:0001250 HP:0001263\" or biomcp search phenotype \"seizure, developmental delay\"";

fn phenotype_query_required_error() -> BioMcpError {
    BioMcpError::InvalidArgument(format!(
        "Phenotype terms are required. Use HPO IDs or symptom phrases. {PHENOTYPE_QUERY_EXAMPLES}"
    ))
}

fn phenotype_query_no_match_error(raw: &str) -> BioMcpError {
    BioMcpError::InvalidArgument(format!(
        "No HPO terms matched query: {raw}. Try HPO IDs like HP:0001250 or refine the symptom phrases."
    ))
}

fn parse_hpo_query_terms(raw: &str) -> Result<Vec<String>, BioMcpError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(phenotype_query_required_error());
    }

    let mut terms = Vec::new();
    let mut seen = HashSet::new();
    for token in raw
        .split(|c: char| c.is_whitespace() || c == ',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let Some(id) = normalize_hpo_id(token) else {
            return Err(BioMcpError::InvalidArgument(format!(
                "Invalid HPO term: {token}. Expected format HP:0001250"
            )));
        };
        if seen.insert(id.clone()) {
            terms.push(id);
        }
    }

    if terms.is_empty() {
        return Err(phenotype_query_required_error());
    }

    Ok(terms)
}

fn split_phenotype_queries(raw: &str) -> Vec<String> {
    let mut queries = raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if queries.is_empty() {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            queries.push(trimmed.to_string());
        }
    }
    queries
}

async fn resolve_phenotype_query_terms(raw: &str) -> Result<Vec<String>, BioMcpError> {
    const MAX_RESOLVED_TERMS: usize = 10;

    let raw = raw.trim();
    if raw.is_empty() {
        return Err(phenotype_query_required_error());
    }

    if let Ok(terms) = parse_hpo_query_terms(raw) {
        return Ok(terms);
    }

    let queries = split_phenotype_queries(raw);
    if queries.is_empty() {
        return Err(phenotype_query_required_error());
    }

    let hpo = HpoClient::new()?;
    let mut resolved = Vec::new();
    let mut seen = HashSet::new();
    for query in queries {
        let ids = hpo.search_term_ids(&query, MAX_RESOLVED_TERMS).await?;
        for id in ids {
            if seen.insert(id.clone()) {
                resolved.push(id);
                if resolved.len() >= MAX_RESOLVED_TERMS {
                    return Ok(resolved);
                }
            }
        }
    }

    if resolved.is_empty() {
        return Err(phenotype_query_no_match_error(raw));
    }

    Ok(resolved)
}

#[allow(dead_code)]
pub async fn search_phenotype(
    hpo_terms: &str,
    limit: usize,
) -> Result<Vec<PhenotypeSearchResult>, BioMcpError> {
    Ok(search_phenotype_page(hpo_terms, limit, 0).await?.results)
}

pub async fn search_phenotype_page(
    hpo_terms: &str,
    limit: usize,
    offset: usize,
) -> Result<SearchPage<PhenotypeSearchResult>, BioMcpError> {
    const MAX_SEARCH_LIMIT: usize = 50;
    if limit == 0 || limit > MAX_SEARCH_LIMIT {
        return Err(BioMcpError::InvalidArgument(format!(
            "--limit must be between 1 and {MAX_SEARCH_LIMIT}"
        )));
    }

    let terms = resolve_phenotype_query_terms(hpo_terms).await?;
    let client = MonarchClient::new()?;
    let fetch_limit = limit.saturating_add(offset).max(limit);
    let mut rows = client
        .phenotype_similarity_search(&terms, fetch_limit)
        .await?;
    rows.sort_by(|a, b| b.score.total_cmp(&a.score));
    let total = rows.len();
    rows.truncate(fetch_limit);

    Ok(SearchPage::offset(
        rows.into_iter()
            .skip(offset)
            .take(limit)
            .map(
                |MonarchPhenotypeMatch {
                     disease_id,
                     disease_name,
                     score,
                 }| PhenotypeSearchResult {
                    disease_id,
                    disease_name,
                    score,
                },
            )
            .collect(),
        Some(total),
    ))
}

#[cfg(test)]
mod tests;
