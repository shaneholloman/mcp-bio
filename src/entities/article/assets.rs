use std::collections::BTreeMap;

use roxmltree::Node;
use sha2::{Digest, Sha256};

use crate::error::BioMcpError;
use crate::sources::ncbi_idconv::NcbiIdConverterClient;
use crate::sources::pmc_oa::{
    PmcOaArchiveEntry, PmcOaArchiveManifest, PmcOaArchivePackage, PmcOaClient,
};

use super::{
    Article, ArticleAssetCoverage, ArticleAssetEntry, ArticleAssetJats, ArticleAssetsManifest,
    ArticleFulltextProvenance, ArticleFulltextProvider, ArticleFulltextReuse, ArticleNotIncluded,
    ArticleOmittedCoverage,
};

const PROVIDER_LABEL: &str = "PMC OA Archive";
const PROVIDER_SOURCE: &str = "PMC OA";

#[derive(Clone, Debug, Default)]
struct JatsAssetFacts {
    kind: Option<&'static str>,
    label: Option<String>,
    caption: Option<String>,
    source_id: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct JatsFacts {
    assets: BTreeMap<String, JatsAssetFacts>,
    complex_tables: usize,
}

pub async fn article_assets_manifest(
    requested_id: &str,
) -> Result<ArticleAssetsManifest, BioMcpError> {
    let article = super::detail::get_article_base(requested_id).await?;
    let pmcid = resolve_article_pmcid(&article, requested_id).await?;
    let package = fetch_package(&pmcid).await?;
    Ok(build_assets_manifest(
        requested_id,
        &article,
        &pmcid,
        package,
    ))
}

pub async fn article_asset_bytes(
    requested_id: &str,
    filename: &str,
) -> Result<Vec<u8>, BioMcpError> {
    let article = super::detail::get_article_base(requested_id).await?;
    let pmcid = resolve_article_pmcid(&article, requested_id).await?;
    let package = fetch_package(&pmcid).await?;
    let wanted = filename.trim();
    package
        .entries
        .into_iter()
        .find(|entry| !entry.is_xml && entry.filename == wanted)
        .map(|entry| entry.bytes)
        .ok_or_else(|| BioMcpError::NotFound {
            entity: "article asset".to_string(),
            id: wanted.to_string(),
            suggestion: format!("List assets: biomcp --json get article {requested_id} assets"),
        })
}

pub(super) async fn attach_not_included(article: &mut Article, requested_id: &str) {
    let pmcid = match resolve_article_pmcid(article, requested_id).await {
        Ok(pmcid) => pmcid,
        Err(err) => {
            tracing::warn!(?err, requested_id, "Article asset coverage unavailable");
            return;
        }
    };
    let package = match fetch_package(&pmcid).await {
        Ok(package) => package,
        Err(err) => {
            tracing::warn!(
                ?err,
                requested_id,
                "PMC OA package unavailable for asset coverage"
            );
            return;
        }
    };
    let manifest = build_assets_manifest(requested_id, article, &pmcid, package);
    article.not_included = manifest.not_included;
}

async fn fetch_package(pmcid: &str) -> Result<PmcOaArchivePackage, BioMcpError> {
    PmcOaClient::new()?
        .get_archive_package(pmcid)
        .await?
        .ok_or_else(|| BioMcpError::NotFound {
            entity: "PMC OA package".to_string(),
            id: pmcid.to_string(),
            suggestion: "Try fulltext or verify the article has a PMC Open Access package."
                .to_string(),
        })
}

async fn resolve_article_pmcid(
    article: &Article,
    requested_id: &str,
) -> Result<String, BioMcpError> {
    if let Some(pmcid) = article
        .pmcid
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(pmcid.to_string());
    }

    let ncbi = NcbiIdConverterClient::new()?;
    if let Some(pmid) = article.pmid.as_deref()
        && let Some(pmcid) = ncbi.pmid_to_pmcid(pmid).await?
    {
        return Ok(pmcid);
    }
    if let Some(doi) = article.doi.as_deref()
        && let Some(pmcid) = ncbi.doi_to_pmcid(doi).await?
    {
        return Ok(pmcid);
    }

