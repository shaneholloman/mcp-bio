use std::collections::{BTreeMap, HashSet};
use std::sync::OnceLock;

use futures::future::join_all;
use regex::Regex;
use serde::Deserialize;
use tracing::warn;

use super::{Disease, DiseaseClinicalFeature};
use crate::error::BioMcpError;
use crate::sources::medlineplus::{MedlinePlusClient, MedlinePlusTopic};

const MEDLINEPLUS_SOURCE: &str = "MedlinePlus";
const CLINICAL_SUMMARY_TIER: &str = "clinical_summary";

#[derive(Debug, Clone, Deserialize)]
struct ClinicalFeatureConfig {
    key: String,
    label: String,
    biomcp_query: String,
    identifiers: BTreeMap<String, String>,
    body_system: Option<String>,
    source_queries: Vec<String>,
    expected_symptoms: Vec<ExpectedSymptom>,
}

#[derive(Debug, Clone, Deserialize)]
struct ExpectedSymptom {
    label: String,
    patterns: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct OfflineTopics {
    topics: Vec<OfflineTopic>,
}

#[derive(Debug, Deserialize)]
struct OfflineTopic {
    title: String,
    url: String,
    summary_excerpt: String,
}

#[derive(Debug, Clone)]
struct ScoredTopic {
    topic: MedlinePlusTopic,
    selection_score: f64,
    selection_relation: &'static str,
    #[allow(dead_code)]
    selection_reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
struct HpoEntry {
    id: &'static str,
    label: &'static str,
    confidence: f64,
    method: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
struct MappingResult {
    normalized_hpo_id: Option<String>,
    normalized_hpo_label: Option<String>,
    mapping_confidence: f64,
    mapping_method: String,
}

const EXTRA_EXTRACTION_PATTERNS: &[(&str, &[&str])] = &[
    ("abdominal pain", &["lower abdomen"]),
    ("urinary frequency", &["urinating peeing often"]),
];

const HPO_MAPPING: &[(&str, HpoEntry)] = &[
    (
        "heavy menstrual bleeding",
        HpoEntry {
            id: "HP:0000132",
            label: "Menorrhagia",
            confidence: 0.86,
            method: "reviewed_fixture_exact_or_synonym",
        },
    ),
    (
        "pelvic pain",
        HpoEntry {
            id: "HP:0034267",
            label: "Pelvic pain",
            confidence: 0.95,
            method: "reviewed_fixture_exact",
        },
    ),
    (
        "lower back pain",
        HpoEntry {
            id: "HP:0003419",
            label: "Low back pain",
            confidence: 0.92,
            method: "reviewed_fixture_exact_or_synonym",
        },
    ),
    (
        "fatigue",
        HpoEntry {
            id: "HP:0012378",
            label: "Fatigue",
            confidence: 0.95,
            method: "reviewed_fixture_exact",
        },
    ),
    (
        "urinary frequency",
        HpoEntry {
            id: "HP:0100515",
            label: "Pollakisuria",
            confidence: 0.78,
            method: "reviewed_fixture_synonym",
        },
    ),
    (
        "constipation",
        HpoEntry {
            id: "HP:0002019",
            label: "Constipation",
            confidence: 0.95,
            method: "reviewed_fixture_exact",
        },
    ),
    (
        "infertility",
        HpoEntry {
            id: "HP:0000789",
            label: "Infertility",
            confidence: 0.90,
            method: "reviewed_fixture_exact",
        },
    ),
    (
        "dyspareunia",
        HpoEntry {
            id: "HP:0030016",
            label: "Dyspareunia",
            confidence: 0.95,
            method: "reviewed_fixture_exact_or_synonym",
        },
    ),
    (
        "dysmenorrhea",
        HpoEntry {
            id: "HP:0100607",
            label: "Dysmenorrhea",
            confidence: 0.95,
            method: "reviewed_fixture_exact_or_synonym",
        },
    ),
    (
        "dyschezia",
        HpoEntry {
            id: "HP:6000222",
            label: "Painful defecation",
            confidence: 0.78,
            method: "reviewed_fixture_synonym",
        },
    ),
    (
        "dysuria",
        HpoEntry {
            id: "HP:0100518",
            label: "Dysuria",
            confidence: 0.90,
            method: "reviewed_fixture_exact_or_synonym",
        },
    ),
    (
        "abdominal pain",
        HpoEntry {
            id: "HP:0002027",
            label: "Abdominal pain",
            confidence: 0.95,
            method: "reviewed_fixture_exact",
        },
    ),
    (
        "leg swelling",
        HpoEntry {
            id: "HP:0010741",
            label: "Pedal edema",
            confidence: 0.68,
            method: "reviewed_fixture_broader_lower_limb_edema",
        },
    ),
    (
        "leg pain",
        HpoEntry {
            id: "HP:0012514",
            label: "Lower limb pain",
            confidence: 0.86,
            method: "reviewed_fixture_synonym",
        },
    ),
    (
        "varicose veins",
        HpoEntry {
            id: "HP:0002619",
            label: "Varicose veins",
            confidence: 0.95,
            method: "reviewed_fixture_exact",
        },
    ),
    (
        "venous ulcer",
        HpoEntry {
            id: "HP:0200042",
            label: "Skin ulcer",
            confidence: 0.62,
            method: "reviewed_fixture_broader_skin_ulcer",
        },
    ),
    (
        "skin discoloration",
        HpoEntry {
            id: "HP:0000953",
            label: "Hyperpigmentation of the skin",
            confidence: 0.62,
            method: "reviewed_fixture_broader_discoloration",
        },
    ),
    (
        "stasis dermatitis",
        HpoEntry {
            id: "HP:0033564",
            label: "Stasis dermatitis",
            confidence: 0.95,
            method: "reviewed_fixture_exact",
        },
    ),
    (
        "itching",
        HpoEntry {
            id: "HP:0000989",
            label: "Pruritus",
            confidence: 0.90,
            method: "reviewed_fixture_synonym",
        },
    ),
];

fn clinical_feature_configs() -> &'static [ClinicalFeatureConfig] {
    static CONFIGS: OnceLock<Vec<ClinicalFeatureConfig>> = OnceLock::new();
    CONFIGS
        .get_or_init(|| {
            serde_json::from_str(include_str!("fixtures/clinical_features_config.json"))
                .expect("valid clinical feature config fixture")
        })
        .as_slice()
}

fn non_alnum_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[^a-z0-9]+").expect("valid regex"))
}

fn evidence_token_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[A-Za-z0-9]+").expect("valid regex"))
}

