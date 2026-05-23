use std::borrow::Cow;

use crate::entities::variant::{
    VariantNormalizationService, VariantNormalizationServiceResult, VariantNormalizationStatus,
};
use crate::error::BioMcpError;

const MUTALYZER_BASE: &str = "https://mutalyzer.nl/api";
const MUTALYZER_API: &str = "mutalyzer";
const MUTALYZER_BASE_ENV: &str = "BIOMCP_MUTALYZER_BASE_URL";

pub struct MutalyzerClient {
    client: reqwest_middleware::ClientWithMiddleware,
    base: Cow<'static, str>,
}

impl MutalyzerClient {
    pub fn new() -> Result<Self, BioMcpError> {
        Ok(Self {
            client: crate::sources::shared_client()?,
            base: crate::sources::env_base(MUTALYZER_BASE, MUTALYZER_BASE_ENV),
        })
    }

    #[cfg(test)]
    fn new_for_test(base: String) -> Result<Self, BioMcpError> {
        Ok(Self {
            client: crate::sources::test_client()?,
            base: Cow::Owned(base),
        })
    }

    fn normalize_url(&self, description: &str) -> Result<reqwest::Url, BioMcpError> {
        let mut url =
            reqwest::Url::parse(self.base.as_ref().trim_end_matches('/')).map_err(|err| {
                BioMcpError::Api {
                    api: MUTALYZER_API.to_string(),
                    message: err.to_string(),
                }
            })?;
        url.path_segments_mut()
            .map_err(|_| BioMcpError::Api {
                api: MUTALYZER_API.to_string(),
                message: "invalid Mutalyzer base URL".to_string(),
            })?
            .pop_if_empty()
            .push("normalize")
            .push(description);
        Ok(url)
    }

    pub async fn normalize(
        &self,
        description: &str,
    ) -> Result<VariantNormalizationServiceResult, BioMcpError> {
        let url = self.normalize_url(description)?;
        let resp = crate::sources::apply_cache_mode(self.client.get(url))
            .send()
            .await?;
        let status = resp.status();
        let content_type = resp.headers().get(reqwest::header::CONTENT_TYPE).cloned();
        let bytes = crate::sources::read_limited_body(resp, MUTALYZER_API).await?;

        if !status.is_success() {
            return Ok(provider_error(status, &bytes));
        }

        if let Err(err) =
            crate::sources::ensure_json_content_type(MUTALYZER_API, content_type.as_ref(), &bytes)
        {
            return Ok(message_result(
                VariantNormalizationStatus::ServiceError,
                err.to_string(),
            ));
        }
        let value: serde_json::Value = match serde_json::from_slice(&bytes) {
            Ok(value) => value,
            Err(source) => {
                return Ok(message_result(
                    VariantNormalizationStatus::ServiceError,
                    BioMcpError::ApiJson {
                        api: MUTALYZER_API.to_string(),
                        source,
                    }
                    .to_string(),
                ));
            }
        };
        Ok(result_from_value(&value))
    }
}

fn result_from_value(value: &serde_json::Value) -> VariantNormalizationServiceResult {
    let warnings = value
        .get("infos")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|item| {
            item.get("details")
                .and_then(|v| v.as_str())
                .or_else(|| item.get("code").and_then(|v| v.as_str()))
        })
        .map(str::to_string)
        .collect::<Vec<_>>();

    let normalized_description = string_field(value, "normalized_description");
    let message = string_field(value, "message");
    let input_description = string_field(value, "input_description").or_else(|| {
        value
            .pointer("/custom/input_description")
            .and_then(|v| v.as_str())
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string)
    });

    if normalized_description.is_none() {
        return VariantNormalizationServiceResult {
            service: VariantNormalizationService::Mutalyzer.as_str().to_string(),
            status: if message.is_some() {
                VariantNormalizationStatus::InvalidInput
            } else {
                VariantNormalizationStatus::ServiceError
            },
            input_description,
            normalized_description: None,
            corrected_description: None,
            transcript_description: None,
            protein: None,
            genomic_descriptions: Vec::new(),
            warnings,
            message: Some(
                message
                    .unwrap_or_else(|| "Mutalyzer returned no normalized description".to_string()),
            ),
        };
    }

    VariantNormalizationServiceResult {
        service: VariantNormalizationService::Mutalyzer.as_str().to_string(),
        status: VariantNormalizationStatus::Success,
        input_description,
        normalized_description,
        corrected_description: string_field(value, "corrected_description"),
        transcript_description: None,
        protein: value.get("protein").cloned(),
        genomic_descriptions: Vec::new(),
        warnings,
        message: None,
    }
}