    Err(BioMcpError::NotFound {
        entity: "PMC OA package".to_string(),
        id: requested_id.to_string(),
        suggestion: "Article has no resolved PMCID for OA asset retrieval.".to_string(),
    })
}

fn build_assets_manifest(
    requested_id: &str,
    article: &Article,
    pmcid: &str,
    package: PmcOaArchivePackage,
) -> ArticleAssetsManifest {
    let facts = jats_facts(&package.entries);
    let provider = provider();
    let reuse = reuse(&package.manifest, article);
    let provenance = provenance(&package.manifest, article);
    let mut assets = package
        .entries
        .iter()
        .filter(|entry| !entry.is_xml)
        .map(|entry| asset_entry(requested_id, entry, &facts, &provider, &reuse, &provenance))
        .collect::<Vec<_>>();
    assets.sort_by(|a, b| a.filename.cmp(&b.filename));
    let mut manifest = ArticleAssetsManifest {
        article_id: requested_id.trim().to_string(),
        pmid: article.pmid.clone(),
        pmcid: Some(pmcid.to_string()),
        provider,
        provenance,
        assets,
        not_included: None,
    };
    manifest.not_included = Some(not_included_from_manifest(&manifest));
    manifest.not_included.as_mut().unwrap().complex_tables.count = facts.complex_tables;
    manifest
}

fn provider() -> ArticleFulltextProvider {
    ArticleFulltextProvider {
        label: PROVIDER_LABEL.to_string(),
        source: PROVIDER_SOURCE.to_string(),
    }
}

fn reuse(manifest: &PmcOaArchiveManifest, article: &Article) -> ArticleFulltextReuse {
    match manifest
        .license
        .clone()
        .or_else(|| article.europepmc_license.clone())
    {
        Some(license) => ArticleFulltextReuse {
            license_present: true,
            license: Some(license),
            reuse_warning: None,
        },
        None => ArticleFulltextReuse {
            license_present: false,
            license: None,
            reuse_warning: Some(
                "License/reuse status is unknown; verify rights before reuse.".to_string(),
            ),
        },
    }
}

fn provenance(manifest: &PmcOaArchiveManifest, article: &Article) -> ArticleFulltextProvenance {
    ArticleFulltextProvenance {
        open_access: article.open_access,
        retracted: manifest.retracted.or(article.europepmc_retracted),
        package_url: Some(manifest.package_url.clone()),
        pdf_fallback_used: false,
    }
}

fn asset_entry(
    requested_id: &str,
    entry: &PmcOaArchiveEntry,
    facts: &JatsFacts,
    provider: &ArticleFulltextProvider,
    reuse: &ArticleFulltextReuse,
    provenance: &ArticleFulltextProvenance,
) -> ArticleAssetEntry {
    let jats = facts.assets.get(&entry.filename);
    let kind = jats
        .and_then(|fact| fact.kind)
        .unwrap_or_else(|| filename_kind(&entry.filename))
        .to_string();
    ArticleAssetEntry {
        filename: entry.filename.clone(),
        kind,
        size_bytes: entry.bytes.len(),
        sha256: sha256_hex(&entry.bytes),
        provider: provider.clone(),
        reuse: reuse.clone(),
        provenance: provenance.clone(),
        jats: jats.and_then(article_asset_jats),
        handle: format!(
            "biomcp get article {} asset {}",
            crate::render::markdown::shell_quote_arg(requested_id.trim()),
            crate::render::markdown::shell_quote_arg(&entry.filename)
        ),
    }
}

fn article_asset_jats(facts: &JatsAssetFacts) -> Option<ArticleAssetJats> {
    if facts.label.is_none() && facts.caption.is_none() && facts.source_id.is_none() {
        return None;
    }
    Some(ArticleAssetJats {
        label: facts.label.clone(),
        caption: facts.caption.clone(),
        source_id: facts.source_id.clone(),
    })
}

