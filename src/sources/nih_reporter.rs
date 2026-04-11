use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashMap;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use time::{Date, Month, OffsetDateTime};

use crate::error::BioMcpError;

const NIH_REPORTER_BASE: &str = "https://api.reporter.nih.gov/v2";
const NIH_REPORTER_API: &str = "nih_reporter";
const NIH_REPORTER_BASE_ENV: &str = "BIOMCP_NIH_REPORTER_BASE";
const NIH_REPORTER_PATH: &str = "projects/search";
const NIH_REPORTER_MAX_RESULTS: usize = 50;
const NIH_REPORTER_DISPLAY_LIMIT: usize = 10;
const NIH_REPORTER_FISCAL_YEAR_WINDOW: i32 = 5;
const NIH_REPORTER_SEARCH_FIELDS: &str = "projecttitle,abstracttext";
const NIH_REPORTER_INCLUDE_FIELDS: &[&str] = &[
    "ProjectTitle",
    "PrincipalInvestigators",
    "ContactPiName",
    "Organization",
    "FiscalYear",
    "AwardAmount",
    "ProjectNum",
    "CoreProjectNum",
    "ProjectDetailUrl",
];

pub struct NihReporterClient {
    client: reqwest_middleware::ClientWithMiddleware,
    base: Cow<'static, str>,
}

impl NihReporterClient {
    pub fn new() -> Result<Self, BioMcpError> {
        Ok(Self {
            client: crate::sources::shared_client()?,
            base: crate::sources::env_base(NIH_REPORTER_BASE, NIH_REPORTER_BASE_ENV),
        })
    }

    #[cfg(test)]
    fn new_for_test(base: String) -> Result<Self, BioMcpError> {
        Ok(Self {
            client: crate::sources::test_client()?,
            base: Cow::Owned(base),
        })
    }

