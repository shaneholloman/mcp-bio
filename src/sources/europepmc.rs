use std::borrow::Cow;

use http_cache_reqwest::CacheMode;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::BioMcpError;
use crate::sources::{RequestPlan, request_from_plan};

const EUROPE_PMC_BASE: &str = "https://www.ebi.ac.uk/europepmc/webservices/rest";
const EUROPE_PMC_API: &str = "europepmc";
const EUROPE_PMC_BASE_ENV: &str = "BIOMCP_EUROPEPMC_BASE";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum EuropePmcSort {
    Date,
    Citations,
    #[default]
    Relevance,
}

#[allow(dead_code)]
pub struct EuropePmcSearchRequestPlan {
    pub method: &'static str,
    pub path: &'static str,
    pub query_params: Vec<(&'static str, String)>,
    pub cache_mode: &'static str,
    pub status_expectation: &'static str,
    pub content_type_expectation: &'static str,
}

#[derive(Clone)]
pub struct EuropePmcClient {
    client: reqwest_middleware::ClientWithMiddleware,
    base: Cow<'static, str>,
}

impl EuropePmcClient {
    pub fn new() -> Result<Self, BioMcpError> {
        Ok(Self {
            client: crate::sources::shared_client()?,
            base: crate::sources::env_base(EUROPE_PMC_BASE, EUROPE_PMC_BASE_ENV),
        })
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        req: reqwest_middleware::RequestBuilder,
    ) -> Result<T, BioMcpError> {
        let resp = crate::sources::apply_cache_mode(req).send().await?;
        let status = resp.status();
        let content_type = resp.headers().get(reqwest::header::CONTENT_TYPE).cloned();
        let bytes = crate::sources::read_limited_body(resp, EUROPE_PMC_API).await?;
        crate::sources::decode_json(EUROPE_PMC_API, status, content_type.as_ref(), &bytes, false)
    }

    pub async fn search_by_doi(&self, doi: &str) -> Result<EuropePmcSearchResponse, BioMcpError> {
        let doi = doi.trim();
        if doi.is_empty() {
            return Err(BioMcpError::InvalidArgument(
                "DOI is required. Example: biomcp get article 10.1056/NEJMoa1203421".into(),
            ));
        }
        if doi.len() > 256 {
            return Err(BioMcpError::InvalidArgument("DOI is too long.".into()));
        }

        self.search_query(&format!("DOI:{doi}"), 1, 1).await
    }

    pub async fn search_by_pmcid(
        &self,
        pmcid: &str,
    ) -> Result<EuropePmcSearchResponse, BioMcpError> {
        let pmcid = pmcid.trim();
        if pmcid.is_empty() {
            return Err(BioMcpError::InvalidArgument(
                "PMCID is required. Example: biomcp get article PMC9984800".into(),
            ));
        }
        if pmcid.len() > 64 {
            return Err(BioMcpError::InvalidArgument("PMCID is too long.".into()));
        }

        let (prefix, rest) = pmcid.split_at(3.min(pmcid.len()));
        if !prefix.eq_ignore_ascii_case("PMC")
            || rest.is_empty()
            || !rest.chars().all(|c| c.is_ascii_digit())
        {
            return Err(BioMcpError::InvalidArgument(
                "PMCID must start with PMC and contain only digits after. Example: biomcp get article PMC9984800"
                    .into(),
            ));
        }

        let normalized = format!("PMC{rest}");
        self.search_query(&format!("PMCID:{normalized}"), 1, 1)
            .await
    }

    pub async fn search_by_pmid(&self, pmid: &str) -> Result<EuropePmcSearchResponse, BioMcpError> {
        let pmid = pmid.trim();
        if pmid.is_empty() {
            return Err(BioMcpError::InvalidArgument(
                "PMID is required. Example: biomcp get article 22663011".into(),
            ));
        }
        if pmid.len() > 32 || !pmid.chars().all(|c| c.is_ascii_digit()) {
            return Err(BioMcpError::InvalidArgument(
                "PMID must be numeric. Example: biomcp get article 22663011".into(),
            ));
        }
        self.search_query(&format!("EXT_ID:{pmid} AND SRC:MED"), 1, 1)
            .await
    }

    pub async fn search_query(
        &self,
        query: &str,
        page: usize,
        page_size: usize,
    ) -> Result<EuropePmcSearchResponse, BioMcpError> {
        self.search_query_with_sort(query, page, page_size, EuropePmcSort::Relevance)
            .await
    }

    pub(crate) fn search_query_plan(
        query: &str,
        page: usize,
        page_size: usize,
        sort: EuropePmcSort,
    ) -> Result<RequestPlan, BioMcpError> {
        let query = query.trim();
        if query.is_empty() {
            return Err(BioMcpError::InvalidArgument(
                "Query is required for Europe PMC search".into(),
            ));
        }
        if query.len() > 2048 {
            return Err(BioMcpError::InvalidArgument(
                "Query is too long for Europe PMC search".into(),
            ));
        }
        if page == 0 {
            return Err(BioMcpError::InvalidArgument(
                "Europe PMC page must be >= 1".into(),
            ));
        }
        if page_size == 0 || page_size > 100 {
            return Err(BioMcpError::InvalidArgument(
                "Europe PMC page size must be between 1 and 100".into(),
            ));
        }

        let mut plan = RequestPlan::get("search")
            .query("query", query)
            .query("format", "json")
            .query("page", page.to_string())
            .query("pageSize", page_size.to_string());
        match sort {
            EuropePmcSort::Date => plan = plan.query("sort", "P_PDATE_D desc"),
            EuropePmcSort::Citations => plan = plan.query("sort", "CITED desc"),
            EuropePmcSort::Relevance => {}
        }
        Ok(plan)
    }

