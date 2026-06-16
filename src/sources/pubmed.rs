#![allow(dead_code)]

use std::borrow::Cow;
use std::collections::HashSet;

use reqwest_middleware::ClientWithMiddleware;
use serde::Deserialize;

use crate::error::BioMcpError;
use crate::sources::{RequestPlan, request_from_plan};

const PUBMED_EUTILS_BASE: &str = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils";
const PUBMED_EUTILS_BASE_ENV: &str = "BIOMCP_PUBMED_BASE";
const PUBMED_EUTILS_API: &str = "pubmed-eutils";

#[derive(Clone)]
pub struct PubMedClient {
    client: ClientWithMiddleware,
    base: Cow<'static, str>,
    api_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PubMedESearchParams {
    pub term: String,
    pub retstart: usize,
    pub retmax: usize,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
}

#[allow(dead_code)]
pub struct PubMedESearchRequestPlan {
    pub method: &'static str,
    pub path: &'static str,
    pub query_params: Vec<(&'static str, String)>,
    pub cache_mode: &'static str,
    pub status_expectation: &'static str,
    pub content_type_expectation: &'static str,
    pub auth_mode: &'static str,
}

#[allow(dead_code)]
pub struct PubMedESummaryRequestPlan {
    pub method: &'static str,
    pub path: &'static str,
    pub query_params: Vec<(&'static str, String)>,
    pub cache_mode: &'static str,
    pub status_expectation: &'static str,
    pub content_type_expectation: &'static str,
    pub auth_mode: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PubMedESearchResponse {
    pub count: u64,
    pub idlist: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ESummaryEntry {
    pub uid: String,
    pub title: String,
    pub sortpubdate: Option<String>,
    pub pubdate: Option<String>,
    pub edat: Option<String>,
    pub lr: Option<String>,
    pub fulljournalname: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ESearchEnvelope {
    esearchresult: ESearchInner,
}

#[derive(Debug, Deserialize)]
struct ESearchInner {
    count: String,
    #[serde(default)]
    idlist: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ESummaryEnvelope {
    result: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct HistoryEntry {
    pubstatus: String,
    date: String,
}

#[derive(Debug, Deserialize)]
struct ESummaryEntryRaw {
    uid: Option<String>,
    title: Option<String>,
    sortpubdate: Option<String>,
    pubdate: Option<String>,
    #[serde(default)]
    history: Vec<HistoryEntry>,
    fulljournalname: Option<String>,
    source: Option<String>,
}

fn format_pubmed_date(value: &str) -> String {
    value.trim().replace('-', "/")
}

impl PubMedClient {
    pub fn new() -> Result<Self, BioMcpError> {
        Ok(Self {
            client: crate::sources::shared_client()?,
            base: crate::sources::env_base(PUBMED_EUTILS_BASE, PUBMED_EUTILS_BASE_ENV),
            api_key: crate::sources::ncbi_api_key(),
        })
    }

    async fn send(
        &self,
        req: reqwest_middleware::RequestBuilder,
        authenticated: bool,
    ) -> Result<
        (
            reqwest::StatusCode,
            Option<reqwest::header::HeaderValue>,
            Vec<u8>,
        ),
        BioMcpError,
    > {
        let resp = crate::sources::apply_cache_mode_with_auth(req, authenticated)
            .send()
            .await?;
        let status = resp.status();
        let content_type = resp.headers().get(reqwest::header::CONTENT_TYPE).cloned();
        let bytes = crate::sources::read_limited_body(resp, PUBMED_EUTILS_API).await?;
        Ok((status, content_type, bytes.to_vec()))
    }

    pub(crate) fn esearch_plan(
        params: &PubMedESearchParams,
        api_key: Option<&str>,
    ) -> Result<RequestPlan, BioMcpError> {
        let term = params.term.trim();
        if term.is_empty() {
            return Err(BioMcpError::InvalidArgument(
                "PubMed ESearch term is required".into(),
            ));
        }
        if term.len() > 4096 {
            return Err(BioMcpError::InvalidArgument(
                "PubMed ESearch term is too long".into(),
            ));
        }
        if params.retmax == 0 || params.retmax > 10_000 {
            return Err(BioMcpError::InvalidArgument(
                "PubMed ESearch retmax must be between 1 and 10000".into(),
            ));
        }

        let mut query_params = vec![
            ("db", "pubmed".to_string()),
            ("retmode", "json".to_string()),
            ("term", term.to_string()),
            ("retstart", params.retstart.to_string()),
            ("retmax", params.retmax.to_string()),
        ];
        if params.date_from.is_some() || params.date_to.is_some() {
            query_params.push(("datetype", "pdat".to_string()));
        }
        if let Some(date_from) = params
            .date_from
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            query_params.push(("mindate", format_pubmed_date(date_from)));
        }
        if let Some(date_to) = params
            .date_to
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            query_params.push(("maxdate", format_pubmed_date(date_to)));
        }
        if let Some(key) = clean_api_key(api_key) {
            query_params.push(("api_key", key.to_string()));
        }

        let mut plan = RequestPlan::get("esearch.fcgi");
        for (key, value) in query_params {
            plan = plan.query(key, value);
        }
        Ok(plan)
    }

    #[allow(dead_code)]
    pub fn esearch_request_plan(
        &self,
        params: &PubMedESearchParams,
    ) -> Result<PubMedESearchRequestPlan, BioMcpError> {
        let plan = Self::esearch_plan(params, self.api_key.as_deref())?;
        Ok(PubMedESearchRequestPlan {
            method: "GET",
            path: "/esearch.fcgi",
            query_params: plan
                .query
                .into_iter()
                .filter(|(key, _)| key != "api_key")
                .map(|(key, value)| (pubmed_query_key(&key), value))
                .collect(),
            cache_mode: if self.api_key.is_some() {
                "auth"
            } else {
                "default"
            },
            status_expectation: "non-2xx => Api",
            content_type_expectation: "json",
            auth_mode: if self.api_key.is_some() {
                "authenticated"
            } else {
                "keyless"
            },
        })
    }

    pub async fn esearch(
        &self,
        params: &PubMedESearchParams,
    ) -> Result<PubMedESearchResponse, BioMcpError> {
        let authenticated = self.api_key.is_some();
        let plan = Self::esearch_plan(params, self.api_key.as_deref())?;
        let req = request_from_plan(&self.client, self.base.as_ref(), &plan);
        let (status, content_type, bytes) = self.send(req, authenticated).await?;
        Self::decode_esearch_response(status, content_type.as_ref(), &bytes)
    }

    pub(crate) fn decode_esearch_response(
        status: reqwest::StatusCode,
        content_type: Option<&reqwest::header::HeaderValue>,
        bytes: &[u8],
    ) -> Result<PubMedESearchResponse, BioMcpError> {
        let response: ESearchEnvelope =
            crate::sources::decode_json(PUBMED_EUTILS_API, status, content_type, bytes, true)?;
        let count = response
            .esearchresult
            .count
            .trim()
            .parse::<u64>()
            .map_err(|_| BioMcpError::Api {
                api: PUBMED_EUTILS_API.to_string(),
                message: format!(
                    "Invalid ESearch count value {:?} from upstream contract",
                    response.esearchresult.count
                ),
            })?;

        Ok(PubMedESearchResponse {
            count,
            idlist: response.esearchresult.idlist,
        })
    }

    pub(crate) fn esummary_plan(
        ids: &[String],
        api_key: Option<&str>,
    ) -> Result<Option<RequestPlan>, BioMcpError> {
        if ids.is_empty() {
            return Ok(None);
        }

        let requested_ids = ids.iter().map(|id| id.trim()).collect::<Vec<_>>();
        if let Some(blank) = requested_ids.iter().find(|id| id.is_empty()) {
            return Err(BioMcpError::InvalidArgument(format!(
                "PubMed ESummary ids must be nonblank (got {:?})",
                blank
            )));
        }

        let mut plan = RequestPlan::get("esummary.fcgi")
            .query("db", "pubmed")
            .query("retmode", "json")
            .query("id", requested_ids.join(","));
        if let Some(key) = clean_api_key(api_key) {
            plan = plan.query("api_key", key);
        }
        Ok(Some(plan))
    }

    #[allow(dead_code)]
    pub fn esummary_request_plan(
        &self,
        ids: &[String],
    ) -> Result<Option<PubMedESummaryRequestPlan>, BioMcpError> {
        let Some(plan) = Self::esummary_plan(ids, self.api_key.as_deref())? else {
            return Ok(None);
        };

        Ok(Some(PubMedESummaryRequestPlan {
            method: "GET",
            path: "/esummary.fcgi",
            query_params: plan
                .query
                .into_iter()
                .filter(|(key, _)| key != "api_key")
                .map(|(key, value)| (pubmed_query_key(&key), value))
                .collect(),
            cache_mode: if self.api_key.is_some() {
                "auth"
            } else {
                "default"
            },
            status_expectation: "non-2xx => Api",
            content_type_expectation: "json",
            auth_mode: if self.api_key.is_some() {
                "authenticated"
            } else {
                "keyless"
            },
        }))
    }

    pub async fn esummary(&self, ids: &[String]) -> Result<Vec<ESummaryEntry>, BioMcpError> {
        let Some(plan) = Self::esummary_plan(ids, self.api_key.as_deref())? else {
            return Ok(Vec::new());
        };

        let authenticated = self.api_key.is_some();
        let req = request_from_plan(&self.client, self.base.as_ref(), &plan);
        let (status, content_type, bytes) = self.send(req, authenticated).await?;
        Self::decode_esummary_response(ids, status, content_type.as_ref(), &bytes)
    }

    pub(crate) fn decode_esummary_response(
        ids: &[String],
        status: reqwest::StatusCode,
        content_type: Option<&reqwest::header::HeaderValue>,
        bytes: &[u8],
    ) -> Result<Vec<ESummaryEntry>, BioMcpError> {
        let requested_ids = ids.iter().map(|id| id.trim()).collect::<Vec<_>>();
        let requested_set = requested_ids.iter().copied().collect::<HashSet<_>>();
        let response: ESummaryEnvelope =
            crate::sources::decode_json(PUBMED_EUTILS_API, status, content_type, bytes, true)?;

        let uids = response
            .result
            .get("uids")
            .and_then(|value| value.as_array())
            .ok_or_else(|| BioMcpError::Api {
                api: PUBMED_EUTILS_API.to_string(),
                message: "ESummary response missing uids array".into(),
            })?;

        let mut upstream_ids = Vec::with_capacity(uids.len());
        let mut upstream_seen = HashSet::with_capacity(uids.len());
        for value in uids {
            let uid = value
                .as_str()
                .map(str::trim)
                .filter(|uid| !uid.is_empty())
                .ok_or_else(|| BioMcpError::Api {
                    api: PUBMED_EUTILS_API.to_string(),
                    message: "ESummary uids must be a string array of nonblank PMIDs".into(),
                })?;
            if !upstream_seen.insert(uid) {
                return Err(BioMcpError::Api {
                    api: PUBMED_EUTILS_API.to_string(),
                    message: format!("ESummary response contains duplicate uid {uid}"),
                });
            }
            upstream_ids.push(uid);
        }

        for requested_id in &requested_ids {
            if !upstream_seen.contains(requested_id) {
                return Err(BioMcpError::Api {
                    api: PUBMED_EUTILS_API.to_string(),
                    message: format!(
                        "ESummary response missing requested PMID {requested_id} in uids"
                    ),
                });
            }
        }
        for upstream_id in &upstream_ids {
            if !requested_set.contains(upstream_id) {
                return Err(BioMcpError::Api {
                    api: PUBMED_EUTILS_API.to_string(),
                    message: format!("ESummary response contains unexpected PMID {upstream_id}"),
                });
            }
        }

        let mut entries = Vec::with_capacity(requested_ids.len());
        for requested_id in requested_ids {
            let raw_value = response
                .result
                .get(requested_id)
                .ok_or_else(|| BioMcpError::Api {
                    api: PUBMED_EUTILS_API.to_string(),
                    message: format!(
                        "ESummary response missing entry for requested PMID {requested_id}"
                    ),
                })?;
            let raw = serde_json::from_value::<ESummaryEntryRaw>(raw_value.clone()).map_err(
                |source| BioMcpError::Api {
                    api: PUBMED_EUTILS_API.to_string(),
                    message: format!(
                        "ESummary entry for PMID {requested_id} failed to parse: {source}"
                    ),
                },
            )?;
            if raw
                .uid
                .as_deref()
                .map(str::trim)
                .filter(|uid| !uid.is_empty())
                .is_some_and(|uid| uid != requested_id)
            {
                return Err(BioMcpError::Api {
                    api: PUBMED_EUTILS_API.to_string(),
                    message: format!(
                        "ESummary entry for PMID {requested_id} had conflicting inner uid {:?}",
                        raw.uid
                    ),
                });
            }
            let edat = raw
                .history
                .iter()
                .find(|h| h.pubstatus == "entrez")
                .or_else(|| raw.history.iter().find(|h| h.pubstatus == "pubmed"))
                .map(|h| h.date.clone());
            let lr = raw
                .history
                .iter()
                .find(|h| h.pubstatus == "medline")
                .map(|h| h.date.clone());
            entries.push(ESummaryEntry {
                uid: requested_id.to_string(),
                title: raw.title.unwrap_or_default(),
                sortpubdate: raw.sortpubdate,
                pubdate: raw.pubdate,
                edat,
                lr,
                fulljournalname: raw.fulljournalname,
                source: raw.source,
            });
        }

        Ok(entries)
    }
}

fn clean_api_key(api_key: Option<&str>) -> Option<&str> {
    api_key.map(str::trim).filter(|key| !key.is_empty())
}

#[allow(dead_code)]
fn pubmed_query_key(key: &str) -> &'static str {
    match key {
        "db" => "db",
        "retmode" => "retmode",
        "term" => "term",
        "retstart" => "retstart",
        "retmax" => "retmax",
        "datetype" => "datetype",
        "mindate" => "mindate",
        "maxdate" => "maxdate",
        "id" => "id",
        "api_key" => "api_key",
        _ => unreachable!("unexpected PubMed query key: {key}"),
    }
}

#[cfg(test)]
mod tests;
