//! Article HTML-to-markdown extraction helpers.

use readability_rust::Readability;

use crate::error::BioMcpError;

pub fn extract_text_from_html(html: &str, base_url: &str) -> Result<String, BioMcpError> {
    let extracted_html = extract_readable_html(html, base_url)?;
    let source_html = if extracted_html.trim().is_empty() {
        html
    } else {
        extracted_html.as_str()
    };

    let markdown = htmd::convert(source_html).map_err(|err| BioMcpError::Api {
        api: "article".to_string(),
        message: format!("HTML to markdown conversion failed: {err}"),
    })?;

    Ok(markdown.trim().to_string())
}

fn extract_readable_html(html: &str, base_url: &str) -> Result<String, BioMcpError> {
    let mut parser =
        Readability::new_with_base_uri(html, base_url, None).map_err(|err| BioMcpError::Api {
            api: "article".to_string(),
            message: format!("HTML readability initialization failed: {err}"),
        })?;

    Ok(parser
        .parse()
        .and_then(|article| article.content)
        .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PMC_ARTICLE_PAGE: &str =
        include_str!("../../../tests/fixtures/article/fulltext/html/pmc_article_page.html");
    const BIORXIV_PREPRINT_PAGE: &str =
        include_str!("../../../tests/fixtures/article/fulltext/html/biorxiv_preprint_page.html");
    const NIH_NEWS_RELEASE_PAGE: &str =
        include_str!("../../../tests/fixtures/article/fulltext/html/nih_news_release.html");

    #[test]
    fn extract_text_from_html_keeps_article_signals_across_fixture_family() {
        let cases: [(&str, &str, &[&str]); 3] = [
            (
                PMC_ARTICLE_PAGE,
                "https://pmc.ncbi.nlm.nih.gov/articles/PMC123457/",
                &["PMC HTML fallback winner", "PMC HTML fallback body text."],
            ),
            (
                BIORXIV_PREPRINT_PAGE,
                "https://www.biorxiv.org/content/10.1101/2025.01.01.123456v1",
                &["Preprint markdown quality guard body."],
            ),
            (
                NIH_NEWS_RELEASE_PAGE,
                "https://www.nih.gov/news-events/news-releases/nih-quality-guard",
                &["News release markdown quality guard body."],
            ),
        ];

        for (html, base_url, expected) in cases {
            let markdown =
                extract_text_from_html(html, base_url).expect("fixture HTML should convert");

            for needle in expected {
                assert!(
                    markdown.contains(needle),
                    "missing HTML fixture signal: {needle}"
                );
            }
        }
    }
}