    fn endpoint(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.base.as_ref().trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    async fn post_json<T: DeserializeOwned, B: Serialize>(
        &self,
        req: reqwest_middleware::RequestBuilder,
        body: &B,
    ) -> Result<T, BioMcpError> {
        let resp = crate::sources::apply_cache_mode(req.json(body))
            .send()
            .await?;
        let status = resp.status();
        let content_type = resp.headers().get(reqwest::header::CONTENT_TYPE).cloned();
        let bytes = crate::sources::read_limited_body(resp, NIH_REPORTER_API).await?;

        if !status.is_success() {
            let excerpt = crate::sources::body_excerpt(&bytes);
            return Err(BioMcpError::Api {
                api: NIH_REPORTER_API.to_string(),
                message: format!("HTTP {status}: {excerpt}"),
            });
        }

        crate::sources::ensure_json_content_type(NIH_REPORTER_API, content_type.as_ref(), &bytes)?;
        serde_json::from_slice(&bytes).map_err(|source| BioMcpError::ApiJson {
            api: NIH_REPORTER_API.to_string(),
            source,
        })
    }

    pub async fn funding(&self, query: &str) -> Result<NihReporterFundingSection, BioMcpError> {
        let query = normalize_query(query)?;
        let request = build_search_request(&query, current_funding_window_date());
        let fiscal_years = request.criteria.fiscal_years.clone();
        let response: NihReporterSearchResponse = self
            .post_json(self.client.post(self.endpoint(NIH_REPORTER_PATH)), &request)
            .await?;

        Ok(NihReporterFundingSection {
            query,
            fiscal_years,
            matching_project_years: response.meta.total,
            grants: deduplicate_grants(
                response
                    .results
                    .into_iter()
                    .filter_map(map_project_year_row)
                    .collect(),
            ),
        })
    }
}

fn current_funding_window_date() -> Date {
    // Prefer the operator's local date so the Oct 1 NIH fiscal-year rollover
    // matches the CLI environment; fall back to UTC if the local offset is
    // unavailable on the current platform.
    OffsetDateTime::now_local()
        .map(|now| now.date())
        .unwrap_or_else(|_| OffsetDateTime::now_utc().date())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NihReporterFundingSection {
    pub query: String,
    pub fiscal_years: Vec<i32>,
    pub matching_project_years: usize,
    pub grants: Vec<NihReporterGrant>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NihReporterGrant {
    pub project_title: String,
    pub project_num: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub core_project_num: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_detail_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pi_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization: Option<String>,
    pub fiscal_year: i32,
    pub award_amount: u64,
}

#[derive(Debug, Clone, Serialize)]
struct NihReporterSearchRequest {
    criteria: NihReporterCriteria,
    include_fields: Vec<&'static str>,
    offset: usize,
    limit: usize,
    sort_field: &'static str,
    sort_order: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct NihReporterCriteria {
    advanced_text_search: NihReporterAdvancedTextSearch,
    fiscal_years: Vec<i32>,
}

#[derive(Debug, Clone, Serialize)]
struct NihReporterAdvancedTextSearch {
    operator: &'static str,
    search_field: &'static str,
    search_text: String,
}

#[derive(Debug, Clone, Deserialize)]
struct NihReporterSearchResponse {
    meta: NihReporterMeta,
    #[serde(default)]
    results: Vec<NihReporterProjectYearRow>,
}

#[derive(Debug, Clone, Deserialize)]
struct NihReporterMeta {
    total: usize,
}

#[derive(Debug, Clone, Deserialize)]
struct NihReporterProjectYearRow {
    project_title: Option<String>,
    #[serde(default)]
    principal_investigators: Vec<NihReporterPrincipalInvestigator>,
    contact_pi_name: Option<String>,
    organization: Option<NihReporterOrganization>,
    fiscal_year: Option<i32>,
    award_amount: Option<u64>,
    project_num: Option<String>,
    core_project_num: Option<String>,
    project_detail_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct NihReporterPrincipalInvestigator {
    full_name: Option<String>,
    first_name: Option<String>,
    middle_name: Option<String>,
    last_name: Option<String>,
    is_contact_pi: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
struct NihReporterOrganization {
    org_name: Option<String>,
}

fn build_search_request(query: &str, today: Date) -> NihReporterSearchRequest {
    NihReporterSearchRequest {
        criteria: NihReporterCriteria {
            advanced_text_search: NihReporterAdvancedTextSearch {
                operator: "and",
                search_field: NIH_REPORTER_SEARCH_FIELDS,
                search_text: exact_phrase_search_text(query),
            },
            fiscal_years: recent_nih_fiscal_years(today),
        },
        include_fields: NIH_REPORTER_INCLUDE_FIELDS.to_vec(),
        offset: 0,
        limit: NIH_REPORTER_MAX_RESULTS,
        sort_field: "award_amount",
        sort_order: "desc",
    }
}

fn normalize_query(query: &str) -> Result<String, BioMcpError> {
    let query = query.trim();
    if query.is_empty() {
        return Err(BioMcpError::InvalidArgument(
            "NIH Reporter funding query is required".into(),
        ));
    }
    Ok(query.to_string())
}

fn exact_phrase_search_text(query: &str) -> String {
    let escaped = query.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn recent_nih_fiscal_years(today: Date) -> Vec<i32> {
    let current = current_nih_fiscal_year(today);
    ((current - (NIH_REPORTER_FISCAL_YEAR_WINDOW - 1))..=current).collect()
}

fn current_nih_fiscal_year(today: Date) -> i32 {
    if today.month() >= Month::October {
        today.year() + 1
    } else {
        today.year()
    }
}

fn map_project_year_row(row: NihReporterProjectYearRow) -> Option<NihReporterGrant> {
    let project_title = clean_required(row.project_title)?;
    let project_num = clean_required(row.project_num)?;
    let fiscal_year = row.fiscal_year?;
    let award_amount = row.award_amount.unwrap_or(0);

    Some(NihReporterGrant {
        project_title,
        project_num,
        core_project_num: clean_optional(row.core_project_num),
        project_detail_url: clean_optional(row.project_detail_url),
        pi_name: select_pi_name(row.contact_pi_name, &row.principal_investigators),
        organization: row
            .organization
            .and_then(|org| clean_optional(org.org_name)),
        fiscal_year,
        award_amount,
    })
}

fn select_pi_name(
    contact_pi_name: Option<String>,
    investigators: &[NihReporterPrincipalInvestigator],
) -> Option<String> {
    clean_optional(contact_pi_name)
        .or_else(|| {
            investigators
                .iter()
                .find(|pi| pi.is_contact_pi.unwrap_or(false))
                .and_then(clean_investigator_name)
        })
        .or_else(|| investigators.first().and_then(clean_investigator_name))
}

fn clean_investigator_name(pi: &NihReporterPrincipalInvestigator) -> Option<String> {
    clean_optional(pi.full_name.clone()).or_else(|| {
        let parts = [
            clean_optional(pi.first_name.clone()),
            clean_optional(pi.middle_name.clone()),
            clean_optional(pi.last_name.clone()),
        ];
        let joined = parts.into_iter().flatten().collect::<Vec<_>>().join(" ");
        clean_optional(Some(joined))
    })
}

fn clean_required(value: Option<String>) -> Option<String> {
    clean_optional(value)
}

fn clean_optional(value: Option<String>) -> Option<String> {
    let value = value?;
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn deduplicate_grants(grants: Vec<NihReporterGrant>) -> Vec<NihReporterGrant> {
    let mut best_by_project: HashMap<String, NihReporterGrant> = HashMap::new();

    for grant in grants {
        let key = grant
            .core_project_num
            .clone()
            .unwrap_or_else(|| grant.project_num.clone());
        match best_by_project.get_mut(&key) {
            Some(existing) if grant_sort_cmp(&grant, existing).is_lt() => *existing = grant,
            Some(_) => {}
            None => {
                best_by_project.insert(key, grant);
            }
        }
    }

    let mut deduped = best_by_project.into_values().collect::<Vec<_>>();
    deduped.sort_by(grant_sort_cmp);
    deduped.truncate(NIH_REPORTER_DISPLAY_LIMIT);
    deduped
}

fn grant_sort_cmp(left: &NihReporterGrant, right: &NihReporterGrant) -> Ordering {
    right
        .award_amount
        .cmp(&left.award_amount)
        .then_with(|| right.fiscal_year.cmp(&left.fiscal_year))
        .then_with(|| left.project_num.cmp(&right.project_num))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_date(year: i32, month: Month, day: u8) -> Date {
        Date::from_calendar_date(year, month, day).expect("valid test date")
    }

    fn test_grant(
        project_num: &str,
        core_project_num: Option<&str>,
        fiscal_year: i32,
        award_amount: u64,
    ) -> NihReporterGrant {
        NihReporterGrant {
            project_title: format!("Project {project_num}"),
            project_num: project_num.to_string(),
            core_project_num: core_project_num.map(str::to_string),
            project_detail_url: Some(format!("https://example.org/{project_num}")),
            pi_name: Some("Example PI".to_string()),
            organization: Some("Example Org".to_string()),
            fiscal_year,
            award_amount,
        }
    }

    #[test]
    fn build_search_request_uses_approved_request_shape() {
        let request = build_search_request("Marfan syndrome", test_date(2026, Month::April, 11));
        let body = serde_json::to_value(&request).expect("request JSON");

        assert!(body["criteria"].get("project_terms").is_none());
        assert_eq!(body["criteria"]["advanced_text_search"]["operator"], "and");
        assert_eq!(
            body["criteria"]["advanced_text_search"]["search_field"],
            NIH_REPORTER_SEARCH_FIELDS
        );
        assert_eq!(
            body["criteria"]["advanced_text_search"]["search_text"],
            "\"Marfan syndrome\""
        );
        assert_eq!(
            body["criteria"]["fiscal_years"],
            json!([2022, 2023, 2024, 2025, 2026])
        );
        assert_eq!(body["include_fields"], json!(NIH_REPORTER_INCLUDE_FIELDS));
        assert_eq!(body["offset"], 0);
        assert_eq!(body["limit"], NIH_REPORTER_MAX_RESULTS);
        assert_eq!(body["sort_field"], "award_amount");
        assert_eq!(body["sort_order"], "desc");
    }

    #[test]
    fn exact_phrase_search_text_escapes_quotes_and_backslashes() {
        assert_eq!(
            exact_phrase_search_text("BCR\\ABL \"fusion\""),
            "\"BCR\\\\ABL \\\"fusion\\\"\""
        );
    }

    #[test]
    fn recent_nih_fiscal_years_roll_over_on_october_boundary() {
        assert_eq!(
            recent_nih_fiscal_years(test_date(2026, Month::September, 30)),
            vec![2022, 2023, 2024, 2025, 2026]
        );
        assert_eq!(
            recent_nih_fiscal_years(test_date(2026, Month::October, 1)),
            vec![2023, 2024, 2025, 2026, 2027]
        );
    }

    #[test]
    fn map_project_year_row_prefers_contact_pi_then_contact_investigator_then_first_pi() {
        let contact_name_row = NihReporterProjectYearRow {
            project_title: Some("Example".to_string()),
            principal_investigators: vec![NihReporterPrincipalInvestigator {
                full_name: Some("Ignored PI".to_string()),
                first_name: None,
                middle_name: None,
                last_name: None,
                is_contact_pi: Some(true),
            }],
            contact_pi_name: Some("  DOE, JANE  ".to_string()),
            organization: None,
            fiscal_year: Some(2026),
            award_amount: Some(1),
            project_num: Some("P1".to_string()),
            core_project_num: None,
            project_detail_url: None,
        };
        assert_eq!(
            map_project_year_row(contact_name_row)
                .and_then(|grant| grant.pi_name)
                .as_deref(),
            Some("DOE, JANE")
        );

        let contact_investigator_row = NihReporterProjectYearRow {
            project_title: Some("Example".to_string()),
            principal_investigators: vec![
                NihReporterPrincipalInvestigator {
                    full_name: Some("Other PI".to_string()),
                    first_name: None,
                    middle_name: None,
                    last_name: None,
                    is_contact_pi: Some(false),
                },
                NihReporterPrincipalInvestigator {
                    full_name: Some("Contact PI".to_string()),
                    first_name: None,
                    middle_name: None,
                    last_name: None,
                    is_contact_pi: Some(true),
                },
            ],
            contact_pi_name: None,
            organization: None,
            fiscal_year: Some(2026),
            award_amount: Some(1),
            project_num: Some("P2".to_string()),
            core_project_num: None,
            project_detail_url: None,
        };
        assert_eq!(
            map_project_year_row(contact_investigator_row)
                .and_then(|grant| grant.pi_name)
                .as_deref(),
            Some("Contact PI")
        );

        let first_investigator_row = NihReporterProjectYearRow {
            project_title: Some("Example".to_string()),
            principal_investigators: vec![NihReporterPrincipalInvestigator {
                full_name: None,
                first_name: Some("Ada".to_string()),
                middle_name: Some("M".to_string()),
                last_name: Some("Lovelace".to_string()),
                is_contact_pi: None,
            }],
            contact_pi_name: None,
            organization: None,
            fiscal_year: Some(2026),
            award_amount: Some(1),
            project_num: Some("P3".to_string()),
            core_project_num: None,
            project_detail_url: None,
        };
        assert_eq!(
            map_project_year_row(first_investigator_row)
                .and_then(|grant| grant.pi_name)
                .as_deref(),
            Some("Ada M Lovelace")
        );
    }

    #[test]
    fn deduplicate_grants_groups_by_core_project_num_then_project_num() {
        let deduped = deduplicate_grants(vec![
            test_grant("P-002", Some("CORE-A"), 2025, 250),
            test_grant("P-001", Some("CORE-A"), 2026, 250),
            test_grant("P-100", None, 2024, 180),
            test_grant("P-100", None, 2023, 400),
            test_grant("P-200", Some("CORE-B"), 2025, 300),
        ]);

        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0].project_num, "P-100");
        assert_eq!(deduped[0].award_amount, 400);
        assert_eq!(deduped[1].project_num, "P-200");
        assert_eq!(deduped[2].project_num, "P-001");
    }

    #[test]
    fn deduplicate_grants_truncates_to_top_ten_after_sorting() {
        let grants = (0..12)
            .map(|idx| test_grant(&format!("P-{idx:03}"), None, 2026, 1_000 - idx))
            .collect::<Vec<_>>();

        let deduped = deduplicate_grants(grants);

        assert_eq!(deduped.len(), 10);
        assert_eq!(deduped[0].project_num, "P-000");
        assert_eq!(deduped[9].project_num, "P-009");
    }

    #[tokio::test]
    async fn funding_posts_to_projects_search_and_maps_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/projects/search"))
            .and(|request: &wiremock::Request| {
                let body: serde_json::Value =
                    serde_json::from_slice(&request.body).expect("request body JSON");
                body["criteria"]["advanced_text_search"]["search_text"] == "\"ERBB2\""
                    && body["criteria"]["advanced_text_search"]["search_field"]
                        == NIH_REPORTER_SEARCH_FIELDS
                    && body["criteria"]["fiscal_years"] == json!([2022, 2023, 2024, 2025, 2026])
                    && body["include_fields"] == json!(NIH_REPORTER_INCLUDE_FIELDS)
                    && body["offset"] == 0
                    && body["limit"] == NIH_REPORTER_MAX_RESULTS
                    && body["sort_field"] == "award_amount"
                    && body["sort_order"] == "desc"
            })
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "meta": {
                    "total": 4
                },
                "results": [
                    {
                        "project_title": "Older lower award duplicate",
                        "principal_investigators": [
                            {"full_name": "Other PI", "is_contact_pi": true}
                        ],
                        "contact_pi_name": "",
                        "organization": {"org_name": "Org B"},
                        "fiscal_year": 2024,
                        "award_amount": 100,
                        "project_num": "P-2",
                        "core_project_num": "CORE-1",
                        "project_detail_url": "https://reporter.nih.gov/project-details/2"
                    },
                    {
                        "project_title": "Winning duplicate",
                        "principal_investigators": [
                            {"full_name": "Ignored PI", "is_contact_pi": true}
                        ],
                        "contact_pi_name": "DOE, JANE",
                        "organization": {"org_name": "Org A"},
                        "fiscal_year": 2025,
                        "award_amount": 500,
                        "project_num": "P-1",
                        "core_project_num": "CORE-1",
                        "project_detail_url": "https://reporter.nih.gov/project-details/1"
                    },
                    {
                        "project_title": "Fallback PI project",
                        "principal_investigators": [
                            {"first_name": "Ada", "middle_name": "", "last_name": "Lovelace", "is_contact_pi": false}
                        ],
                        "contact_pi_name": null,
                        "organization": {"org_name": "Org C"},
                        "fiscal_year": 2026,
                        "award_amount": 250,
                        "project_num": "P-3",
                        "core_project_num": null,
                        "project_detail_url": null
                    }
                ]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = NihReporterClient::new_for_test(server.uri()).expect("client");
        let section = client.funding("ERBB2").await.expect("funding section");

        assert_eq!(section.query, "ERBB2");
        assert_eq!(section.fiscal_years, vec![2022, 2023, 2024, 2025, 2026]);
        assert_eq!(section.matching_project_years, 4);
        assert_eq!(section.grants.len(), 2);
        assert_eq!(section.grants[0].project_num, "P-1");
        assert_eq!(section.grants[0].pi_name.as_deref(), Some("DOE, JANE"));
        assert_eq!(section.grants[0].organization.as_deref(), Some("Org A"));
        assert_eq!(section.grants[1].project_num, "P-3");
        assert_eq!(section.grants[1].pi_name.as_deref(), Some("Ada Lovelace"));
    }
}
