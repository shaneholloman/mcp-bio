use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashMap;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::error::BioMcpError;

const CLINGEN_BASE: &str = "https://search.clinicalgenome.org";
const CLINGEN_API: &str = "clingen";
const CLINGEN_BASE_ENV: &str = "BIOMCP_CLINGEN_BASE";
const CLINGEN_VALIDITY_PATH: &str = "kb/gene-validity/download";
const CLINGEN_DOSAGE_PATH: &str = "kb/gene-dosage/download";

pub struct ClinGenClient {
    client: reqwest_middleware::ClientWithMiddleware,
    base: Cow<'static, str>,
}

impl ClinGenClient {
    pub fn new() -> Result<Self, BioMcpError> {
        Ok(Self {
            client: crate::sources::shared_client()?,
            base: crate::sources::env_base(CLINGEN_BASE, CLINGEN_BASE_ENV),
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

    async fn get_text(
        &self,
        req: reqwest_middleware::RequestBuilder,
        api: &str,
    ) -> Result<String, BioMcpError> {
        let resp = crate::sources::apply_cache_mode(req).send().await?;
        let status = resp.status();
        let bytes = crate::sources::read_limited_body(resp, api).await?;

        if !status.is_success() {
            let excerpt = crate::sources::body_excerpt(&bytes);
            return Err(BioMcpError::Api {
                api: api.to_string(),
                message: format!("HTTP {status}: {excerpt}"),
            });
        }

        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        req: reqwest_middleware::RequestBuilder,
        api: &str,
    ) -> Result<T, BioMcpError> {
        let resp = crate::sources::apply_cache_mode(req).send().await?;
        let status = resp.status();
        let content_type = resp.headers().get(reqwest::header::CONTENT_TYPE).cloned();
        let bytes = crate::sources::read_limited_body(resp, api).await?;

        if !status.is_success() {
            let excerpt = crate::sources::body_excerpt(&bytes);
            return Err(BioMcpError::Api {
                api: api.to_string(),
                message: format!("HTTP {status}: {excerpt}"),
            });
        }

        // ClinGen's gene lookup endpoint can return JSON with a text/html content type.
        // Accept JSON-shaped payloads in that specific mismatch case.
        let allow_mislabeled_json = content_type
            .as_ref()
            .is_some_and(|header| is_html_content_type(header) && looks_like_json(&bytes));
        if !allow_mislabeled_json {
            crate::sources::ensure_json_content_type(api, content_type.as_ref(), &bytes)?;
        }

        serde_json::from_slice(&bytes).map_err(|source| BioMcpError::ApiJson {
            api: api.to_string(),
            source,
        })
    }

    #[cfg(test)]
    pub async fn gene_validity(
        &self,
        gene_symbol: &str,
    ) -> Result<Vec<ClinGenValidity>, BioMcpError> {
        let symbol = normalize_gene_symbol(gene_symbol)?;
        let hgnc_id = self.lookup_hgnc_id(&symbol).await.unwrap_or_else(|err| {
            warn!(symbol = %symbol, "ClinGen gene lookup failed, falling back to symbol matching: {err}");
            None
        });

        let csv_payload = self
            .get_text(
                self.client.get(self.endpoint(CLINGEN_VALIDITY_PATH)),
                CLINGEN_API,
            )
            .await?;
        parse_validity_csv(&csv_payload, &symbol, hgnc_id.as_deref())
    }

    #[cfg(test)]
    pub async fn dosage_sensitivity(
        &self,
        gene_symbol: &str,
    ) -> Result<(Option<String>, Option<String>), BioMcpError> {
        let symbol = normalize_gene_symbol(gene_symbol)?;
        let hgnc_id = self.lookup_hgnc_id(&symbol).await.unwrap_or_else(|err| {
            warn!(symbol = %symbol, "ClinGen gene lookup failed, falling back to symbol matching: {err}");
            None
        });

        let csv_payload = self
            .get_text(
                self.client.get(self.endpoint(CLINGEN_DOSAGE_PATH)),
                CLINGEN_API,
            )
            .await?;
        parse_dosage_csv(&csv_payload, &symbol, hgnc_id.as_deref())
    }

    pub async fn gene_context(&self, gene_symbol: &str) -> Result<GeneClinGen, BioMcpError> {
        let symbol = normalize_gene_symbol(gene_symbol)?;
        let hgnc_fut = self.lookup_hgnc_id(&symbol);
        let validity_fut = self.get_text(
            self.client.get(self.endpoint(CLINGEN_VALIDITY_PATH)),
            CLINGEN_API,
        );
        let dosage_fut = self.get_text(
            self.client.get(self.endpoint(CLINGEN_DOSAGE_PATH)),
            CLINGEN_API,
        );
        let (hgnc_result, validity_csv, dosage_csv) =
            tokio::join!(hgnc_fut, validity_fut, dosage_fut);
        let hgnc_id = hgnc_result.unwrap_or_else(|err| {
            warn!(symbol = %symbol, "ClinGen gene lookup failed, falling back to symbol matching: {err}");
            None
        });
        let validity_csv = validity_csv?;
        let dosage_csv = dosage_csv?;
        let validity = parse_validity_csv(&validity_csv, &symbol, hgnc_id.as_deref())?;
        let (haploinsufficiency, triplosensitivity) =
            parse_dosage_csv(&dosage_csv, &symbol, hgnc_id.as_deref())?;

        Ok(GeneClinGen {
            validity,
            haploinsufficiency,
            triplosensitivity,
        })
    }

    async fn lookup_hgnc_id(&self, gene_symbol: &str) -> Result<Option<String>, BioMcpError> {
        let url = self.endpoint(&format!("api/genes/look/{gene_symbol}"));
        let rows: Vec<ClinGenLookupGeneRow> =
            self.get_json(self.client.get(&url), CLINGEN_API).await?;
        if rows.is_empty() {
            return Ok(None);
        }

        let is_exact = |row: &ClinGenLookupGeneRow| {
            row.label
                .as_deref()
                .map(str::trim)
                .is_some_and(|label| label.eq_ignore_ascii_case(gene_symbol))
        };

        let pick = rows
            .iter()
            .find(|row| row.curated.unwrap_or(false) && is_exact(row))
            .or_else(|| rows.iter().find(|row| is_exact(row)))
            .or_else(|| rows.iter().find(|row| row.curated.unwrap_or(false)))
            .or_else(|| rows.first());

        Ok(pick.and_then(|row| clean_optional(row.hgnc.clone())))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GeneClinGen {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub validity: Vec<ClinGenValidity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub haploinsufficiency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub triplosensitivity: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClinGenValidity {
    pub disease: String,
    pub classification: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moi: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ClinGenLookupGeneRow {
    label: Option<String>,
    hgnc: Option<String>,
    curated: Option<bool>,
}

fn normalize_gene_symbol(value: &str) -> Result<String, BioMcpError> {
    let normalized = value.trim().to_ascii_uppercase();
    if normalized.is_empty() {
        return Err(BioMcpError::InvalidArgument(
            "Gene symbol is required for ClinGen".into(),
        ));
    }
    if !crate::sources::is_valid_gene_symbol(&normalized) {
        return Err(BioMcpError::InvalidArgument(format!(
            "Invalid gene symbol: {value}"
        )));
    }
    Ok(normalized)
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn is_html_content_type(header: &reqwest::header::HeaderValue) -> bool {
    let Ok(raw) = header.to_str() else {
        return false;
    };
    let media_type = raw
        .split(';')
        .next()
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(media_type.as_str(), "text/html" | "application/xhtml+xml")
}

fn looks_like_json(body: &[u8]) -> bool {
    body.iter()
        .find(|b| !b.is_ascii_whitespace())
        .is_some_and(|b| matches!(*b, b'{' | b'['))
}

fn clean_field(record: &csv::StringRecord, headers: &HashMap<String, usize>, name: &str) -> String {
    headers
        .get(name)
        .and_then(|idx| record.get(*idx))
        .map(str::trim)
        .unwrap_or("")
        .to_string()
}

fn normalize_header(value: &str) -> String {
    value
        .trim_matches('\u{feff}')
        .trim()
        .to_ascii_uppercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_separator_row(record: &csv::StringRecord) -> bool {
    record
        .iter()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .all(|value| value.chars().all(|ch| ch == '+'))
}

fn header_map(record: &csv::StringRecord) -> HashMap<String, usize> {
    record
        .iter()
        .enumerate()
        .map(|(idx, col)| (normalize_header(col), idx))
        .collect()
}

fn matches_gene(symbol: &str, hgnc_id: Option<&str>, row_symbol: &str, row_hgnc: &str) -> bool {
    if let Some(hgnc_id) = hgnc_id
        && !hgnc_id.trim().is_empty()
        && row_hgnc.eq_ignore_ascii_case(hgnc_id.trim())
    {
        return true;
    }
    row_symbol.eq_ignore_ascii_case(symbol)
}

fn normalize_review_date(value: &str) -> Option<String> {
    let value = value.trim();
    if value.len() < 10 {
        return None;
    }
    let prefix = &value[..10];
    let bytes = prefix.as_bytes();
    let valid = bytes.len() == 10
        && bytes[0..4].iter().all(|b| b.is_ascii_digit())
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(|b| b.is_ascii_digit())
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(|b| b.is_ascii_digit());
    valid.then(|| prefix.to_string())
}

fn parse_validity_csv(
    csv_payload: &str,
    symbol: &str,
    hgnc_id: Option<&str>,
) -> Result<Vec<ClinGenValidity>, BioMcpError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(csv_payload.as_bytes());

    let mut headers: Option<HashMap<String, usize>> = None;
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for row in reader.records() {
        let row = row.map_err(|err| BioMcpError::Api {
            api: CLINGEN_API.to_string(),
            message: format!("Failed to parse gene validity CSV: {err}"),
        })?;

        if row.iter().all(|value| value.trim().is_empty()) || is_separator_row(&row) {
            continue;
        }

        if headers.is_none() {
            let map = header_map(&row);
            if map.contains_key("GENE SYMBOL")
                && map.contains_key("DISEASE LABEL")
                && map.contains_key("CLASSIFICATION")
            {
                headers = Some(map);
            }
            continue;
        }

        let headers = headers.as_ref().expect("header map initialized");
        let row_symbol = clean_field(&row, headers, "GENE SYMBOL");
        let row_hgnc = clean_field(&row, headers, "GENE ID (HGNC)");
        if !matches_gene(symbol, hgnc_id, &row_symbol, &row_hgnc) {
            continue;
        }

        let disease = clean_field(&row, headers, "DISEASE LABEL");
        let classification = clean_field(&row, headers, "CLASSIFICATION");
        if disease.is_empty() || classification.is_empty() {
            continue;
        }

        let review_date = normalize_review_date(&clean_field(&row, headers, "CLASSIFICATION DATE"));
        let moi = clean_optional(Some(clean_field(&row, headers, "MOI")));
        let unique_key = format!(
            "{disease}|{classification}|{}",
            review_date.as_deref().unwrap_or("")
        );
        if !seen.insert(unique_key) {
            continue;
        }

        out.push(ClinGenValidity {
            disease,
            classification,
            review_date,
            moi,
        });
    }

    out.sort_by(|a, b| {
        b.review_date
            .cmp(&a.review_date)
            .then_with(|| a.disease.cmp(&b.disease))
            .then_with(|| a.classification.cmp(&b.classification))
    });
    out.truncate(5);
    Ok(out)
}

fn parse_dosage_csv(
    csv_payload: &str,
    symbol: &str,
    hgnc_id: Option<&str>,
) -> Result<(Option<String>, Option<String>), BioMcpError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(csv_payload.as_bytes());

    let mut headers: Option<HashMap<String, usize>> = None;
    let mut best: Option<(Option<String>, Option<String>, Option<String>)> = None;

    for row in reader.records() {
        let row = row.map_err(|err| BioMcpError::Api {
            api: CLINGEN_API.to_string(),
            message: format!("Failed to parse dosage CSV: {err}"),
        })?;

        if row.iter().all(|value| value.trim().is_empty()) || is_separator_row(&row) {
            continue;
        }

        if headers.is_none() {
            let map = header_map(&row);
            if map.contains_key("GENE SYMBOL")
                && map.contains_key("HGNC ID")
                && map.contains_key("HAPLOINSUFFICIENCY")
                && map.contains_key("TRIPLOSENSITIVITY")
            {
                headers = Some(map);
            }
            continue;
        }

        let headers = headers.as_ref().expect("header map initialized");
        let row_symbol = clean_field(&row, headers, "GENE SYMBOL");
        let row_hgnc = clean_field(&row, headers, "HGNC ID");
        if !matches_gene(symbol, hgnc_id, &row_symbol, &row_hgnc) {
            continue;
        }

        let haplo = clean_optional(Some(clean_field(&row, headers, "HAPLOINSUFFICIENCY")));
        let triplo = clean_optional(Some(clean_field(&row, headers, "TRIPLOSENSITIVITY")));
        if haplo.is_none() && triplo.is_none() {
            continue;
        }
        let date = normalize_review_date(&clean_field(&row, headers, "DATE"));

        let replace = match &best {
            None => true,
            Some((_, _, current_date)) => match (date.as_ref(), current_date.as_ref()) {
                (Some(new), Some(current)) => new.cmp(current) == Ordering::Greater,
                (Some(_), None) => true,
                _ => false,
            },
        };

        if replace {
            best = Some((haplo, triplo, date));
        }
    }

    Ok(best
        .map(|(haplo, triplo, _)| (haplo, triplo))
        .unwrap_or((None, None)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const LOOKUP_BRAF: &str = r#"[{"label":"BRAF","hgnc":"HGNC:1097","curated":true}]"#;
    const LOOKUP_BRAF_HTML_CONTENT_TYPE: &str =
        r#"[{"label":"BRAF","hgnc":"HGNC:1097","curated":true}]"#;

    const VALIDITY_FIXTURE: &str = r#""CLINGEN GENE DISEASE VALIDITY CURATIONS","","","","","","","","",""
"FILE CREATED: 2026-03-06","","","","","","","","",""
"+++++++++++","++++++++++++++","+++++++++++++","++++++++++++++++++","+++++++++","+++++++++","++++++++++++++","+++++++++++++","+++++++++++++++++++","+++++++++++++++++++"
"GENE SYMBOL","GENE ID (HGNC)","DISEASE LABEL","DISEASE ID (MONDO)","MOI","SOP","CLASSIFICATION","ONLINE REPORT","CLASSIFICATION DATE","GCEP"
"+++++++++++","++++++++++++++","+++++++++++++","++++++++++++++++++","+++++++++","+++++++++","++++++++++++++","+++++++++++++","+++++++++++++++++++","+++++++++++++++++++"
"BRAF","HGNC:1097","Noonan syndrome","MONDO:0018997","AD","SOP10","Moderate","https://example.org/r1","2023-05-01T16:00:00.000Z","Panel A"
"BRAF","HGNC:1097","cardiofaciocutaneous syndrome","MONDO:0015280","AD","SOP10","Definitive","https://example.org/r2","2024-01-12T16:00:00.000Z","Panel B"
"TP53","HGNC:11998","Li-Fraumeni syndrome","MONDO:0018874","AD","SOP9","Definitive","https://example.org/r3","2022-03-01T16:00:00.000Z","Panel C"
"#;

    const VALIDITY_HGNC_ONLY_FIXTURE: &str = r#""GENE SYMBOL","GENE ID (HGNC)","DISEASE LABEL","CLASSIFICATION","CLASSIFICATION DATE","MOI"
"BRAF1","HGNC:1097","Noonan syndrome","Moderate","2024-11-01T16:00:00.000Z","AD"
"#;

    const DOSAGE_FIXTURE: &str = r#""CLINGEN DOSAGE SENSITIVITY CURATIONS","","","","",""
"FILE CREATED: 2026-03-06","","","","",""
"+++++++++++","+++++++","++++++++++++++++++","+++++++++++++++++","+++++++++++++","++++"
"GENE SYMBOL","HGNC ID","HAPLOINSUFFICIENCY","TRIPLOSENSITIVITY","ONLINE REPORT","DATE"
"+++++++++++","+++++++","++++++++++++++++++","+++++++++++++++++","+++++++++++++","++++"
"BRAF","HGNC:1097","No Evidence for Haploinsufficiency","No Evidence for Triplosensitivity","https://example.org/d1","2024-07-01T10:00:00+00:00"
"BRAF","HGNC:1097","Sufficient Evidence for Haploinsufficiency","No Evidence for Triplosensitivity","https://example.org/d2","2025-09-24T13:02:09-04:00"
"TP53","HGNC:11998","Sufficient Evidence for Haploinsufficiency","No Evidence for Triplosensitivity","https://example.org/d3","2024-01-01T10:00:00+00:00"
"#;

    #[tokio::test]
    async fn gene_validity_parses_csv_with_metadata_rows() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/genes/look/BRAF"))
            .respond_with(ResponseTemplate::new(200).set_body_string(LOOKUP_BRAF))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/kb/gene-validity/download"))
            .respond_with(ResponseTemplate::new(200).set_body_string(VALIDITY_FIXTURE))
            .mount(&server)
            .await;

        let client = ClinGenClient::new_for_test(server.uri()).expect("client");
        let validity = client.gene_validity("BRAF").await.expect("validity");

        assert_eq!(validity.len(), 2);
        assert_eq!(validity[0].disease, "cardiofaciocutaneous syndrome");
        assert_eq!(validity[0].classification, "Definitive");
        assert_eq!(validity[0].review_date.as_deref(), Some("2024-01-12"));
        assert_eq!(validity[1].review_date.as_deref(), Some("2023-05-01"));
    }

    #[tokio::test]
    async fn dosage_sensitivity_parses_csv_and_picks_latest_row() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/genes/look/BRAF"))
            .respond_with(ResponseTemplate::new(200).set_body_string(LOOKUP_BRAF))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/kb/gene-dosage/download"))
            .respond_with(ResponseTemplate::new(200).set_body_string(DOSAGE_FIXTURE))
            .mount(&server)
            .await;

        let client = ClinGenClient::new_for_test(server.uri()).expect("client");
        let (haplo, triplo) = client.dosage_sensitivity("BRAF").await.expect("dosage");

        assert_eq!(
            haplo.as_deref(),
            Some("Sufficient Evidence for Haploinsufficiency")
        );
        assert_eq!(triplo.as_deref(), Some("No Evidence for Triplosensitivity"));
    }

    #[tokio::test]
    async fn gene_context_reuses_lookup_for_validity_and_dosage() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/genes/look/BRAF"))
            .respond_with(ResponseTemplate::new(200).set_body_string(LOOKUP_BRAF))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/kb/gene-validity/download"))
            .respond_with(ResponseTemplate::new(200).set_body_string(VALIDITY_FIXTURE))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/kb/gene-dosage/download"))
            .respond_with(ResponseTemplate::new(200).set_body_string(DOSAGE_FIXTURE))
            .expect(1)
            .mount(&server)
            .await;

        let client = ClinGenClient::new_for_test(server.uri()).expect("client");
        let context = client.gene_context("BRAF").await.expect("context");

        assert_eq!(context.validity.len(), 2);
        assert_eq!(
            context.haploinsufficiency.as_deref(),
            Some("Sufficient Evidence for Haploinsufficiency")
        );
        assert_eq!(
            context.triplosensitivity.as_deref(),
            Some("No Evidence for Triplosensitivity")
        );
    }

    #[tokio::test]
    async fn clingen_parsers_handle_missing_gene_rows_cleanly() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/genes/look/BRAF"))
            .respond_with(ResponseTemplate::new(200).set_body_string(LOOKUP_BRAF))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/kb/gene-validity/download"))
            .respond_with(ResponseTemplate::new(200).set_body_string(VALIDITY_FIXTURE))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/kb/gene-dosage/download"))
            .respond_with(ResponseTemplate::new(200).set_body_string(DOSAGE_FIXTURE))
            .mount(&server)
            .await;

        let client = ClinGenClient::new_for_test(server.uri()).expect("client");
        let validity = client.gene_validity("NRAS").await.expect("validity");
        let dosage = client.dosage_sensitivity("NRAS").await.expect("dosage");
        assert!(validity.is_empty());
        assert_eq!(dosage, (None, None));
    }

    #[tokio::test]
    async fn lookup_accepts_json_with_html_content_type() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/genes/look/BRAF"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/html; charset=UTF-8")
                    .set_body_string(LOOKUP_BRAF_HTML_CONTENT_TYPE),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/kb/gene-validity/download"))
            .respond_with(ResponseTemplate::new(200).set_body_string(VALIDITY_HGNC_ONLY_FIXTURE))
            .mount(&server)
            .await;

        let client = ClinGenClient::new_for_test(server.uri()).expect("client");
        let validity = client.gene_validity("BRAF").await.expect("validity");
        assert_eq!(validity.len(), 1);
        assert_eq!(validity[0].disease, "Noonan syndrome");
    }
}
