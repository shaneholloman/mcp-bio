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

    const PMC_OA_ARTICLE_PDF: &[u8] =
        include_bytes!("../../../tests/fixtures/article/fulltext/pdf/pmc_oa_article_pdf.pdf");
    const DAILYMED_KEYTRUDA_LABEL_PDF: &[u8] =
        include_bytes!("../../../tests/fixtures/article/fulltext/pdf/dailymed_keytruda_label.pdf");
    const CDC_STI_GUIDELINE_PDF: &[u8] =
        include_bytes!("../../../tests/fixtures/article/fulltext/pdf/cdc_sti_guideline.pdf");

    #[test]
    fn extract_text_from_pdf_renders_fixture_family_text() {
        let cases = [
            (PMC_OA_ARTICLE_PDF, "PDF fallback body text."),
            (
                DAILYMED_KEYTRUDA_LABEL_PDF,
                "Keytruda label quality guard dosing section.",
            ),
            (
                CDC_STI_GUIDELINE_PDF,
                "CDC STI guideline quality guard excerpt.",
            ),
        ];

        for (bytes, needle) in cases {
            let markdown = extract_text_from_pdf(bytes, 12).expect("fixture PDF should render");
            assert!(
                markdown.contains(needle),
                "missing PDF fixture signal: {needle}"
            );
        }
    }

    #[test]
    fn extract_text_from_pdf_rejects_zero_page_limit() {
        let err =
            extract_text_from_pdf(PMC_OA_ARTICLE_PDF, 0).expect_err("zero page limit should fail");
        assert!(matches!(err, BioMcpError::InvalidArgument(_)));
    }
}
