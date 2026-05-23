use std::borrow::Cow;

use reqwest::Url;
use reqwest_middleware::ClientWithMiddleware;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};

use crate::error::BioMcpError;

const LITSENSE2_BASE: &str = "https://www.ncbi.nlm.nih.gov/research/litsense2-api/api";
const LITSENSE2_API: &str = "litsense2";
const LITSENSE2_BASE_ENV: &str = "BIOMCP_LITSENSE2_BASE";

#[allow(dead_code)]
pub struct LitSense2SearchRequestPlan {
    pub method: &'static str,
    pub path: &'static str,
    pub query_params: Vec<(&'static str, String)>,
    pub cache_mode: &'static str,
    pub status_expectation: &'static str,
    pub content_type_expectation: &'static str,
}

#[derive(Clone)]
pub struct LitSense2Client {
    client: ClientWithMiddleware,
    base: Cow<'static, str>,
}

impl LitSense2Client {
    pub fn new() -> Result<Self, BioMcpError> {
        Ok(Self {
            client: crate::sources::shared_client()?,
            base: crate::sources::env_base(LITSENSE2_BASE, LITSENSE2_BASE_ENV),
        })
    }

    #[cfg(test)]
    fn new_for_test(base: String) -> Result<Self, BioMcpError> {
        Ok(Self {
            client: Self::test_client()?,
            base: Cow::Owned(base),
        })
    }

