use std::borrow::Cow;
use std::time::Duration;

use reqwest::header::{CONTENT_TYPE, HeaderValue};
use roxmltree::{Document, Node};

use crate::error::BioMcpError;

const VAERS_BASE: &str = "https://wonder.cdc.gov";
const VAERS_API: &str = "vaers";
pub(crate) const VAERS_BASE_ENV: &str = "BIOMCP_VAERS_BASE";
const VAERS_REQUEST_PATH: &str = "/controller/datarequest/D8";
const VAERS_MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const LIVE_REQUEST_GAP: Duration = Duration::from_secs(16);
const REQUEST_TEMPLATE: &str = include_str!("../../spec/fixtures/vaers/reactions-request.xml");
const REQUEST_RESTRICTIONS_FLAG: (&str, &str) = ("accept_datause_restrictions", "true");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VaersAggregateKind {
    Reactions,
    Seriousness,
    Age,
}

impl VaersAggregateKind {
    fn group_by_code(self) -> &'static str {
        match self {
            Self::Reactions => "D8.V13-level2",
            Self::Seriousness => "D8.V10",
            Self::Age => "D8.V1",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VaersAggregateRow {
    pub label: String,
    pub count: usize,
    pub percentage: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VaersAggregateTable {
    pub total_events: usize,
    pub rows: Vec<VaersAggregateRow>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VaersSeriousBreakdown {
    pub total_reports: usize,
    pub serious_reports: usize,
    pub non_serious_reports: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VaersSummaryTables {
    pub total_reports: usize,
    pub reactions: Vec<VaersAggregateRow>,
    pub serious_reports: usize,
    pub non_serious_reports: usize,
    pub age_distribution: Vec<VaersAggregateRow>,
}

pub(crate) struct VaersClient {
    client: reqwest_middleware::ClientWithMiddleware,
    base: Cow<'static, str>,
}

impl VaersClient {
    pub(crate) fn new() -> Result<Self, BioMcpError> {
        Ok(Self {
            client: crate::sources::shared_client()?,
            base: crate::sources::env_base(VAERS_BASE, VAERS_BASE_ENV),
        })
    }

    #[cfg(test)]
    fn new_for_test(base: String) -> Result<Self, BioMcpError> {
        Ok(Self {
            client: crate::sources::test_client()?,
            base: Cow::Owned(base),
        })
    }

    fn endpoint(&self) -> String {
        format!(
            "{}/{}",
            self.base.as_ref().trim_end_matches('/'),
            VAERS_REQUEST_PATH.trim_start_matches('/')
        )
    }

    fn uses_live_rate_limit_delay(&self) -> bool {
        self.base.as_ref().contains("wonder.cdc.gov")
    }

    async fn pause_between_live_requests(&self) {
        if self.uses_live_rate_limit_delay() {
            tokio::time::sleep(LIVE_REQUEST_GAP).await;
        }
    }

    pub(crate) async fn health_check(&self) -> Result<(), BioMcpError> {
        self.reaction_counts("MMR").await.map(|_| ())
    }

    pub(crate) async fn reaction_counts(
        &self,
        wonder_code: &str,
    ) -> Result<VaersAggregateTable, BioMcpError> {
        self.fetch_aggregate_table(VaersAggregateKind::Reactions, wonder_code)
            .await
    }

    pub(crate) async fn seriousness_breakdown(
        &self,
        wonder_code: &str,
    ) -> Result<VaersSeriousBreakdown, BioMcpError> {
        let table = self
            .fetch_aggregate_table(VaersAggregateKind::Seriousness, wonder_code)
            .await?;
        let serious_reports = table
            .rows
            .iter()
            .find(|row| row.label.eq_ignore_ascii_case("yes"))
            .map(|row| row.count)
            .unwrap_or_default();
        let non_serious_reports = table
            .rows
            .iter()
            .find(|row| row.label.eq_ignore_ascii_case("no"))
            .map(|row| row.count)
            .unwrap_or_default();

        Ok(VaersSeriousBreakdown {
            total_reports: table.total_events,
            serious_reports,
            non_serious_reports,
        })
    }

    pub(crate) async fn age_distribution(
        &self,
        wonder_code: &str,
    ) -> Result<VaersAggregateTable, BioMcpError> {
        self.fetch_aggregate_table(VaersAggregateKind::Age, wonder_code)
            .await
    }

    pub(crate) async fn summary(
        &self,
        wonder_code: &str,
    ) -> Result<VaersSummaryTables, BioMcpError> {
        let reactions = self.reaction_counts(wonder_code).await?;
        self.pause_between_live_requests().await;
        let seriousness = self.seriousness_breakdown(wonder_code).await?;
        self.pause_between_live_requests().await;
        let age_distribution = self.age_distribution(wonder_code).await?;

        let total_reports = reactions
            .total_events
            .max(seriousness.total_reports)
            .max(age_distribution.total_events);

        Ok(VaersSummaryTables {
            total_reports,
            reactions: reactions.rows,
            serious_reports: seriousness.serious_reports,
            non_serious_reports: seriousness.non_serious_reports,
            age_distribution: age_distribution.rows,
        })
    }

    async fn fetch_aggregate_table(
        &self,
        kind: VaersAggregateKind,
        wonder_code: &str,
    ) -> Result<VaersAggregateTable, BioMcpError> {
        let request_xml = build_request_xml(kind, wonder_code)?;
        let response = self
            .client
            .post(self.endpoint())
            .form(&[
                ("request_xml", request_xml.as_str()),
                REQUEST_RESTRICTIONS_FLAG,
            ])
            .send()
            .await?;
        let status = response.status();
        let content_type = response.headers().get(CONTENT_TYPE).cloned();
        let body =
            crate::sources::read_limited_body_with_limit(response, VAERS_API, VAERS_MAX_BODY_BYTES)
                .await?;

        if !status.is_success() {
            let excerpt = crate::sources::summarize_http_error_body(content_type.as_ref(), &body);
            return Err(BioMcpError::Api {
                api: VAERS_API.to_string(),
                message: format!("CDC WONDER VAERS HTTP {status}: {excerpt}"),
            });
        }

        reject_html_gateway(content_type.as_ref(), &body)?;

        let xml = String::from_utf8(body).map_err(|_| BioMcpError::Api {
            api: VAERS_API.to_string(),
            message: "CDC WONDER VAERS response body was not valid UTF-8 XML".to_string(),
        })?;

        tokio::task::spawn_blocking(move || parse_aggregate_response(&xml))
            .await
            .map_err(|err| BioMcpError::Api {
                api: VAERS_API.to_string(),
                message: format!("CDC WONDER VAERS XML parse task failed: {err}"),
            })?
    }
}

fn build_request_xml(kind: VaersAggregateKind, wonder_code: &str) -> Result<String, BioMcpError> {
    let wonder_code = wonder_code.trim();
    if wonder_code.is_empty() {
        return Err(BioMcpError::InvalidArgument(
            "CDC WONDER VAERS query requires a vaccine code".into(),
        ));
    }

    let request = REQUEST_TEMPLATE.replace(
        "<name>B_1</name><value>D8.V13-level2</value>",
        &format!("<name>B_1</name><value>{}</value>", kind.group_by_code()),
    );
    Ok(request.replace(
        "<name>F_D8.V14</name><value>MMR</value>",
        &format!(
            "<name>F_D8.V14</name><value>{}</value>",
            xml_escape(wonder_code)
        ),
    ))
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn reject_html_gateway(content_type: Option<&HeaderValue>, body: &[u8]) -> Result<(), BioMcpError> {
    let sniff = String::from_utf8_lossy(&body[..body.len().min(128)])
        .trim_start()
        .to_ascii_lowercase();
    if sniff.starts_with("<!doctype html") || sniff.starts_with("<html") {
        return Err(BioMcpError::Api {
            api: VAERS_API.to_string(),
            message: format!(
                "CDC WONDER VAERS returned an HTML gateway page: {}",
                crate::sources::body_excerpt(body)
            ),
        });
    }

    let Some(content_type) = content_type else {
        return Ok(());
    };
    let Ok(raw) = content_type.to_str() else {
        return Ok(());
    };
    let media_type = raw
        .split(';')
        .next()
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(media_type.as_str(), "text/html" | "application/xhtml+xml")
        && !(sniff.starts_with("<?xml") || sniff.starts_with("<page"))
    {
        return Err(BioMcpError::Api {
            api: VAERS_API.to_string(),
            message: format!(
                "CDC WONDER VAERS returned unexpected HTML content: {}",
                crate::sources::body_excerpt(body)
            ),
        });
    }
    Ok(())
}

fn parse_aggregate_response(xml: &str) -> Result<VaersAggregateTable, BioMcpError> {
    let doc = Document::parse(xml).map_err(|source| BioMcpError::Api {
        api: VAERS_API.to_string(),
        message: format!("Invalid CDC WONDER VAERS XML response: {source}"),
    })?;

    if let Some(message) = processing_error_message(&doc) {
        return Err(BioMcpError::Api {
            api: VAERS_API.to_string(),
            message,
        });
    }

    let data_table = doc
        .descendants()
        .find(|node| node.has_tag_name("data-table"))
        .ok_or_else(|| BioMcpError::Api {
            api: VAERS_API.to_string(),
            message: "CDC WONDER VAERS response did not contain a data-table".to_string(),
        })?;
    let total_events = total_events_message(&doc).ok_or_else(|| BioMcpError::Api {
        api: VAERS_API.to_string(),
        message: "CDC WONDER VAERS response did not report total events".to_string(),
    })?;

    let mut rows = Vec::new();
    for row in element_children(data_table).filter(|node| node.has_tag_name("r")) {
        let cells = element_children(row)
            .filter(|node| node.has_tag_name("c"))
            .collect::<Vec<_>>();
        if cells.len() < 3 {
            continue;
        }
        if cells[1].attribute("dt").is_some() {
            continue;
        }
        let Some(label) = cells[0]
            .attribute("l")
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let Some(count) = cells[1].attribute("v").and_then(parse_number) else {
            continue;
        };
        let Some(percentage) = cells[2].attribute("v").and_then(parse_percentage) else {
            continue;
        };
        rows.push(VaersAggregateRow {
            label: label.to_string(),
            count,
            percentage,
        });
    }

    Ok(VaersAggregateTable { total_events, rows })
}

fn processing_error_message(doc: &Document<'_>) -> Option<String> {
    let title = doc
        .descendants()
        .find(|node| node.has_tag_name("title"))
        .and_then(node_text)
        .unwrap_or_default();
    let message = doc
        .descendants()
        .filter(|node| node.has_tag_name("message"))
        .find_map(node_text)
        .unwrap_or_default();

    if title.eq_ignore_ascii_case("Processing Error") {
        let trimmed = message.trim();
        return Some(if trimmed.is_empty() {
            "CDC WONDER VAERS returned a processing error".to_string()
        } else {
            format!("CDC WONDER VAERS returned a processing error: {trimmed}")
        });
    }
    None
}

fn total_events_message(doc: &Document<'_>) -> Option<usize> {
    doc.descendants()
        .filter(|node| node.has_tag_name("message"))
        .filter_map(node_text)
        .find_map(|text| {
            let marker = "These results are for ";
            let start = text.find(marker)?;
            let remainder = &text[start + marker.len()..];
            let end = remainder.find(" total events")?;
            parse_number(&remainder[..end])
        })
}

fn parse_number(raw: &str) -> Option<usize> {
    let digits = raw
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        None
    } else {
        digits.parse::<usize>().ok()
    }
}

fn parse_percentage(raw: &str) -> Option<f64> {
    raw.trim().trim_end_matches('%').parse::<f64>().ok()
}

fn element_children<'a>(node: Node<'a, 'a>) -> impl Iterator<Item = Node<'a, 'a>> {
    node.children().filter(|child| child.is_element())
}

fn node_text(node: Node<'_, '_>) -> Option<String> {
    let mut text = String::new();
    for child in node.children() {
        if let Some(part) = child.text() {
            text.push_str(part);
        }
    }
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use roxmltree::Document;
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::{
        REQUEST_TEMPLATE, VAERS_REQUEST_PATH, VaersAggregateKind, VaersClient, build_request_xml,
        node_text, parse_aggregate_response,
    };

    const REACTIONS_REQUEST_FIXTURE: &str =
        include_str!("../../spec/fixtures/vaers/reactions-request.xml");
    const SERIOUS_REQUEST_FIXTURE: &str =
        include_str!("../../spec/fixtures/vaers/serious-request.xml");
    const AGE_REQUEST_FIXTURE: &str = include_str!("../../spec/fixtures/vaers/age-request.xml");
    const REACTIONS_RESPONSE_FIXTURE: &str =
        include_str!("../../spec/fixtures/vaers/reactions-response.xml");
    const SERIOUS_RESPONSE_FIXTURE: &str =
        include_str!("../../spec/fixtures/vaers/serious-response.xml");
    const AGE_RESPONSE_FIXTURE: &str = include_str!("../../spec/fixtures/vaers/age-response.xml");

    fn parameter_map(xml: &str) -> BTreeMap<String, String> {
        let doc = Document::parse(xml).expect("request fixture should parse");
        doc.descendants()
            .filter(|node| node.has_tag_name("parameter"))
            .filter_map(|parameter| {
                let name = parameter
                    .children()
                    .find(|node| node.has_tag_name("name"))
                    .and_then(node_text)?;
                let value = parameter
                    .children()
                    .find(|node| node.has_tag_name("value"))
                    .map(|node| node.text().unwrap_or_default().to_string())
                    .unwrap_or_default();
                Some((name, value))
            })
            .collect()
    }

    #[test]
    fn request_template_tracks_captured_fixture() {
        assert_eq!(REQUEST_TEMPLATE, REACTIONS_REQUEST_FIXTURE);
    }

    #[test]
    fn build_request_xml_matches_reaction_fixture_parameters() {
        let built = build_request_xml(VaersAggregateKind::Reactions, "MMR").expect("request");
        assert_eq!(
            parameter_map(&built),
            parameter_map(REACTIONS_REQUEST_FIXTURE)
        );
    }

    #[test]
    fn build_request_xml_matches_serious_fixture_parameters() {
        let built = build_request_xml(VaersAggregateKind::Seriousness, "MMR").expect("request");
        assert_eq!(
            parameter_map(&built),
            parameter_map(SERIOUS_REQUEST_FIXTURE)
        );
    }

    #[test]
    fn build_request_xml_matches_age_fixture_parameters() {
        let built = build_request_xml(VaersAggregateKind::Age, "MMR").expect("request");
        assert_eq!(parameter_map(&built), parameter_map(AGE_REQUEST_FIXTURE));
    }

    #[test]
    fn parse_reaction_response_extracts_total_events_and_rows() {
        let table = parse_aggregate_response(REACTIONS_RESPONSE_FIXTURE).expect("parse reactions");

        assert_eq!(table.total_events, 83_359);
        assert_eq!(
            table.rows.first().map(|row| row.label.as_str()),
            Some("ABASIA")
        );
        assert_eq!(table.rows.first().map(|row| row.count), Some(179));
        assert_eq!(table.rows.first().map(|row| row.percentage), Some(0.21));
    }

    #[test]
    fn parse_serious_response_extracts_yes_and_no_rows() {
        let table = parse_aggregate_response(SERIOUS_RESPONSE_FIXTURE).expect("parse serious");

        assert_eq!(table.total_events, 83_359);
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0].label, "Yes");
        assert_eq!(table.rows[0].count, 5_795);
        assert_eq!(table.rows[1].label, "No");
        assert_eq!(table.rows[1].count, 77_564);
    }

    #[test]
    fn parse_age_response_extracts_buckets_and_skips_total_row() {
        let table = parse_aggregate_response(AGE_RESPONSE_FIXTURE).expect("parse age");

        assert_eq!(table.total_events, 83_359);
        assert_eq!(
            table.rows.first().map(|row| row.label.as_str()),
            Some("< 6 months")
        );
        assert_eq!(
            table.rows.last().map(|row| row.label.as_str()),
            Some("Unknown")
        );
        assert_eq!(table.rows.last().map(|row| row.count), Some(10_133));
    }

    #[test]
    fn parse_processing_error_returns_api_message() {
        let err = parse_aggregate_response(
            r#"<?xml version="1.0"?><page><title>Processing Error</title><message>Request rate exceeded.</message></page>"#,
        )
        .expect_err("processing error should fail");

        assert!(err.to_string().contains("Request rate exceeded"));
    }

    #[tokio::test]
    async fn reaction_counts_posts_form_encoded_request_xml() {
        let server = MockServer::start().await;
        let client = VaersClient::new_for_test(server.uri()).expect("client");

        Mock::given(method("POST"))
            .and(path(VAERS_REQUEST_PATH))
            .and(body_string_contains("request_xml="))
            .and(body_string_contains("accept_datause_restrictions=true"))
            .and(body_string_contains("B_1"))
            .and(body_string_contains("D8.V13-level2"))
            .and(body_string_contains("MMR"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/html; charset=ISO-8859-1")
                    .set_body_raw(REACTIONS_RESPONSE_FIXTURE, "text/html; charset=ISO-8859-1"),
            )
            .mount(&server)
            .await;

        let table = client
            .reaction_counts("MMR")
            .await
            .expect("reaction counts");

        assert_eq!(table.total_events, 83_359);
        assert!(!table.rows.is_empty());
    }
}
