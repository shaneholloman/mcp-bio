use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::sync::OnceLock;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::error::BioMcpError;

const GTEX_BASE: &str = "https://gtexportal.org";
const GTEX_API: &str = "gtex";
const GTEX_BASE_ENV: &str = "BIOMCP_GTEX_BASE";
const GTEX_DATASET_ID: &str = "gtex_v8";
const GTEX_GENCODE_VERSION: &str = "v26";
const GTEX_TOP_TISSUES: usize = 10;
const GTEX_LOW_TISSUES: usize = 3;

pub struct GtexClient {
    client: reqwest_middleware::ClientWithMiddleware,
    base: Cow<'static, str>,
}

impl GtexClient {
    pub fn new() -> Result<Self, BioMcpError> {
        Ok(Self {
            client: crate::sources::shared_client()?,
            base: crate::sources::env_base(GTEX_BASE, GTEX_BASE_ENV),
        })
    }

    #[cfg(test)]
    fn new_for_test(base: String) -> Result<Self, BioMcpError> {
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

    async fn get_json<T: DeserializeOwned>(
        &self,
        req: reqwest_middleware::RequestBuilder,
    ) -> Result<T, BioMcpError> {
        let resp = crate::sources::apply_cache_mode(req).send().await?;
        let status = resp.status();
        let content_type = resp.headers().get(reqwest::header::CONTENT_TYPE).cloned();
        let bytes = crate::sources::read_limited_body(resp, GTEX_API).await?;

        if !status.is_success() {
            let excerpt = crate::sources::body_excerpt(&bytes);
            return Err(BioMcpError::Api {
                api: GTEX_API.to_string(),
                message: format!("HTTP {status}: {excerpt}"),
            });
        }

        crate::sources::ensure_json_content_type(GTEX_API, content_type.as_ref(), &bytes)?;
        serde_json::from_slice(&bytes).map_err(|source| BioMcpError::ApiJson {
            api: GTEX_API.to_string(),
            source,
        })
    }

    #[allow(dead_code)]
    pub async fn resolve_versioned_gencode_id(
        &self,
        ensembl_id: &str,
    ) -> Result<Option<String>, BioMcpError> {
        let ensembl_id = normalize_ensembl_id(ensembl_id)?;
        let _guard = gtex_sequence_lock().lock().await;
        self.resolve_versioned_gencode_id_unlocked(&ensembl_id)
            .await
    }

    pub async fn median_gene_expression(
        &self,
        ensembl_id: &str,
    ) -> Result<Vec<TissueExpression>, BioMcpError> {
        let ensembl_id = normalize_ensembl_id(ensembl_id)?;
        let _guard = gtex_sequence_lock().lock().await;
        let Some(versioned_id) = self
            .resolve_versioned_gencode_id_unlocked(&ensembl_id)
            .await?
        else {
            return Ok(Vec::new());
        };
        let rows = self.fetch_median_expression_unlocked(&versioned_id).await?;
        Ok(compact_tissue_rows(rows))
    }

    async fn resolve_versioned_gencode_id_unlocked(
        &self,
        ensembl_id: &str,
    ) -> Result<Option<String>, BioMcpError> {
        let url = self.endpoint("api/v2/reference/geneSearch");
        let resp: GtexGeneSearchResponse = self
            .get_json(self.client.get(&url).query(&[
                ("geneId", ensembl_id),
                ("gencodeVersion", GTEX_GENCODE_VERSION),
            ]))
            .await?;

        let mut first_non_empty: Option<String> = None;
        for row in resp.data {
            let Some(gencode_id) = clean_optional(row.gencode_id) else {
                continue;
            };

            if first_non_empty.is_none() {
                first_non_empty = Some(gencode_id.clone());
            }

            if gencode_id == ensembl_id
                || gencode_id
                    .strip_suffix(".0")
                    .is_some_and(|base| base == ensembl_id)
                || gencode_id.starts_with(&format!("{ensembl_id}."))
            {
                return Ok(Some(gencode_id));
            }
        }

        Ok(first_non_empty)
    }

    async fn fetch_median_expression_unlocked(
        &self,
        versioned_gencode_id: &str,
    ) -> Result<Vec<TissueExpression>, BioMcpError> {
        let url = self.endpoint("api/v2/expression/medianGeneExpression");
        let resp: GtexMedianExpressionResponse = self
            .get_json(self.client.get(&url).query(&[
                ("gencodeId", versioned_gencode_id),
                ("datasetId", GTEX_DATASET_ID),
            ]))
            .await?;

        let mut rows: Vec<TissueExpression> = resp
            .data
            .into_iter()
            .filter_map(|row| {
                let tissue = normalize_tissue_label(row.tissue_site_detail_id)?;
                let median_tpm = row.median?;
                median_tpm
                    .is_finite()
                    .then_some(TissueExpression { tissue, median_tpm })
            })
            .collect();

        rows.sort_by(|a, b| {
            b.median_tpm
                .partial_cmp(&a.median_tpm)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.tissue.cmp(&b.tissue))
        });
        Ok(rows)
    }
}

fn gtex_sequence_lock() -> &'static Mutex<()> {
    static GTEX_SEQUENCE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    GTEX_SEQUENCE_LOCK.get_or_init(|| Mutex::new(()))
}