    #[cfg(test)]
    fn test_client() -> Result<ClientWithMiddleware, BioMcpError> {
        let base = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .connect_timeout(std::time::Duration::from_secs(5))
            .user_agent(concat!("biomcp-cli-test/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(BioMcpError::HttpClientInit)?;
        Ok(reqwest_middleware::ClientBuilder::new(base).build())
    }

    fn endpoint_url(&self, path: &str) -> Result<Url, BioMcpError> {
        Url::parse(&format!(
            "{}/{}",
            self.base.as_ref().trim_end_matches('/'),
            path.trim_start_matches('/')
        ))
        .map_err(|err| BioMcpError::Api {
            api: LITSENSE2_API.to_string(),
            message: format!("invalid LitSense2 base URL: {err}"),
        })
    }

    async fn send_json<T: DeserializeOwned>(
        &self,
        req: reqwest_middleware::RequestBuilder,
    ) -> Result<T, BioMcpError> {
        let resp = crate::sources::apply_cache_mode(req).send().await?;
        let status = resp.status();
        let content_type = resp.headers().get(reqwest::header::CONTENT_TYPE).cloned();
        let bytes = crate::sources::read_limited_body(resp, LITSENSE2_API).await?;
        if !status.is_success() {
            let excerpt = crate::sources::body_excerpt(&bytes);
            return Err(BioMcpError::Api {
                api: LITSENSE2_API.to_string(),
                message: format!("HTTP {status}: {excerpt}"),
            });
        }
        crate::sources::ensure_json_content_type(LITSENSE2_API, content_type.as_ref(), &bytes)?;
        serde_json::from_slice(&bytes).map_err(|source| BioMcpError::ApiJson {
            api: LITSENSE2_API.to_string(),
            source,
        })
    }

    pub fn search_request_plan(
        &self,
        path: &str,
        query: &str,
    ) -> Result<LitSense2SearchRequestPlan, BioMcpError> {
        let query = query.trim();
        if query.is_empty() {
            return Err(BioMcpError::InvalidArgument(
                "LitSense2 query is required".into(),
            ));
        }
        if query.len() > 4096 {
            return Err(BioMcpError::InvalidArgument(
                "LitSense2 query is too long".into(),
            ));
        }
        let path = match path.trim_start_matches('/') {
            "sentences/" => "/sentences/",
            "passages/" => "/passages/",
            _ => {
                return Err(BioMcpError::InvalidArgument(
                    "LitSense2 path must be /sentences/ or /passages/".into(),
                ));
            }
        };

        Ok(LitSense2SearchRequestPlan {
            method: "GET",
            path,
            query_params: vec![("query", query.to_string()), ("rerank", "true".to_string())],
            cache_mode: "default",
            status_expectation: "non-2xx => Api",
            content_type_expectation: "json",
        })
    }

    async fn search(
        &self,
        path: &str,
        query: &str,
    ) -> Result<Vec<LitSense2SearchHit>, BioMcpError> {
        let plan = self.search_request_plan(path, query)?;
        let url = self.endpoint_url(plan.path)?;
        let req = self.client.get(url).query(&plan.query_params);
        self.send_json(req).await
    }

    pub async fn sentence_search(
        &self,
        query: &str,
    ) -> Result<Vec<LitSense2SearchHit>, BioMcpError> {
        self.search("sentences/", query).await
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn paragraph_search(
        &self,
        query: &str,
    ) -> Result<Vec<LitSense2SearchHit>, BioMcpError> {
        self.search("passages/", query).await
    }
}

fn deserialize_optional_trimmed_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    Ok(value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty()))
}

fn deserialize_string_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Vec<String>>::deserialize(deserializer)?;
    Ok(value.unwrap_or_default())
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct LitSense2SearchHit {
    pub pmid: u64,
    #[serde(default, deserialize_with = "deserialize_optional_trimmed_string")]
    pub pmcid: Option<String>,
    pub text: String,
    pub score: f64,
    #[serde(default, deserialize_with = "deserialize_optional_trimmed_string")]
    pub section: Option<String>,
    #[serde(default, deserialize_with = "deserialize_string_vec")]
    pub annotations: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn ticket_376_article_source_contracts_litsense2_request_plan_covers_hydration_path() {
        let client = LitSense2Client::new_for_test("http://127.0.0.1".into()).expect("client");
        let sentence: LitSense2SearchRequestPlan = client
            .search_request_plan("sentences/", "BRAF")
            .expect("LitSense2SearchRequestPlan");
        assert_eq!(sentence.method, "GET");
        assert_eq!(sentence.path, "/sentences/");
        assert!(
            sentence
                .query_params
                .contains(&("query", "BRAF".to_string()))
        );
        assert!(
            sentence
                .query_params
                .contains(&("rerank", "true".to_string()))
        );

        let passage = client
            .search_request_plan("passages/", "BRAF")
            .expect("passage plan");
        assert_eq!(passage.path, "/passages/");

        let ids = vec!["22663011".to_string()];
        let hydration: crate::sources::pubmed::PubMedESummaryRequestPlan =
            crate::sources::pubmed::PubMedClient::new()
                .expect("pubmed client")
                .esummary_request_plan(&ids)
                .expect("hydration plan")
                .expect("PubMedESummaryRequestPlan");
        assert_eq!(hydration.path, "/esummary.fcgi");
        assert!(
            hydration
                .query_params
                .contains(&("id", "22663011".to_string()))
        );
    }

    #[tokio::test]
    async fn sentence_search_sends_query_and_parses_results() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/sentences/"))
            .and(query_param("query", "hirschsprung disease"))
            .and(query_param("rerank", "true"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "pmid": 36741595,
                    "pmcid": "PMC9891841",
                    "text": "Sentence match",
                    "score": 0.84,
                    "section": "INTRO",
                    "annotations": ["0|12|disease|MESH:D006627"]
                }
            ])))
            .mount(&server)
            .await;

        let client = LitSense2Client::new_for_test(server.uri()).expect("client");
        let hits = client
            .sentence_search("hirschsprung disease")
            .await
            .expect("sentence search");

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].pmid, 36_741_595);
        assert_eq!(hits[0].pmcid.as_deref(), Some("PMC9891841"));
        assert_eq!(hits[0].score, 0.84);
        assert_eq!(hits[0].section.as_deref(), Some("INTRO"));
        assert_eq!(hits[0].annotations, vec!["0|12|disease|MESH:D006627"]);
    }

    #[tokio::test]
    async fn paragraph_search_tolerates_null_annotations() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/passages/"))
            .and(query_param("query", "melanoma"))
            .and(query_param("rerank", "true"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "pmid": 123,
                    "pmcid": null,
                    "text": "Paragraph match",
                    "score": 0.5,
                    "section": null,
                    "annotations": null
                }
            ])))
            .mount(&server)
            .await;

        let client = LitSense2Client::new_for_test(server.uri()).expect("client");
        let hits = client
            .paragraph_search("melanoma")
            .await
            .expect("paragraph search");

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].annotations, Vec::<String>::new());
        assert!(hits[0].pmcid.is_none());
        assert!(hits[0].section.is_none());
    }

    #[tokio::test]
    async fn sentence_search_surfaces_http_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/sentences/"))
            .respond_with(ResponseTemplate::new(500).set_body_string("upstream failure"))
            .mount(&server)
            .await;

        let client = LitSense2Client::new_for_test(server.uri()).expect("client");
        let err = client
            .sentence_search("hirschsprung disease")
            .await
            .expect_err("server error should bubble up");

        assert!(err.to_string().contains("HTTP 500"));
        assert!(err.to_string().contains("upstream failure"));
    }
}
