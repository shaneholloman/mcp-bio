use roxmltree::{Document, ParsingOptions};

pub(crate) const ARTICLE_XML_NODE_LIMIT: u32 = 1_000_000;

/// Element-nesting ceiling for external XML. The node limit bounds
/// breadth; this bounds depth, which is what overflows the 2-4 MiB
/// blocking-thread stacks through the recursive JATS and ClinVar
/// walkers. Measured: the deepest tracked fixture nests 9 levels
/// (VAERS responses); JATS publisher markup stays an order below
/// this cap (ticket 1243).
pub(crate) const EXTERNAL_XML_DEPTH_LIMIT: u32 = 64;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ExternalXmlError {
    #[error("XML entity declarations are not supported")]
    EntityDeclaration,
    #[error("invalid XML")]
    Parse(#[source] roxmltree::Error),
    #[error("XML nesting exceeds {0} levels")]
    DepthLimitExceeded(u32),
}

pub(crate) fn parse_external_xml(
    xml: &str,
    nodes_limit: u32,
) -> Result<Document<'_>, ExternalXmlError> {
    let bytes = xml.as_bytes();
    let mut index = 0;
    let mut in_markup = false;
    let mut quote = None;
    // Depth is counted in the pre-parse scan, not after parsing: the
    // acceptance nesting-bomb test proved roxmltree's own parse
    // recurses per open tag, so the rejection must happen before
    // Document::parse or the parser stack overflows first (ticket
    // 1243; the ticket's post-parse-walk plan was revised on that
    // evidence). Comments, CDATA, and processing instructions are
    // skipped wholesale below and never move the counter.
    let mut depth: u32 = 0;
    while index < bytes.len() {
        if let Some(delimiter) = quote {
            if bytes[index] == delimiter {
                quote = None;
            }
            index += 1;
            continue;
        }

        let remainder = &bytes[index..];
        let skipped = if remainder.starts_with(b"<!--") {
            remainder
                .windows(3)
                .position(|window| window == b"-->")
                .map(|end| end + 3)
        } else if remainder.starts_with(b"<![CDATA[") {
            remainder
                .windows(3)
                .position(|window| window == b"]]>")
                .map(|end| end + 3)
        } else if remainder.starts_with(b"<?") {
            remainder
                .windows(2)
                .position(|window| window == b"?>")
                .map(|end| end + 2)
        } else {
            None
        };
        if let Some(skipped) = skipped {
            index += skipped;
            continue;
        }
        if remainder.starts_with(b"<!--")
            || remainder.starts_with(b"<![CDATA[")
            || remainder.starts_with(b"<?")
        {
            break;
        }

        if !in_markup {
            if bytes[index] != b'<' {
                index += 1;
                continue;
            }
            in_markup = true;
            // An opening tag (not a close, declaration, or PI).
            if !matches!(bytes.get(index + 1), Some(b'/') | Some(b'!') | Some(b'?')) {
                depth += 1;
                if depth > EXTERNAL_XML_DEPTH_LIMIT {
                    return Err(ExternalXmlError::DepthLimitExceeded(
                        EXTERNAL_XML_DEPTH_LIMIT,
                    ));
                }
            } else if bytes.get(index + 1) == Some(&b'/') {
                depth = depth.saturating_sub(1);
            }
        }
        if matches!(bytes[index], b'\'' | b'"') {
            quote = Some(bytes[index]);
        } else if remainder.starts_with(b"<!ENTITY") {
            return Err(ExternalXmlError::EntityDeclaration);
        } else if bytes[index] == b'>' && in_markup {
            // A self-closing tag opened one level above; its `/>`
            // closes what it opened.
            if bytes.get(index.wrapping_sub(1)) == Some(&b'/') {
                depth = depth.saturating_sub(1);
            }
            in_markup = false;
        }
        index += 1;
    }

    Document::parse_with_options(
        xml,
        ParsingOptions {
            allow_dtd: true,
            nodes_limit,
        },
    )
    .map_err(ExternalXmlError::Parse)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ordinary_declared_and_external_doctype_xml() {
        for xml in [
            "<article><body>plain</body></article>",
            r#"<?xml version="1.0"?><article><body>declared</body></article>"#,
            r#"<?xml version="1.0"?>
<!DOCTYPE article PUBLIC
  "-//NLM//DTD JATS Journal Archiving DTD v1.4 20241031//EN"
  "https://example.invalid/JATS-archivearticle1-4.dtd">
<article><body>70 &#181;m</body></article>"#,
        ] {
            let doc = parse_external_xml(xml, 32).expect("external XML should parse");
            assert_eq!(doc.root_element().tag_name().name(), "article");
        }

        let doc = parse_external_xml("<article><body>70 &#181;m</body></article>", 32).unwrap();
        let body = doc
            .descendants()
            .find(|node| node.has_tag_name("body"))
            .expect("body");
        assert_eq!(body.text(), Some("70 µm"));
    }

    #[test]
    fn maps_malformed_xml_to_parse_error() {
        assert!(matches!(
            parse_external_xml("<article>", 8),
            Err(ExternalXmlError::Parse(_))
        ));
    }

    #[test]
    fn rejects_entity_declaration_variants_before_parsing() {
        for xml in [
            r#"<!DOCTYPE article [<!ENTITY name "value">]><article />"#,
            r#"<!DOCTYPE article [<!ENTITY % local "value">]><article />"#,
            r#"<!DOCTYPE article [<!ENTITY external SYSTEM "https://example.invalid/entity">]><article />"#,
            r#"<!DOCTYPE article SYSTEM "<!--" [<!ENTITY hidden "value"><!-- -->]><article>&hidden;</article>"#,
            r#"<!DOCTYPE article [<?pi "?><!ENTITY hidden "value">]><article>&hidden;</article>"#,
        ] {
            assert!(matches!(
                parse_external_xml(xml, 32),
                Err(ExternalXmlError::EntityDeclaration)
            ));
        }
    }

    #[test]
    fn accepts_entity_token_text_in_comments_and_cdata() {
        let xml =
            "<article><!-- <!ENTITY ignored> --><body><![CDATA[<!ENTITY text>]]></body></article>";
        let doc = parse_external_xml(xml, 16).expect("tokens are not declarations");
        let body = doc
            .descendants()
            .find(|node| node.has_tag_name("body"))
            .expect("body");
        assert_eq!(body.text(), Some("<!ENTITY text>"));
    }

    #[test]
    fn rejects_a_nesting_bomb_with_an_error() {
        // the scan rejects the document before parse: roxmltree's
        // parser recurses per open tag (the recorded deviation), so a
        // recursive parse would overflow the blocking-thread stack
        // here without the pre-parse rejection.
        let opens = "<b>".repeat(100_000);
        let bomb = format!("<a>{opens}");
        assert!(matches!(
            parse_external_xml(&bomb, 1_000_000),
            Err(ExternalXmlError::DepthLimitExceeded(_))
        ));
    }

    #[test]
    fn accepts_nesting_well_under_the_cap() {
        let opens = "<b>".repeat(20);
        let closes = "</b>".repeat(20);
        let ok = format!("<a>{opens}{closes}</a>");
        let doc = parse_external_xml(&ok, 64).expect("20 levels are well under the cap");
        assert!(doc.root_element().has_tag_name("a"));
    }

    #[test]
    fn comments_cdata_and_self_closing_tags_do_not_move_the_depth() {
        let xml = "<a><b>text<c/><!-- <d><d><d> --><?pi <a><a> ?><d/></b></a>";
        let doc = parse_external_xml(xml, 32).expect("parses");
        assert!(doc.root_element().has_tag_name("a"));
    }

    #[test]
    fn enforces_exact_node_limit() {
        parse_external_xml("<article />", 2).expect("root and article fit");
        assert!(matches!(
            parse_external_xml("<article><body /></article>", 2),
            Err(ExternalXmlError::Parse(roxmltree::Error::NodesLimitReached))
        ));
    }
}
