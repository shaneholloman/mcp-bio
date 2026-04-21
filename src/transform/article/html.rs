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

    const SAMPLE_HTML: &str =
        include_str!("../../../tests/fixtures/article/fulltext/pmc-html-fallback.html");

    #[test]
    fn extract_text_from_html_keeps_readable_article_content() {
        let markdown = extract_text_from_html(
            SAMPLE_HTML,
            "https://pmc.ncbi.nlm.nih.gov/articles/PMC123457/",
        )
        .expect("fixture HTML should convert");

        assert!(markdown.contains("PMC HTML fallback winner"));
        assert!(markdown.contains("PMC HTML fallback body text."));
    }
}
