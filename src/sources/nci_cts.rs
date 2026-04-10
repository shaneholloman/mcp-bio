use std::borrow::Cow;

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::error::BioMcpError;

const NCI_CTS_BASE: &str = "https://clinicaltrialsapi.cancer.gov/api/v2";
const NCI_CTS_API: &str = "nci_cts";
const NCI_CTS_BASE_ENV: &str = "BIOMCP_NCI_CTS_BASE";
const NCI_API_KEY_ENV: &str = "NCI_API_KEY";

#[derive(Clone)]
pub struct NciCtsClient {
    client: reqwest_middleware::ClientWithMiddleware,
    base: Cow<'static, str>,
    api_key: String,
}

#[derive(Debug, Clone)]
pub enum NciDiseaseFilter {
    Keyword(String),
    ConceptId(String),
}

#[derive(Debug, Clone)]
pub enum NciStatusFilter {
    CurrentTrialStatus(String),
    SiteRecruitmentStatus(String),
}

#[derive(Debug, Clone)]
pub struct NciGeoFilter {
    pub lat: f64,
    pub lon: f64,
    pub distance_miles: u32,
}

#[derive(Debug, Clone, Default)]
pub struct NciSearchParams {
    pub disease: Option<NciDiseaseFilter>,
    pub interventions: Option<String>,
    pub sites_org_name: Option<String>,
    pub status: Option<NciStatusFilter>,
    pub phases: Vec<String>,
    pub geo: Option<NciGeoFilter>,
    pub biomarkers: Option<String>,
    pub size: usize,
    pub from: usize,
}

#[derive(Debug, Deserialize)]
pub struct NciSearchResponse {
    #[serde(default)]
    pub data: Vec<serde_json::Value>,
    #[serde(default)]
    pub trials: Vec<serde_json::Value>,
    #[serde(default, alias = "total", alias = "total_count", alias = "totalCount")]
    pub total: Option<usize>,
}

impl NciSearchResponse {
    pub fn hits(&self) -> &[serde_json::Value] {
        if !self.data.is_empty() {
            &self.data
        } else {
            &self.trials
        }
    }
}