fn normalize_text(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    non_alnum_re()
        .replace_all(&lower, " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn slugify(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    non_alnum_re().replace_all(&lower, "").to_string()
}

fn source_native_id(url: &str) -> String {
    let value = url
        .trim()
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default();
    value.split('.').next().unwrap_or_default().to_string()
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn floor_char_boundary(value: &str, mut index: usize) -> usize {
    index = index.min(value.len());
    while index > 0 && !value.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn ceil_char_boundary(value: &str, mut index: usize) -> usize {
    index = index.min(value.len());
    while index < value.len() && !value.is_char_boundary(index) {
        index += 1;
    }
    index
}

fn compact_evidence(text: &str, pattern: &str, radius: usize) -> String {
    let tokens = evidence_token_re()
        .find_iter(pattern)
        .map(|token| regex::escape(token.as_str()))
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        return collapse_whitespace(&text.chars().take(radius * 2).collect::<String>());
    }

    let normalized_pattern = tokens.join(r"\W+");
    let re = Regex::new(&format!("(?i){normalized_pattern}")).expect("valid evidence regex");
    let Some(found) = re.find(text) else {
        return collapse_whitespace(&text.chars().take(radius * 2).collect::<String>());
    };

    let start = floor_char_boundary(text, found.start().saturating_sub(radius));
    let end = ceil_char_boundary(text, (found.end() + radius).min(text.len()));
    let mut evidence = collapse_whitespace(&text[start..end]);
    if start > 0 {
        evidence.insert_str(0, "...");
    }
    if end < text.len() {
        evidence.push_str("...");
    }
    evidence
}

fn config_match_terms(config: &ClinicalFeatureConfig) -> Vec<String> {
    vec![
        normalize_text(&config.biomcp_query),
        normalize_text(&config.label),
        normalize_text(&config.key.replace('_', " ")),
    ]
}

fn text_matches_config(value: &str, config: &ClinicalFeatureConfig) -> bool {
    let value = normalize_text(value);
    if value.is_empty() {
        return false;
    }
    config_match_terms(config)
        .iter()
        .any(|term| &value == term || (!term.is_empty() && value.contains(term)))
}

fn identifier_forms(value: &str) -> Vec<String> {
    let value = value.trim();
    if value.is_empty() {
        return Vec::new();
    }
    let mut out = vec![value.to_ascii_uppercase()];
    if let Some((_, stripped)) = value.rsplit_once(':') {
        let stripped = stripped.trim();
        if !stripped.is_empty() {
            let stripped = stripped.to_ascii_uppercase();
            if !out.iter().any(|form| form == &stripped) {
                out.push(stripped);
            }
        }
    }
    out
}

fn identifier_matches_config(value: &str, config: &ClinicalFeatureConfig) -> bool {
    let config_ids = config
        .identifiers
        .values()
        .flat_map(|value| identifier_forms(value))
        .collect::<HashSet<_>>();
    identifier_forms(value)
        .iter()
        .any(|form| config_ids.contains(form))
}

fn clinical_feature_config_for(
    disease: &Disease,
    requested_lookup: Option<&str>,
) -> Option<ClinicalFeatureConfig> {
    for config in clinical_feature_configs() {
        if requested_lookup
            .map(|value| text_matches_config(value, config))
            .unwrap_or(false)
            || text_matches_config(&disease.name, config)
            || disease
                .synonyms
                .iter()
                .any(|synonym| text_matches_config(synonym, config))
        {
            return Some(config.clone());
        }

        if requested_lookup
            .map(|value| identifier_matches_config(value, config))
            .unwrap_or(false)
            || identifier_matches_config(&disease.id, config)
            || disease
                .xrefs
                .values()
                .any(|xref| identifier_matches_config(xref, config))
        {
            return Some(config.clone());
        }
    }
    None
}

fn load_offline_topics(key: &str) -> Result<Vec<MedlinePlusTopic>, BioMcpError> {
    let raw = match key {
        "uterine_fibroid" => include_str!("fixtures/medlineplus/uterine_fibroid_topics.json"),
        "endometriosis" => include_str!("fixtures/medlineplus/endometriosis_topics.json"),
        "chronic_venous_insufficiency" => {
            include_str!("fixtures/medlineplus/chronic_venous_insufficiency_topics.json")
        }
        _ => return Ok(Vec::new()),
    };
    let fixture: OfflineTopics = serde_json::from_str(raw)?;
    Ok(fixture
        .topics
        .into_iter()
        .map(|topic| MedlinePlusTopic {
            title: topic.title,
            url: topic.url,
            summary_excerpt: topic.summary_excerpt,
        })
        .collect())
}

async fn load_topics_for_disease(
    config: &ClinicalFeatureConfig,
    client: &MedlinePlusClient,
) -> Result<Vec<MedlinePlusTopic>, BioMcpError> {
    let results = join_all(
        config
            .source_queries
            .iter()
            .map(|query| client.search_n(query, 5)),
    )
    .await;

    let mut topics = Vec::new();
    let mut seen_urls = HashSet::new();
    for result in results {
        let Ok(rows) = result else {
            continue;
        };
        for topic in rows {
            if topic.url.trim().is_empty() || !seen_urls.insert(topic.url.clone()) {
                continue;
            }
            topics.push(topic);
        }
    }

    if topics.is_empty() {
        return load_offline_topics(config.key.as_str());
    }
    Ok(topics)
}

fn direct_title_queries(config: &ClinicalFeatureConfig) -> HashSet<String> {
    let mut queries = config.source_queries.clone();
    queries.push(config.label.clone());
    let normalized = queries
        .iter()
        .map(|query| normalize_text(query))
        .collect::<HashSet<_>>();
    let mut variants = normalized.clone();
    for query in normalized {
        if query.ends_with('s') {
            variants.insert(query.trim_end_matches('s').to_string());
        } else {
            variants.insert(format!("{query}s"));
        }
    }
    variants
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn topic_score(
    topic: &MedlinePlusTopic,
    config: &ClinicalFeatureConfig,
) -> (f64, &'static str, Vec<String>) {
    let title_norm = normalize_text(&topic.title);
    let summary_norm = normalize_text(&topic.summary_excerpt);
    let slug = slugify(&topic.url);
    let direct_queries = direct_title_queries(config);
    let query_slugs = config
        .source_queries
        .iter()
        .chain(std::iter::once(&config.label))
        .map(|query| slugify(query))
        .collect::<HashSet<_>>();

    let mut score = 0.0;
    let mut reasons = Vec::new();
    if direct_queries.contains(&title_norm) {
        score += 100.0;
        reasons.push("exact_title".to_string());
    }
    if query_slugs.contains(&source_native_id(&topic.url)) || query_slugs.contains(&slug) {
        score += 80.0;
        reasons.push("exact_url_slug".to_string());
    }

    let label_tokens = normalize_text(&config.source_queries.join(" "))
        .split_whitespace()
        .filter(|token| token.len() > 3)
        .map(str::to_string)
        .collect::<HashSet<_>>();
    let title_tokens = title_norm
        .split_whitespace()
        .map(str::to_string)
        .collect::<HashSet<_>>();
    if !label_tokens.is_empty() {
        let overlap =
            label_tokens.intersection(&title_tokens).count() as f64 / label_tokens.len() as f64;
        score += round2(overlap * 35.0);
        if overlap > 0.0 {
            reasons.push(format!("title_token_overlap:{overlap:.2}"));
        }
    }

    if config
        .source_queries
        .iter()
        .any(|query| summary_norm.contains(&normalize_text(query)))
    {
        score += 15.0;
        reasons.push("query_in_summary".to_string());
    }

    let score = round2(score);
    let relation = if score >= 90.0 { "direct" } else { "related" };
    (score, relation, reasons)
}

fn compare_scored_topics(left: &ScoredTopic, right: &ScoredTopic) -> std::cmp::Ordering {
    right
        .selection_score
        .partial_cmp(&left.selection_score)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| left.topic.title.cmp(&right.topic.title))
}

fn select_topics(config: &ClinicalFeatureConfig, topics: &[MedlinePlusTopic]) -> Vec<ScoredTopic> {
    let mut scored = topics
        .iter()
        .map(|topic| {
            let (selection_score, selection_relation, selection_reasons) =
                topic_score(topic, config);
            ScoredTopic {
                topic: topic.clone(),
                selection_score,
                selection_relation,
                selection_reasons,
            }
        })
        .collect::<Vec<_>>();
    scored.sort_by(compare_scored_topics);

    let direct = scored
        .iter()
        .filter(|topic| topic.selection_relation == "direct")
        .cloned()
        .collect::<Vec<_>>();
    if !direct.is_empty() {
        return direct;
    }
    scored.into_iter().take(3).collect()
}

fn first_matching_pattern(text: &str, patterns: &[String]) -> Option<String> {
    let text_norm = normalize_text(text);
    patterns.iter().find_map(|pattern| {
        let pattern_norm = normalize_text(pattern);
        if pattern_norm.is_empty() {
            return None;
        }
        text_norm
            .contains(&pattern_norm)
            .then(|| pattern.to_string())
    })
}

fn extra_patterns_for(label: &str) -> Vec<String> {
    EXTRA_EXTRACTION_PATTERNS
        .iter()
        .find(|(key, _)| *key == label)
        .map(|(_, patterns)| {
            patterns
                .iter()
                .map(|pattern| (*pattern).to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn map_feature(label: &str) -> MappingResult {
    let label = normalize_text(label);
    for (candidate, entry) in HPO_MAPPING {
        if normalize_text(candidate) == label {
            return MappingResult {
                normalized_hpo_id: Some(entry.id.to_string()),
                normalized_hpo_label: Some(entry.label.to_string()),
                mapping_confidence: entry.confidence,
                mapping_method: entry.method.to_string(),
            };
        }
    }
    MappingResult {
        normalized_hpo_id: None,
        normalized_hpo_label: None,
        mapping_confidence: 0.0,
        mapping_method: "unmapped".to_string(),
    }
}

fn extract_features(
    config: &ClinicalFeatureConfig,
    selected_topics: &[ScoredTopic],
) -> Vec<DiseaseClinicalFeature> {
    let topic_texts = selected_topics
        .iter()
        .map(|topic| {
            (
                topic,
                format!("{} {}", topic.topic.title, topic.topic.summary_excerpt)
                    .trim()
                    .to_string(),
            )
        })
        .collect::<Vec<_>>();
    let mut features = Vec::new();
    let mut rank = 1_u16;

    for expected in &config.expected_symptoms {
        let mut patterns = expected.patterns.clone();
        patterns.extend(extra_patterns_for(&expected.label));
        let mut best: Option<&ScoredTopic> = None;
        let mut best_pattern: Option<String> = None;
        let mut best_text = "";

        for (topic, text) in &topic_texts {
            let Some(pattern) = first_matching_pattern(text, &patterns) else {
                continue;
            };
            if best
                .map(|candidate| topic.selection_score > candidate.selection_score)
                .unwrap_or(true)
            {
                best = Some(topic);
                best_pattern = Some(pattern);
                best_text = text;
            }
        }

        let (Some(topic), Some(pattern)) = (best, best_pattern) else {
            continue;
        };
        let mapping = map_feature(&expected.label);
        features.push(DiseaseClinicalFeature {
            rank,
            label: expected.label.clone(),
            feature_type: "symptom".to_string(),
            source: MEDLINEPLUS_SOURCE.to_string(),
            source_url: Some(topic.topic.url.clone()),
            source_native_id: source_native_id(&topic.topic.url),
            evidence_tier: CLINICAL_SUMMARY_TIER.to_string(),
            evidence_text: compact_evidence(best_text, &pattern, 150),
            evidence_match: pattern,
            body_system: config.body_system.clone(),
            topic_title: Some(topic.topic.title.clone()),
            topic_relation: Some(topic.selection_relation.to_string()),
            topic_selection_score: Some(topic.selection_score),
            normalized_hpo_id: mapping.normalized_hpo_id,
            normalized_hpo_label: mapping.normalized_hpo_label,
            mapping_confidence: mapping.mapping_confidence,
            mapping_method: mapping.mapping_method,
        });
        rank += 1;
    }

    features
}

pub(super) async fn add_clinical_features_section(
    disease: &mut Disease,
    requested_lookup: Option<&str>,
) -> Result<(), BioMcpError> {
    let Some(config) = clinical_feature_config_for(disease, requested_lookup) else {
        return Ok(());
    };

    let topics = match MedlinePlusClient::new() {
        Ok(client) => load_topics_for_disease(&config, &client).await?,
        Err(err) => {
            warn!("MedlinePlus client unavailable for disease clinical features: {err}");
            load_offline_topics(config.key.as_str())?
        }
    };
    let selected_topics = select_topics(&config, &topics);
    disease.clinical_features = extract_features(&config, &selected_topics);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::entities::disease::test_support::test_disease;

    fn config(key: &str) -> ClinicalFeatureConfig {
        clinical_feature_configs()
            .iter()
            .find(|config| config.key == key)
            .expect("configured disease")
            .clone()
    }

    fn topic(title: &str, url: &str, summary: &str) -> MedlinePlusTopic {
        MedlinePlusTopic {
            title: title.to_string(),
            url: url.to_string(),
            summary_excerpt: summary.to_string(),
        }
    }

    fn topic_response(url: &str, title: &str, summary: &str) -> ResponseTemplate {
        ResponseTemplate::new(200)
            .insert_header("content-type", "application/xml")
            .set_body_string(format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<nlmSearchResult>
  <list num="1" start="0" per="1">
    <document rank="0" url="{url}">
      <content name="title">{title}</content>
      <content name="FullSummary">{summary}</content>
    </document>
  </list>
</nlmSearchResult>"#
            ))
    }

    fn empty_response() -> ResponseTemplate {
        ResponseTemplate::new(200)
            .insert_header("content-type", "application/xml")
            .set_body_string(
                r#"<?xml version="1.0" encoding="UTF-8"?><nlmSearchResult><list /></nlmSearchResult>"#,
            )
    }

    async fn mount_empty_queries(server: &MockServer, config: &ClinicalFeatureConfig) {
        for query in &config.source_queries {
            Mock::given(method("GET"))
                .and(path("/ws/query"))
                .and(query_param("db", "healthTopics"))
                .and(query_param("term", query.as_str()))
                .and(query_param("retmax", "5"))
                .respond_with(empty_response())
                .expect(1)
                .mount(server)
                .await;
        }
    }

    fn sha256_hex(value: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(value.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn round3(value: f64) -> f64 {
        (value * 1000.0).round() / 1000.0
    }

    fn feature_checksum(features: &[DiseaseClinicalFeature]) -> String {
        let rows = features
            .iter()
            .map(|feature| {
                let mut row = BTreeMap::new();
                row.insert("hpo".to_string(), json!(feature.normalized_hpo_id));
                row.insert("label".to_string(), json!(feature.label));
                row.insert("source".to_string(), json!(feature.source_native_id));
                row
            })
            .collect::<Vec<BTreeMap<String, Value>>>();
        sha256_hex(&serde_json::to_string(&rows).expect("feature checksum JSON"))
    }

    fn features_for_config(config: &ClinicalFeatureConfig) -> Vec<DiseaseClinicalFeature> {
        let topics = load_offline_topics(&config.key).expect("offline topics");
        let selected = select_topics(config, &topics);
        extract_features(config, &selected)
    }

    #[test]
    fn normalize_text_matches_spike_contract() {
        assert_eq!(
            normalize_text("Uterine Fibroids: Heavy/Painful periods"),
            "uterine fibroids heavy painful periods"
        );
    }

    #[test]
    fn slugify_matches_spike_contract() {
        assert_eq!(
            slugify("https://medlineplus.gov/uterine-fibroids.html"),
            "httpsmedlineplusgovuterinefibroidshtml"
        );
    }

    #[test]
    fn source_native_id_strips_path_and_extension() {
        assert_eq!(
            source_native_id("https://medlineplus.gov/uterinefibroids.html/"),
            "uterinefibroids"
        );
    }

    #[test]
    fn compact_evidence_handles_unicode_no_match() {
        let text = "αβγ uterine summary with unicode boundaries";
        assert_eq!(compact_evidence(text, "not present", 4), "αβγ uter");
    }

    #[test]
    fn clinical_feature_config_for_matches_request_name_identifier_and_unsupported() {
        let mut disease = test_disease("D007889", "Uterine Fibroids");
        disease.synonyms.push("uterine leiomyoma".to_string());
        disease
            .xrefs
            .insert("MESH".to_string(), "MESH:D007889".to_string());

        assert_eq!(
            clinical_feature_config_for(&disease, Some("uterine leiomyoma"))
                .expect("request lookup match")
                .key,
            "uterine_fibroid"
        );
        assert_eq!(
            clinical_feature_config_for(&disease, None)
                .expect("plural disease name match")
                .key,
            "uterine_fibroid"
        );

        let disease = test_disease("MESH:D007889", "unmapped name");
        assert_eq!(
            clinical_feature_config_for(&disease, None)
                .expect("identifier match")
                .key,
            "uterine_fibroid"
        );

        let disease = test_disease("MONDO:0005105", "melanoma");
        assert!(clinical_feature_config_for(&disease, Some("melanoma")).is_none());
    }

    #[tokio::test]
    async fn load_topics_runs_all_queries_with_retmax_five_and_deduplicates_urls() {
        let server = MockServer::start().await;
        let config = config("uterine_fibroid");
        let urls = [
            "https://medlineplus.gov/uterinefibroids.html",
            "https://medlineplus.gov/uterinefibroids.html",
            "https://medlineplus.gov/leiomyoma.html",
        ];
        for (query, url) in config.source_queries.iter().zip(urls) {
            Mock::given(method("GET"))
                .and(path("/ws/query"))
                .and(query_param("db", "healthTopics"))
                .and(query_param("term", query.as_str()))
                .and(query_param("retmax", "5"))
                .respond_with(topic_response(url, query, "summary"))
                .expect(1)
                .mount(&server)
                .await;
        }

        let client = MedlinePlusClient::new_for_test(server.uri()).expect("client");
        let topics = load_topics_for_disease(&config, &client)
            .await
            .expect("topics");

        assert_eq!(
            topics
                .iter()
                .map(|topic| topic.url.as_str())
                .collect::<Vec<_>>(),
            vec![
                "https://medlineplus.gov/uterinefibroids.html",
                "https://medlineplus.gov/leiomyoma.html"
            ]
        );
    }

    #[tokio::test]
    async fn load_topics_uses_embedded_fixture_when_no_live_topics() {
        let server = MockServer::start().await;
        let config = config("endometriosis");
        mount_empty_queries(&server, &config).await;

        let client = MedlinePlusClient::new_for_test(server.uri()).expect("client");
        let topics = load_topics_for_disease(&config, &client)
            .await
            .expect("topics");

        assert_eq!(topics[0].title, "Endometriosis");
    }

    #[tokio::test]
    async fn load_topics_uses_embedded_fixture_when_live_queries_fail() {
        let server = MockServer::start().await;
        let config = config("chronic_venous_insufficiency");
        for query in &config.source_queries {
            Mock::given(method("GET"))
                .and(path("/ws/query"))
                .and(query_param("db", "healthTopics"))
                .and(query_param("term", query.as_str()))
                .and(query_param("retmax", "5"))
                .respond_with(ResponseTemplate::new(500).set_body_string("failed"))
                .expect(1)
                .mount(&server)
                .await;
        }

        let client = MedlinePlusClient::new_for_test(server.uri()).expect("client");
        let topics = load_topics_for_disease(&config, &client)
            .await
            .expect("topics");

        assert_eq!(topics.len(), 4);
        assert!(topics.iter().any(|topic| topic.title == "Varicose Veins"));
    }

    #[test]
    fn select_topics_prefers_direct_pages_when_any_exist() {
        let config = config("uterine_fibroid");
        let topics = load_offline_topics("uterine_fibroid").expect("offline topics");
        let selected = select_topics(&config, &topics);

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].topic.title, "Uterine Fibroids");
        assert_eq!(selected[0].selection_score, 123.75);
        assert_eq!(selected[0].selection_relation, "direct");
        assert!(
            selected[0]
                .selection_reasons
                .contains(&"exact_title".to_string())
        );
    }

    #[test]
    fn select_topics_falls_back_to_top_three_related_when_no_direct_page() {
        let config = config("chronic_venous_insufficiency");
        let topics = load_offline_topics("chronic_venous_insufficiency").expect("offline topics");
        let selected = select_topics(&config, &topics);

        assert_eq!(
            selected
                .iter()
                .map(|topic| topic.topic.title.as_str())
                .collect::<Vec<_>>(),
            vec![
                "Deep Vein Thrombosis",
                "Leg Injuries and Disorders",
                "Varicose Veins"
            ]
        );
        assert!(
            selected
                .iter()
                .all(|topic| topic.selection_relation == "related")
        );
    }

    #[test]
    fn map_feature_returns_reviewed_hpo_for_known_concepts() {
        assert_eq!(
            map_feature("heavy menstrual bleeding"),
            MappingResult {
                normalized_hpo_id: Some("HP:0000132".to_string()),
                normalized_hpo_label: Some("Menorrhagia".to_string()),
                mapping_confidence: 0.86,
                mapping_method: "reviewed_fixture_exact_or_synonym".to_string(),
            }
        );
    }

    #[test]
    fn map_feature_returns_unmapped_for_unknown_concept() {
        assert_eq!(
            map_feature("not a reviewed feature"),
            MappingResult {
                normalized_hpo_id: None,
                normalized_hpo_label: None,
                mapping_confidence: 0.0,
                mapping_method: "unmapped".to_string(),
            }
        );
    }

    #[test]
    fn extract_features_preserves_source_native_rows_and_mapping() {
        let config = config("endometriosis");
        let topics = load_offline_topics("endometriosis").expect("offline topics");
        let selected = select_topics(&config, &topics);
        let features = extract_features(&config, &selected);

        assert_eq!(features.len(), 6);
        assert!(features.iter().any(|feature| {
            feature.label == "dysmenorrhea"
                && feature.source == MEDLINEPLUS_SOURCE
                && feature.source_url.as_deref()
                    == Some("https://medlineplus.gov/endometriosis.html")
                && feature.source_native_id == "endometriosis"
                && feature.evidence_tier == CLINICAL_SUMMARY_TIER
                && feature.normalized_hpo_id.as_deref() == Some("HP:0100607")
        }));
    }

    #[test]
    fn three_disease_checksum_regression() {
        let mut recall = BTreeMap::new();
        recall.insert(
            "expected_symptom_recall".to_string(),
            json!(round3(6.0 / 7.0)),
        );
        assert_eq!(
            serde_json::to_string(&recall).expect("recall json"),
            r#"{"expected_symptom_recall":0.857}"#
        );

        let mut rows = Vec::new();
        for key in [
            "uterine_fibroid",
            "endometriosis",
            "chronic_venous_insufficiency",
        ] {
            let config = config(key);
            let features = features_for_config(&config);
            let matched = features.len();
            let mut coverage = BTreeMap::new();
            coverage.insert("clinical_feature_count".to_string(), json!(features.len()));
            coverage.insert("expected_symptom_matched".to_string(), json!(matched));
            coverage.insert(
                "expected_symptom_recall".to_string(),
                json!(round3(
                    matched as f64 / config.expected_symptoms.len() as f64
                )),
            );
            coverage.insert(
                "mapped_feature_count".to_string(),
                json!(
                    features
                        .iter()
                        .filter(|feature| feature.normalized_hpo_id.is_some())
                        .count()
                ),
            );

            let mut row = BTreeMap::new();
            row.insert("coverage".to_string(), json!(coverage));
            row.insert("disease_key".to_string(), json!(key));
            row.insert(
                "feature_checksum".to_string(),
                json!(feature_checksum(&features)),
            );
            rows.push(row);
        }

        assert_eq!(
            rows[0]["feature_checksum"],
            json!("9db51d8df19b269518c3526bbdb4ed4af1c7f73d84edd18cdec2a0ed06677f4d")
        );
        assert_eq!(
            rows[1]["feature_checksum"],
            json!("97894289fd3c418d5d5405f8f6acfae8e003f9a345c0ab248f173b55d1825106")
        );
        assert_eq!(
            rows[2]["feature_checksum"],
            json!("8765bf6252d14be68b08593d152c08d39d746c6554995c1e820aa1a638575d11")
        );
        assert_eq!(
            sha256_hex(&serde_json::to_string(&rows).expect("outer checksum json")),
            "f08c35ff31306ff4696bd953eaba4b00aeed9e6746a1228469e1479238e3d34f"
        );
    }

    #[test]
    fn topic_helper_builds_medlineplus_topic() {
        let row = topic("Title", "https://example.test/topic.html", "Summary");
        assert_eq!(row.title, "Title");
        assert_eq!(source_native_id(&row.url), "topic");
    }
}
