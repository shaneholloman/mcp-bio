use std::borrow::Cow;
use std::collections::HashSet;

use http_cache_reqwest::CacheMode;
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::error::BioMcpError;

const WIKIPATHWAYS_BASE: &str = "https://www.wikipathways.org/json";
const WIKIPATHWAYS_API: &str = "wikipathways";
const WIKIPATHWAYS_BASE_ENV: &str = "BIOMCP_WIKIPATHWAYS_BASE";
const WIKIPATHWAYS_MAX_BODY_BYTES: usize = 24 * 1024 * 1024;

pub struct WikiPathwaysClient {
    client: reqwest_middleware::ClientWithMiddleware,
    base: Cow<'static, str>,
}

impl WikiPathwaysClient {
    pub fn new() -> Result<Self, BioMcpError> {
        Ok(Self {
            client: crate::sources::shared_client()?,
            base: crate::sources::env_base(WIKIPATHWAYS_BASE, WIKIPATHWAYS_BASE_ENV),
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

    fn search_request(&self) -> reqwest_middleware::RequestBuilder {
        let url = self.endpoint("findPathwaysByText.json");
        self.client.get(&url).with_extension(CacheMode::NoStore)
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        req: reqwest_middleware::RequestBuilder,
    ) -> Result<T, BioMcpError> {
        let resp = crate::sources::apply_cache_mode(req).send().await?;
        let status = resp.status();
        let content_type = resp.headers().get(reqwest::header::CONTENT_TYPE).cloned();
        let bytes = crate::sources::read_limited_body_with_limit(
            resp,
            WIKIPATHWAYS_API,
            WIKIPATHWAYS_MAX_BODY_BYTES,
        )
        .await?;
        if !status.is_success() {
            let excerpt = crate::sources::summarize_http_error_body(content_type.as_ref(), &bytes);
            return Err(BioMcpError::Api {
                api: WIKIPATHWAYS_API.to_string(),
                message: format!("HTTP {status}: {excerpt}"),
            });
        }
        crate::sources::ensure_json_content_type(WIKIPATHWAYS_API, content_type.as_ref(), &bytes)?;
        serde_json::from_slice(&bytes).map_err(|source| BioMcpError::ApiJson {
            api: WIKIPATHWAYS_API.to_string(),
            source,
        })
    }

    pub async fn search_pathways(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<WikiPathwaysHit>, BioMcpError> {
        let query = query.trim();
        if query.is_empty() {
            return Err(BioMcpError::InvalidArgument(
                "WikiPathways query is required".into(),
            ));
        }

        let resp: WikiPathwaysSearchResponse = self.get_json(self.search_request()).await?;

        let mut ranked = Vec::new();
        let mut seen = HashSet::new();
        for row in resp.entries() {
            let is_human = row.species_is_human();
            let Some(score) = row.match_score(query) else {
                continue;
            };
            let Some(id) = row
                .id
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty() && is_wikipathways_id(value) && is_human)
            else {
                continue;
            };
            let Some(name) = row
                .name
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            if !seen.insert(id.clone()) {
                continue;
            }
            ranked.push((score, id, name));
        }

        ranked.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.2.cmp(&right.2))
                .then_with(|| left.1.cmp(&right.1))
        });

        Ok(ranked
            .into_iter()
            .take(limit.clamp(1, 25))
            .map(|(_, id, name)| WikiPathwaysHit { id, name })
            .collect())
    }

