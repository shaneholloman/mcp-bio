use std::borrow::Cow;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use http_cache_reqwest::CacheMode;
use regex::Regex;

use crate::error::BioMcpError;

// PubMed Central Open Access (OA) service
// Docs: https://www.ncbi.nlm.nih.gov/pmc/tools/oa/
const PMC_OA_BASE: &str = "https://www.ncbi.nlm.nih.gov/pmc/utils/oa/oa.fcgi";
const PMC_OA_API: &str = "pmc-oa";
const PMC_OA_BASE_ENV: &str = "BIOMCP_PMC_OA_BASE";
const MAX_TGZ_BYTES: usize = 64 * 1024 * 1024;
const MAX_ARCHIVE_ENTRY_BYTES: u64 = 8 * 1024 * 1024;

static TGZ_HREF_RE: OnceLock<Regex> = OnceLock::new();
static LICENSE_ATTR_RE: OnceLock<Regex> = OnceLock::new();
static RETRACTED_ATTR_RE: OnceLock<Regex> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmcOaArchiveManifest {
    pub tgz_url: String,
    pub package_url: String,
    pub license: Option<String>,
    pub retracted: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmcOaArchiveEntry {
    pub filename: String,
    pub bytes: Vec<u8>,
    pub is_xml: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmcOaArchivePackage {
    pub manifest: PmcOaArchiveManifest,
    pub entries: Vec<PmcOaArchiveEntry>,
}

#[derive(Clone)]
pub struct PmcOaClient {
    client: reqwest_middleware::ClientWithMiddleware,
    base: Cow<'static, str>,
    api_key: Option<String>,
}

impl PmcOaClient {
    pub fn new() -> Result<Self, BioMcpError> {
        Ok(Self {
            client: crate::sources::shared_client()?,
            base: crate::sources::env_base(PMC_OA_BASE, PMC_OA_BASE_ENV),
            api_key: crate::sources::ncbi_api_key(),
        })
    }

    #[cfg(test)]
    fn new_for_test(base: String, api_key: Option<String>) -> Result<Self, BioMcpError> {
        Ok(Self {
            client: crate::sources::test_client()?,
            base: Cow::Owned(base),
            api_key: api_key
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty()),
        })
    }

    fn endpoint(&self) -> String {
        self.base.as_ref().trim_end_matches('/').to_string()
    }

    async fn get_text(
        &self,
        req: reqwest_middleware::RequestBuilder,
    ) -> Result<String, BioMcpError> {
        let resp = req.with_extension(CacheMode::NoStore).send().await?;
        let status = resp.status();
        let bytes = crate::sources::read_limited_body(resp, PMC_OA_API).await?;
        if !status.is_success() {
            let excerpt = crate::sources::body_excerpt(&bytes);
            return Err(BioMcpError::Api {
                api: PMC_OA_API.to_string(),
                message: format!("HTTP {status}: {excerpt}"),
            });
        }
        Ok(String::from_utf8_lossy(&bytes).to_string())
    }

    async fn oa_archive_manifest(
        &self,
        pmcid: &str,
    ) -> Result<Option<PmcOaArchiveManifest>, BioMcpError> {
        let pmcid = pmcid.trim();
        if pmcid.is_empty() {
            return Ok(None);
        }
        if pmcid.len() > 64 {
            return Err(BioMcpError::InvalidArgument("PMCID is too long.".into()));
        }

        let url = self.endpoint();
        let req = self.client.get(&url).query(&[("id", pmcid)]);
        let req = crate::sources::append_ncbi_api_key(req, self.api_key.as_deref());
        let xml = self.get_text(req).await?;

        let re = TGZ_HREF_RE.get_or_init(|| {
            Regex::new(r#"<link[^>]*format="tgz"[^>]*href="([^"]+)""#)
                .expect("valid tgz href regex")
        });

        let Some(caps) = re.captures(&xml) else {
            return Ok(None);
        };
        let Some(raw_href) = caps
            .get(1)
            .map(|m| m.as_str().trim())
            .filter(|s| !s.is_empty())
        else {
            return Ok(None);
        };

        let href = if raw_href.starts_with("ftp://ftp.ncbi.nlm.nih.gov/") {
            raw_href.replacen(
                "ftp://ftp.ncbi.nlm.nih.gov/",
                "https://ftp.ncbi.nlm.nih.gov/",
                1,
            )
        } else if raw_href.starts_with("ftp://") {
            raw_href.replacen("ftp://", "https://", 1)
        } else {
            raw_href.to_string()
        };

        let license = LICENSE_ATTR_RE
            .get_or_init(|| Regex::new(r#"\blicense="([^"]+)""#).expect("valid license regex"))
            .captures(&xml)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().trim().to_string())
            .filter(|s| !s.is_empty());
        let retracted = RETRACTED_ATTR_RE
            .get_or_init(|| Regex::new(r#"\bretracted="([^"]+)""#).expect("valid retracted regex"))
            .captures(&xml)
            .and_then(|caps| caps.get(1))
            .and_then(|value| parse_boolish(value.as_str()));

        Ok(Some(PmcOaArchiveManifest {
            tgz_url: href.clone(),
            package_url: href,
            license,
            retracted,
        }))
    }

    async fn archive_bytes(&self, manifest: &PmcOaArchiveManifest) -> Result<Vec<u8>, BioMcpError> {
        let resp = self
            .client
            .get(&manifest.tgz_url)
            .with_extension(CacheMode::NoStore)
            .send()
            .await?;
        let status = resp.status();
        let bytes =
            crate::sources::read_limited_body_with_limit(resp, PMC_OA_API, MAX_TGZ_BYTES).await?;
        if !status.is_success() {
            let excerpt = crate::sources::body_excerpt(&bytes);
            return Err(BioMcpError::Api {
                api: PMC_OA_API.to_string(),
                message: format!("HTTP {status}: {excerpt}"),
            });
        }
        Ok(bytes.to_vec())
    }

    pub async fn get_full_text_xml_with_manifest(
        &self,
        pmcid: &str,
    ) -> Result<Option<(String, PmcOaArchiveManifest)>, BioMcpError> {
        let Some(manifest) = self.oa_archive_manifest(pmcid).await? else {
            return Ok(None);
        };

        let bytes = self.archive_bytes(&manifest).await?;
        let xml = tokio::task::spawn_blocking(move || extract_first_nxml(&bytes))
            .await
            .map_err(|err| BioMcpError::Api {
                api: PMC_OA_API.to_string(),
                message: format!("Task join error: {err}"),
            })??;

        Ok(xml.map(|xml| (xml, manifest)))
    }

    pub async fn get_archive_package(
        &self,
        pmcid: &str,
    ) -> Result<Option<PmcOaArchivePackage>, BioMcpError> {
        let Some(manifest) = self.oa_archive_manifest(pmcid).await? else {
            return Ok(None);
        };
        let bytes = self.archive_bytes(&manifest).await?;
        let entries = tokio::task::spawn_blocking(move || extract_archive_entries(&bytes))
            .await
            .map_err(|err| BioMcpError::Api {
                api: PMC_OA_API.to_string(),
                message: format!("Task join error: {err}"),
            })??;
        Ok(Some(PmcOaArchivePackage { manifest, entries }))
    }
}

fn parse_boolish(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "y" | "yes" | "true" | "1" => Some(true),
        "n" | "no" | "false" | "0" => Some(false),
        _ => None,
    }
}

fn is_xml_name(filename: &str) -> bool {
    let lower = filename.to_ascii_lowercase();
    lower.ends_with(".nxml") || lower.ends_with(".xml")
}

fn safe_archive_name(path: &Path) -> Option<String> {
    let raw = path.to_str()?.trim();
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
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn extract_archive_entries(tgz_bytes: &[u8]) -> Result<Vec<PmcOaArchiveEntry>, BioMcpError> {
    use std::io::Read;

    if tgz_bytes.len() > MAX_TGZ_BYTES {
        return Err(BioMcpError::Api {
            api: PMC_OA_API.to_string(),
            message: format!("PMC OA archive exceeded {MAX_TGZ_BYTES} bytes"),
        });
    }

    let gz = flate2::read::GzDecoder::new(tgz_bytes);
    let mut archive = tar::Archive::new(gz);
    let entries = archive.entries()?;
    let mut out = Vec::new();

    for entry in entries {
        let entry = entry?;
        if !entry.header().entry_type().is_file() || entry.size() > MAX_ARCHIVE_ENTRY_BYTES {
            continue;
        }
        let path = entry.path()?;
        let Some(filename) = safe_archive_name(&path) else {
            continue;
        };

        let mut bytes = Vec::new();
        let mut reader = entry.take(MAX_ARCHIVE_ENTRY_BYTES + 1);
        reader.read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_ARCHIVE_ENTRY_BYTES || bytes.is_empty() {
            continue;
        }
        let is_xml = is_xml_name(&filename);
        out.push(PmcOaArchiveEntry {
            filename,
            bytes,
            is_xml,
        });
    }

    Ok(out)
}

fn extract_first_nxml(tgz_bytes: &[u8]) -> Result<Option<String>, BioMcpError> {
    for entry in extract_archive_entries(tgz_bytes)? {
        if entry.is_xml {
            return Ok(Some(String::from_utf8_lossy(&entry.bytes).to_string()));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;
    use tar::{Builder, Header};
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn tgz_with_entries(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut tar_buf = Vec::new();
        {
            let mut builder = Builder::new(&mut tar_buf);
            for (name, body) in entries {
                let mut header = Header::new_gnu();
                header.set_size(body.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                builder
                    .append_data(&mut header, *name, *body)
                    .expect("archive entry should append");
            }
            builder.finish().expect("tar should finish");
        }

        let mut gz = GzEncoder::new(Vec::new(), Compression::default());
        gz.write_all(&tar_buf).expect("gzip should write tar");
        gz.finish().expect("gzip should finish")
    }

    #[tokio::test]
    async fn oa_tgz_url_rewrites_ftp_to_https() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .and(query_param("id", "PMC123"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"<records><record><link format="tgz" href="ftp://ftp.ncbi.nlm.nih.gov/pub/pmc/file.tar.gz"/></record></records>"#,
            ))
            .mount(&server)
            .await;

        let client = PmcOaClient::new_for_test(server.uri(), None).unwrap();
        let manifest = client.oa_archive_manifest("PMC123").await.unwrap().unwrap();
        assert_eq!(
            manifest.tgz_url,
            "https://ftp.ncbi.nlm.nih.gov/pub/pmc/file.tar.gz"
        );
    }

    #[tokio::test]
    async fn oa_tgz_url_includes_api_key_when_configured() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .and(query_param("id", "PMC123"))
            .and(query_param("api_key", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"<records><record><link format="tgz" href="ftp://ftp.ncbi.nlm.nih.gov/pub/pmc/file.tar.gz"/></record></records>"#,
            ))
            .expect(1)
            .mount(&server)
            .await;

        let client = PmcOaClient::new_for_test(server.uri(), Some("test-key".into())).unwrap();
        let manifest = client.oa_archive_manifest("PMC123").await.unwrap().unwrap();
        assert_eq!(
            manifest.tgz_url,
            "https://ftp.ncbi.nlm.nih.gov/pub/pmc/file.tar.gz"
        );
    }

    #[tokio::test]
    async fn get_full_text_xml_accepts_archive_larger_than_default_body_limit() {
        let server = MockServer::start().await;
        let mut tar_buf = Vec::new();
        {
            let mut builder = Builder::new(&mut tar_buf);
            let mut state = 0x1234_5678_u32;
            let filler = (0..(9 * 1024 * 1024))
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 17;
                    state ^= state << 5;
                    (state & 0xff) as u8
                })
                .collect::<Vec<_>>();
            let mut filler_header = Header::new_gnu();
            filler_header.set_size(filler.len() as u64);
            filler_header.set_mode(0o644);
            filler_header.set_cksum();
            builder
                .append_data(&mut filler_header, "supplement.bin", &filler[..])
                .unwrap();

            let contents = b"<article><body>large-ok</body></article>";
            let mut xml_header = Header::new_gnu();
            xml_header.set_size(contents.len() as u64);
            xml_header.set_mode(0o644);
            xml_header.set_cksum();
            builder
                .append_data(&mut xml_header, "sample.nxml", &contents[..])
                .unwrap();
            builder.finish().unwrap();
        }

        let mut gz = GzEncoder::new(Vec::new(), Compression::default());
        gz.write_all(&tar_buf).unwrap();
        let tgz = gz.finish().unwrap();
        assert!(tgz.len() > 8 * 1024 * 1024);

        Mock::given(method("GET"))
            .and(path("/"))
            .and(query_param("id", "PMC123"))
            .respond_with(ResponseTemplate::new(200).set_body_string(format!(
                r#"<records><record><link format="tgz" href="{}/archive.tgz"/></record></records>"#,
                server.uri()
            )))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/archive.tgz"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(tgz))
            .mount(&server)
            .await;

        let client = PmcOaClient::new_for_test(server.uri(), None).unwrap();
        let (xml, manifest) = client
            .get_full_text_xml_with_manifest("PMC123")
            .await
            .expect("large archive should succeed")
            .expect("nxml should be extracted");
        assert_eq!(
            manifest.package_url,
            format!("{}/archive.tgz", server.uri())
        );
        assert!(xml.contains("large-ok"));
    }

    #[test]
    fn extract_first_nxml_reads_xml_entry() {
        let tgz = tgz_with_entries(&[("sample.nxml", b"<article><body>ok</body></article>")]);

        let xml = extract_first_nxml(&tgz).unwrap().unwrap();
        assert!(xml.contains("<article>"));
    }

    #[tokio::test]
    async fn get_archive_package_enumerates_non_xml_and_preserves_binary_bytes() {
        let server = MockServer::start().await;
        let image_bytes = b"\x89PNG\r\n\x1a\n\0\xfffixture";
        let tgz = tgz_with_entries(&[
            ("article.nxml", b"<article><body>ok</body></article>"),
            ("figures/panel.png", image_bytes),
            ("supplement/traces.csv", b"time,value\n0,1\n"),
        ]);

        Mock::given(method("GET"))
            .and(path("/"))
            .and(query_param("id", "PMC123"))
            .respond_with(ResponseTemplate::new(200).set_body_string(format!(
                r#"<records><record license="CC BY" retracted="no"><link format="tgz" href="{}/archive.tgz"/></record></records>"#,
                server.uri()
            )))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/archive.tgz"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(tgz))
            .expect(1)
            .mount(&server)
            .await;

        let client = PmcOaClient::new_for_test(server.uri(), None).unwrap();
        let package = client
            .get_archive_package("PMC123")
            .await
            .expect("package request should succeed")
            .expect("package should exist");
        assert_eq!(package.manifest.license.as_deref(), Some("CC BY"));
        assert_eq!(package.manifest.retracted, Some(false));
        let image = package
            .entries
            .iter()
            .find(|entry| entry.filename == "figures/panel.png")
            .expect("image entry should be listed");
        assert!(!image.is_xml);
        assert_eq!(image.bytes, image_bytes);
        assert!(
            package
                .entries
                .iter()
                .any(|entry| entry.filename == "article.nxml" && entry.is_xml)
        );
    }

    #[test]
    fn extract_archive_entries_rejects_unsafe_empty_and_oversized_members() {
        assert_eq!(
            safe_archive_name(Path::new("safe\\readme.txt")).as_deref(),
            Some("safe/readme.txt")
        );
        assert!(safe_archive_name(Path::new("../secret.csv")).is_none());
        assert!(safe_archive_name(Path::new("..\\secret.csv")).is_none());
        assert!(safe_archive_name(Path::new("/absolute.csv")).is_none());
        assert!(safe_archive_name(Path::new("C:\\absolute.csv")).is_none());

        let oversized = vec![b'x'; MAX_ARCHIVE_ENTRY_BYTES as usize + 1];
        let tgz = tgz_with_entries(&[
            ("article.nxml", &b"<article/>"[..]),
            ("safe/readme.txt", b"ok"),
            ("empty.bin", b""),
            ("huge.bin", oversized.as_slice()),
        ]);

        let entries = extract_archive_entries(&tgz).expect("archive should parse");
        let names = entries
            .iter()
            .map(|entry| entry.filename.as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"article.nxml"));
        assert!(names.contains(&"safe/readme.txt"));
        assert!(!names.contains(&"empty.bin"));
        assert!(!names.contains(&"huge.bin"));
    }
}
