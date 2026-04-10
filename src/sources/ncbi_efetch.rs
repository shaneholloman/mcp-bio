use std::borrow::Cow;
use std::sync::OnceLock;

use regex::Regex;
use roxmltree::Document;

use crate::error::BioMcpError;

const NCBI_EFETCH_BASE: &str = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils";
const NCBI_EFETCH_API: &str = "pubmed-eutils";
const NCBI_EFETCH_BASE_ENV: &str = "BIOMCP_PUBMED_BASE";

static DOCTYPE_RE: OnceLock<Regex> = OnceLock::new();

#[derive(Clone)]
pub struct NcbiEfetchClient {
    client: reqwest_middleware::ClientWithMiddleware,
    base: Cow<'static, str>,
    api_key: Option<String>,
}

impl NcbiEfetchClient {
    pub fn new() -> Result<Self, BioMcpError> {
        Ok(Self {
            client: crate::sources::shared_client()?,
            base: crate::sources::env_base(NCBI_EFETCH_BASE, NCBI_EFETCH_BASE_ENV),
            api_key: crate::sources::ncbi_api_key(),
        })
    }

    #[cfg(test)]
    fn new_for_test(base: String, api_key: Option<String>) -> Result<Self, BioMcpError> {
        Ok(Self {
            client: crate::sources::test_client()?,
            base: Cow::Owned(base),
            api_key: api_key
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
        })
    }

    fn endpoint(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.base.as_ref().trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    fn normalize_pmcid(&self, pmcid: &str) -> Result<Option<String>, BioMcpError> {
        let pmcid = pmcid.trim();
        if pmcid.is_empty() {
            return Ok(None);
        }
        if pmcid.len() > 64 {
            return Err(BioMcpError::InvalidArgument("PMCID is too long.".into()));
        }

        let numeric = if pmcid
            .get(..3)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("PMC"))
        {
            &pmcid[3..]
        } else {
            pmcid
        };

        if numeric.is_empty() || !numeric.chars().all(|ch| ch.is_ascii_digit()) {
            return Err(BioMcpError::InvalidArgument(
                "PMCID must start with PMC and contain only digits after.".into(),
            ));
        }
        if numeric.len() > 32 {
            return Err(BioMcpError::InvalidArgument("PMCID is too long.".into()));
        }

        Ok(Some(numeric.to_string()))
    }

    async fn get_text(
        &self,
        req: reqwest_middleware::RequestBuilder,
    ) -> Result<String, BioMcpError> {
        let resp = crate::sources::apply_cache_mode_with_auth(req, self.api_key.is_some())
            .send()
            .await?;
        let status = resp.status();
        let bytes = crate::sources::read_limited_body(resp, NCBI_EFETCH_API).await?;
        if !status.is_success() {
            let excerpt = crate::sources::body_excerpt(&bytes);
            return Err(BioMcpError::Api {
                api: NCBI_EFETCH_API.to_string(),
                message: format!("HTTP {status}: {excerpt}"),
            });
        }
        Ok(String::from_utf8_lossy(&bytes).to_string())
    }

    pub async fn get_full_text_xml(&self, pmcid: &str) -> Result<Option<String>, BioMcpError> {
        let Some(numeric_pmcid) = self.normalize_pmcid(pmcid)? else {
            return Ok(None);
        };

        let url = self.endpoint("efetch.fcgi");
        let req = self.client.get(&url).query(&[
            ("db", "pmc"),
            ("id", numeric_pmcid.as_str()),
            ("rettype", "xml"),
        ]);
        let req = crate::sources::append_ncbi_api_key(req, self.api_key.as_deref());
        let xml = self.get_text(req).await?;
        normalize_article_xml(&xml)
    }
}

fn strip_doctype_declaration(xml: &str) -> String {
    let re = DOCTYPE_RE
        .get_or_init(|| Regex::new(r#"(?is)<!DOCTYPE[^>]*>"#).expect("valid doctype regex"));
    re.replace(xml, "").to_string()
}

fn normalize_article_xml(xml: &str) -> Result<Option<String>, BioMcpError> {
    let sanitized = strip_doctype_declaration(xml);
    let trimmed = sanitized.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let doc = match Document::parse(trimmed) {
        Ok(doc) => doc,
        Err(_) => return Ok(Some(trimmed.to_string())),
    };

    let article = doc
        .descendants()
        .find(|node| node.is_element() && node.has_tag_name("article"));
    let Some(article) = article else {
        return Ok(Some(trimmed.to_string()));
    };

    Ok(Some(trimmed[article.range()].to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn get_full_text_xml_uses_numeric_pmcid_and_extracts_article_xml() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/efetch.fcgi"))
            .and(query_param("db", "pmc"))
            .and(query_param("id", "123456"))
            .and(query_param("rettype", "xml"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"<?xml version="1.0"?><!DOCTYPE pmc-articleset><pmc-articleset><article><front><article-meta><title-group><article-title>Wrapped</article-title></title-group></article-meta></front><body><p>Body text.</p></body></article></pmc-articleset>"#,
            ))
            .expect(1)
            .mount(&server)
            .await;

        let client = NcbiEfetchClient::new_for_test(server.uri(), None).unwrap();
        let xml = client
            .get_full_text_xml("PMC123456")
            .await
            .unwrap()
            .unwrap();
        assert!(xml.starts_with("<article"));
        assert!(xml.contains("<article-title>Wrapped</article-title>"));
        assert!(!xml.contains("<pmc-articleset>"));
    }

    #[tokio::test]
    async fn get_full_text_xml_includes_api_key_when_configured() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/efetch.fcgi"))
            .and(query_param("db", "pmc"))
            .and(query_param("id", "123456"))
            .and(query_param("rettype", "xml"))
            .and(query_param("api_key", "test-key"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("<article><body><p>Body text.</p></body></article>"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = NcbiEfetchClient::new_for_test(server.uri(), Some("test-key".into())).unwrap();
        let xml = client.get_full_text_xml("PMC123456").await.unwrap();
        assert!(xml.is_some());
    }
}