    pub async fn get_pathway(&self, pw_id: &str) -> Result<WikiPathwaysRecord, BioMcpError> {
        let pw_id = validate_wikipathways_id(pw_id)?;
        let url = self.endpoint("getPathwayInfo.json");
        let resp: WikiPathwaysGetResponse = self.get_json(self.client.get(&url)).await?;
        let row = resp
            .entries()
            .into_iter()
            .find(|row| row.id.as_deref().map(str::trim) == Some(pw_id.as_str()))
            .ok_or_else(|| BioMcpError::NotFound {
                entity: "pathway".to_string(),
                id: pw_id.clone(),
                suggestion:
                    "Try searching by pathway name, for example: biomcp search pathway -q apoptosis"
                        .to_string(),
            })?;
        let id = row
            .id
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty() && is_wikipathways_id(value))
            .ok_or_else(|| BioMcpError::Api {
                api: WIKIPATHWAYS_API.to_string(),
                message: "WikiPathways detail response missing pathwayInfo.id".to_string(),
            })?;
        let name = row
            .name
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| BioMcpError::Api {
                api: WIKIPATHWAYS_API.to_string(),
                message: "WikiPathways detail response missing pathwayInfo.name".to_string(),
            })?;

        Ok(WikiPathwaysRecord {
            id,
            name,
            species: row
                .species
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
        })
    }

    pub async fn pathway_entrez_gene_ids(&self, pw_id: &str) -> Result<Vec<String>, BioMcpError> {
        let pw_id = validate_wikipathways_id(pw_id)?;
        let url = self.endpoint("findPathwaysByXref.json");
        let resp: WikiPathwaysXrefResponse = self.get_json(self.client.get(&url)).await?;

        let mut out = Vec::new();
        let mut seen = HashSet::new();
        for xref in resp.xrefs.unwrap_or_default() {
            let xref = xref.trim();
            if xref.is_empty() || !xref.chars().all(|ch| ch.is_ascii_digit()) {
                continue;
            }
            if !seen.insert(xref.to_string()) {
                continue;
            }
            out.push(xref.to_string());
        }
        if let Some(row) = resp
            .pathway_info
            .into_iter()
            .find(|row| row.id.as_deref().map(str::trim) == Some(pw_id.as_str()))
        {
            for xref in row.ncbigene.unwrap_or_default().split([',', ';']) {
                let xref = xref.trim();
                let xref = xref.strip_prefix("ncbigene:").unwrap_or(xref).trim();
                if xref.is_empty() || !xref.chars().all(|ch| ch.is_ascii_digit()) {
                    continue;
                }
                if !seen.insert(xref.to_string()) {
                    continue;
                }
                out.push(xref.to_string());
            }
        }
        Ok(out)
    }
}

pub(crate) fn is_wikipathways_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 3 && bytes.starts_with(b"WP") && bytes[2..].iter().all(u8::is_ascii_digit)
}

fn validate_wikipathways_id(value: &str) -> Result<String, BioMcpError> {
    let value = value.trim();
    if !is_wikipathways_id(value) {
        return Err(BioMcpError::InvalidArgument(
            "WikiPathways ID must look like WP254. Example: biomcp get pathway WP254".into(),
        ));
    }
    Ok(value.to_string())
}

#[derive(Debug, Clone)]
pub struct WikiPathwaysHit {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct WikiPathwaysRecord {
    pub id: String,
    pub name: String,
    pub species: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WikiPathwaysSearchResponse {
    #[serde(default)]
    result: Vec<WikiPathwaysSearchEntry>,
    #[serde(rename = "pathwayInfo", default)]
    pathway_info: Vec<WikiPathwaysSearchEntry>,
}

impl WikiPathwaysSearchResponse {
    fn entries(self) -> Vec<WikiPathwaysSearchEntry> {
        if self.pathway_info.is_empty() {
            self.result
        } else {
            self.pathway_info
        }
    }
}

#[derive(Debug, Deserialize)]
struct WikiPathwaysSearchEntry {
    id: Option<String>,
    name: Option<String>,
    species: Option<String>,
    description: Option<String>,
    datanodes: Option<String>,
    annotations: Option<String>,
}

impl WikiPathwaysSearchEntry {
    fn species_is_human(&self) -> bool {
        self.species
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| value.eq_ignore_ascii_case("Homo sapiens"))
    }

