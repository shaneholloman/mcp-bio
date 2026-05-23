use std::borrow::Cow;

use serde::Deserialize;

use crate::error::BioMcpError;

const OLS4_BASE: &str = "https://www.ebi.ac.uk/ols4";
const OLS4_API: &str = "ols4";
const OLS4_BASE_ENV: &str = "BIOMCP_OLS4_BASE";
const OLS4_ONTOLOGIES: &str = "hgnc,mesh,mondo,doid,hp,go,chebi,dron,ncit,ordo,wikipathways,so";

#[allow(dead_code)]
pub struct OlsSearchRequestPlan {
    pub method: &'static str,
    pub path: Option<&'static str>,
    pub query_params: Vec<(&'static str, String)>,
    pub source_label: &'static str,
    pub base_url: String,
    pub cache_mode: &'static str,
    pub status_expectation: &'static str,
    pub content_type_expectation: &'static str,
}

pub struct OlsClient {
    client: reqwest_middleware::ClientWithMiddleware,
    base: Cow<'static, str>,
}

impl OlsClient {
    pub fn new() -> Result<Self, BioMcpError> {
        Ok(Self {
            client: crate::sources::shared_client()?,
            base: crate::sources::env_base(OLS4_BASE, OLS4_BASE_ENV),
        })
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(base: String) -> Result<Self, BioMcpError> {
        Ok(Self {
            client: crate::sources::test_client()?,
            base: Cow::Owned(base),
        })
    }

    fn endpoint(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.base.as_ref().trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    pub fn search_request_plan(&self, query: &str) -> OlsSearchRequestPlan {
        let query = query.trim();
        OlsSearchRequestPlan {
            method: "GET",
            path: (!query.is_empty()).then_some("/api/search"),
            query_params: if query.is_empty() {
                Vec::new()
            } else {
                vec![
                    ("q", query.to_string()),
                    ("rows", "10".to_string()),
                    ("groupField", "iri".to_string()),
                    ("ontology", OLS4_ONTOLOGIES.to_string()),
                ]
            },
            source_label: "ols4",
            base_url: self.base.to_string(),
            cache_mode: "default",
            status_expectation: "non-2xx => Api",
            content_type_expectation: "json",
        }
    }

    pub async fn search(&self, query: &str) -> Result<Vec<OlsDoc>, BioMcpError> {
        let plan = self.search_request_plan(query);
        let Some(path) = plan.path else {
            return Ok(Vec::new());
        };

        let resp = crate::sources::apply_cache_mode(self.client.get(self.endpoint(path)))
            .query(&plan.query_params)
            .send()
            .await?;
        let status = resp.status();
        let content_type = resp.headers().get(reqwest::header::CONTENT_TYPE).cloned();
        let bytes = crate::sources::read_limited_body(resp, OLS4_API).await?;

        if !status.is_success() {
            return Err(BioMcpError::Api {
                api: OLS4_API.to_string(),
                message: format!("HTTP {status}: {}", crate::sources::body_excerpt(&bytes)),
            });
        }

        crate::sources::ensure_json_content_type(OLS4_API, content_type.as_ref(), &bytes)?;
        let response: OlsSearchEnvelope =
            serde_json::from_slice(&bytes).map_err(|source| BioMcpError::ApiJson {
                api: OLS4_API.to_string(),
                source,
            })?;
        Ok(response.response.docs)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct OlsSearchEnvelope {
    response: OlsSearchResponse,
}

#[derive(Debug, Clone, Deserialize)]
struct OlsSearchResponse {
    #[serde(default)]
    docs: Vec<OlsDoc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OlsDoc {
    pub iri: String,
    #[allow(dead_code)]
    #[serde(default)]
    pub ontology_name: String,
    #[serde(default)]
    pub ontology_prefix: String,
    #[serde(default)]
    pub short_form: Option<String>,
    #[serde(default)]
    pub obo_id: Option<String>,
    #[serde(default)]
    pub label: String,
    #[allow(dead_code)]
    #[serde(default)]
    pub description: Vec<String>,
    #[serde(default)]
    pub exact_synonyms: Vec<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub is_defining_ontology: bool,
    #[allow(dead_code)]
    #[serde(default, rename = "type")]
    pub doc_type: Option<String>,
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::{OLS4_ONTOLOGIES, OlsClient};

    #[test]
    fn search_request_plan_exposes_canonical_query_contract() {
        let client = OlsClient::new_for_test("http://127.0.0.1/base".into()).expect("client");
        let plan = client.search_request_plan(" ERBB1 ");

        assert_eq!(plan.method, "GET");
        assert_eq!(plan.path, Some("/api/search"));
        assert_eq!(plan.source_label, "ols4");
        assert_eq!(plan.base_url, "http://127.0.0.1/base");
        assert_eq!(plan.cache_mode, "default");
        assert_eq!(plan.status_expectation, "non-2xx => Api");
        assert_eq!(plan.content_type_expectation, "json");
        assert_eq!(
            plan.query_params,
            vec![
                ("q", "ERBB1".to_string()),
                ("rows", "10".to_string()),
                ("groupField", "iri".to_string()),
                ("ontology", OLS4_ONTOLOGIES.to_string()),
            ]
        );
    }

    #[test]
    fn search_request_plan_keeps_empty_query_as_no_request() {
        let client = OlsClient::new_for_test("http://127.0.0.1".into()).expect("client");
        let plan = client.search_request_plan("   ");

        assert_eq!(plan.method, "GET");
        assert_eq!(plan.path, None);
        assert!(plan.query_params.is_empty());
    }

    #[tokio::test]
    async fn search_uses_required_query_contract() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/search"))
            .and(query_param("q", "ERBB1"))
            .and(query_param("rows", "10"))
            .and(query_param("groupField", "iri"))
            .and(query_param(
                "ontology",
                "hgnc,mesh,mondo,doid,hp,go,chebi,dron,ncit,ordo,wikipathways,so",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "response": {
                    "docs": [
                        {
                            "iri": "http://example.org/hgnc/3236",
                            "ontology_name": "hgnc",
                            "ontology_prefix": "hgnc",
                            "short_form": "hgnc:3236",
                            "obo_id": "HGNC:3236",
                            "label": "EGFR",
                            "description": [],
                            "exact_synonyms": ["ERBB1"],
                            "type": "class"
                        }
                    ]
                }
            })))
            .mount(&server)
            .await;

        let client = OlsClient::new_for_test(server.uri()).expect("client");
        let rows = client.search("ERBB1").await.expect("search");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label, "EGFR");
    }
}