fn provider_error(status: reqwest::StatusCode, bytes: &[u8]) -> VariantNormalizationServiceResult {
    let value = serde_json::from_slice::<serde_json::Value>(bytes).ok();
    let message = value
        .as_ref()
        .and_then(|v| v.get("message"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| format!("HTTP {status}: {}", crate::sources::body_excerpt(bytes)));
    let input_description = value
        .as_ref()
        .and_then(|v| v.pointer("/custom/input_description"))
        .and_then(|v| v.as_str())
        .map(str::to_string);

    VariantNormalizationServiceResult {
        service: VariantNormalizationService::Mutalyzer.as_str().to_string(),
        status: match status.as_u16() {
            400 | 422 => VariantNormalizationStatus::InvalidInput,
            404 => VariantNormalizationStatus::NotFound,
            _ => VariantNormalizationStatus::ServiceError,
        },
        input_description,
        normalized_description: None,
        corrected_description: None,
        transcript_description: None,
        protein: None,
        genomic_descriptions: Vec::new(),
        warnings: Vec::new(),
        message: Some(message),
    }
}

fn message_result(
    status: VariantNormalizationStatus,
    message: String,
) -> VariantNormalizationServiceResult {
    VariantNormalizationServiceResult {
        service: VariantNormalizationService::Mutalyzer.as_str().to_string(),
        status,
        input_description: None,
        normalized_description: None,
        corrected_description: None,
        transcript_description: None,
        protein: None,
        genomic_descriptions: Vec::new(),
        warnings: Vec::new(),
        message: Some(message),
    }
}

fn string_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .filter(|v| !v.trim().is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{any, method};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn normalize_encodes_transcript_path_and_parses_success() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(any())
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "input_description": "NM_004448.2:c.829G>T",
                "normalized_description": "NM_004448.2:c.829G>T",
                "protein": {"description": "NP_004439.2:p.(Asp277Tyr)"},
                "infos": [{"details": "source warning"}]
            })))
            .mount(&server)
            .await;

        let result = MutalyzerClient::new_for_test(server.uri())
            .unwrap()
            .normalize("NM_004448.2:c.829G>T")
            .await
            .unwrap();

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests[0].url.path(), "/normalize/NM_004448.2:c.829G%3ET");
        assert_eq!(result.status, VariantNormalizationStatus::Success);
        assert_eq!(
            result.normalized_description.as_deref(),
            Some("NM_004448.2:c.829G>T")
        );
        assert!(
            result
                .warnings
                .iter()
                .any(|warning| warning == "source warning")
        );
    }

    #[tokio::test]
    async fn normalize_maps_provider_invalid_input() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(422).set_body_json(serde_json::json!({
                "message": "Errors encountered. Check the custom field.",
                "custom": {"input_description": "NM_000248.3:c."}
            })))
            .mount(&server)
            .await;

        let result = MutalyzerClient::new_for_test(server.uri())
            .unwrap()
            .normalize("NM_000248.3:c.")
            .await
            .unwrap();

        assert_eq!(result.status, VariantNormalizationStatus::InvalidInput);
        assert_eq!(result.input_description.as_deref(), Some("NM_000248.3:c."));
    }

    #[tokio::test]
    async fn normalize_maps_success_status_error_payload_to_invalid_input() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": "Errors encountered. Check the custom field.",
                "custom": {"input_description": "NM_000248.3:c."}
            })))
            .mount(&server)
            .await;

        let result = MutalyzerClient::new_for_test(server.uri())
            .unwrap()
            .normalize("NM_000248.3:c.")
            .await
            .unwrap();

        assert_eq!(result.status, VariantNormalizationStatus::InvalidInput);
        assert_eq!(result.input_description.as_deref(), Some("NM_000248.3:c."));
        assert!(result.normalized_description.is_none());
    }

    #[tokio::test]
    async fn normalize_maps_not_found_and_http_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
            .mount(&server)
            .await;

        let result = MutalyzerClient::new_for_test(server.uri())
            .unwrap()
            .normalize("NM_000248.3:c.135del")
            .await
            .unwrap();

        assert_eq!(result.status, VariantNormalizationStatus::NotFound);
        assert!(
            result
                .message
                .as_deref()
                .unwrap_or_default()
                .contains("HTTP 404")
        );

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(500).set_body_string("upstream failed"))
            .mount(&server)
            .await;

        let result = MutalyzerClient::new_for_test(server.uri())
            .unwrap()
            .normalize("NM_000248.3:c.135del")
            .await
            .unwrap();

        assert_eq!(result.status, VariantNormalizationStatus::ServiceError);
        assert!(
            result
                .message
                .as_deref()
                .unwrap_or_default()
                .contains("HTTP 500")
        );
    }

    #[tokio::test]
    async fn normalize_maps_html_response_to_service_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/html")
                    .set_body_string("<html>maintenance</html>"),
            )
            .mount(&server)
            .await;

        let result = MutalyzerClient::new_for_test(server.uri())
            .unwrap()
            .normalize("NM_000248.3:c.135del")
            .await
            .unwrap();

        assert_eq!(result.status, VariantNormalizationStatus::ServiceError);
        assert!(
            result
                .message
                .as_deref()
                .is_some_and(|message| !message.is_empty())
        );
    }
}
