use std::borrow::Cow;

use crate::entities::variant::{
    VariantNormalizationService, VariantNormalizationServiceResult, VariantNormalizationStatus,
};
use crate::error::BioMcpError;

const VARIANTVALIDATOR_BASE: &str = "https://rest.variantvalidator.org";
const VARIANTVALIDATOR_API: &str = "variantvalidator";
const VARIANTVALIDATOR_BASE_ENV: &str = "BIOMCP_VARIANTVALIDATOR_BASE_URL";

#[allow(dead_code)]
pub struct VariantValidatorNormalizeRequestPlan {
    pub method: &'static str,
    pub path: String,
    pub query_params: Vec<(&'static str, String)>,
    pub cache_mode: &'static str,
    pub status_expectation: &'static str,
    pub content_type_expectation: &'static str,
}

pub struct VariantValidatorClient {
    client: reqwest_middleware::ClientWithMiddleware,
    base: Cow<'static, str>,
}

impl VariantValidatorClient {
    pub fn new() -> Result<Self, BioMcpError> {
        Ok(Self {
            client: crate::sources::shared_client()?,
            base: crate::sources::env_base(VARIANTVALIDATOR_BASE, VARIANTVALIDATOR_BASE_ENV),
        })
    }

    #[cfg(test)]
    fn new_for_test(base: String) -> Result<Self, BioMcpError> {
        Ok(Self {
            client: crate::sources::test_client()?,
            base: Cow::Owned(base),
        })
    }

    fn normalize_path(description: &str) -> Result<String, BioMcpError> {
        let mut url =
            reqwest::Url::parse("https://biomcp.local").map_err(|err| BioMcpError::Api {
                api: VARIANTVALIDATOR_API.to_string(),
                message: err.to_string(),
            })?;
        url.path_segments_mut()
            .map_err(|_| BioMcpError::Api {
                api: VARIANTVALIDATOR_API.to_string(),
                message: "invalid VariantValidator request path".to_string(),
            })?
            .push("VariantValidator")
            .push("variantvalidator")
            .push("GRCh38")
            .push(description)
            .push("all");
        Ok(url.path().to_string())
    }

    fn endpoint_url(&self, path: &str) -> Result<reqwest::Url, BioMcpError> {
        reqwest::Url::parse(&format!(
            "{}/{}",
            self.base.as_ref().trim_end_matches('/'),
            path.trim_start_matches('/')
        ))
        .map_err(|err| BioMcpError::Api {
            api: VARIANTVALIDATOR_API.to_string(),
            message: err.to_string(),
        })
    }

    pub fn normalize_request_plan(
        &self,
        description: &str,
    ) -> Result<VariantValidatorNormalizeRequestPlan, BioMcpError> {
        let path = Self::normalize_path(description)?;
        debug_assert!(path.starts_with("/VariantValidator/variantvalidator/GRCh38/"));
        Ok(VariantValidatorNormalizeRequestPlan {
            method: "GET",
            path,
            query_params: vec![("content-type", "application/json".to_string())],
            cache_mode: "default",
            status_expectation: "400/422 invalid_input; 404 not_found; other non-2xx service_error",
            content_type_expectation: "application/json",
        })
    }

