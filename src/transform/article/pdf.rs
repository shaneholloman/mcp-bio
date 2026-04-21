//! Article PDF-to-markdown extraction helpers.

use std::convert::TryFrom;

use unpdf::render;

use crate::error::BioMcpError;

pub fn extract_text_from_pdf(bytes: &[u8], page_limit: usize) -> Result<String, BioMcpError> {
    if page_limit == 0 {
        return Err(BioMcpError::InvalidArgument(
            "PDF page limit must be at least 1".into(),
        ));
    }

    let page_end = u32::try_from(page_limit)
        .map_err(|_| BioMcpError::InvalidArgument("PDF page limit is too large".into()))?;
    let document = unpdf::parse_bytes(bytes).map_err(|err| BioMcpError::Api {
        api: "article".to_string(),
        message: format!("PDF parsing failed: {err}"),
    })?;
    let options = render::RenderOptions::default()
        .with_heading_analysis()
        .with_page_range(1..=page_end);
    let markdown = render::to_markdown(&document, &options).map_err(|err| BioMcpError::Api {
        api: "article".to_string(),
        message: format!("PDF rendering failed: {err}"),
    })?;

    Ok(markdown.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_PDF: &[u8] =
        include_bytes!("../../../tests/fixtures/article/fulltext/semantic-scholar-fallback.pdf");

    #[test]
    fn extract_text_from_pdf_renders_basic_fixture_text() {
        let markdown = extract_text_from_pdf(SAMPLE_PDF, 12).expect("fixture PDF should render");

        assert!(markdown.contains("PDF fallback body text."));
    }

    #[test]
    fn extract_text_from_pdf_rejects_zero_page_limit() {
        let err = extract_text_from_pdf(SAMPLE_PDF, 0).expect_err("zero page limit should fail");
        assert!(matches!(err, BioMcpError::InvalidArgument(_)));
    }
}