fn trimmed_non_empty(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

impl NciCtsClient {
    pub fn new() -> Result<Self, BioMcpError> {
        let api_key = std::env::var(NCI_API_KEY_ENV)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| BioMcpError::ApiKeyRequired {
                api: NCI_CTS_API.to_string(),
                env_var: NCI_API_KEY_ENV.to_string(),
                docs_url: "https://clinicaltrialsapi.cancer.gov/".to_string(),
            })?;

        Ok(Self {
            client: crate::sources::shared_client()?,
            base: crate::sources::env_base(NCI_CTS_BASE, NCI_CTS_BASE_ENV),
            api_key,
        })
    }

    #[cfg(test)]
    fn new_for_test(base: String, api_key: String) -> Result<Self, BioMcpError> {
        Ok(Self {
            client: crate::sources::test_client()?,
            base: Cow::Owned(base),
            api_key,
        })
    }

    fn endpoint(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.base.as_ref().trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        req: reqwest_middleware::RequestBuilder,
    ) -> Result<T, BioMcpError> {
        let resp = crate::sources::apply_cache_mode_with_auth(req, true)
            .send()
            .await?;
        let status = resp.status();
        let bytes = crate::sources::read_limited_body(resp, NCI_CTS_API).await?;
        if !status.is_success() {
            let excerpt = crate::sources::body_excerpt(&bytes);
            return Err(BioMcpError::Api {
                api: NCI_CTS_API.to_string(),
                message: format!("HTTP {status}: {excerpt}"),
            });
        }
        serde_json::from_slice(&bytes).map_err(|source| BioMcpError::ApiJson {
            api: NCI_CTS_API.to_string(),
            source,
        })
    }

    pub async fn search(&self, params: &NciSearchParams) -> Result<NciSearchResponse, BioMcpError> {
        let url = self.endpoint("trials");
        let mut req = self.client.get(&url).header("X-API-KEY", &self.api_key);

        if let Some(disease) = &params.disease {
            match disease {
                NciDiseaseFilter::Keyword(v) => {
                    if let Some(v) = trimmed_non_empty(v.as_str()) {
                        req = req.query(&[("keyword", v)]);
                    }
                }
                NciDiseaseFilter::ConceptId(v) => {
                    if let Some(v) = trimmed_non_empty(v.as_str()) {
                        req = req.query(&[("diseases.nci_thesaurus_concept_id", v)]);
                    }
                }
            }
        }
        if let Some(v) = params
            .interventions
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            req = req.query(&[("interventions", v)]);
        }
        if let Some(v) = params
            .sites_org_name
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            req = req.query(&[("sites.org_name", v)]);
        }
        if let Some(status) = &params.status {
            match status {
                NciStatusFilter::CurrentTrialStatus(v) => {
                    if let Some(v) = trimmed_non_empty(v.as_str()) {
                        req = req.query(&[("current_trial_status", v)]);
                    }
                }
                NciStatusFilter::SiteRecruitmentStatus(v) => {
                    if let Some(v) = trimmed_non_empty(v.as_str()) {
                        req = req.query(&[("sites.recruitment_status", v)]);
                    }
                }
            }
        }
        for phase in &params.phases {
            let phase = phase.trim();
            if phase.is_empty() {
                continue;
            }
            req = req.query(&[("phase", phase)]);
        }
        if let Some(geo) = &params.geo {
            req = req.query(&[("sites.org_coordinates_lat", geo.lat)]);
            req = req.query(&[("sites.org_coordinates_lon", geo.lon)]);
            let distance = format!("{}mi", geo.distance_miles);
            req = req.query(&[("sites.org_coordinates_dist", distance.as_str())]);
        }
        if let Some(v) = params
            .biomarkers
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            req = req.query(&[("biomarkers", v)]);
        }

        let size = params.size.to_string();
        req = req.query(&[("size", size.as_str())]);
        let from = params.from.to_string();
        req = req.query(&[("from", from.as_str())]);

        self.get_json(req).await
    }

    pub async fn get(&self, nct_id: &str) -> Result<serde_json::Value, BioMcpError> {
        let url = self.endpoint(&format!("trials/{nct_id}"));
        self.get_json(self.client.get(&url).header("X-API-KEY", &self.api_key))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path, query_param, query_param_is_missing};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn search_includes_api_key_header_and_params() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/trials"))
            .and(header("X-API-KEY", "test-key"))
            .and(query_param("keyword", "melanoma"))
            .and(query_param("size", "2"))
            .and(query_param("from", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": []
            })))
            .mount(&server)
            .await;

        let client = NciCtsClient::new_for_test(server.uri(), "test-key".into()).unwrap();
        let _ = client
            .search(&NciSearchParams {
                disease: Some(NciDiseaseFilter::Keyword("melanoma".into())),
                interventions: None,
                sites_org_name: None,
                status: None,
                phases: Vec::new(),
                geo: None,
                biomarkers: None,
                size: 2,
                from: 0,
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn search_surfaces_http_error_context() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/trials"))
            .and(header("X-API-KEY", "test-key"))
            .respond_with(ResponseTemplate::new(500).set_body_string("upstream failure"))
            .mount(&server)
            .await;

        let client = NciCtsClient::new_for_test(server.uri(), "test-key".into()).unwrap();
        let err = client
            .search(&NciSearchParams {
                disease: Some(NciDiseaseFilter::Keyword("melanoma".into())),
                interventions: None,
                sites_org_name: None,
                status: None,
                phases: Vec::new(),
                geo: None,
                biomarkers: None,
                size: 2,
                from: 0,
            })
            .await
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("nci_cts"));
        assert!(msg.contains("500"));
    }

    #[tokio::test]
    async fn search_includes_sites_org_name_param() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/trials"))
            .and(header("X-API-KEY", "test-key"))
            .and(query_param("keyword", "melanoma"))
            .and(query_param("sites.org_name", "MD Anderson"))
            .and(query_param("size", "2"))
            .and(query_param("from", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": []
            })))
            .mount(&server)
            .await;

        let client = NciCtsClient::new_for_test(server.uri(), "test-key".into()).unwrap();
        let _ = client
            .search(&NciSearchParams {
                disease: Some(NciDiseaseFilter::Keyword("melanoma".into())),
                interventions: None,
                sites_org_name: Some("MD Anderson".into()),
                status: None,
                phases: Vec::new(),
                geo: None,
                biomarkers: None,
                size: 2,
                from: 0,
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn search_serializes_nci_contract_params() {
        let server = MockServer::start().await;
        let client = NciCtsClient::new_for_test(server.uri(), "test-key".into()).unwrap();

        Mock::given(method("GET"))
            .and(path("/trials"))
            .and(header("X-API-KEY", "test-key"))
            .and(query_param("keyword", "melanoma"))
            .and(query_param_is_missing("diseases"))
            .and(query_param("sites.recruitment_status", "ACTIVE"))
            .and(query_param_is_missing("recruitment_status"))
            .and(query_param("phase", "I_II"))
            .and(query_param("sites.org_coordinates_lat", "41.9742"))
            .and(query_param("sites.org_coordinates_lon", "-87.8073"))
            .and(query_param("sites.org_coordinates_dist", "100mi"))
            .and(query_param("size", "2"))
            .and(query_param("from", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": []
            })))
            .expect(1)
            .mount(&server)
            .await;

        let _ = client
            .search(&NciSearchParams {
                disease: Some(NciDiseaseFilter::Keyword("melanoma".into())),
                interventions: None,
                sites_org_name: None,
                status: Some(NciStatusFilter::SiteRecruitmentStatus("ACTIVE".into())),
                phases: vec!["I_II".into()],
                geo: Some(NciGeoFilter {
                    lat: 41.9742,
                    lon: -87.8073,
                    distance_miles: 100,
                }),
                biomarkers: None,
                size: 2,
                from: 0,
            })
            .await
            .unwrap();

        Mock::given(method("GET"))
            .and(path("/trials"))
            .and(header("X-API-KEY", "test-key"))
            .and(query_param("diseases.nci_thesaurus_concept_id", "C3224"))
            .and(query_param_is_missing("keyword"))
            .and(query_param("current_trial_status", "Complete"))
            .and(query_param_is_missing("recruitment_status"))
            .and(query_param("size", "1"))
            .and(query_param("from", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": []
            })))
            .expect(1)
            .mount(&server)
            .await;

        let _ = client
            .search(&NciSearchParams {
                disease: Some(NciDiseaseFilter::ConceptId("C3224".into())),
                interventions: None,
                sites_org_name: None,
                status: Some(NciStatusFilter::CurrentTrialStatus("Complete".into())),
                phases: Vec::new(),
                geo: None,
                biomarkers: None,
                size: 1,
                from: 0,
            })
            .await
            .unwrap();
    }
}
