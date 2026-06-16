use std::borrow::Cow;

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::error::BioMcpError;
use crate::sources::{RequestPlan, request_from_plan};

const INTERPRO_BASE: &str = "https://www.ebi.ac.uk/interpro/api";
const INTERPRO_API: &str = "interpro";
const INTERPRO_BASE_ENV: &str = "BIOMCP_INTERPRO_BASE";

pub struct InterProClient {
    client: reqwest_middleware::ClientWithMiddleware,
    base: Cow<'static, str>,
}

impl InterProClient {
    pub fn new() -> Result<Self, BioMcpError> {
        Ok(Self {
            client: crate::sources::shared_client()?,
            base: crate::sources::env_base(INTERPRO_BASE, INTERPRO_BASE_ENV),
        })
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        req: reqwest_middleware::RequestBuilder,
    ) -> Result<T, BioMcpError> {
        let resp = crate::sources::apply_cache_mode(req).send().await?;
        let status = resp.status();
        let bytes = crate::sources::read_limited_body(resp, INTERPRO_API).await?;
        if !status.is_success() {
            let excerpt = crate::sources::body_excerpt(&bytes);
            return Err(BioMcpError::Api {
                api: INTERPRO_API.to_string(),
                message: format!("HTTP {status}: {excerpt}"),
            });
        }
        serde_json::from_slice(&bytes).map_err(|source| BioMcpError::ApiJson {
            api: INTERPRO_API.to_string(),
            source,
        })
    }

    pub(crate) fn domains_plan(
        uniprot_accession: &str,
        limit: usize,
    ) -> Result<RequestPlan, BioMcpError> {
        let uniprot_accession = uniprot_accession.trim();
        if uniprot_accession.is_empty() {
            return Err(BioMcpError::InvalidArgument(
                "InterPro requires a UniProt accession".into(),
            ));
        }

        let page_size = limit.clamp(1, 25).to_string();
        Ok(RequestPlan::get(format!(
            "entry/interpro/protein/uniprot/{uniprot_accession}/"
        ))
        .query("page_size", page_size))
    }

    fn decode_domains_response(resp: InterProResponse, limit: usize) -> Vec<InterProDomain> {
        let mut out = Vec::new();
        for row in resp.results.into_iter().take(limit.clamp(1, 25)) {
            let Some(meta) = row.metadata else { continue };
            let Some(accession) = meta.accession.map(|v| v.trim().to_string()) else {
                continue;
            };
            if accession.is_empty() {
                continue;
            }
            let name = meta
                .name
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty());
            let domain_type = meta
                .r#type
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty());
            out.push(InterProDomain {
                accession,
                name,
                domain_type,
            });
        }

        out
    }

    pub async fn domains(
        &self,
        uniprot_accession: &str,
        limit: usize,
    ) -> Result<Vec<InterProDomain>, BioMcpError> {
        let plan = Self::domains_plan(uniprot_accession, limit)?;
        let resp: InterProResponse = self
            .get_json(request_from_plan(&self.client, self.base.as_ref(), &plan))
            .await?;
        Ok(Self::decode_domains_response(resp, limit))
    }
}

#[derive(Debug, Clone)]
pub struct InterProDomain {
    pub accession: String,
    pub name: Option<String>,
    pub domain_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct InterProResponse {
    #[serde(default)]
    results: Vec<InterProResult>,
}

#[derive(Debug, Deserialize)]
struct InterProResult {
    metadata: Option<InterProMetadata>,
}

#[derive(Debug, Deserialize)]
struct InterProMetadata {
    accession: Option<String>,
    name: Option<String>,
    #[serde(rename = "type")]
    r#type: Option<String>,
}

#[cfg(test)]
mod tests;
