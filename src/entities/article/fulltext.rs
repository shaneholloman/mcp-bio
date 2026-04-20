use crate::error::BioMcpError;
use crate::sources::europepmc::EuropePmcClient;
use crate::sources::ncbi_efetch::NcbiEfetchClient;
use crate::sources::ncbi_idconv::NcbiIdConverterClient;
use crate::sources::pmc_oa::PmcOaClient;
use crate::transform;
use crate::utils::download;
use tracing::warn;

use super::{Article, ArticleFulltextKind, ArticleFulltextSource};

const FULLTEXT_CACHE_VERSION: &str = "v3";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum XmlWaterfallWinner {
    EuropePmcPmc,
    NcbiEfetchPmc,
    PmcOaArchive,
    EuropePmcMed,
}

fn cache_kind_name(kind: ArticleFulltextKind) -> &'static str {
    match kind {
        ArticleFulltextKind::JatsXml => "jats_xml",
        ArticleFulltextKind::Html => "html",
        ArticleFulltextKind::Pdf => "pdf",
    }
}

fn xml_source_metadata(winner: XmlWaterfallWinner) -> ArticleFulltextSource {
    let (label, source) = match winner {
        XmlWaterfallWinner::EuropePmcPmc => ("Europe PMC XML", "Europe PMC"),
        XmlWaterfallWinner::NcbiEfetchPmc => ("NCBI EFetch PMC XML", "NCBI EFetch"),
        XmlWaterfallWinner::PmcOaArchive => ("PMC OA Archive XML", "PMC OA"),
        XmlWaterfallWinner::EuropePmcMed => ("Europe PMC MED XML", "Europe PMC"),
    };
    ArticleFulltextSource {
        kind: ArticleFulltextKind::JatsXml,
        label: label.to_string(),
        source: source.to_string(),
    }
}

pub(super) fn fulltext_cache_key(kind: ArticleFulltextKind, id: &str) -> String {
    format!(
        "article-fulltext-{FULLTEXT_CACHE_VERSION}:{}:{id}",
        cache_kind_name(kind)
    )
}

async fn render_fulltext_xml(xml: String) -> Result<String, BioMcpError> {
    tokio::task::spawn_blocking(move || transform::article::extract_text_from_xml(&xml))
        .await
        .map_err(|err| BioMcpError::Api {
            api: "article".to_string(),
            message: format!("Full text render worker failed: {err}"),
        })
}