    fn match_score(&self, query: &str) -> Option<u8> {
        let query = query.trim().to_ascii_lowercase();
        if query.is_empty() {
            return None;
        }

        let name = self.name.as_deref().map(str::trim).unwrap_or_default();
        let name_lower = name.to_ascii_lowercase();
        if name_lower == query {
            return Some(0);
        }
        if name_lower.starts_with(&query) {
            return Some(1);
        }
        if name_lower.contains(&query) {
            return Some(2);
        }
        if self
            .annotations
            .as_deref()
            .map(str::to_ascii_lowercase)
            .is_some_and(|value| value.contains(&query))
        {
            return Some(3);
        }
        if self
            .datanodes
            .as_deref()
            .map(str::to_ascii_lowercase)
            .is_some_and(|value| value.contains(&query))
        {
            return Some(4);
        }
        if self
            .description
            .as_deref()
            .map(str::to_ascii_lowercase)
            .is_some_and(|value| value.contains(&query))
        {
            return Some(5);
        }

        let searchable = format!(
            "{}\n{}\n{}\n{}",
            name_lower,
            self.description
                .as_deref()
                .map(str::to_ascii_lowercase)
                .unwrap_or_default(),
            self.datanodes
                .as_deref()
                .map(str::to_ascii_lowercase)
                .unwrap_or_default(),
            self.annotations
                .as_deref()
                .map(str::to_ascii_lowercase)
                .unwrap_or_default()
        );
        let tokens = query.split_whitespace().collect::<Vec<_>>();
        if tokens.iter().all(|token| searchable.contains(token)) {
            return Some(6);
        }

        None
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum WikiPathwaysGetResponse {
    Legacy {
        #[serde(rename = "pathwayInfo")]
        pathway_info: WikiPathwaysGetEntry,
    },
    Bulk {
        #[serde(rename = "pathwayInfo")]
        pathway_info: Vec<WikiPathwaysGetEntry>,
    },
}

impl WikiPathwaysGetResponse {
    fn entries(self) -> Vec<WikiPathwaysGetEntry> {
        match self {
            Self::Legacy { pathway_info } => vec![pathway_info],
            Self::Bulk { pathway_info } => pathway_info,
        }
    }
}

#[derive(Debug, Deserialize)]
struct WikiPathwaysGetEntry {
    id: Option<String>,
    name: Option<String>,
    species: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WikiPathwaysXrefResponse {
    xrefs: Option<Vec<String>>,
    #[serde(rename = "pathwayInfo", default)]
    pathway_info: Vec<WikiPathwaysXrefEntry>,
}

#[derive(Debug, Deserialize)]
struct WikiPathwaysXrefEntry {
    id: Option<String>,
    ncbigene: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::set_env_var;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn env_lock_async() -> tokio::sync::MutexGuard<'static, ()> {
        crate::test_support::env_lock().lock().await
    }

    #[test]
    fn validates_wikipathways_id_shape() {
        assert!(is_wikipathways_id("WP254"));
        assert!(!is_wikipathways_id("wp254"));
        assert!(!is_wikipathways_id("R-HSA-5673001"));
        assert!(!is_wikipathways_id("WP25A"));
    }

    #[tokio::test]
    async fn search_pathways_filters_non_human_invalid_and_duplicate_rows() {
        let server = MockServer::start().await;
        let client = WikiPathwaysClient::new_for_test(server.uri()).unwrap();

        let body = r#"{
          "pathwayInfo": [
            {"id": "WP111", "name": "Alpha", "species": "Homo sapiens"},
            {"id": "WP111", "name": "Alpha duplicate", "species": "Homo sapiens"},
            {"id": "WP222", "name": "Mouse only", "species": "Mus musculus"},
            {"id": "BAD", "name": "Bad", "species": "Homo sapiens"},
            {"id": "WP333", "name": "", "species": "Homo sapiens"},
            {"id": "WP444", "name": "Alpha beta", "species": "Homo sapiens"}
          ]
        }"#;

        Mock::given(method("GET"))
            .and(path("/findPathwaysByText.json"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
            .expect(1)
            .mount(&server)
            .await;

        let hits = client.search_pathways("alpha", 10).await.unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].id, "WP111");
        assert_eq!(hits[1].id, "WP444");
    }

    #[tokio::test]
    async fn get_pathway_parses_minimal_detail_payload() {
        let server = MockServer::start().await;
        let client = WikiPathwaysClient::new_for_test(server.uri()).unwrap();

        Mock::given(method("GET"))
            .and(path("/getPathwayInfo.json"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{"pathwayInfo":[{"id":"WP254","name":"Apoptosis","species":"Homo sapiens","revision":"140926"}]}"#,
                "application/json",
            ))
            .expect(1)
            .mount(&server)
            .await;

        let record = client.get_pathway("WP254").await.unwrap();
        assert_eq!(record.id, "WP254");
        assert_eq!(record.name, "Apoptosis");
        assert_eq!(record.species.as_deref(), Some("Homo sapiens"));
    }

    #[tokio::test]
    async fn pathway_entrez_gene_ids_dedupes_and_filters_non_numeric_rows() {
        let server = MockServer::start().await;
        let client = WikiPathwaysClient::new_for_test(server.uri()).unwrap();

        Mock::given(method("GET"))
            .and(path("/findPathwaysByXref.json"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{"pathwayInfo":[{"id":"WP254","ncbigene":"ncbigene:7157, ncbigene:1956, ncbigene:7157; ncbigene:BAD, , ncbigene:672"}]}"#,
                "application/json",
            ))
            .expect(1)
            .mount(&server)
            .await;

        let ids = client.pathway_entrez_gene_ids("WP254").await.unwrap();
        assert_eq!(ids, vec!["7157", "1956", "672"]);
    }

    #[tokio::test]
    async fn search_rejects_html_content_type_before_json_parse() {
        let server = MockServer::start().await;
        let client = WikiPathwaysClient::new_for_test(server.uri()).unwrap();

        Mock::given(method("GET"))
            .and(path("/findPathwaysByText.json"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_raw("<html><body>error page</body></html>", "text/html"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let err = client.search_pathways("apoptosis", 1).await.unwrap_err();
        assert!(err.to_string().contains("Unexpected HTML response"));
    }

    #[tokio::test]
    async fn search_bypasses_persistent_cache_when_mock_responses_change() {
        let _guard = env_lock_async().await;
        let server = MockServer::start().await;
        let _base = set_env_var("BIOMCP_WIKIPATHWAYS_BASE", Some(&server.uri()));
        let client = WikiPathwaysClient::new().unwrap();

        Mock::given(method("GET"))
            .and(path("/findPathwaysByText.json"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_raw("<html><body>error page</body></html>", "text/html"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let first_err = client.search_pathways("apoptosis", 1).await.unwrap_err();
        assert!(first_err.to_string().contains("Unexpected HTML response"));

        server.reset().await;

        Mock::given(method("GET"))
            .and(path("/findPathwaysByText.json"))
            .respond_with(ResponseTemplate::new(404).set_body_raw(
                "<!DOCTYPE html><html><head><title>404</title></head><body>File not found</body></html>",
                "text/html; charset=utf-8",
            ))
            .expect(1)
            .mount(&server)
            .await;

        let second_err = client.search_pathways("apoptosis", 1).await.unwrap_err();
        let second_msg = second_err.to_string();
        assert!(second_msg.contains("HTTP 404"));
        assert!(second_msg.contains("HTML error page"));
    }

    #[tokio::test]
    async fn search_sanitizes_404_html_error_body() {
        let server = MockServer::start().await;
        let client = WikiPathwaysClient::new_for_test(server.uri()).unwrap();

        Mock::given(method("GET"))
            .and(path("/findPathwaysByText.json"))
            .respond_with(ResponseTemplate::new(404).set_body_raw(
                "<!DOCTYPE html><html><head><title>404</title></head><body>File not found</body></html>",
                "text/html; charset=utf-8",
            ))
            .expect(1)
            .mount(&server)
            .await;

        let err = client.search_pathways("apoptosis", 1).await.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("HTTP 404"));
        assert!(msg.contains("HTML error page"));
        assert!(!msg.contains("<!DOCTYPE"));
        assert!(!msg.contains("<html"));
        assert!(!msg.contains("<head"));
    }

    #[tokio::test]
    async fn get_pathway_rejects_invalid_ids_before_request() {
        let client = WikiPathwaysClient::new_for_test("http://127.0.0.1".into()).unwrap();
        let err = client.get_pathway("not-a-pathway").await.unwrap_err();
        assert!(matches!(err, BioMcpError::InvalidArgument(_)));
        assert!(err.to_string().contains("WP254"));
    }
}