fn not_included_from_manifest(manifest: &ArticleAssetsManifest) -> ArticleNotIncluded {
    let figure_count = manifest
        .assets
        .iter()
        .filter(|asset| asset.kind == "figure-image")
        .count();
    let supplement_count = manifest
        .assets
        .iter()
        .filter(|asset| asset.kind == "supplementary-file")
        .count();
    let retrieve_with = format!(
        "biomcp --json get article {} assets",
        crate::render::markdown::shell_quote_arg(&manifest.article_id)
    );
    let mut next_commands = vec![retrieve_with.clone()];
    if let Some(handle) = manifest
        .assets
        .iter()
        .find_map(|asset| (asset.kind == "supplementary-file").then(|| asset.handle.clone()))
    {
        next_commands.push(handle);
    }
    ArticleNotIncluded {
        figure_images: ArticleAssetCoverage {
            count: figure_count,
            retrieve_with: retrieve_with.clone(),
        },
        supplementary_files: ArticleAssetCoverage {
            count: supplement_count,
            retrieve_with: retrieve_with.clone(),
        },
        complex_tables: ArticleOmittedCoverage {
            count: 0,
            retrieve_with,
        },
        next_commands,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn filename_kind(filename: &str) -> &'static str {
    let lower = filename.to_ascii_lowercase();
    if matches!(
        lower.rsplit('.').next(),
        Some("png" | "jpg" | "jpeg" | "gif" | "tif" | "tiff" | "svg" | "webp")
    ) {
        return "figure-image";
    }
    if lower.contains("supp")
        || lower.contains("suppl")
        || lower.contains("s1")
        || matches!(
            lower.rsplit('.').next(),
            Some("csv" | "tsv" | "xlsx" | "xls" | "doc" | "docx" | "pdf")
        )
    {
        return "supplementary-file";
    }
    "other"
}

fn jats_facts(entries: &[PmcOaArchiveEntry]) -> JatsFacts {
    entries
        .iter()
        .find(|entry| entry.is_xml)
        .and_then(|entry| std::str::from_utf8(&entry.bytes).ok())
        .and_then(parse_jats_facts)
        .unwrap_or_default()
}

fn parse_jats_facts(xml: &str) -> Option<JatsFacts> {
    let doc = roxmltree::Document::parse(xml).ok()?;
    let mut out = JatsFacts {
        complex_tables: doc
            .descendants()
            .filter(|node| node.is_element() && node.tag_name().name() == "table-wrap")
            .filter(|node| {
                node.descendants().any(|desc| {
                    desc.is_element()
                        && (desc.attribute("rowspan").is_some()
                            || desc.attribute("colspan").is_some())
                })
            })
            .count(),
        ..JatsFacts::default()
    };

    for node in doc.descendants().filter(|node| node.is_element()) {
        match node.tag_name().name() {
            "fig" => add_asset_facts(&mut out, node, "figure-image"),
            "supplementary-material" => add_asset_facts(&mut out, node, "supplementary-file"),
            _ => {}
        }
    }
    Some(out)
}

fn add_asset_facts(out: &mut JatsFacts, node: Node<'_, '_>, kind: &'static str) {
    let label = child_text(node, "label");
    let caption = child_text(node, "caption");
    let source_id = node.attribute("id").map(str::to_string);
    for href in node
        .descendants()
        .filter(|desc| desc.is_element())
        .filter_map(node_href)
        .filter_map(normalize_href)
    {
        let entry = out.assets.entry(href).or_default();
        entry.kind = Some(kind);
        if entry.label.is_none() {
            entry.label = label.clone();
        }
        if entry.caption.is_none() {
            entry.caption = caption.clone();
        }
        if entry.source_id.is_none() {
            entry.source_id = source_id.clone();
        }
    }
}

fn node_href<'a, 'input>(node: Node<'a, 'input>) -> Option<&'a str> {
    const XLINK_NS: &str = "http://www.w3.org/1999/xlink";
    node.attribute((XLINK_NS, "href"))
        .or_else(|| node.attribute("href"))
}