pub(super) async fn resolve_fulltext(
    article: &mut Article,
    requested_id: &str,
) -> Result<(), BioMcpError> {
    let europe = EuropePmcClient::new()?;
    let mut full_text_err: Option<BioMcpError> = None;
    let mut resolved_pmcid = article.pmcid.clone();
    let mut resolved_xml: Option<(String, XmlWaterfallWinner)> = None;

    article.full_text_source = None;

    if resolved_pmcid.is_none() {
        match NcbiIdConverterClient::new() {
            Ok(ncbi) => {
                if let Some(pmid) = article.pmid.as_deref() {
                    match ncbi.pmid_to_pmcid(pmid).await {
                        Ok(value) => resolved_pmcid = value,
                        Err(err) => full_text_err = Some(err),
                    }
                } else if let Some(doi) = article.doi.as_deref() {
                    match ncbi.doi_to_pmcid(doi).await {
                        Ok(value) => resolved_pmcid = value,
                        Err(err) => full_text_err = Some(err),
                    }
                }
            }
            Err(err) => full_text_err = Some(err),
        }
    }

    if article.pmcid.is_none() {
        article.pmcid = resolved_pmcid.clone();
    }

    if let Some(pmcid) = resolved_pmcid.as_deref() {
        match europe.get_full_text_xml("PMC", pmcid).await {
            Ok(Some(value)) => {
                resolved_xml = Some((value, XmlWaterfallWinner::EuropePmcPmc));
            }
            Ok(None) => {}
            Err(err) => full_text_err = Some(err),
        }
    }
    if resolved_xml.is_none()
        && let Some(pmcid) = resolved_pmcid.as_deref()
    {
        match NcbiEfetchClient::new() {
            Ok(ncbi_efetch) => match ncbi_efetch.get_full_text_xml(pmcid).await {
                Ok(Some(value)) => {
                    resolved_xml = Some((value, XmlWaterfallWinner::NcbiEfetchPmc));
                }
                Ok(None) => {}
                Err(err) => full_text_err = Some(err),
            },
            Err(err) => full_text_err = Some(err),
        }
    }
    if resolved_xml.is_none()
        && let Some(pmcid) = resolved_pmcid.as_deref()
    {
        match PmcOaClient::new() {
            Ok(pmc_oa) => match pmc_oa.get_full_text_xml(pmcid).await {
                Ok(Some(value)) => {
                    resolved_xml = Some((value, XmlWaterfallWinner::PmcOaArchive));
                }
                Ok(None) => {}
                Err(err) => full_text_err = Some(err),
            },
            Err(err) => full_text_err = Some(err),
        }
    }
    if resolved_xml.is_none()
        && let Some(pmid) = article.pmid.as_deref()
    {
        match europe.get_full_text_xml("MED", pmid).await {
            Ok(Some(value)) => {
                resolved_xml = Some((value, XmlWaterfallWinner::EuropePmcMed));
            }
            Ok(None) => {}
            Err(err) => full_text_err = Some(err),
        }
    }

    if let Some((xml, winner)) = resolved_xml {
        let text = render_fulltext_xml(xml).await?;
        let key = article
            .pmid
            .as_deref()
            .or(article.doi.as_deref())
            .or(article.pmcid.as_deref())
            .unwrap_or(requested_id);
        let path = download::save_atomic(
            &fulltext_cache_key(ArticleFulltextKind::JatsXml, key),
            &text,
        )
        .await?;
        article.full_text_path = Some(path);
        article.full_text_note = None;
        article.full_text_source = Some(xml_source_metadata(winner));
    } else if let Some(err) = full_text_err {
        warn!(?err, requested_id, "Full text retrieval failed");
        article.full_text_path = None;
        article.full_text_note = Some("Full text not available: API error".into());
    } else if article.pmcid.is_none() {
        article.full_text_path = None;
        article.full_text_note =
            Some("Full text not available: Article not in PubMed Central".into());
    } else {
        article.full_text_path = None;
        article.full_text_note = Some(
            "Full text not available: Full text not available from PMC full-text sources".into(),
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_source_metadata_is_truthful() {
        let cases = [
            (
                XmlWaterfallWinner::EuropePmcPmc,
                ArticleFulltextSource {
                    kind: ArticleFulltextKind::JatsXml,
                    label: "Europe PMC XML".to_string(),
                    source: "Europe PMC".to_string(),
                },
            ),
            (
                XmlWaterfallWinner::NcbiEfetchPmc,
                ArticleFulltextSource {
                    kind: ArticleFulltextKind::JatsXml,
                    label: "NCBI EFetch PMC XML".to_string(),
                    source: "NCBI EFetch".to_string(),
                },
            ),
            (
                XmlWaterfallWinner::PmcOaArchive,
                ArticleFulltextSource {
                    kind: ArticleFulltextKind::JatsXml,
                    label: "PMC OA Archive XML".to_string(),
                    source: "PMC OA".to_string(),
                },
            ),
            (
                XmlWaterfallWinner::EuropePmcMed,
                ArticleFulltextSource {
                    kind: ArticleFulltextKind::JatsXml,
                    label: "Europe PMC MED XML".to_string(),
                    source: "Europe PMC".to_string(),
                },
            ),
        ];

        for (winner, expected) in cases {
            assert_eq!(xml_source_metadata(winner), expected);
        }
    }

    #[test]
    fn fulltext_cache_key_is_kind_aware_and_versioned() {
        assert_eq!(
            fulltext_cache_key(ArticleFulltextKind::JatsXml, "22663011"),
            "article-fulltext-v3:jats_xml:22663011"
        );
        assert_eq!(
            fulltext_cache_key(ArticleFulltextKind::Html, "10.1000/example"),
            "article-fulltext-v3:html:10.1000/example"
        );
    }
}