fn normalize_ensembl_id(value: &str) -> Result<String, BioMcpError> {
    let raw = value.trim().to_ascii_uppercase();
    if raw.is_empty() {
        return Err(BioMcpError::InvalidArgument(
            "Ensembl gene ID is required for GTEx expression".into(),
        ));
    }

    let core = raw.split('.').next().unwrap_or(&raw).trim();
    if core.is_empty() || !core.starts_with("ENSG") {
        return Err(BioMcpError::InvalidArgument(format!(
            "Invalid Ensembl gene ID: {value}"
        )));
    }
    if !core.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(BioMcpError::InvalidArgument(format!(
            "Invalid Ensembl gene ID: {value}"
        )));
    }
    Ok(core.to_string())
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn normalize_tissue_label(value: Option<String>) -> Option<String> {
    let value = value?;
    let value = value.replace('_', " ");
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn compact_tissue_rows(rows: Vec<TissueExpression>) -> Vec<TissueExpression> {
    if rows.len() <= GTEX_TOP_TISSUES + GTEX_LOW_TISSUES {
        return rows;
    }

    let mut out = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for row in rows.iter().take(GTEX_TOP_TISSUES) {
        let key = row.tissue.to_ascii_lowercase();
        if seen.insert(key) {
            out.push(row.clone());
        }
    }

    for row in rows.iter().rev().take(GTEX_LOW_TISSUES) {
        let key = row.tissue.to_ascii_lowercase();
        if seen.insert(key) {
            out.push(row.clone());
        }
    }

    out
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GeneExpression {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tissues: Vec<TissueExpression>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TissueExpression {
    pub tissue: String,
    pub median_tpm: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct GtexGeneSearchResponse {
    #[serde(default)]
    data: Vec<GtexGeneSearchRow>,
}

#[derive(Debug, Clone, Deserialize)]
struct GtexMedianExpressionResponse {
    #[serde(default)]
    data: Vec<GtexMedianExpressionRow>,
}

#[derive(Debug, Clone, Deserialize)]
struct GtexGeneSearchRow {
    #[serde(rename = "gencodeId")]
    gencode_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct GtexMedianExpressionRow {
    median: Option<f64>,
    #[serde(rename = "tissueSiteDetailId")]
    tissue_site_detail_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn resolve_versioned_id_uses_gene_search_endpoint() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/v2/reference/geneSearch"))
            .and(query_param("geneId", "ENSG00000157764"))
            .and(query_param("gencodeVersion", "v26"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {"gencodeId": "ENSG00000157764.12"}
                ]
            })))
            .mount(&server)
            .await;

        let client = GtexClient::new_for_test(server.uri()).expect("client");
        let resolved = client
            .resolve_versioned_gencode_id("ENSG00000157764")
            .await
            .expect("resolved");
        assert_eq!(resolved.as_deref(), Some("ENSG00000157764.12"));
    }

    #[tokio::test]
    async fn median_expression_sorts_and_compacts_to_top_and_low_tissues() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/v2/reference/geneSearch"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {"gencodeId": "ENSG00000157764.12"}
                ]
            })))
            .mount(&server)
            .await;

        let rows = (1..=14)
            .map(|idx| {
                serde_json::json!({
                    "median": idx as f64,
                    "tissueSiteDetailId": format!("Tissue_{idx}")
                })
            })
            .collect::<Vec<_>>();

        Mock::given(method("GET"))
            .and(path("/api/v2/expression/medianGeneExpression"))
            .and(query_param("gencodeId", "ENSG00000157764.12"))
            .and(query_param("datasetId", "gtex_v8"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "data": rows })),
            )
            .mount(&server)
            .await;

        let client = GtexClient::new_for_test(server.uri()).expect("client");
        let tissues = client
            .median_gene_expression("ENSG00000157764")
            .await
            .expect("expression");

        assert_eq!(tissues.len(), 13);
        assert_eq!(
            tissues.first().map(|row| row.tissue.as_str()),
            Some("Tissue 14")
        );
        assert!(tissues.iter().any(|row| row.tissue == "Tissue 1"));
        assert!(!tissues.iter().any(|row| row.tissue == "Tissue 4"));
    }

    #[tokio::test]
    async fn median_expression_returns_empty_when_gene_search_has_no_match() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/v2/reference/geneSearch"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": []
            })))
            .mount(&server)
            .await;

        let client = GtexClient::new_for_test(server.uri()).expect("client");
        let tissues = client
            .median_gene_expression("ENSG00000157764")
            .await
            .expect("expression");
        assert!(tissues.is_empty());
    }

    #[tokio::test]
    async fn median_expression_requests_are_serialized() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/v2/reference/geneSearch"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_millis(100))
                    .set_body_json(serde_json::json!({
                        "data": [
                            {"gencodeId": "ENSG00000157764.12"}
                        ]
                    })),
            )
            .expect(2)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/api/v2/expression/medianGeneExpression"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_millis(100))
                    .set_body_json(serde_json::json!({
                        "data": [
                            {"median": 1.0, "tissueSiteDetailId": "Tissue_1"}
                        ]
                    })),
            )
            .expect(2)
            .mount(&server)
            .await;

        let client = std::sync::Arc::new(GtexClient::new_for_test(server.uri()).expect("client"));
        let elapsed = crate::sources::with_no_cache(true, async {
            let start = tokio::time::Instant::now();
            let first = {
                let client = client.clone();
                async move { client.median_gene_expression("ENSG00000157764").await }
            };
            let second = {
                let client = client.clone();
                async move { client.median_gene_expression("ENSG00000157764").await }
            };
            let (one, two) = tokio::join!(first, second);
            one.expect("first request");
            two.expect("second request");
            start.elapsed()
        })
        .await;

        assert!(elapsed >= Duration::from_millis(350));
    }
}