fn normalize_href(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_start_matches("./");
    if trimmed.is_empty() || trimmed.starts_with('/') || trimmed.contains("..") {
        return None;
    }
    Some(trimmed.rsplit('/').next().unwrap_or(trimmed).to_string())
}

fn child_text(node: Node<'_, '_>, child_name: &str) -> Option<String> {
    let child = node
        .children()
        .find(|child| child.is_element() && child.tag_name().name() == child_name)?;
    let text = child
        .descendants()
        .filter(|desc| desc.is_text())
        .filter_map(|desc| desc.text())
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_article() -> Article {
        Article {
            pmid: Some("22663011".to_string()),
            pmcid: Some("PMC123456".to_string()),
            doi: None,
            title: "Fixture article".to_string(),
            authors: Vec::new(),
            journal: None,
            date: None,
            citation_count: None,
            publication_type: None,
            open_access: Some(true),
            abstract_text: None,
            full_text_path: None,
            full_text_note: None,
            full_text_source: None,
            full_text_manifest: None,
            not_included: None,
            europepmc_license: None,
            europepmc_retracted: None,
            annotations: None,
            semantic_scholar: None,
            pubtator_fallback: false,
        }
    }

    #[test]
    fn build_manifest_hashes_binary_bytes_and_quotes_retrieval_commands() {
        let binary = vec![0, 0xff, b'P', b'N', b'G', b'\n'];
        let csv = b"time,value\n0,1\n".to_vec();
        let jats_xml = br#"
<article xmlns:xlink="http://www.w3.org/1999/xlink">
  <body>
    <fig id="f1">
      <label>Figure 1</label>
      <caption><p>Binary panel.</p></caption>
      <graphic xlink:href="fig 1.png" />
    </fig>
    <supplementary-material id="s1" xlink:href="traces-s1.csv">
      <label>Supplement S1</label>
      <caption><p>Trace data.</p></caption>
    </supplementary-material>
    <table-wrap><table><tr><td rowspan="2">x</td></tr></table></table-wrap>
  </body>
</article>
"#;
        let package = PmcOaArchivePackage {
            manifest: PmcOaArchiveManifest {
                package_url: "https://example.test/archive.tgz".to_string(),
                tgz_url: "https://example.test/archive.tgz".to_string(),
                license: Some("CC BY".to_string()),
                retracted: Some(false),
            },
            entries: vec![
                PmcOaArchiveEntry {
                    filename: "article.nxml".to_string(),
                    bytes: jats_xml.to_vec(),
                    is_xml: true,
                },
                PmcOaArchiveEntry {
                    filename: "fig 1.png".to_string(),
                    bytes: binary.clone(),
                    is_xml: false,
                },
                PmcOaArchiveEntry {
                    filename: "traces-s1.csv".to_string(),
                    bytes: csv.clone(),
                    is_xml: false,
                },
            ],
        };

        let manifest =
            build_assets_manifest("10.1000/foo bar", &sample_article(), "PMC123456", package);
        let fig = manifest
            .assets
            .iter()
            .find(|asset| asset.filename == "fig 1.png")
            .expect("figure asset should be listed");
        assert_eq!(fig.kind, "figure-image");
        assert_eq!(fig.size_bytes, binary.len());
        assert_eq!(fig.sha256, sha256_hex(&binary));
        assert_eq!(
            fig.handle,
            "biomcp get article \"10.1000/foo bar\" asset \"fig 1.png\""
        );
        assert_eq!(
            fig.jats.as_ref().and_then(|jats| jats.label.as_deref()),
            Some("Figure 1")
        );

        let not_included = manifest.not_included.expect("coverage summary");
        assert_eq!(not_included.figure_images.count, 1);
        assert_eq!(not_included.supplementary_files.count, 1);
        assert_eq!(not_included.complex_tables.count, 1);
        assert_eq!(
            not_included.figure_images.retrieve_with,
            "biomcp --json get article \"10.1000/foo bar\" assets"
        );
    }
}
