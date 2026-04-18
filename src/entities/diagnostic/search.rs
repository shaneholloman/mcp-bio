use crate::entities::SearchPage;
use crate::error::BioMcpError;
use crate::sources::gtr::{GtrClient, GtrIndex, GtrRecord, GtrSyncMode};

use super::{DiagnosticSearchFilters, DiagnosticSearchResult, search_result};

const MAX_SEARCH_LIMIT: usize = 50;
const ZERO_FILTER_ERROR: &str =
    "diagnostic search requires at least one of --gene, --disease, --type, or --manufacturer";

#[derive(Debug, Clone)]
struct NormalizedSearchFilters {
    gene: Option<String>,
    disease: Option<String>,
    test_type: Option<String>,
    manufacturer: Option<String>,
}

impl NormalizedSearchFilters {
    fn from_filters(filters: &DiagnosticSearchFilters) -> Result<Self, BioMcpError> {
        let normalized = Self {
            gene: normalized_exact(filters.gene.as_deref()),
            disease: normalized_contains(filters.disease.as_deref()),
            test_type: normalized_exact(filters.test_type.as_deref()),
            manufacturer: normalized_contains(filters.manufacturer.as_deref()),
        };

        if normalized.gene.is_none()
            && normalized.disease.is_none()
            && normalized.test_type.is_none()
            && normalized.manufacturer.is_none()
        {
            return Err(BioMcpError::InvalidArgument(ZERO_FILTER_ERROR.to_string()));
        }

        Ok(normalized)
    }

    fn matches(&self, record: &GtrRecord, index: &GtrIndex) -> bool {
        if let Some(gene) = self.gene.as_deref()
            && !index
                .merged_genes(&record.accession)
                .iter()
                .any(|candidate| candidate.trim().eq_ignore_ascii_case(gene))
        {
            return false;
        }

        if let Some(disease) = self.disease.as_deref()
            && !index
                .conditions(&record.accession)
                .iter()
                .any(|candidate| candidate.to_ascii_lowercase().contains(disease))
        {
            return false;
        }

        if let Some(test_type) = self.test_type.as_deref()
            && !record.test_type.trim().eq_ignore_ascii_case(test_type)
        {
            return false;
        }

        if let Some(manufacturer) = self.manufacturer.as_deref()
            && !manufacturer_matches(record, manufacturer)
        {
            return false;
        }

        true
    }
}

fn normalized_exact(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn normalized_contains(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase)
}

fn manufacturer_matches(record: &GtrRecord, needle: &str) -> bool {
    [
        record.manufacturer_test_name.as_str(),
        record.name_of_laboratory.as_str(),
        record.lab_test_name.as_str(),
    ]
    .into_iter()
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .any(|value| value.to_ascii_lowercase().contains(needle))
}

fn result_sort_key(result: &DiagnosticSearchResult) -> (String, String) {
    (
        result.name.trim().to_ascii_lowercase(),
        result.accession.clone(),
    )
}

#[allow(dead_code)]
pub async fn search(
    filters: &DiagnosticSearchFilters,
    limit: usize,
) -> Result<Vec<DiagnosticSearchResult>, BioMcpError> {
    Ok(search_page(filters, limit, 0).await?.results)
}

pub async fn search_page(
    filters: &DiagnosticSearchFilters,
    limit: usize,
    offset: usize,
) -> Result<SearchPage<DiagnosticSearchResult>, BioMcpError> {
    if limit == 0 || limit > MAX_SEARCH_LIMIT {
        return Err(BioMcpError::InvalidArgument(format!(
            "--limit must be between 1 and {MAX_SEARCH_LIMIT}"
        )));
    }

    let filters = NormalizedSearchFilters::from_filters(filters)?;
    let client = GtrClient::ready(GtrSyncMode::Auto).await?;
    let index = client.load_index()?;

    let mut results = index
        .records_by_id
        .values()
        .filter(|record| filters.matches(record, &index))
        .map(|record| search_result(record, &index))
        .collect::<Vec<_>>();
    results.sort_by_key(result_sort_key);

    let total = results.len();
    let results = results.into_iter().skip(offset).take(limit).collect();
    Ok(SearchPage::offset(results, Some(total)))
}

pub fn search_query_summary(filters: &DiagnosticSearchFilters) -> String {
    let mut parts = Vec::new();
    if let Some(gene) = filters
        .gene
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("gene={gene}"));
    }
    if let Some(disease) = filters
        .disease
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("disease={disease}"));
    }
    if let Some(test_type) = filters
        .test_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("type={test_type}"));
    }
    if let Some(manufacturer) = filters
        .manufacturer
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("manufacturer={manufacturer}"));
    }
    parts.join(", ")
}