    pub async fn normalize(
        &self,
        description: &str,
    ) -> Result<VariantNormalizationServiceResult, BioMcpError> {
        let plan = self.normalize_request_plan(description)?;
        let mut url = self.endpoint_url(&plan.path)?;
        url.query_pairs_mut().extend_pairs(
            plan.query_params
                .iter()
                .map(|(name, value)| (*name, value.as_str())),
        );
        let resp = crate::sources::apply_cache_mode(self.client.get(url))
            .send()
            .await?;
        let status = resp.status();
        let content_type = resp.headers().get(reqwest::header::CONTENT_TYPE).cloned();
        let bytes = crate::sources::read_limited_body(resp, VARIANTVALIDATOR_API).await?;

        if !status.is_success() {
            return Ok(http_error(status, &bytes));
        }

        if let Err(err) = crate::sources::ensure_json_content_type(
            VARIANTVALIDATOR_API,
            content_type.as_ref(),
            &bytes,
        ) {
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
                        api: VARIANTVALIDATOR_API.to_string(),
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
    let Some((_, result)) = value.as_object().and_then(|object| {
        object.iter().find(|(key, value)| {
            key.as_str() != "flag" && key.as_str() != "metadata" && value.is_object()
        })
    }) else {
        return message_result(
            VariantNormalizationStatus::ServiceError,
            "VariantValidator returned no result object".to_string(),
        );
    };

    let warnings = result
        .get("validation_warnings")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .filter(|v| !v.trim().is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let transcript = string_field(result, "hgvs_transcript_variant");
    let genomic_descriptions = genomic_descriptions(result);
    let protein = result.get("hgvs_predicted_protein_consequence").cloned();

    if transcript.is_none() {
        return VariantNormalizationServiceResult {
            service: VariantNormalizationService::VariantValidator
                .as_str()
                .to_string(),
            status: if warnings.is_empty() {
                VariantNormalizationStatus::ServiceError
            } else {
                VariantNormalizationStatus::InvalidInput
            },
            input_description: string_field(result, "submitted_variant"),
            normalized_description: None,
            corrected_description: None,
            transcript_description: None,
            protein: None,
            genomic_descriptions,
            warnings,
            message: Some(
                "VariantValidator did not return a normalized transcript variant".to_string(),
            ),
        };
    }

    VariantNormalizationServiceResult {
        service: VariantNormalizationService::VariantValidator
            .as_str()
            .to_string(),
        status: VariantNormalizationStatus::Success,
        input_description: string_field(result, "submitted_variant"),
        normalized_description: transcript.clone(),
        corrected_description: None,
        transcript_description: transcript,
        protein,
        genomic_descriptions,
        warnings,
        message: None,
    }
}

fn http_error(status: reqwest::StatusCode, bytes: &[u8]) -> VariantNormalizationServiceResult {
    let message = format!("HTTP {status}: {}", crate::sources::body_excerpt(bytes));
    message_result(
        match status.as_u16() {
            400 | 422 => VariantNormalizationStatus::InvalidInput,
            404 => VariantNormalizationStatus::NotFound,
            _ => VariantNormalizationStatus::ServiceError,
        },
        message,
    )
}

fn message_result(
    status: VariantNormalizationStatus,
    message: String,
) -> VariantNormalizationServiceResult {
    VariantNormalizationServiceResult {
        service: VariantNormalizationService::VariantValidator
            .as_str()
            .to_string(),
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

fn genomic_descriptions(result: &serde_json::Value) -> Vec<String> {
    let mut values = result
        .get("primary_assembly_loci")
        .and_then(|v| v.as_object())
        .into_iter()
        .flat_map(|object| object.values())
        .filter_map(|locus| locus.get("hgvs_genomic_description"))
        .filter_map(|v| v.as_str())
        .filter(|v| !v.trim().is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
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
    use wiremock::matchers::{any, method, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn ticket_376_variant_normalization_contracts_variantvalidator_request_plan_and_mapping() {
        let client = VariantValidatorClient::new_for_test("http://127.0.0.1".into()).unwrap();
        let plan: VariantValidatorNormalizeRequestPlan = client
            .normalize_request_plan("NM_000248.3:c.135del")
            .expect("VariantValidatorNormalizeRequestPlan");
        assert_eq!(plan.method, "GET");
        assert_eq!(
            plan.path,
            "/VariantValidator/variantvalidator/GRCh38/NM_000248.3:c.135del/all"
        );
        assert!(
            plan.query_params
                .contains(&("content-type", "application/json".to_string()))
        );
        let encoded = client
            .normalize_request_plan("NM_004448.2:c.829G>T")
            .expect("encoded plan");
        assert_eq!(
            encoded.path,
            "/VariantValidator/variantvalidator/GRCh38/NM_004448.2:c.829G%3ET/all"
        );

        let value = serde_json::json!({
            "NM_000248.3:c.135del": {
                "submitted_variant": "NM_000248.3:c.135del",
                "hgvs_transcript_variant": "NM_000248.3:c.135del",
                "primary_assembly_loci": {
                    "grch38": {"hgvs_genomic_description": "NC_000003.12:g.69937923del"}
                },
                "validation_warnings": ["TranscriptVersionWarning: transcript updated"]
            }
        });
        let result = result_from_value(&value);
        assert_eq!(result.status, VariantNormalizationStatus::Success);
        assert!(
            result
                .warnings
                .iter()
                .any(|warning| warning.contains("TranscriptVersionWarning"))
        );
        assert!(
            result
                .genomic_descriptions
                .iter()
                .any(|value| value == "NC_000003.12:g.69937923del")
        );
    }

    #[tokio::test]
    async fn normalize_encodes_transcript_path_and_extracts_warnings() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(any())
            .and(query_param("content-type", "application/json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "NM_004448.2:c.829G>T": {
                    "submitted_variant": "NM_004448.2:c.829G>T",
                    "hgvs_transcript_variant": "NM_004448.2:c.829G>T",
                    "primary_assembly_loci": {
                        "grch38": {"hgvs_genomic_description": "NC_000017.11:g.39710409G>T"}
                    },
                    "validation_warnings": ["TranscriptVersionWarning: newer transcript exists"]
                },
                "flag": "gene_variant",
                "metadata": {}
            })))
            .mount(&server)
            .await;

        let result = VariantValidatorClient::new_for_test(format!("{}/api", server.uri()))
            .unwrap()
            .normalize("NM_004448.2:c.829G>T")
            .await
            .unwrap();

        let requests = server.received_requests().await.unwrap();
        assert_eq!(
            requests[0].url.path(),
            "/api/VariantValidator/variantvalidator/GRCh38/NM_004448.2:c.829G%3ET/all"
        );
        assert_eq!(result.status, VariantNormalizationStatus::Success);
        assert!(result.warnings[0].contains("TranscriptVersionWarning"));
        assert!(
            result
                .genomic_descriptions
                .iter()
                .any(|value| value == "NC_000017.11:g.39710409G>T")
        );
    }

    #[test]
    fn result_from_value_maps_warning_without_transcript_to_invalid_input() {
        let value = serde_json::json!({
            "flag": "warning",
            "validation_warning_1": {
                "submitted_variant": "NM_000248.3:c.",
                "validation_warnings": ["LovdSyntaxcheckInvalid"]
            }
        });

        let result = result_from_value(&value);
        assert_eq!(result.status, VariantNormalizationStatus::InvalidInput);
        assert_eq!(result.input_description.as_deref(), Some("NM_000248.3:c."));
    }

    #[test]
    fn result_from_value_maps_missing_transcript_to_service_error() {
        let value = serde_json::json!({
            "flag": "empty",
            "result": {
                "submitted_variant": "NM_000248.3:c.135del"
            }
        });

        let result = result_from_value(&value);
        assert_eq!(result.status, VariantNormalizationStatus::ServiceError);
        assert!(result.normalized_description.is_none());
    }

    #[tokio::test]
    async fn normalize_maps_not_found_and_http_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
            .mount(&server)
            .await;

        let result = VariantValidatorClient::new_for_test(server.uri())
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

        let result = VariantValidatorClient::new_for_test(server.uri())
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

        let result = VariantValidatorClient::new_for_test(server.uri())
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
