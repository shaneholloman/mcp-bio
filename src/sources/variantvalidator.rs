use std::borrow::Cow;

use crate::entities::variant::{
    VariantNormalizationService, VariantNormalizationServiceResult, VariantNormalizationStatus,
};
use crate::error::BioMcpError;

const VARIANTVALIDATOR_BASE: &str = "https://rest.variantvalidator.org";
const VARIANTVALIDATOR_API: &str = "variantvalidator";
const VARIANTVALIDATOR_BASE_ENV: &str = "BIOMCP_VARIANTVALIDATOR_BASE_URL";

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

    fn normalize_url(&self, description: &str) -> Result<reqwest::Url, BioMcpError> {
        let mut url =
            reqwest::Url::parse(self.base.as_ref().trim_end_matches('/')).map_err(|err| {
                BioMcpError::Api {
                    api: VARIANTVALIDATOR_API.to_string(),
                    message: err.to_string(),
                }
            })?;
        url.path_segments_mut()
            .map_err(|_| BioMcpError::Api {
                api: VARIANTVALIDATOR_API.to_string(),
                message: "invalid VariantValidator base URL".to_string(),
            })?
            .pop_if_empty()
            .push("VariantValidator")
            .push("variantvalidator")
            .push("GRCh38")
            .push(description)
            .push("all");
        url.query_pairs_mut()
            .append_pair("content-type", "application/json");
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

        let result = VariantValidatorClient::new_for_test(server.uri())
            .unwrap()
            .normalize("NM_004448.2:c.829G>T")
            .await
            .unwrap();

        let requests = server.received_requests().await.unwrap();
        assert_eq!(
            requests[0].url.path(),
            "/VariantValidator/variantvalidator/GRCh38/NM_004448.2:c.829G%3ET/all"
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