    #[allow(dead_code)]
    pub fn search_query_request_plan(
        &self,
        query: &str,
        page: usize,
        page_size: usize,
        sort: EuropePmcSort,
    ) -> Result<EuropePmcSearchRequestPlan, BioMcpError> {
        let plan = Self::search_query_plan(query, page, page_size, sort)?;
        Ok(EuropePmcSearchRequestPlan {
            method: "GET",
            path: "/search",
            query_params: plan
                .query
                .iter()
                .map(|(key, value)| {
                    let key = match key.as_str() {
                        "query" => "query",
                        "format" => "format",
                        "page" => "page",
                        "pageSize" => "pageSize",
                        "sort" => "sort",
                        _ => "",
                    };
                    (key, value.clone())
                })
                .collect(),
            cache_mode: "default",
            status_expectation: "non-2xx => Api",
            content_type_expectation: "json",
        })
    }

    pub async fn search_query_with_sort(
        &self,
        query: &str,
        page: usize,
        page_size: usize,
        sort: EuropePmcSort,
    ) -> Result<EuropePmcSearchResponse, BioMcpError> {
        let plan = Self::search_query_plan(query, page, page_size, sort)?;
        let req = request_from_plan(&self.client, self.base.as_ref(), &plan);
        self.get_json(req).await
    }

    pub(crate) fn full_text_xml_plan(
        source: &str,
        id: &str,
    ) -> Result<Option<RequestPlan>, BioMcpError> {
        let source = source.trim();
        let id = id.trim();
        if source.is_empty() || id.is_empty() {
            return Ok(None);
        }

        let normalized_id = if source.eq_ignore_ascii_case("PMC")
            && !id
                .get(..3)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("PMC"))
        {
            format!("PMC{id}")
        } else {
            id.to_string()
        };
        Ok(Some(RequestPlan::get(format!(
            "{normalized_id}/fullTextXML"
        ))))
    }

    pub(crate) fn decode_full_text_xml(
        status: reqwest::StatusCode,
        bytes: &[u8],
    ) -> Result<Option<String>, BioMcpError> {
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !status.is_success() {
            let excerpt = crate::sources::body_excerpt(bytes);
            return Err(BioMcpError::Api {
                api: EUROPE_PMC_API.to_string(),
                message: format!("HTTP {status}: {excerpt}"),
            });
        }

        Ok(Some(String::from_utf8_lossy(bytes).to_string()))
    }

    pub async fn get_full_text_xml(
        &self,
        source: &str,
        id: &str,
    ) -> Result<Option<String>, BioMcpError> {
        let Some(plan) = Self::full_text_xml_plan(source, id)? else {
            return Ok(None);
        };

        let req = request_from_plan(&self.client, self.base.as_ref(), &plan);
        let resp = req.with_extension(CacheMode::NoStore).send().await?;
        let status = resp.status();
        let bytes = crate::sources::read_limited_body(resp, EUROPE_PMC_API).await?;
        Self::decode_full_text_xml(status, &bytes)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EuropePmcSearchResponse {
    #[serde(rename = "hitCount")]
    pub hit_count: Option<u64>,
    #[serde(rename = "resultList")]
    pub result_list: Option<EuropePmcResultList>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EuropePmcResultList {
    #[serde(default)]
    pub result: Vec<EuropePmcResult>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EuropePmcResult {
    pub id: Option<String>,
    pub title: Option<String>,
    pub pmid: Option<String>,
    pub pmcid: Option<String>,
    pub doi: Option<String>,
    #[serde(rename = "journalTitle")]
    pub journal_title: Option<String>,
    #[serde(rename = "firstPublicationDate")]
    pub first_publication_date: Option<String>,
    #[serde(rename = "firstIndexDate")]
    pub first_index_date: Option<String>,
    #[serde(rename = "authorString")]
    pub author_string: Option<String>,
    #[serde(rename = "pubYear")]
    pub pub_year: Option<String>,
    #[serde(rename = "citedByCount")]
    pub cited_by_count: Option<serde_json::Value>,
    #[serde(rename = "pubType")]
    pub pub_type: Option<serde_json::Value>,
    #[serde(rename = "pubTypeList")]
    pub pub_type_list: Option<serde_json::Value>,
    #[serde(rename = "isOpenAccess")]
    pub is_open_access: Option<serde_json::Value>,
    pub license: Option<String>,
    #[serde(rename = "fullTextIdList")]
    pub full_text_id_list: Option<serde_json::Value>,
    #[serde(rename = "fullTextUrlList")]
    pub full_text_url_list: Option<serde_json::Value>,
    #[serde(rename = "abstractText")]
    pub abstract_text: Option<String>,
}

#[cfg(test)]
mod tests;
