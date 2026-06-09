use std::borrow::Cow;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use http_cache_reqwest::CacheMode;
use reqwest::{StatusCode, Url, header::CONTENT_TYPE};
use serde::{Deserialize, Serialize};

use crate::error::BioMcpError;

const FIGSHARE_BASE: &str = "https://api.figshare.com";
const FIGSHARE_BASE_ENV: &str = "BIOMCP_FIGSHARE_BASE";
const FIGSHARE_API: &str = "figshare";
const MAX_FIGSHARE_FILE_BYTES: usize = crate::sources::DEFAULT_MAX_BODY_BYTES;
const FIGSHARE_SEARCH_PAGE_SIZE: usize = 100;
const FIGSHARE_DOWNLOAD_ACCEPTED_RETRIES: usize = 3;
const FIGSHARE_DOWNLOAD_ACCEPTED_BACKOFF_MS: u64 = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FigshareArticleRef {
    pub article_id: u64,
    pub file_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FigshareArticle {
    pub article_id: u64,
    pub title: Option<String>,
    pub doi: Option<String>,
    pub api_url: Option<String>,
    pub public_url: Option<String>,
    pub license: Option<FigshareLicense>,
    pub files: Vec<FigshareFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FigshareLicense {
    pub name: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FigshareArticleSearchResult {
    pub article_id: u64,
    pub title: Option<String>,
    pub doi: Option<String>,
    pub api_url: Option<String>,
    pub public_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FigshareFile {
    pub id: u64,
    pub filename: String,
    pub size: Option<usize>,
    pub md5: Option<String>,
    pub mimetype: Option<String>,
    pub download_url: String,
}

#[derive(Clone)]
pub struct FigshareClient {
    client: reqwest_middleware::ClientWithMiddleware,
    base: Cow<'static, str>,
}

#[derive(Debug, Deserialize)]
struct FigshareArticleResponse {
    id: Option<u64>,
    title: Option<String>,
    doi: Option<String>,
    url_api: Option<String>,
    url_public_html: Option<String>,
    license: Option<FigshareLicenseResponse>,
    #[serde(default)]
    files: Vec<FigshareFileResponse>,
}

#[derive(Debug, Deserialize)]
struct FigshareLicenseResponse {
    name: Option<String>,
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FigshareFileResponse {
    id: Option<u64>,
    name: Option<String>,
    size: Option<usize>,
    md5: Option<String>,
    mimetype: Option<String>,
    download_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FigshareArticleSearchResponse {
    id: Option<u64>,
    title: Option<String>,
    doi: Option<String>,
    url_api: Option<String>,
    url_public_html: Option<String>,
}

#[derive(Debug, Serialize)]
struct FigshareArticleSearchRequest<'a> {
    search_for: &'a str,
    page_size: usize,
}

impl FigshareClient {
    pub fn new() -> Result<Self, BioMcpError> {
        Ok(Self {
            client: crate::sources::shared_client()?,
            base: crate::sources::env_base(FIGSHARE_BASE, FIGSHARE_BASE_ENV),
        })
    }

    #[cfg(test)]
    fn new_for_test(base: String) -> Result<Self, BioMcpError> {
        Ok(Self {
            client: crate::sources::test_client()?,
            base: Cow::Owned(base),
        })
    }

    fn endpoint_url(&self, path: &str) -> Result<Url, BioMcpError> {
        Url::parse(&format!(
            "{}/{}",
            self.base.as_ref().trim_end_matches('/'),
            path.trim_start_matches('/')
        ))
        .map_err(|err| BioMcpError::Api {
            api: FIGSHARE_API.to_string(),
            message: format!("invalid Figshare base URL: {err}"),
        })
    }

    pub async fn article(
        &self,
        reference: &FigshareArticleRef,
    ) -> Result<FigshareArticle, BioMcpError> {
        let url = self.endpoint_url(&format!("v2/articles/{}", reference.article_id))?;
        let resp = self
            .client
            .get(url)
            .with_extension(CacheMode::NoStore)
            .send()
            .await?;
        let status = resp.status();
        let content_type = resp.headers().get(CONTENT_TYPE).cloned();
        let bytes = crate::sources::read_limited_body(resp, FIGSHARE_API).await?;
        if !status.is_success() {
            let excerpt = crate::sources::summarize_http_error_body(content_type.as_ref(), &bytes);
            return Err(BioMcpError::Api {
                api: FIGSHARE_API.to_string(),
                message: format!("HTTP {status}: {excerpt}"),
            });
        }
        let raw: FigshareArticleResponse =
            serde_json::from_slice(&bytes).map_err(|source| BioMcpError::ApiJson {
                api: FIGSHARE_API.to_string(),
                source,
            })?;
        Ok(normalize_article(raw, reference))
    }

    pub async fn search_articles(
        &self,
        search_for: &str,
    ) -> Result<Vec<FigshareArticleSearchResult>, BioMcpError> {
        let search_for = search_for.trim();
        if search_for.is_empty() {
            return Ok(Vec::new());
        }
        let url = self.endpoint_url("v2/articles/search")?;
        let body = FigshareArticleSearchRequest {
            search_for,
            page_size: FIGSHARE_SEARCH_PAGE_SIZE,
        };
        let resp = self
            .client
            .post(url)
            .json(&body)
            .with_extension(CacheMode::NoStore)
            .send()
            .await?;
        let status = resp.status();
        let content_type = resp.headers().get(CONTENT_TYPE).cloned();
        let bytes = crate::sources::read_limited_body(resp, FIGSHARE_API).await?;
        if !status.is_success() {
            let excerpt = crate::sources::summarize_http_error_body(content_type.as_ref(), &bytes);
            return Err(BioMcpError::Api {
                api: FIGSHARE_API.to_string(),
                message: format!("HTTP {status}: {excerpt}"),
            });
        }
        let raw: Vec<FigshareArticleSearchResponse> =
            serde_json::from_slice(&bytes).map_err(|source| BioMcpError::ApiJson {
                api: FIGSHARE_API.to_string(),
                source,
            })?;
        Ok(raw
            .into_iter()
            .filter_map(normalize_search_result)
            .collect())
    }

    pub async fn download_file(&self, file: &FigshareFile) -> Result<Vec<u8>, BioMcpError> {
        let mut accepted_retries = 0;
        loop {
            let resp = self
                .client
                .get(&file.download_url)
                .with_extension(CacheMode::NoStore)
                .send()
                .await?;
            let status = resp.status();
            let content_type = resp.headers().get(CONTENT_TYPE).cloned();
            let bytes = crate::sources::read_limited_body_with_limit(
                resp,
                FIGSHARE_API,
                MAX_FIGSHARE_FILE_BYTES,
            )
            .await?;
            if status == StatusCode::ACCEPTED {
                if accepted_retries >= FIGSHARE_DOWNLOAD_ACCEPTED_RETRIES {
                    return Err(BioMcpError::Api {
                        api: FIGSHARE_API.to_string(),
                        message: format!(
                            "HTTP {status}: Figshare file still staging after {accepted_retries} retries"
                        ),
                    });
                }
                accepted_retries += 1;
                tokio::time::sleep(Duration::from_millis(FIGSHARE_DOWNLOAD_ACCEPTED_BACKOFF_MS))
                    .await;
                continue;
            }
            if !status.is_success() {
                let excerpt =
                    crate::sources::summarize_http_error_body(content_type.as_ref(), &bytes);
                return Err(BioMcpError::Api {
                    api: FIGSHARE_API.to_string(),
                    message: format!("HTTP {status}: {excerpt}"),
                });
            }
            return Ok(bytes);
        }
    }
}

pub fn parse_figshare_article_url(raw: &str) -> Option<FigshareArticleRef> {
    let url = Url::parse(raw.trim()).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    if host != "figshare.com" && !host.ends_with(".figshare.com") {
        return None;
    }

    let segments = url
        .path_segments()
        .map(|segments| segments.collect::<Vec<_>>())
        .unwrap_or_default();
    let file_path_index = segments.iter().position(|segment| *segment == "files");
    let article_id = if host == "api.figshare.com" {
        segments.windows(2).find_map(|window| {
            (window[0] == "articles")
                .then(|| parse_u64(window[1]))
                .flatten()
        })?
    } else if let Some(article_path_index) =
        segments.iter().position(|segment| *segment == "articles")
    {
        let end = file_path_index.unwrap_or(segments.len());
        segments[article_path_index + 1..end]
            .iter()
            .find_map(|segment| parse_u64(segment))?
    } else if let Some(index) = file_path_index {
        segments[..index]
            .iter()
            .rev()
            .find_map(|segment| parse_u64(segment))?
    } else {
        segments
            .iter()
            .rev()
            .find_map(|segment| parse_u64(segment))?
    };
    let file_id = url
        .query_pairs()
        .find_map(|(key, value)| (key == "file").then(|| parse_u64(&value)).flatten())
        .or_else(|| {
            segments.windows(2).find_map(|window| {
                (window[0] == "files")
                    .then(|| parse_u64_prefix(window[1]))
                    .flatten()
            })
        });

    Some(FigshareArticleRef {
        article_id,
        file_id,
    })
}

fn parse_u64(value: &str) -> Option<u64> {
    value.trim().parse::<u64>().ok()
}

fn parse_u64_prefix(value: &str) -> Option<u64> {
    let value = value.trim();
    let end = value
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(value.len());
    if end == 0 {
        None
    } else {
        value[..end].parse::<u64>().ok()
    }
}

fn normalize_article(
    raw: FigshareArticleResponse,
    reference: &FigshareArticleRef,
) -> FigshareArticle {
    let preferred_file_id = reference.file_id;
    let mut files = raw
        .files
        .into_iter()
        .filter_map(normalize_file)
        .collect::<Vec<_>>();
    files.sort_by(|left, right| {
        let left_preferred = Some(left.id) == preferred_file_id;
        let right_preferred = Some(right.id) == preferred_file_id;
        right_preferred
            .cmp(&left_preferred)
            .then_with(|| left.filename.cmp(&right.filename))
    });

    FigshareArticle {
        article_id: raw.id.unwrap_or(reference.article_id),
        title: clean_string(raw.title),
        doi: clean_string(raw.doi),
        api_url: clean_string(raw.url_api),
        public_url: clean_string(raw.url_public_html),
        license: raw.license.map(|license| FigshareLicense {
            name: clean_string(license.name),
            url: clean_string(license.url),
        }),
        files,
    }
}

fn normalize_search_result(
    raw: FigshareArticleSearchResponse,
) -> Option<FigshareArticleSearchResult> {
    Some(FigshareArticleSearchResult {
        article_id: raw.id?,
        title: clean_string(raw.title),
        doi: clean_string(raw.doi),
        api_url: clean_string(raw.url_api),
        public_url: clean_string(raw.url_public_html),
    })
}

fn normalize_file(raw: FigshareFileResponse) -> Option<FigshareFile> {
    let id = raw.id?;
    let filename = safe_filename(raw.name.as_deref()?)?;
    let download_url = clean_string(raw.download_url)?;
    Some(FigshareFile {
        id,
        filename,
        size: raw.size,
        md5: clean_string(raw.md5),
        mimetype: clean_string(raw.mimetype),
        download_url,
    })
}

fn clean_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn safe_filename(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let normalized = raw.replace('\\', "/");
    let mut out = PathBuf::new();
    for component in Path::new(&normalized).components() {
        match component {
            Component::Normal(part) => {
                if part.to_str().is_some_and(|value| value.ends_with(':')) {
                    return None;
                }
                out.push(part);
            }
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => return None,
        }
    }
    out.to_str()
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.contains('/'))
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn parses_aacr_public_article_url_with_file_id() {
        let parsed = parse_figshare_article_url(
            "https://aacr.figshare.com/articles/journal_contribution/Foo/22474820?file=39926318",
        )
        .unwrap();

        assert_eq!(parsed.article_id, 22474820);
        assert_eq!(parsed.file_id, Some(39926318));
    }

    #[test]
    fn parses_public_article_url_with_file_path_id() {
        let parsed = parse_figshare_article_url(
            "https://figshare.com/articles/dataset/Foo/22474820/files/39926318",
        )
        .unwrap();

        assert_eq!(parsed.article_id, 22474820);
        assert_eq!(parsed.file_id, Some(39926318));
    }

    #[test]
    fn parses_versioned_public_article_url_with_file_path_id() {
        let parsed = parse_figshare_article_url(
            "https://aacr.figshare.com/articles/journal_contribution/Foo/22474820/1/files/39926318.pdf",
        )
        .unwrap();

        assert_eq!(parsed.article_id, 22474820);
        assert_eq!(parsed.file_id, Some(39926318));
    }

    #[test]
    fn parses_api_article_url() {
        let parsed =
            parse_figshare_article_url("https://api.figshare.com/v2/articles/22474820").unwrap();

        assert_eq!(parsed.article_id, 22474820);
        assert_eq!(parsed.file_id, None);
    }

    #[test]
    fn rejects_non_figshare_urls_and_unsafe_names() {
        assert!(parse_figshare_article_url("https://example.org/file.pdf").is_none());
        assert!(safe_filename("../secret.pdf").is_none());
        assert!(safe_filename("nested/secret.pdf").is_none());
        assert_eq!(
            safe_filename(" supplement.pdf ").as_deref(),
            Some("supplement.pdf")
        );
    }

    #[tokio::test]
    async fn article_fetch_normalizes_files_and_uses_download_url() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/articles/22474820"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 22474820,
                "url_api": format!("{}/v2/articles/22474820", server.uri()),
                "url_public_html": "https://aacr.figshare.com/articles/journal_contribution/Foo/22474820",
                "license": {"name": "CC BY 4.0", "url": "https://creativecommons.org/licenses/by/4.0/"},
                "files": [
                    {"id": 1, "name": "other.txt", "size": 5, "download_url": format!("{}/download/other", server.uri())},
                    {"id": 39926318, "name": "figshare-supplement.pdf", "size": 8, "md5": "0123456789abcdef0123456789abcdef", "mimetype": "application/pdf", "download_url": format!("{}/download/supp", server.uri())},
                    {"id": 3, "name": "../unsafe.pdf", "download_url": format!("{}/download/bad", server.uri())}
                ]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/download/supp"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"PDF bytes".to_vec()))
            .expect(1)
            .mount(&server)
            .await;

        let client = FigshareClient::new_for_test(server.uri()).unwrap();
        let reference = FigshareArticleRef {
            article_id: 22474820,
            file_id: Some(39926318),
        };
        let article = client.article(&reference).await.unwrap();

        assert_eq!(article.files.len(), 2);
        assert_eq!(article.files[0].filename, "figshare-supplement.pdf");
        assert_eq!(
            article
                .license
                .as_ref()
                .and_then(|license| license.name.as_deref()),
            Some("CC BY 4.0")
        );
        assert_eq!(
            client.download_file(&article.files[0]).await.unwrap(),
            b"PDF bytes"
        );
    }

    #[tokio::test]
    async fn download_rejects_oversized_file_bytes() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/download/large"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![
                b'x';
                MAX_FIGSHARE_FILE_BYTES
                    + 1
            ]))
            .mount(&server)
            .await;

        let client = FigshareClient::new_for_test(server.uri()).unwrap();
        let file = FigshareFile {
            id: 1,
            filename: "large.pdf".to_string(),
            size: Some(MAX_FIGSHARE_FILE_BYTES + 1),
            md5: None,
            mimetype: Some("application/pdf".to_string()),
            download_url: format!("{}/download/large", server.uri()),
        };

        assert!(client.download_file(&file).await.is_err());
    }

    #[tokio::test]
    async fn search_articles_normalizes_rows() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v2/articles/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "id": 22474817,
                    "title": " Example ",
                    "doi": "10.1000/example",
                    "url_api": format!("{}/v2/articles/22474817", server.uri()),
                    "url_public_html": "https://figshare.com/articles/example/22474817"
                },
                {"title": "missing id"}
            ])))
            .expect(1)
            .mount(&server)
            .await;

        let client = FigshareClient::new_for_test(server.uri()).unwrap();
        let rows = client.search_articles("10.1000/example").await.unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].article_id, 22474817);
        assert_eq!(rows[0].title.as_deref(), Some("Example"));
        assert_eq!(rows[0].doi.as_deref(), Some("10.1000/example"));
    }

    #[tokio::test]
    async fn download_error_sanitizes_html_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/download/error"))
            .respond_with(ResponseTemplate::new(503).set_body_raw(
                "<html><body>upstream detail</body></html>",
                "text/html; charset=utf-8",
            ))
            .expect(1)
            .mount(&server)
            .await;

        let client = FigshareClient::new_for_test(server.uri()).unwrap();
        let file = FigshareFile {
            id: 1,
            filename: "error.pdf".to_string(),
            size: None,
            md5: None,
            mimetype: Some("application/pdf".to_string()),
            download_url: format!("{}/download/error", server.uri()),
        };
        let err = client.download_file(&file).await.unwrap_err();
        let message = err.to_string();

        assert!(message.contains("HTML error page"));
        assert!(!message.contains("<html"));
        assert!(!message.contains("upstream detail"));
    }

    #[tokio::test]
    async fn download_errors_after_repeated_accepted_responses() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/download/staged"))
            .respond_with(ResponseTemplate::new(202))
            .expect(FIGSHARE_DOWNLOAD_ACCEPTED_RETRIES as u64 + 1)
            .mount(&server)
            .await;

        let client = FigshareClient::new_for_test(server.uri()).unwrap();
        let file = FigshareFile {
            id: 1,
            filename: "staged.pdf".to_string(),
            size: None,
            md5: None,
            mimetype: Some("application/pdf".to_string()),
            download_url: format!("{}/download/staged", server.uri()),
        };
        let err = client.download_file(&file).await.unwrap_err();

        assert!(err.to_string().contains("still staging"));
    }
}
