use std::collections::{HashMap, HashSet};
use std::time::Duration;

use futures::future::join_all;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::entities::SearchPage;
use crate::entities::drug::{self, DrugSearchFilters};
use crate::entities::trial::{self, TrialSearchFilters, TrialSource};
use crate::error::BioMcpError;
use crate::sources::civic::{CivicClient, CivicContext};
use crate::sources::disgenet::{DisgenetAssociationRecord, DisgenetClient};
use crate::sources::hpo::HpoClient;
use crate::sources::monarch::{
    MonarchClient, MonarchGeneAssociation, MonarchModelAssociation, MonarchPhenotypeMatch,
};
use crate::sources::mydisease::{MyDiseaseClient, MyDiseaseHit};
use crate::sources::nih_reporter::{NihReporterClient, NihReporterFundingSection};
use crate::sources::ols4::OlsClient;
use crate::sources::opentargets::OpenTargetsClient;
use crate::sources::reactome::ReactomeClient;
use crate::sources::seer::{SeerClient, SeerSurvivalPayload, resolve_site};
use crate::transform;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Disease {
    pub id: String, // e.g., MONDO:0005105
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definition: Option<String>,
    #[serde(default)]
    pub synonyms: Vec<String>,
    #[serde(default)]
    pub parents: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub associated_genes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gene_associations: Vec<DiseaseGeneAssociation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub top_genes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub top_gene_scores: Vec<DiseaseTargetScore>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub treatment_landscape: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recruiting_trial_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pathways: Vec<DiseasePathway>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phenotypes: Vec<DiseasePhenotype>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub key_features: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<DiseaseVariantAssociation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_variant: Option<DiseaseVariantAssociation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<DiseaseModelAssociation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prevalence: Vec<DiseasePrevalenceEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prevalence_note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub survival: Option<DiseaseSurvival>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub survival_note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub funding: Option<NihReporterFundingSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub funding_note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub civic: Option<CivicContext>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disgenet: Option<DiseaseDisgenet>,
    #[serde(default)]
    pub xrefs: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiseasePathway {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiseasePhenotype {
    pub hpo_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_qualifier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onset_qualifier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sex_qualifier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage_qualifier: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub qualifiers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiseaseGeneAssociation {
    pub gene: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opentargets_score: Option<DiseaseAssociationScoreSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiseaseAssociationScoreSummary {
    pub overall_score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gwas_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rare_variant_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub somatic_mutation_score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiseaseTargetScore {
    pub symbol: String,
    #[serde(flatten)]
    pub summary: DiseaseAssociationScoreSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiseaseVariantAssociation {
    pub variant: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiseaseModelAssociation {
    pub model: String,
    #[serde(skip)]
    pub model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organism: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiseasePrevalenceEvidence {
    pub estimate: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiseaseSurvival {
    pub site_code: u16,
    pub site_label: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub series: Vec<DiseaseSurvivalSeries>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiseaseSurvivalSeries {
    pub sex: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_observed: Option<DiseaseSurvivalPoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_modeled: Option<DiseaseSurvivalPoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<DiseaseSurvivalPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiseaseSurvivalPoint {
    pub year: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relative_survival_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub standard_error: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lower_ci: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upper_ci: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modeled_relative_survival_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub case_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiseaseDisgenetAssociation {
    pub symbol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entrez_id: Option<u32>,
    pub score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publication_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clinical_trial_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_index: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_level: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiseaseDisgenet {
    pub associations: Vec<DiseaseDisgenetAssociation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiseaseSearchResult {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synonyms_preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_via: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhenotypeSearchResult {
    pub disease_id: String,
    pub disease_name: String,
    pub score: f64,
}

#[derive(Debug, Clone, Default)]
pub struct DiseaseSearchFilters {
    pub query: Option<String>,
    pub source: Option<String>,
    pub inheritance: Option<String>,
    pub phenotype: Option<String>,
    pub onset: Option<String>,
}

const DISEASE_SECTION_GENES: &str = "genes";
const DISEASE_SECTION_PATHWAYS: &str = "pathways";
const DISEASE_SECTION_PHENOTYPES: &str = "phenotypes";
const DISEASE_SECTION_VARIANTS: &str = "variants";
const DISEASE_SECTION_MODELS: &str = "models";
const DISEASE_SECTION_PREVALENCE: &str = "prevalence";
const DISEASE_SECTION_SURVIVAL: &str = "survival";
const DISEASE_SECTION_FUNDING: &str = "funding";
const DISEASE_SECTION_CIVIC: &str = "civic";
const DISEASE_SECTION_DISGENET: &str = "disgenet";
const DISEASE_SECTION_ALL: &str = "all";

pub const DISEASE_SECTION_NAMES: &[&str] = &[
    DISEASE_SECTION_GENES,
    DISEASE_SECTION_PATHWAYS,
    DISEASE_SECTION_PHENOTYPES,
    DISEASE_SECTION_VARIANTS,
    DISEASE_SECTION_MODELS,
    DISEASE_SECTION_PREVALENCE,
    DISEASE_SECTION_SURVIVAL,
    DISEASE_SECTION_FUNDING,
    DISEASE_SECTION_CIVIC,
    DISEASE_SECTION_DISGENET,
    DISEASE_SECTION_ALL,
];

const OPTIONAL_ENRICHMENT_TIMEOUT: Duration = Duration::from_secs(8);
const MIN_DIRECT_DISEASE_MATCH_SCORE: i32 = 120;
const SURVIVAL_NO_DATA_NOTE: &str = "SEER survival data not available for this condition.";
const SURVIVAL_UNAVAILABLE_NOTE: &str = "SEER survival data is temporarily unavailable.";
const FUNDING_NO_DATA_NOTE: &str = "No NIH funding data found for this query.";
const FUNDING_UNAVAILABLE_NOTE: &str = "NIH Reporter funding data is temporarily unavailable.";

#[derive(Debug, Clone, Copy, Default)]
struct DiseaseSections {
    include_genes: bool,
    include_pathways: bool,
    include_phenotypes: bool,
    include_variants: bool,
    include_models: bool,
    include_prevalence: bool,
    include_survival: bool,
    include_funding: bool,
    include_civic: bool,
    include_disgenet: bool,
}

fn parse_sections(sections: &[String]) -> Result<DiseaseSections, BioMcpError> {
    let mut out = DiseaseSections::default();
    let mut include_all = false;

    for raw in sections {
        let section = raw.trim().to_ascii_lowercase();
        if section.is_empty() {
            continue;
        }
        if section == "--json" || section == "-j" {
            continue;
        }

        match section.as_str() {
            DISEASE_SECTION_GENES => out.include_genes = true,
            DISEASE_SECTION_PATHWAYS => out.include_pathways = true,
            DISEASE_SECTION_PHENOTYPES => out.include_phenotypes = true,
            DISEASE_SECTION_VARIANTS => out.include_variants = true,
            DISEASE_SECTION_MODELS => out.include_models = true,
            DISEASE_SECTION_PREVALENCE => out.include_prevalence = true,
            DISEASE_SECTION_SURVIVAL => out.include_survival = true,
            DISEASE_SECTION_FUNDING => out.include_funding = true,
            DISEASE_SECTION_CIVIC => out.include_civic = true,
            DISEASE_SECTION_DISGENET => out.include_disgenet = true,
            DISEASE_SECTION_ALL => include_all = true,
            _ => {
                return Err(BioMcpError::InvalidArgument(format!(
                    "Unknown section \"{section}\" for disease. Available: {}",
                    DISEASE_SECTION_NAMES.join(", ")
                )));
            }
        }
    }

    if include_all {
        out.include_genes = true;
        out.include_pathways = true;
        out.include_phenotypes = true;
        out.include_variants = true;
        out.include_models = true;
        out.include_prevalence = true;
        out.include_survival = true;
        out.include_civic = true;
    }

    Ok(out)
}

fn normalize_disease_id(value: &str) -> Option<String> {
    let v = value.trim();
    if v.is_empty() {
        return None;
    }
    if v.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return None;
    }
    let (prefix, rest) = v.split_once(':')?;
    let rest = rest.trim();
    if rest.is_empty() {
        return None;
    }
    if prefix.eq_ignore_ascii_case("MONDO") {
        return Some(format!("MONDO:{rest}"));
    }
    if prefix.eq_ignore_ascii_case("DOID") {
        return Some(format!("DOID:{rest}"));
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DiseaseLookupInput {
    CanonicalOntologyId(String),
    CrosswalkId(DiseaseXrefKind, String),
    FreeText,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum DiseaseXrefKind {
    Mesh,
    Omim,
    Icd10Cm,
}

impl DiseaseXrefKind {
    fn source_key(self) -> &'static str {
        match self {
            Self::Mesh => "mesh",
            Self::Omim => "omim",
            Self::Icd10Cm => "icd10cm",
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::Mesh => "MESH",
            Self::Omim => "OMIM",
            Self::Icd10Cm => "ICD10CM",
        }
    }

    fn resolved_via_label(self) -> String {
        format!("{} crosswalk", self.display_name())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RankedDiseaseFallbackCandidate {
    label: String,
    synonyms: Vec<String>,
    match_tier: crate::entities::discover::MatchTier,
    confidence: crate::entities::discover::DiscoverConfidence,
    source_ids: Vec<DiseaseFallbackId>,
    original_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum DiseaseFallbackId {
    CanonicalOntology(String),
    Crosswalk(DiseaseXrefKind, String),
}

const MAX_DISEASE_SEARCH_LIMIT: usize = 50;
const FALLBACK_LOOKUP_CAP: usize = 12;

fn parse_disease_lookup_input(value: &str) -> DiseaseLookupInput {
    if let Some(id) = normalize_disease_id(value) {
        return DiseaseLookupInput::CanonicalOntologyId(id);
    }

    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return DiseaseLookupInput::FreeText;
    }

    let Some((prefix, raw_value)) = trimmed.split_once(':') else {
        return DiseaseLookupInput::FreeText;
    };
    let raw_value = raw_value.trim();
    if raw_value.is_empty() {
        return DiseaseLookupInput::FreeText;
    }

    let kind = if prefix.eq_ignore_ascii_case("MESH") {
        Some(DiseaseXrefKind::Mesh)
    } else if prefix.eq_ignore_ascii_case("OMIM") {
        Some(DiseaseXrefKind::Omim)
    } else if prefix.eq_ignore_ascii_case("ICD10CM") {
        Some(DiseaseXrefKind::Icd10Cm)
    } else {
        None
    };

    if let Some(kind) = kind {
        DiseaseLookupInput::CrosswalkId(kind, raw_value.to_string())
    } else {
        DiseaseLookupInput::FreeText
    }
}

fn preferred_crosswalk_hit(hits: Vec<MyDiseaseHit>) -> Option<MyDiseaseHit> {
    hits.into_iter().min_by(|left, right| {
        let rank = |id: &str| {
            if id.starts_with("MONDO:") {
                0u8
            } else if id.starts_with("DOID:") {
                1u8
            } else {
                2u8
            }
        };
        rank(&left.id)
            .cmp(&rank(&right.id))
            .then_with(|| left.id.cmp(&right.id))
    })
}

fn match_tier_rank(value: crate::entities::discover::MatchTier) -> u8 {
    match value {
        crate::entities::discover::MatchTier::Exact => 0,
        crate::entities::discover::MatchTier::Prefix => 1,
        crate::entities::discover::MatchTier::Contains => 2,
        crate::entities::discover::MatchTier::Weak => 3,
    }
}

fn discover_confidence_rank(value: crate::entities::discover::DiscoverConfidence) -> u8 {
    match value {
        crate::entities::discover::DiscoverConfidence::CanonicalId => 0,
        crate::entities::discover::DiscoverConfidence::UmlsOnly => 1,
        crate::entities::discover::DiscoverConfidence::LabelOnly => 2,
    }
}

fn is_generic_disease_label(value: &str) -> bool {
    let tokens = normalize_disease_text(value)
        .split_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        return false;
    }

    const GENERIC: &[&str] = &["disease", "disorder", "syndrome", "condition"];
    tokens.len() <= 2
        && tokens
            .iter()
            .all(|token| GENERIC.iter().any(|generic| token == generic))
}

fn contains_all_query_tokens(query_tokens: &[String], haystacks: &[String]) -> bool {
    if query_tokens.is_empty() {
        return false;
    }

    haystacks.iter().any(|haystack| {
        let normalized = normalize_disease_text(haystack);
        query_tokens
            .iter()
            .all(|token| normalized.contains(token.as_str()))
    })
}

fn normalize_supported_discover_source_id(source: &str, id: &str) -> Option<DiseaseFallbackId> {
    let source = source.trim();
    let id = id.trim();
    if source.is_empty() || id.is_empty() {
        return None;
    }

    if source.eq_ignore_ascii_case("MONDO") || source.eq_ignore_ascii_case("DOID") {
        return normalize_disease_id(&format!("{source}:{id}"))
            .map(DiseaseFallbackId::CanonicalOntology);
    }

    if source.eq_ignore_ascii_case("MESH") {
        let value = id
            .trim_start_matches("MESH:")
            .trim_start_matches("mesh:")
            .trim();
        if !value.is_empty() {
            return Some(DiseaseFallbackId::Crosswalk(
                DiseaseXrefKind::Mesh,
                value.to_string(),
            ));
        }
        return None;
    }

    if source.eq_ignore_ascii_case("OMIM") {
        let value = id
            .trim_start_matches("OMIM:")
            .trim_start_matches("omim:")
            .trim();
        if !value.is_empty() {
            return Some(DiseaseFallbackId::Crosswalk(
                DiseaseXrefKind::Omim,
                value.to_string(),
            ));
        }
        return None;
    }

    if source.eq_ignore_ascii_case("ICD10CM") || source.eq_ignore_ascii_case("ICD10") {
        let value = id
            .trim_start_matches("ICD10CM:")
            .trim_start_matches("icd10cm:")
            .trim_start_matches("ICD10:")
            .trim_start_matches("icd10:")
            .trim();
        if !value.is_empty() {
            return Some(DiseaseFallbackId::Crosswalk(
                DiseaseXrefKind::Icd10Cm,
                value.to_string(),
            ));
        }
        return None;
    }

    None
}

fn ranked_fallback_source_ids(
    concept: &crate::entities::discover::DiscoverConcept,
) -> Vec<DiseaseFallbackId> {
    let mut canonical_ids = Vec::new();
    let mut per_kind: HashMap<DiseaseXrefKind, Vec<String>> = HashMap::new();
    let mut seen = HashSet::new();
    let mut push = |value: DiseaseFallbackId| {
        let key = match &value {
            DiseaseFallbackId::CanonicalOntology(id) => {
                format!("canonical:{}", id.to_ascii_uppercase())
            }
            DiseaseFallbackId::Crosswalk(kind, id) => {
                format!("{}:{}", kind.display_name(), id.to_ascii_uppercase())
            }
        };
        if seen.insert(key) {
            match value {
                DiseaseFallbackId::CanonicalOntology(id) => canonical_ids.push(id),
                DiseaseFallbackId::Crosswalk(kind, id) => {
                    per_kind.entry(kind).or_default().push(id);
                }
            }
        }
    };

    if let Some(primary_id) = concept.primary_id.as_deref()
        && let Some((source, value)) = primary_id.split_once(':')
        && let Some(normalized) = normalize_supported_discover_source_id(source, value)
    {
        push(normalized);
    }

    for xref in &concept.xrefs {
        if let Some(normalized) = normalize_supported_discover_source_id(&xref.source, &xref.id) {
            push(normalized);
        }
    }

    let mut ranked = Vec::new();
    for id in canonical_ids {
        ranked.push(DiseaseFallbackId::CanonicalOntology(id));
    }
    for kind in [
        DiseaseXrefKind::Mesh,
        DiseaseXrefKind::Omim,
        DiseaseXrefKind::Icd10Cm,
    ] {
        if let Some(values) = per_kind.remove(&kind) {
            for value in values {
                ranked.push(DiseaseFallbackId::Crosswalk(kind, value));
            }
        }
    }
    ranked
}

fn rank_disease_fallback_candidates(
    query: &str,
    concepts: &[crate::entities::discover::DiscoverConcept],
) -> Vec<RankedDiseaseFallbackCandidate> {
    let query_tokens = normalize_disease_text(query)
        .split_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();

    let mut candidates = concepts
        .iter()
        .enumerate()
        .filter(|(_, concept)| {
            concept.primary_type == crate::entities::discover::DiscoverType::Disease
        })
        .filter_map(|(index, concept)| {
            let source_ids = ranked_fallback_source_ids(concept);
            if source_ids.is_empty() {
                return None;
            }

            Some(RankedDiseaseFallbackCandidate {
                label: concept.label.clone(),
                synonyms: concept.synonyms.clone(),
                match_tier: concept.match_tier,
                confidence: concept.confidence,
                source_ids,
                original_index: index,
            })
        })
        .collect::<Vec<_>>();

    candidates.sort_by(|left, right| {
        let left_haystacks = std::iter::once(left.label.clone())
            .chain(left.synonyms.iter().cloned())
            .collect::<Vec<_>>();
        let right_haystacks = std::iter::once(right.label.clone())
            .chain(right.synonyms.iter().cloned())
            .collect::<Vec<_>>();
        let left_token_match = contains_all_query_tokens(&query_tokens, &left_haystacks);
        let right_token_match = contains_all_query_tokens(&query_tokens, &right_haystacks);
        let left_generic = is_generic_disease_label(&left.label);
        let right_generic = is_generic_disease_label(&right.label);

        right_token_match
            .cmp(&left_token_match)
            .then_with(|| left_generic.cmp(&right_generic))
            .then_with(|| match_tier_rank(left.match_tier).cmp(&match_tier_rank(right.match_tier)))
            .then_with(|| {
                discover_confidence_rank(left.confidence)
                    .cmp(&discover_confidence_rank(right.confidence))
            })
            .then_with(|| left.original_index.cmp(&right.original_index))
    });

    candidates
}

async fn resolve_fallback_row(
    client: &MyDiseaseClient,
    prefer_doid: bool,
    source_id: &DiseaseFallbackId,
) -> Result<Option<DiseaseSearchResult>, BioMcpError> {
    let row = match source_id {
        DiseaseFallbackId::CanonicalOntology(id) => {
            let hit = match client.get(id).await {
                Ok(hit) => hit,
                Err(BioMcpError::NotFound { .. }) => return Ok(None),
                Err(err) => return Err(err),
            };
            let mut row = transform::disease::from_mydisease_search_hit(&hit);
            if prefer_doid && let Some(doid) = transform::disease::doid_from_mydisease_hit(&hit) {
                row.id = doid;
            }
            row.resolved_via = Some(if id.starts_with("DOID:") {
                "DOID canonical".to_string()
            } else {
                "MONDO canonical".to_string()
            });
            row.source_id = Some(id.clone());
            row
        }
        DiseaseFallbackId::Crosswalk(kind, source_value) => {
            let response = client
                .lookup_disease_by_xref(kind.source_key(), source_value, 5)
                .await?;
            let Some(best) = preferred_crosswalk_hit(response.hits) else {
                return Ok(None);
            };
            let mut row = transform::disease::from_mydisease_search_hit(&best);
            if prefer_doid && let Some(doid) = transform::disease::doid_from_mydisease_hit(&best) {
                row.id = doid;
            }
            row.resolved_via = Some(kind.resolved_via_label());
            row.source_id = Some(format!("{}:{source_value}", kind.display_name()));
            row
        }
    };

    Ok(Some(row))
}

async fn collect_fallback_search_page<F, Fut>(
    query: &str,
    limit: usize,
    offset: usize,
    candidates: Vec<RankedDiseaseFallbackCandidate>,
    mut resolve_source_id: F,
) -> Result<Option<SearchPage<DiseaseSearchResult>>, BioMcpError>
where
    F: FnMut(DiseaseFallbackId) -> Fut,
    Fut: std::future::Future<Output = Result<Option<DiseaseSearchResult>, BioMcpError>>,
{
    let needed = offset.saturating_add(limit);
    let query_tokens = normalize_disease_text(query)
        .split_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut lookups = 0usize;
    let mut deduped = Vec::new();
    let mut seen_ids = HashSet::new();

    for candidate in candidates {
        if deduped.len() >= needed || lookups >= FALLBACK_LOOKUP_CAP {
            break;
        }

        for source_id in candidate.source_ids {
            if deduped.len() >= needed || lookups >= FALLBACK_LOOKUP_CAP {
                break;
            }

            lookups += 1;
            let Some(mut row) = (match resolve_source_id(source_id.clone()).await {
                Ok(row) => row,
                Err(err) => {
                    warn!("Disease search fallback row resolution failed: {err}");
                    continue;
                }
            }) else {
                continue;
            };

            let candidate_score = disease_candidate_score(query, &candidate.label);
            let row_score = disease_candidate_score(query, &row.name);
            if row.name == row.id || candidate_score > row_score {
                row.name = candidate.label.clone();
            }
            let mut haystacks = vec![row.name.clone()];
            haystacks.extend(candidate.synonyms.iter().cloned());
            if !contains_all_query_tokens(&query_tokens, &haystacks) {
                continue;
            }
            if !seen_ids.insert(row.id.clone()) {
                break;
            }

            deduped.push(row);
            break;
        }
    }

    if deduped.len() <= offset {
        return Ok(None);
    }

    let total = deduped.len();
    let results = deduped
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect::<Vec<_>>();
    if results.is_empty() {
        return Ok(None);
    }

    Ok(Some(SearchPage::offset(results, Some(total))))
}

pub(crate) async fn fallback_search_page(
    filters: &DiseaseSearchFilters,
    limit: usize,
    offset: usize,
) -> Result<Option<SearchPage<DiseaseSearchResult>>, BioMcpError> {
    if limit == 0 || limit > MAX_DISEASE_SEARCH_LIMIT {
        return Err(BioMcpError::InvalidArgument(format!(
            "--limit must be between 1 and {MAX_DISEASE_SEARCH_LIMIT}"
        )));
    }

    let query = filters
        .query
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            BioMcpError::InvalidArgument(
                "Query is required. Example: biomcp search disease -q melanoma".into(),
            )
        })?;

    if filters
        .source
        .as_deref()
        .map(str::trim)
        .is_some_and(|source| source.eq_ignore_ascii_case("mesh"))
    {
        return Ok(None);
    }

    let discover = match crate::entities::discover::resolve_query(
        query,
        crate::entities::discover::DiscoverMode::AliasFallback,
    )
    .await
    {
        Ok(result) => result,
        Err(err) => {
            warn!("Disease search discover fallback unavailable: {err}");
            return Ok(None);
        }
    };

    let candidates = rank_disease_fallback_candidates(query, &discover.concepts);
    if candidates.is_empty() {
        return Ok(None);
    }

    let prefer_doid = filters
        .source
        .as_deref()
        .map(str::trim)
        .is_some_and(|source| source.eq_ignore_ascii_case("doid"));
    let client = MyDiseaseClient::new()?;
    collect_fallback_search_page(query, limit, offset, candidates, |source_id| {
        let client = client.clone();
        async move { resolve_fallback_row(&client, prefer_doid, &source_id).await }
    })
    .await
}

fn normalize_disease_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch.is_whitespace() {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(' ');
        }
    }
    let out = out
        .replace("carcinoma", "cancer")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    out.trim().to_string()
}

fn disease_exact_rank(name: &str, query: &str) -> u8 {
    let name = name.trim().to_ascii_lowercase();
    let query = query.trim().to_ascii_lowercase();
    if name == query {
        3
    } else if name.starts_with(&query) {
        2
    } else if name.contains(&query) {
        1
    } else {
        0
    }
}

fn has_subtype_marker(value: &str) -> bool {
    let normalized = normalize_disease_text(value);
    if normalized.is_empty() {
        return false;
    }

    let markers = [
        "sporadic",
        "hereditary",
        "familial",
        "metastatic",
        "recurrent",
        "adenocarcinoma",
        "squamous",
        "triple negative",
        "triple positive",
        "er positive",
        "er negative",
        "pr positive",
        "pr negative",
        "her2 positive",
        "her2 negative",
        "in situ",
    ];
    if markers.iter().any(|marker| normalized.contains(marker)) {
        return true;
    }

    let words = normalized.split_whitespace().collect::<Vec<_>>();
    for pair in words.windows(2) {
        if pair[0] == "type" && pair[1].chars().all(|c| c.is_ascii_digit()) {
            return true;
        }
    }
    false
}

fn disease_candidate_score(query: &str, candidate_label: &str) -> i32 {
    let query_trimmed = query.trim();
    let candidate_trimmed = candidate_label.trim();
    if query_trimmed.is_empty() || candidate_trimmed.is_empty() {
        return i32::MIN / 2;
    }

    let query_norm = normalize_disease_text(query_trimmed);
    let candidate_norm = normalize_disease_text(candidate_trimmed);
    let mut score = 0;

    if candidate_trimmed.eq_ignore_ascii_case(query_trimmed) {
        score += 200;
    }
    if candidate_norm == query_norm {
        score += 120;
    } else if candidate_norm.contains(&query_norm) {
        score += 40;
    } else if query_norm.contains(&candidate_norm) {
        score += 20;
    }

    let query_has_subtype = has_subtype_marker(query_trimmed);
    let candidate_has_subtype = has_subtype_marker(candidate_trimmed);
    if candidate_has_subtype && !query_has_subtype {
        score -= 60;
    }
    if !candidate_has_subtype && query_has_subtype {
        score -= 20;
    }

    score
}

fn collect_json_strings(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(v) => {
            let v = v.trim();
            if !v.is_empty() {
                out.push(v.to_string());
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_json_strings(value, out);
            }
        }
        serde_json::Value::Object(values) => {
            for value in values.values() {
                collect_json_strings(value, out);
            }
        }
        _ => {}
    }
}

fn disease_candidate_labels(hit: &MyDiseaseHit) -> Vec<String> {
    let mut labels = vec![transform::disease::name_from_mydisease_hit(hit)];
    if let Some(value) = hit.mondo.as_ref().and_then(|v| v.get("synonym")) {
        collect_json_strings(value, &mut labels);
    }
    if let Some(value) = hit
        .disease_ontology
        .as_ref()
        .and_then(|v| v.get("synonyms"))
    {
        collect_json_strings(value, &mut labels);
    }

    let mut deduped = Vec::new();
    for label in labels {
        if deduped
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(&label))
        {
            continue;
        }
        deduped.push(label);
    }
    deduped
}

fn best_disease_candidate_score(query: &str, hit: &MyDiseaseHit) -> i32 {
    disease_candidate_labels(hit)
        .into_iter()
        .map(|label| disease_candidate_score(query, &label))
        .max()
        .unwrap_or(i32::MIN / 2)
}

fn best_disease_candidate_score_for_queries(queries: &[String], hit: &MyDiseaseHit) -> i32 {
    queries
        .iter()
        .map(|query| best_disease_candidate_score(query, hit))
        .max()
        .unwrap_or(i32::MIN / 2)
}

fn scored_best_candidate_for_queries(
    queries: &[String],
    hits: Vec<MyDiseaseHit>,
) -> Option<MyDiseaseHit> {
    if queries.is_empty() {
        return None;
    }

    let mut ranked: Vec<(i32, u8, usize, String, MyDiseaseHit)> = hits
        .into_iter()
        .map(|hit| {
            let primary_name = transform::disease::name_from_mydisease_hit(&hit);
            let best_score = best_disease_candidate_score_for_queries(queries, &hit);
            let best_exact_rank = queries
                .iter()
                .map(|query| disease_exact_rank(&primary_name, query))
                .max()
                .unwrap_or(0);
            let normalized_len = normalize_disease_text(&primary_name).len();
            (
                best_score,
                best_exact_rank,
                normalized_len,
                hit.id.clone(),
                hit,
            )
        })
        .collect();

    ranked.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| a.2.cmp(&b.2))
            .then_with(|| a.3.cmp(&b.3))
    });
    ranked.into_iter().next().map(|(_, _, _, _, hit)| hit)
}

fn resolver_queries(name_or_id: &str) -> Vec<String> {
    let query = name_or_id.trim();
    if query.is_empty() {
        return Vec::new();
    }

    let mut queries = vec![query.to_string()];
    let mut push_query = |candidate: String| {
        if queries
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&candidate))
        {
            return;
        }
        queries.push(candidate);
    };

    let lower = query.to_ascii_lowercase();
    if lower.contains("cancer") {
        push_query(lower.replace("cancer", "carcinoma"));
    }
    for (from, to) in [
        ("myeloid leukemia", "myelogenous leukemia"),
        ("myeloid leukaemia", "myelogenous leukaemia"),
        ("myelogenous leukemia", "myeloid leukemia"),
        ("myelogenous leukaemia", "myeloid leukaemia"),
        ("hodgkin lymphoma", "hodgkins lymphoma"),
        ("hodgkins lymphoma", "hodgkin lymphoma"),
    ] {
        if lower.contains(from) {
            push_query(lower.replace(from, to));
        }
    }
    if lower == "chronic myeloid leukemia" {
        push_query("chronic myelogenous leukemia, bcr-abl1 positive".to_string());
    }
    if matches!(lower.as_str(), "hodgkin lymphoma" | "hodgkins lymphoma") {
        push_query("hodgkin disease".to_string());
    }
    queries
}

struct DiseaseSearchCandidate {
    hit: MyDiseaseHit,
    first_seen_query_idx: usize,
    first_seen_upstream_idx: usize,
}

fn rerank_disease_search_hits(
    query: &str,
    query_hits: Vec<(usize, Vec<MyDiseaseHit>)>,
) -> Vec<MyDiseaseHit> {
    let mut deduped: HashMap<String, DiseaseSearchCandidate> = HashMap::new();
    for (query_idx, hits) in query_hits {
        for (upstream_idx, hit) in hits.into_iter().enumerate() {
            deduped
                .entry(hit.id.clone())
                .or_insert(DiseaseSearchCandidate {
                    hit,
                    first_seen_query_idx: query_idx,
                    first_seen_upstream_idx: upstream_idx,
                });
        }
    }

    let mut ranked = deduped
        .into_values()
        .map(|candidate| {
            let display_name = transform::disease::name_from_mydisease_hit(&candidate.hit);
            (
                best_disease_candidate_score(query, &candidate.hit),
                disease_exact_rank(&display_name, query),
                candidate.first_seen_query_idx,
                candidate.first_seen_upstream_idx,
                candidate.hit.id.clone(),
                candidate.hit,
            )
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| a.2.cmp(&b.2))
            .then_with(|| a.3.cmp(&b.3))
            .then_with(|| a.4.cmp(&b.4))
    });
    ranked.into_iter().map(|(_, _, _, _, _, hit)| hit).collect()
}

pub(crate) async fn resolve_disease_hit_by_name(
    client: &MyDiseaseClient,
    name_or_id: &str,
) -> Result<MyDiseaseHit, BioMcpError> {
    if let Some(best) = resolve_disease_hit_by_name_direct(client, name_or_id).await? {
        return Ok(best);
    }
    if let Some(best) = resolve_disease_hit_via_discover_fallback(client, name_or_id).await? {
        return Ok(best);
    }

    Err(BioMcpError::NotFound {
        entity: "disease".into(),
        id: name_or_id.into(),
        suggestion: format!("Try searching: biomcp search disease -q \"{name_or_id}\""),
    })
}

async fn resolve_disease_hit_by_name_direct(
    client: &MyDiseaseClient,
    name_or_id: &str,
) -> Result<Option<MyDiseaseHit>, BioMcpError> {
    let queries = resolver_queries(name_or_id);
    if queries.is_empty() {
        return Ok(None);
    }

    let mut candidates: HashMap<String, MyDiseaseHit> = HashMap::new();
    for query in &queries {
        let resp = client.query(query, 15, 0, None, None, None, None).await?;
        for hit in resp.hits {
            candidates.entry(hit.id.clone()).or_insert(hit);
        }
    }

    Ok(
        scored_best_candidate_for_queries(&queries, candidates.into_values().collect()).filter(
            |hit| {
                best_disease_candidate_score_for_queries(&queries, hit)
                    >= MIN_DIRECT_DISEASE_MATCH_SCORE
            },
        ),
    )
}

async fn resolve_disease_hit_via_discover_fallback(
    client: &MyDiseaseClient,
    name_or_id: &str,
) -> Result<Option<MyDiseaseHit>, BioMcpError> {
    for query in resolver_queries(name_or_id) {
        let filters = DiseaseSearchFilters {
            query: Some(query),
            ..Default::default()
        };
        let Some(page) = fallback_search_page(&filters, 1, 0).await? else {
            continue;
        };
        let Some(row) = page.results.into_iter().next() else {
            continue;
        };

        match client.get(&row.id).await {
            Ok(hit) => return Ok(Some(hit)),
            Err(err) => {
                warn!("Disease get fallback canonical fetch failed: {err}");
            }
        }
    }

    Ok(None)
}

async fn add_genes_section(disease: &mut Disease) -> Result<(), BioMcpError> {
    let mut queries: Vec<String> = Vec::new();
    let mut push_query = |candidate: &str| {
        let candidate = candidate.trim();
        if candidate.is_empty() {
            return;
        }
        if queries.iter().any(|q| q.eq_ignore_ascii_case(candidate)) {
            return;
        }
        queries.push(candidate.to_string());
    };
    let synonym_candidates = disease.synonyms.iter().take(3).map(String::as_str);
    for candidate in std::iter::once(disease.name.as_str())
        .chain(synonym_candidates)
        .chain(std::iter::once(disease.id.as_str()))
    {
        if candidate.contains('/') {
            for segment in candidate.split('/') {
                push_query(segment);
            }
        }
        push_query(candidate);
    }
    if queries.is_empty() {
        return Ok(());
    }

    let client = OpenTargetsClient::new()?;
    for query in queries {
        let rows = client.disease_associated_targets(&query, 20).await?;
        if rows.is_empty() {
            continue;
        }

        let mut associated_genes: Vec<String> = Vec::new();
        let mut top_gene_scores = Vec::new();

        for row in rows {
            let symbol = row.symbol.trim();
            if symbol.is_empty() {
                continue;
            }
            if associated_genes
                .iter()
                .any(|v| v.eq_ignore_ascii_case(symbol))
            {
                continue;
            }
            associated_genes.push(symbol.to_string());
            if let Some(summary) = disease_association_summary(&row) {
                top_gene_scores.push(DiseaseTargetScore {
                    symbol: symbol.to_string(),
                    summary,
                });
            }
        }

        if !associated_genes.is_empty() {
            disease.associated_genes = associated_genes;
            disease.top_gene_scores = top_gene_scores;
            disease.associated_genes.truncate(20);
            disease.top_gene_scores.truncate(20);
            return Ok(());
        }
    }

    disease.associated_genes.truncate(20);
    disease.top_gene_scores.clear();
    Ok(())
}

async fn add_pathways_section(disease: &mut Disease) -> Result<(), BioMcpError> {
    if disease.associated_genes.is_empty() {
        add_genes_section(disease).await?;
    }
    if disease.associated_genes.is_empty() {
        return Ok(());
    }

    let reactome = ReactomeClient::new()?;
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out: Vec<DiseasePathway> = Vec::new();

    for gene in disease.associated_genes.iter().take(6) {
        let (rows, _) = reactome.search_pathways(gene, 6).await?;
        for row in rows {
            let id = row.id.trim().to_string();
            let name = row.name.trim().to_string();
            if id.is_empty() || name.is_empty() {
                continue;
            }
            if !seen.insert(id.to_ascii_uppercase()) {
                continue;
            }
            out.push(DiseasePathway { id, name });
            if out.len() >= 10 {
                disease.pathways = out;
                return Ok(());
            }
        }
    }

    disease.pathways = out;
    Ok(())
}

fn normalize_hpo_id(value: &str) -> Option<String> {
    let mut id = value.trim().to_ascii_uppercase();
    if id.is_empty() {
        return None;
    }
    id = id.replace('_', ":");
    if !id.starts_with("HP:") {
        return None;
    }
    let suffix = id.trim_start_matches("HP:");
    if suffix.is_empty() || !suffix.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(format!("HP:{suffix}"))
}

async fn add_phenotypes_section(disease: &mut Disease) -> Result<(), BioMcpError> {
    if disease.phenotypes.is_empty() {
        return Ok(());
    }

    let mut ids: Vec<String> = Vec::new();
    for row in &disease.phenotypes {
        if let Some(id) = normalize_hpo_id(&row.hpo_id) {
            ids.push(id);
        }
        if let Some(freq) = row.frequency.as_deref().and_then(normalize_hpo_id) {
            ids.push(freq);
        }
    }
    if ids.is_empty() {
        return Ok(());
    }

    let client = HpoClient::new()?;
    let names = client.resolve_terms(&ids, 20).await?;
    for row in &mut disease.phenotypes {
        if row.name.is_none()
            && let Some(id) = normalize_hpo_id(&row.hpo_id)
        {
            row.name = names.get(&id).cloned();
        }
        if let Some(freq_id) = row.frequency.as_deref().and_then(normalize_hpo_id)
            && let Some(label) = names.get(&freq_id)
        {
            row.frequency = Some(label.clone());
        }
    }
    disease.phenotypes.truncate(20);
    Ok(())
}

fn push_associated_gene(disease: &mut Disease, symbol: &str) {
    let symbol = symbol.trim();
    if symbol.is_empty() {
        return;
    }
    if disease
        .associated_genes
        .iter()
        .any(|v| v.eq_ignore_ascii_case(symbol))
    {
        return;
    }
    disease.associated_genes.push(symbol.to_string());
}

fn map_monarch_gene_association(row: MonarchGeneAssociation) -> Option<DiseaseGeneAssociation> {
    let gene = row.gene.trim();
    if gene.is_empty() {
        return None;
    }
    Some(DiseaseGeneAssociation {
        gene: gene.to_string(),
        relationship: row
            .relationship
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string),
        source: row
            .source
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string),
        opentargets_score: None,
    })
}

fn disease_association_summary(
    row: &crate::sources::opentargets::OpenTargetsAssociatedGene,
) -> Option<DiseaseAssociationScoreSummary> {
    Some(DiseaseAssociationScoreSummary {
        overall_score: row.overall_score?,
        gwas_score: row.gwas_score,
        rare_variant_score: row.rare_variant_score,
        somatic_mutation_score: row.somatic_mutation_score,
    })
}

fn attach_opentargets_scores(disease: &mut Disease) {
    let score_map = disease
        .top_gene_scores
        .iter()
        .map(|row| (row.symbol.to_ascii_lowercase(), row.summary.clone()))
        .collect::<HashMap<_, _>>();

    for association in &mut disease.gene_associations {
        association.opentargets_score = score_map
            .get(&association.gene.to_ascii_lowercase())
            .cloned();
    }
}

fn normalize_ols_disease_id(value: &str) -> Option<String> {
    normalize_disease_id(value).or_else(|| normalize_disease_id(&value.replace('_', ":")))
}

async fn enrich_sparse_disease_identity(disease: &mut Disease) -> Result<(), BioMcpError> {
    let name = disease.name.trim();
    let id = disease.id.trim();
    if !name.eq_ignore_ascii_case(id) || !disease.synonyms.is_empty() {
        return Ok(());
    }

    let canonical_id = match normalize_disease_id(id) {
        Some(id) => id,
        None => return Ok(()),
    };

    let client = OlsClient::new()?;
    let exact = client.search(&canonical_id).await?.into_iter().find(|doc| {
        doc.obo_id
            .as_deref()
            .and_then(normalize_ols_disease_id)
            .is_some_and(|value| value == canonical_id)
            || doc
                .short_form
                .as_deref()
                .and_then(normalize_ols_disease_id)
                .is_some_and(|value| value == canonical_id)
    });
    let Some(doc) = exact else {
        return Ok(());
    };

    let label = doc.label.trim();
    if !label.is_empty() {
        disease.name = label.to_string();
    }

    let mut seen = disease
        .synonyms
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    seen.insert(disease.name.to_ascii_lowercase());
    for synonym in doc.exact_synonyms {
        let synonym = synonym.trim();
        if synonym.is_empty() {
            continue;
        }
        let key = synonym.to_ascii_lowercase();
        if !seen.insert(key) {
            continue;
        }
        disease.synonyms.push(synonym.to_string());
        if disease.synonyms.len() >= 10 {
            break;
        }
    }

    Ok(())
}

async fn add_monarch_gene_section(disease: &mut Disease) -> Result<(), BioMcpError> {
    let disease_id = match normalize_disease_id(&disease.id) {
        Some(v) => v,
        None => return Ok(()),
    };

    let client = MonarchClient::new()?;
    let rows = client.disease_gene_associations(&disease_id, 50).await?;

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for row in rows {
        let Some(mapped) = map_monarch_gene_association(row) else {
            continue;
        };

        let key = mapped.gene.to_ascii_lowercase();
        if !seen.insert(key) {
            continue;
        }
        push_associated_gene(disease, &mapped.gene);
        out.push(mapped);
        if out.len() >= 20 {
            break;
        }
    }

    disease.gene_associations = out;
    disease.associated_genes.truncate(20);
    Ok(())
}

fn normalize_gene_source_label(label: &str) -> Option<String> {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        return None;
    }

    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("monarch") {
        Some("Monarch".to_string())
    } else if lower.contains("civic") {
        Some("CIViC".to_string())
    } else if lower.contains("opentarget") || lower.contains("open targets") {
        Some("OpenTargets".to_string())
    } else {
        Some(trimmed.to_string())
    }
}

fn merge_gene_source(existing: &mut Option<String>, new_source: &str) {
    let mut labels: Vec<String> = existing
        .as_deref()
        .into_iter()
        .flat_map(|value| value.split(';'))
        .filter_map(normalize_gene_source_label)
        .collect();
    if let Some(new_label) = normalize_gene_source_label(new_source)
        && !labels
            .iter()
            .any(|value| value.eq_ignore_ascii_case(&new_label))
    {
        labels.push(new_label);
    }

    let mut merged = Vec::new();
    for preferred in ["Monarch", "CIViC", "OpenTargets"] {
        if labels.iter().any(|value| value == preferred) {
            merged.push(preferred.to_string());
        }
    }
    for label in labels {
        if merged
            .iter()
            .any(|value| value.eq_ignore_ascii_case(&label))
        {
            continue;
        }
        merged.push(label);
    }

    *existing = if merged.is_empty() {
        None
    } else {
        Some(merged.join("; "))
    };
}

async fn add_monarch_phenotypes(disease: &mut Disease) -> Result<(), BioMcpError> {
    let disease_id = match normalize_disease_id(&disease.id) {
        Some(v) => v,
        None => return Ok(()),
    };

    let client = MonarchClient::new()?;
    let rows = client.disease_phenotypes(&disease_id, 80).await?;
    if rows.is_empty() {
        return Ok(());
    }

    for row in rows {
        let normalized = normalize_hpo_id(&row.hpo_id);
        let Some(hpo_id) = normalized else { continue };

        if let Some(existing) = disease
            .phenotypes
            .iter_mut()
            .find(|p| normalize_hpo_id(&p.hpo_id).is_some_and(|id| id == hpo_id))
        {
            if existing.name.is_none() {
                existing.name = row
                    .label
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(str::to_string);
            }
            if existing.frequency_qualifier.is_none() {
                existing.frequency_qualifier = row.frequency_qualifier;
            }
            if existing.onset_qualifier.is_none() {
                existing.onset_qualifier = row.onset_qualifier;
            }
            if existing.sex_qualifier.is_none() {
                existing.sex_qualifier = row.sex_qualifier;
            }
            if existing.stage_qualifier.is_none() {
                existing.stage_qualifier = row.stage_qualifier;
            }
            if existing.source.is_none() {
                existing.source = row.source;
            }
            for qualifier in row.qualifiers {
                if qualifier.trim().is_empty() {
                    continue;
                }
                if existing
                    .qualifiers
                    .iter()
                    .any(|v| v.eq_ignore_ascii_case(&qualifier))
                {
                    continue;
                }
                existing.qualifiers.push(qualifier);
            }
            continue;
        }

        disease.phenotypes.push(DiseasePhenotype {
            hpo_id,
            name: row
                .label
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_string),
            evidence: row.relationship,
            frequency: None,
            frequency_qualifier: row.frequency_qualifier,
            onset_qualifier: row.onset_qualifier,
            sex_qualifier: row.sex_qualifier,
            stage_qualifier: row.stage_qualifier,
            qualifiers: row.qualifiers,
            source: row.source,
        });
    }

    disease.phenotypes.truncate(40);
    Ok(())
}

fn looks_like_protein_change(token: &str) -> bool {
    let chars = token.chars().collect::<Vec<_>>();
    if chars.len() < 3 {
        return false;
    }
    chars.first().is_some_and(char::is_ascii_alphabetic)
        && chars.last().is_some_and(char::is_ascii_alphabetic)
        && chars[1..chars.len() - 1].iter().all(char::is_ascii_digit)
}

fn is_hgnc_symbol_candidate(token: &str) -> bool {
    let token = token.trim();
    if token.len() < 2 || token.len() > 15 {
        return false;
    }
    if !token.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return false;
    }
    if !token
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic())
    {
        return false;
    }
    if looks_like_protein_change(token) {
        return false;
    }

    let upper = token.to_ascii_uppercase();
    let excluded = [
        "MUTATION",
        "MUTATIONS",
        "AMPLIFICATION",
        "DELETION",
        "FUSION",
        "WILD",
        "TYPE",
        "LOSS",
        "GAIN",
    ];
    !excluded.contains(&upper.as_str())
}

fn civic_gene_symbol_from_profile(profile: &str) -> Option<String> {
    for token in profile.split(|c: char| !c.is_ascii_alphanumeric() && c != '-') {
        let token = token.trim();
        if token.is_empty() || !is_hgnc_symbol_candidate(token) {
            continue;
        }
        return Some(token.to_ascii_uppercase());
    }
    None
}

async fn augment_genes_with_civic(disease: &mut Disease) -> Result<(), BioMcpError> {
    let Some(query) = disease_query_value(disease) else {
        return Ok(());
    };

    let client = CivicClient::new()?;
    let context = client.by_disease(&query, 25).await?;
    let mut seen = disease
        .gene_associations
        .iter()
        .map(|row| row.gene.to_ascii_lowercase())
        .collect::<HashSet<_>>();

    for symbol in context
        .evidence_items
        .iter()
        .filter_map(|row| civic_gene_symbol_from_profile(&row.molecular_profile))
        .chain(
            context
                .assertions
                .iter()
                .filter_map(|row| civic_gene_symbol_from_profile(&row.molecular_profile)),
        )
    {
        let key = symbol.to_ascii_lowercase();
        if !seen.insert(key) {
            continue;
        }
        push_associated_gene(disease, &symbol);
        disease.gene_associations.push(DiseaseGeneAssociation {
            gene: symbol,
            relationship: Some("associated with disease".into()),
            source: Some("CIViC".into()),
            opentargets_score: None,
        });
        if disease.gene_associations.len() >= 20 {
            break;
        }
    }

    disease.gene_associations.truncate(20);
    disease.associated_genes.truncate(20);
    Ok(())
}

async fn augment_genes_with_opentargets(disease: &mut Disease) -> Result<(), BioMcpError> {
    for score in disease.top_gene_scores.clone() {
        let existing = disease
            .gene_associations
            .iter_mut()
            .find(|row| row.gene.eq_ignore_ascii_case(&score.symbol));
        if let Some(row) = existing {
            merge_gene_source(&mut row.source, "OpenTargets");
            continue;
        }
        if disease.gene_associations.len() >= 20 {
            break;
        }

        push_associated_gene(disease, &score.symbol);
        disease.gene_associations.push(DiseaseGeneAssociation {
            gene: score.symbol,
            relationship: Some("associated with disease".into()),
            source: Some("OpenTargets".into()),
            opentargets_score: None,
        });
    }

    disease.gene_associations.truncate(20);
    disease.associated_genes.truncate(20);
    Ok(())
}

async fn add_civic_variants(disease: &mut Disease) -> Result<(), BioMcpError> {
    let Some(query) = disease_query_value(disease) else {
        return Ok(());
    };

    let client = CivicClient::new()?;
    let context = client.by_disease(&query, 25).await?;

    let mut counts: HashMap<String, (String, u32)> = HashMap::new();
    for profile in context
        .evidence_items
        .iter()
        .map(|row| row.molecular_profile.as_str())
        .chain(
            context
                .assertions
                .iter()
                .map(|row| row.molecular_profile.as_str()),
        )
    {
        let profile = profile.trim();
        if profile.is_empty() {
            continue;
        }
        let key = profile.to_ascii_lowercase();
        let entry = counts
            .entry(key)
            .or_insert_with(|| (profile.to_string(), 0));
        entry.1 += 1;
    }

    let mut rows = counts
        .into_values()
        .map(|(variant, evidence_count)| DiseaseVariantAssociation {
            variant,
            relationship: Some("associated with disease".into()),
            source: Some("CIViC".into()),
            evidence_count: Some(evidence_count),
        })
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| {
        b.evidence_count
            .unwrap_or_default()
            .cmp(&a.evidence_count.unwrap_or_default())
            .then_with(|| a.variant.cmp(&b.variant))
    });
    rows.truncate(20);
    disease.top_variant = rows.first().cloned();
    disease.variants = rows;
    Ok(())
}

fn map_monarch_model(row: MonarchModelAssociation) -> Option<DiseaseModelAssociation> {
    let model = row.model.trim();
    if model.is_empty() {
        return None;
    }
    Some(DiseaseModelAssociation {
        model: model.to_string(),
        model_id: row
            .model_id
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string),
        organism: row
            .organism
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string),
        relationship: row.relationship,
        source: row.source,
        evidence_count: row.evidence_count,
    })
}

async fn add_monarch_models(disease: &mut Disease) -> Result<(), BioMcpError> {
    let disease_id = match normalize_disease_id(&disease.id) {
        Some(v) => v,
        None => return Ok(()),
    };

    let client = MonarchClient::new()?;
    let rows = client.disease_models(&disease_id, 50).await?;
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for row in rows {
        let Some(mapped) = map_monarch_model(row) else {
            continue;
        };
        let key = mapped.model.to_ascii_lowercase();
        if !seen.insert(key) {
            continue;
        }
        out.push(mapped);
        if out.len() >= 20 {
            break;
        }
    }
    disease.models = out;
    Ok(())
}

fn disease_query_value(disease: &Disease) -> Option<String> {
    if !disease.name.trim().is_empty() {
        Some(disease.name.trim().to_string())
    } else if !disease.id.trim().is_empty() {
        Some(disease.id.trim().to_string())
    } else {
        None
    }
}

fn disease_funding_query_value(
    disease: &Disease,
    requested_lookup: Option<&str>,
) -> Option<String> {
    if let Some(requested_lookup) = requested_lookup {
        let requested_lookup = requested_lookup.trim();
        if !requested_lookup.is_empty()
            && matches!(
                parse_disease_lookup_input(requested_lookup),
                DiseaseLookupInput::FreeText
            )
        {
            return Some(requested_lookup.to_string());
        }
    }

    if !disease.name.trim().is_empty() {
        return Some(disease.name.trim().to_string());
    }

    disease.synonyms.iter().find_map(|synonym| {
        let synonym = synonym.trim();
        (!synonym.is_empty()).then(|| synonym.to_string())
    })
}

async fn add_treatment_landscape(disease: &mut Disease) -> Result<(), BioMcpError> {
    let Some(query) = disease_query_value(disease) else {
        return Ok(());
    };

    let filters = DrugSearchFilters {
        indication: Some(query),
        ..Default::default()
    };
    let rows = drug::search(&filters, 5).await?;

    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out: Vec<String> = Vec::new();
    for row in rows {
        let name = row.name.trim();
        if name.is_empty() {
            continue;
        }
        let key = name.to_ascii_lowercase();
        if !seen.insert(key) {
            continue;
        }
        out.push(name.to_string());
        if out.len() >= 5 {
            break;
        }
    }

    disease.treatment_landscape = out;
    Ok(())
}

async fn add_recruiting_trial_count(disease: &mut Disease) -> Result<(), BioMcpError> {
    let Some(query) = disease_query_value(disease) else {
        return Ok(());
    };

    let filters = TrialSearchFilters {
        condition: Some(query),
        status: Some("recruiting".to_string()),
        source: TrialSource::ClinicalTrialsGov,
        ..Default::default()
    };

    let (rows, total) = trial::search(&filters, 5, 0).await?;
    disease.recruiting_trial_count = total.or(Some(rows.len() as u32));
    Ok(())
}

async fn add_prevalence_section(disease: &mut Disease) -> Result<(), BioMcpError> {
    let mut queries: Vec<String> = Vec::new();
    for query in [disease.id.trim(), disease.name.trim()] {
        if query.is_empty() {
            continue;
        }
        if queries.iter().any(|q| q.eq_ignore_ascii_case(query)) {
            continue;
        }
        queries.push(query.to_string());
    }
    if queries.is_empty() {
        disease.prevalence.clear();
        disease.prevalence_note = Some("No prevalence data available from OpenTargets.".into());
        return Ok(());
    }

    let client = OpenTargetsClient::new()?;
    for query in queries {
        let rows = client.disease_prevalence(&query, 8).await?;
        if rows.is_empty() {
            continue;
        }
        disease.prevalence = rows
            .into_iter()
            .map(|row| DiseasePrevalenceEvidence {
                estimate: row.estimate,
                context: row.context,
                source: row.source,
            })
            .collect();
        disease.prevalence_note = None;
        return Ok(());
    }

    disease.prevalence.clear();
    disease.prevalence_note = Some("No prevalence data available from OpenTargets.".into());
    Ok(())
}

fn map_survival_payload(payload: SeerSurvivalPayload) -> DiseaseSurvival {
    DiseaseSurvival {
        site_code: payload.site_code,
        site_label: payload.site_label,
        series: payload
            .series
            .into_iter()
            .map(map_survival_series)
            .collect(),
    }
}

fn map_survival_series(series: crate::sources::seer::SeerSurvivalSeries) -> DiseaseSurvivalSeries {
    let points = series
        .points
        .into_iter()
        .map(map_survival_point)
        .collect::<Vec<_>>();
    let latest_observed = points
        .iter()
        .rev()
        .find(|point| point.relative_survival_rate.is_some())
        .cloned();
    let latest_observed_year = latest_observed.as_ref().map(|point| point.year);
    let latest_modeled = points
        .iter()
        .rev()
        .find(|point| {
            point.modeled_relative_survival_rate.is_some()
                && latest_observed_year.is_none_or(|year| point.year > year)
        })
        .cloned();

    DiseaseSurvivalSeries {
        sex: series.sex_label,
        latest_observed,
        latest_modeled,
        points,
    }
}

fn map_survival_point(point: crate::sources::seer::SeerSurvivalPoint) -> DiseaseSurvivalPoint {
    DiseaseSurvivalPoint {
        year: point.year,
        relative_survival_rate: point.relative_survival_rate,
        standard_error: point.standard_error,
        lower_ci: point.lower_ci,
        upper_ci: point.upper_ci,
        modeled_relative_survival_rate: point.modeled_relative_survival_rate,
        case_count: point.case_count,
    }
}

async fn add_survival_section(disease: &mut Disease) -> Result<(), BioMcpError> {
    let client = match SeerClient::new() {
        Ok(client) => client,
        Err(err) => {
            warn!("SEER Explorer unavailable for disease survival section: {err}");
            disease.survival = None;
            disease.survival_note = Some(SURVIVAL_UNAVAILABLE_NOTE.into());
            return Ok(());
        }
    };

    let catalog = match client.site_catalog().await {
        Ok(catalog) => catalog,
        Err(err) => {
            warn!("SEER Explorer catalog unavailable for disease survival section: {err}");
            disease.survival = None;
            disease.survival_note = Some(SURVIVAL_UNAVAILABLE_NOTE.into());
            return Ok(());
        }
    };

    let Some(site) = resolve_site(disease, &catalog) else {
        disease.survival = None;
        disease.survival_note = Some(SURVIVAL_NO_DATA_NOTE.into());
        return Ok(());
    };

    match client.fetch_survival(site.site_code, &catalog).await {
        Ok(payload) => {
            disease.survival = Some(map_survival_payload(payload));
            disease.survival_note = None;
        }
        Err(err) => {
            warn!(
                "SEER Explorer survival unavailable for disease {} at site {}: {err}",
                disease.id, site.site_code
            );
            disease.survival = None;
            disease.survival_note = Some(SURVIVAL_UNAVAILABLE_NOTE.into());
        }
    }

    Ok(())
}

async fn add_civic_section(disease: &mut Disease) {
    let Some(query) = disease_query_value(disease) else {
        disease.civic = Some(CivicContext::default());
        return;
    };

    let civic_fut = async {
        let client = CivicClient::new()?;
        client.by_disease(&query, 10).await
    };

    match tokio::time::timeout(OPTIONAL_ENRICHMENT_TIMEOUT, civic_fut).await {
        Ok(Ok(context)) => disease.civic = Some(context),
        Ok(Err(err)) => {
            warn!(query = %query, "CIViC unavailable for disease section: {err}");
            disease.civic = Some(CivicContext::default());
        }
        Err(_) => {
            warn!(
                query = %query,
                timeout_secs = OPTIONAL_ENRICHMENT_TIMEOUT.as_secs(),
                "CIViC disease section timed out"
            );
            disease.civic = Some(CivicContext::default());
        }
    }
}

fn empty_funding_section(query: String) -> NihReporterFundingSection {
    NihReporterFundingSection {
        query,
        fiscal_years: Vec::new(),
        matching_project_years: 0,
        grants: Vec::new(),
    }
}

async fn add_funding_section(disease: &mut Disease, requested_lookup: Option<&str>) {
    let Some(query) = disease_funding_query_value(disease, requested_lookup) else {
        disease.funding = Some(empty_funding_section(String::new()));
        disease.funding_note = Some(FUNDING_NO_DATA_NOTE.into());
        return;
    };

    let funding_fut = async {
        let client = NihReporterClient::new()?;
        client.funding(&query).await
    };

    match tokio::time::timeout(OPTIONAL_ENRICHMENT_TIMEOUT, funding_fut).await {
        Ok(Ok(section)) => {
            let no_hits = section.matching_project_years == 0 && section.grants.is_empty();
            disease.funding = Some(section);
            disease.funding_note = if no_hits {
                Some(FUNDING_NO_DATA_NOTE.into())
            } else {
                None
            };
        }
        Ok(Err(err)) => {
            warn!(query = %query, "NIH Reporter unavailable for disease funding section: {err}");
            disease.funding = None;
            disease.funding_note = Some(FUNDING_UNAVAILABLE_NOTE.into());
        }
        Err(_) => {
            warn!(
                query = %query,
                timeout_secs = OPTIONAL_ENRICHMENT_TIMEOUT.as_secs(),
                "NIH Reporter disease funding section timed out"
            );
            disease.funding = None;
            disease.funding_note = Some(FUNDING_UNAVAILABLE_NOTE.into());
        }
    }
}

fn map_disgenet_disease_association(row: DisgenetAssociationRecord) -> DiseaseDisgenetAssociation {
    DiseaseDisgenetAssociation {
        symbol: row.gene_symbol,
        entrez_id: row.gene_ncbi_id,
        score: row.score,
        publication_count: row.publication_count,
        clinical_trial_count: row.clinical_trial_count,
        evidence_index: row.evidence_index,
        evidence_level: row.evidence_level,
    }
}

async fn add_disgenet_section(disease: &mut Disease) -> Result<(), BioMcpError> {
    let client = DisgenetClient::new()?;
    let associations = client
        .fetch_disease_associations(disease, 10)
        .await?
        .into_iter()
        .map(map_disgenet_disease_association)
        .collect();
    disease.disgenet = Some(DiseaseDisgenet { associations });
    Ok(())
}

async fn enrich_base_context(disease: &mut Disease) {
    if let Err(err) = add_genes_section(disease).await {
        warn!("OpenTargets unavailable for disease genes context: {err}");
    }

    disease.top_genes = if disease.top_gene_scores.is_empty() {
        disease.associated_genes.iter().take(5).cloned().collect()
    } else {
        disease
            .top_gene_scores
            .iter()
            .take(5)
            .map(|row| row.symbol.clone())
            .collect()
    };

    if let Err(err) = add_treatment_landscape(disease).await {
        warn!("Drug lookup unavailable for disease treatment landscape: {err}");
    }

    if let Err(err) = add_recruiting_trial_count(disease).await {
        warn!("Trial lookup unavailable for disease recruiting count: {err}");
    }
}

async fn apply_requested_sections(
    disease: &mut Disease,
    sections: DiseaseSections,
    requested_lookup: Option<&str>,
) -> Result<(), BioMcpError> {
    if sections.include_genes {
        if let Err(err) = add_monarch_gene_section(disease).await {
            warn!("Monarch unavailable for disease genes section: {err}");
        }
        if let Err(err) = augment_genes_with_civic(disease).await {
            warn!("CIViC unavailable for disease gene augmentation: {err}");
        }
        if let Err(err) = augment_genes_with_opentargets(disease).await {
            warn!("OpenTargets unavailable for disease gene augmentation: {err}");
        }
        attach_opentargets_scores(disease);
    }
    if sections.include_pathways
        && let Err(err) = add_pathways_section(disease).await
    {
        warn!("Reactome unavailable for disease pathways section: {err}");
    }
    if sections.include_phenotypes {
        if let Err(err) = add_monarch_phenotypes(disease).await {
            warn!("Monarch unavailable for disease phenotypes section: {err}");
        }
        if let Err(err) = add_phenotypes_section(disease).await {
            warn!("HPO unavailable for disease phenotypes section: {err}");
        }
    }
    if sections.include_variants
        && let Err(err) = add_civic_variants(disease).await
    {
        warn!("CIViC unavailable for disease variants section: {err}");
    }
    if sections.include_models
        && let Err(err) = add_monarch_models(disease).await
    {
        warn!("Monarch unavailable for disease models section: {err}");
    }
    if sections.include_prevalence
        && let Err(err) = add_prevalence_section(disease).await
    {
        warn!("OpenTargets unavailable for disease prevalence section: {err}");
        disease.prevalence.clear();
        disease.prevalence_note = Some("No prevalence data available from OpenTargets.".into());
    }
    if sections.include_survival {
        add_survival_section(disease).await?;
    }
    if sections.include_funding {
        add_funding_section(disease, requested_lookup).await;
    }
    if sections.include_civic {
        add_civic_section(disease).await;
    }
    if sections.include_disgenet {
        add_disgenet_section(disease).await?;
    }

    if !sections.include_genes && !sections.include_pathways {
        disease.associated_genes.clear();
        disease.gene_associations.clear();
    }
    if !sections.include_phenotypes {
        disease.phenotypes.clear();
    }
    if !sections.include_variants {
        disease.variants.clear();
        disease.top_variant = None;
    }
    if !sections.include_models {
        disease.models.clear();
    }
    if !sections.include_prevalence {
        disease.prevalence.clear();
        disease.prevalence_note = None;
    }
    if !sections.include_survival {
        disease.survival = None;
        disease.survival_note = None;
    }
    if !sections.include_funding {
        disease.funding = None;
        disease.funding_note = None;
    }
    if !sections.include_civic {
        disease.civic = None;
    }
    if !sections.include_disgenet {
        disease.disgenet = None;
    }

    disease.key_features = transform::disease::derive_key_features(disease);

    Ok(())
}

pub async fn get(name_or_id: &str, sections: &[String]) -> Result<Disease, BioMcpError> {
    let parsed_sections = parse_sections(sections)?;
    let name_or_id = name_or_id.trim();
    if name_or_id.is_empty() {
        return Err(BioMcpError::InvalidArgument(
            "Disease name or ID is required. Example: biomcp get disease melanoma".into(),
        ));
    }
    if name_or_id.len() > 512 {
        return Err(BioMcpError::InvalidArgument(
            "Disease name/ID is too long.".into(),
        ));
    }

    let client = MyDiseaseClient::new()?;

    match parse_disease_lookup_input(name_or_id) {
        DiseaseLookupInput::CanonicalOntologyId(id) => {
            let hit = client.get(&id).await?;
            let mut disease = transform::disease::from_mydisease_hit(hit);
            if let Err(err) = enrich_sparse_disease_identity(&mut disease).await {
                warn!("OLS4 unavailable for sparse disease identity repair: {err}");
            }
            disease.parents = resolve_parent_names(&client, &disease.parents).await;
            enrich_base_context(&mut disease).await;
            apply_requested_sections(&mut disease, parsed_sections, Some(name_or_id)).await?;
            return Ok(disease);
        }
        DiseaseLookupInput::CrosswalkId(kind, value) => {
            let resp = client
                .lookup_disease_by_xref(kind.source_key(), &value, 5)
                .await?;
            let best = preferred_crosswalk_hit(resp.hits).ok_or_else(|| BioMcpError::NotFound {
                entity: "disease".into(),
                id: name_or_id.trim().to_string(),
                suggestion: "Try biomcp discover \"<disease name>\" to resolve a supported disease identifier.".into(),
            })?;
            let hit = client.get(&best.id).await?;
            let mut disease = transform::disease::from_mydisease_hit(hit);
            if let Err(err) = enrich_sparse_disease_identity(&mut disease).await {
                warn!("OLS4 unavailable for sparse disease identity repair: {err}");
            }
            disease.parents = resolve_parent_names(&client, &disease.parents).await;
            enrich_base_context(&mut disease).await;
            apply_requested_sections(&mut disease, parsed_sections, Some(name_or_id)).await?;
            return Ok(disease);
        }
        DiseaseLookupInput::FreeText => {}
    }

    let best = resolve_disease_hit_by_name(&client, name_or_id).await?;

    let hit = client.get(&best.id).await?;
    let mut disease = transform::disease::from_mydisease_hit(hit);
    if let Err(err) = enrich_sparse_disease_identity(&mut disease).await {
        warn!("OLS4 unavailable for sparse disease identity repair: {err}");
    }
    disease.parents = resolve_parent_names(&client, &disease.parents).await;
    enrich_base_context(&mut disease).await;
    apply_requested_sections(&mut disease, parsed_sections, Some(name_or_id)).await?;
    Ok(disease)
}

async fn resolve_parent_label(client: &MyDiseaseClient, parent_id: &str) -> String {
    let parent_id = parent_id.trim();
    if parent_id.is_empty() {
        return String::new();
    }

    if let Ok(hit) = client.get(parent_id).await {
        let parent_name = transform::disease::name_from_mydisease_hit(&hit);
        if !parent_name.eq_ignore_ascii_case(parent_id) {
            return format!("{parent_name} ({parent_id})");
        }
    }

    if let Ok(resp) = client.query(parent_id, 1, 0, None, None, None, None).await
        && let Some(hit) = resp.hits.first()
    {
        let parent_name = transform::disease::name_from_mydisease_hit(hit);
        if !parent_name.eq_ignore_ascii_case(parent_id) {
            return format!("{parent_name} ({parent_id})");
        }
    }

    parent_id.to_string()
}

async fn resolve_parent_names(client: &MyDiseaseClient, parents: &[String]) -> Vec<String> {
    let mut lookups = Vec::new();
    for parent in parents {
        let parent_id = parent.trim();
        if parent_id.is_empty() {
            continue;
        }
        lookups.push(async move { resolve_parent_label(client, parent_id).await });
    }
    join_all(lookups)
        .await
        .into_iter()
        .filter(|v| !v.is_empty())
        .collect()
}

fn inheritance_matches(hit: &crate::sources::mydisease::MyDiseaseHit, expected: &str) -> bool {
    let needle = expected.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return true;
    }
    hit.hpo
        .as_ref()
        .map(|hpo| {
            hpo.inheritance.iter().any(|row| {
                row.hpo_name
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|v| v.to_ascii_lowercase().contains(&needle))
                    || row
                        .hpo_id
                        .as_deref()
                        .map(str::trim)
                        .is_some_and(|v| v.to_ascii_lowercase().contains(&needle))
            })
        })
        .unwrap_or(false)
}

fn phenotype_matches(hit: &crate::sources::mydisease::MyDiseaseHit, expected: &str) -> bool {
    let needle = expected.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return true;
    }
    hit.hpo
        .as_ref()
        .map(|hpo| {
            hpo.phenotype_related_to_disease.iter().any(|row| {
                row.hpo_id
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|v| v.to_ascii_lowercase().contains(&needle))
            })
        })
        .unwrap_or(false)
}

fn onset_matches(hit: &crate::sources::mydisease::MyDiseaseHit, expected: &str) -> bool {
    let needle = expected.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return true;
    }
    hit.hpo
        .as_ref()
        .map(|hpo| {
            hpo.clinical_course.iter().any(|row| {
                row.hpo_name
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|v| v.to_ascii_lowercase().contains(&needle))
            })
        })
        .unwrap_or(false)
}

#[allow(dead_code)]
pub async fn search(
    filters: &DiseaseSearchFilters,
    limit: usize,
) -> Result<Vec<DiseaseSearchResult>, BioMcpError> {
    Ok(search_page(filters, limit, 0).await?.results)
}

pub async fn search_page(
    filters: &DiseaseSearchFilters,
    limit: usize,
    offset: usize,
) -> Result<SearchPage<DiseaseSearchResult>, BioMcpError> {
    if limit == 0 || limit > MAX_DISEASE_SEARCH_LIMIT {
        return Err(BioMcpError::InvalidArgument(format!(
            "--limit must be between 1 and {MAX_DISEASE_SEARCH_LIMIT}"
        )));
    }

    let query = filters
        .query
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| {
            BioMcpError::InvalidArgument(
                "Query is required. Example: biomcp search disease -q melanoma".into(),
            )
        })?;

    let inheritance = filters
        .inheritance
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let phenotype = filters
        .phenotype
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let onset = filters
        .onset
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());

    let client = MyDiseaseClient::new()?;
    let needed = limit.saturating_add(offset).max(limit);
    let fetch_size = if needed >= 50 {
        needed
    } else {
        (needed.saturating_mul(5)).clamp(needed, 50)
    };
    let prefer_doid = filters
        .source
        .as_deref()
        .map(str::trim)
        .is_some_and(|s| s.eq_ignore_ascii_case("doid"));

    let mut merged_total = 0usize;
    let mut query_hits = Vec::new();
    for (query_idx, resolved_query) in resolver_queries(query).into_iter().enumerate() {
        let resp = client
            .query(
                &resolved_query,
                fetch_size,
                0,
                filters.source.as_deref(),
                inheritance,
                phenotype,
                onset,
            )
            .await?;
        merged_total = merged_total.max(resp.total);
        let hits = resp
            .hits
            .into_iter()
            .filter(|hit| {
                inheritance.is_none_or(|value| inheritance_matches(hit, value))
                    && phenotype.is_none_or(|value| phenotype_matches(hit, value))
                    && onset.is_none_or(|value| onset_matches(hit, value))
            })
            .collect::<Vec<_>>();
        query_hits.push((query_idx, hits));
    }

    let ranked_hits = rerank_disease_search_hits(query, query_hits);
    let total = Some(merged_total.max(ranked_hits.len()));
    let results = ranked_hits
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|hit| {
            let mut row = transform::disease::from_mydisease_search_hit(&hit);
            if prefer_doid && let Some(doid) = transform::disease::doid_from_mydisease_hit(&hit) {
                row.id = doid;
            }
            row
        })
        .collect::<Vec<_>>();

    Ok(SearchPage::offset(results, total))
}

pub fn search_query_summary(filters: &DiseaseSearchFilters) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(v) = filters
        .query
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        parts.push(v.to_string());
    }
    if let Some(v) = filters
        .source
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        parts.push(format!("source={v}"));
    }
    if let Some(v) = filters
        .inheritance
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        parts.push(format!("inheritance={v}"));
    }
    if let Some(v) = filters
        .phenotype
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        parts.push(format!("phenotype={v}"));
    }
    if let Some(v) = filters
        .onset
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        parts.push(format!("onset={v}"));
    }
    parts.join(", ")
}

const PHENOTYPE_QUERY_EXAMPLES: &str = "Examples: biomcp search phenotype \"HP:0001250 HP:0001263\" or biomcp search phenotype \"seizure, developmental delay\"";

fn phenotype_query_required_error() -> BioMcpError {
    BioMcpError::InvalidArgument(format!(
        "Phenotype terms are required. Use HPO IDs or symptom phrases. {PHENOTYPE_QUERY_EXAMPLES}"
    ))
}

fn phenotype_query_no_match_error(raw: &str) -> BioMcpError {
    BioMcpError::InvalidArgument(format!(
        "No HPO terms matched query: {raw}. Try HPO IDs like HP:0001250 or refine the symptom phrases."
    ))
}

fn parse_hpo_query_terms(raw: &str) -> Result<Vec<String>, BioMcpError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(phenotype_query_required_error());
    }

    let mut terms = Vec::new();
    let mut seen = HashSet::new();
    for token in raw
        .split(|c: char| c.is_whitespace() || c == ',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let Some(id) = normalize_hpo_id(token) else {
            return Err(BioMcpError::InvalidArgument(format!(
                "Invalid HPO term: {token}. Expected format HP:0001250"
            )));
        };
        if seen.insert(id.clone()) {
            terms.push(id);
        }
    }

    if terms.is_empty() {
        return Err(phenotype_query_required_error());
    }

    Ok(terms)
}

fn split_phenotype_queries(raw: &str) -> Vec<String> {
    let mut queries = raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if queries.is_empty() {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            queries.push(trimmed.to_string());
        }
    }
    queries
}

async fn resolve_phenotype_query_terms(raw: &str) -> Result<Vec<String>, BioMcpError> {
    const MAX_RESOLVED_TERMS: usize = 10;

    let raw = raw.trim();
    if raw.is_empty() {
        return Err(phenotype_query_required_error());
    }

    if let Ok(terms) = parse_hpo_query_terms(raw) {
        return Ok(terms);
    }

    let queries = split_phenotype_queries(raw);
    if queries.is_empty() {
        return Err(phenotype_query_required_error());
    }

    let hpo = HpoClient::new()?;
    let mut resolved = Vec::new();
    let mut seen = HashSet::new();
    for query in queries {
        let ids = hpo.search_term_ids(&query, MAX_RESOLVED_TERMS).await?;
        for id in ids {
            if seen.insert(id.clone()) {
                resolved.push(id);
                if resolved.len() >= MAX_RESOLVED_TERMS {
                    return Ok(resolved);
                }
            }
        }
    }

    if resolved.is_empty() {
        return Err(phenotype_query_no_match_error(raw));
    }

    Ok(resolved)
}

#[allow(dead_code)]
pub async fn search_phenotype(
    hpo_terms: &str,
    limit: usize,
) -> Result<Vec<PhenotypeSearchResult>, BioMcpError> {
    Ok(search_phenotype_page(hpo_terms, limit, 0).await?.results)
}

pub async fn search_phenotype_page(
    hpo_terms: &str,
    limit: usize,
    offset: usize,
) -> Result<SearchPage<PhenotypeSearchResult>, BioMcpError> {
    const MAX_SEARCH_LIMIT: usize = 50;
    if limit == 0 || limit > MAX_SEARCH_LIMIT {
        return Err(BioMcpError::InvalidArgument(format!(
            "--limit must be between 1 and {MAX_SEARCH_LIMIT}"
        )));
    }

    let terms = resolve_phenotype_query_terms(hpo_terms).await?;
    let client = MonarchClient::new()?;
    let fetch_limit = limit.saturating_add(offset).max(limit);
    let mut rows = client
        .phenotype_similarity_search(&terms, fetch_limit)
        .await?;
    rows.sort_by(|a, b| b.score.total_cmp(&a.score));
    let total = rows.len();
    rows.truncate(fetch_limit);

    Ok(SearchPage::offset(
        rows.into_iter()
            .skip(offset)
            .take(limit)
            .map(
                |MonarchPhenotypeMatch {
                     disease_id,
                     disease_name,
                     score,
                 }| PhenotypeSearchResult {
                    disease_id,
                    disease_name,
                    score,
                },
            )
            .collect(),
        Some(total),
    ))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use wiremock::matchers::{body_string_contains, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn lock_env() -> tokio::sync::MutexGuard<'static, ()> {
        crate::test_support::env_lock().lock().await
    }

    struct EnvVarGuard {
        name: &'static str,
        previous: Option<String>,
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            // Safety: tests serialize environment mutation with `lock_env()`.
            unsafe {
                match &self.previous {
                    Some(value) => std::env::set_var(self.name, value),
                    None => std::env::remove_var(self.name),
                }
            }
        }
    }

    fn set_env_var(name: &'static str, value: Option<&str>) -> EnvVarGuard {
        let previous = std::env::var(name).ok();
        // Safety: tests serialize environment mutation with `lock_env()`.
        unsafe {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
        EnvVarGuard { name, previous }
    }

    fn test_disease(id: &str, name: &str) -> Disease {
        Disease {
            id: id.to_string(),
            name: name.to_string(),
            definition: None,
            synonyms: Vec::new(),
            parents: Vec::new(),
            associated_genes: Vec::new(),
            gene_associations: Vec::new(),
            top_genes: Vec::new(),
            top_gene_scores: Vec::new(),
            treatment_landscape: Vec::new(),
            recruiting_trial_count: None,
            pathways: Vec::new(),
            phenotypes: Vec::new(),
            key_features: Vec::new(),
            variants: Vec::new(),
            top_variant: None,
            models: Vec::new(),
            prevalence: Vec::new(),
            prevalence_note: None,
            survival: None,
            survival_note: None,
            funding: None,
            funding_note: None,
            civic: None,
            disgenet: None,
            xrefs: HashMap::new(),
        }
    }

    async fn mock_empty_monarch(server: &MockServer) {
        Mock::given(method("GET"))
            .and(path("/v3/api/association"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": []
            })))
            .mount(server)
            .await;
    }

    async fn mock_empty_civic(server: &MockServer) {
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "evidenceItems": {
                        "totalCount": 0,
                        "nodes": []
                    },
                    "assertions": {
                        "totalCount": 0,
                        "nodes": []
                    }
                }
            })))
            .mount(server)
            .await;
    }

    async fn mock_empty_mychem(server: &MockServer) {
        Mock::given(method("GET"))
            .and(path("/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "total": 0,
                "hits": []
            })))
            .mount(server)
            .await;
    }

    async fn mock_empty_ctgov(server: &MockServer) {
        Mock::given(method("GET"))
            .and(path("/studies"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "studies": [],
                "nextPageToken": null,
                "totalCount": 0
            })))
            .mount(server)
            .await;
    }

    async fn mock_seer_catalog(server: &MockServer) {
        Mock::given(method("GET"))
            .and(path("/get_var_formats.php"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "VariableFormats": {
                    "site": {
                        "1": "All Cancer Sites Combined",
                        "83": "Hodgkin Lymphoma",
                        "97": "Chronic Myeloid Leukemia (CML)"
                    },
                    "sex": {
                        "1": "Both Sexes",
                        "2": "Male",
                        "3": "Female"
                    },
                    "race": {
                        "1": "All Races / Ethnicities"
                    },
                    "age_range": {
                        "1": "All Ages"
                    }
                },
                "CancerSites": [
                    {"value": 1, "active": true},
                    {"value": 83, "active": true},
                    {"value": 97, "active": true}
                ]
            })))
            .mount(server)
            .await;
    }

    #[test]
    fn normalize_disease_id_basic() {
        assert_eq!(
            normalize_disease_id("MONDO:0005105"),
            Some("MONDO:0005105".into())
        );
        assert_eq!(
            normalize_disease_id("mondo:0005105"),
            Some("MONDO:0005105".into())
        );
        assert_eq!(
            normalize_disease_id(" DOID:1909 "),
            Some("DOID:1909".into())
        );
        assert_eq!(normalize_disease_id("lung cancer"), None);
        assert_eq!(normalize_disease_id("MONDO:"), None);
        assert_eq!(normalize_disease_id("HP:0002861"), None);
    }

    #[test]
    fn parse_disease_lookup_input_distinguishes_canonical_crosswalk_and_text() {
        assert_eq!(
            parse_disease_lookup_input("MONDO:0005105"),
            DiseaseLookupInput::CanonicalOntologyId("MONDO:0005105".into())
        );
        assert_eq!(
            parse_disease_lookup_input("mesh:D008545"),
            DiseaseLookupInput::CrosswalkId(DiseaseXrefKind::Mesh, "D008545".into())
        );
        assert_eq!(
            parse_disease_lookup_input("OMIM:155600"),
            DiseaseLookupInput::CrosswalkId(DiseaseXrefKind::Omim, "155600".into())
        );
        assert_eq!(
            parse_disease_lookup_input("ICD10CM:Q07.0"),
            DiseaseLookupInput::CrosswalkId(DiseaseXrefKind::Icd10Cm, "Q07.0".into())
        );
        assert_eq!(
            parse_disease_lookup_input("Arnold Chiari syndrome"),
            DiseaseLookupInput::FreeText
        );
    }

    #[test]
    fn preferred_crosswalk_hit_prefers_mondo_then_doid_then_lexicographic_id() {
        let best = preferred_crosswalk_hit(vec![
            test_disease_hit("DOID:1909", "melanoma", &[], &[]),
            test_disease_hit("MONDO:0005105", "melanoma", &[], &[]),
            test_disease_hit("MESH:D008545", "melanoma", &[], &[]),
        ])
        .expect("a best hit should be selected");
        assert_eq!(best.id, "MONDO:0005105");
    }

    #[test]
    fn parse_sections_supports_new_disease_sections() {
        let flags = parse_sections(&[
            "phenotypes".to_string(),
            "variants".to_string(),
            "models".to_string(),
            "prevalence".to_string(),
            "survival".to_string(),
            "funding".to_string(),
            "disgenet".to_string(),
            "all".to_string(),
        ])
        .expect("sections should parse");
        assert!(flags.include_genes);
        assert!(flags.include_pathways);
        assert!(flags.include_phenotypes);
        assert!(flags.include_variants);
        assert!(flags.include_models);
        assert!(flags.include_prevalence);
        assert!(flags.include_survival);
        assert!(flags.include_funding);
        assert!(flags.include_civic);
        assert!(flags.include_disgenet);
    }

    #[test]
    fn parse_sections_all_keeps_disgenet_opt_in() {
        let flags = parse_sections(&["all".to_string()]).expect("sections should parse");
        assert!(flags.include_survival);
        assert!(!flags.include_funding);
        assert!(!flags.include_disgenet);
    }

    #[test]
    fn funding_query_prefers_free_text_lookup() {
        let disease = test_disease(
            "MONDO:0011996",
            "chronic myelogenous leukemia, BCR-ABL1 positive",
        );

        assert_eq!(
            disease_funding_query_value(&disease, Some("chronic myeloid leukemia")),
            Some("chronic myeloid leukemia".to_string())
        );
    }

    #[test]
    fn funding_query_uses_canonical_name_for_identifier_lookups() {
        let disease = test_disease(
            "MONDO:0011996",
            "chronic myelogenous leukemia, BCR-ABL1 positive",
        );

        assert_eq!(
            disease_funding_query_value(&disease, Some("MONDO:0011996")),
            Some("chronic myelogenous leukemia, BCR-ABL1 positive".to_string())
        );
    }

    #[tokio::test]
    async fn apply_requested_sections_clears_funding_when_not_requested() {
        let mut disease = test_disease("MONDO:0007947", "Marfan syndrome");
        disease.funding = Some(empty_funding_section("Marfan syndrome".to_string()));
        disease.funding_note = Some(FUNDING_NO_DATA_NOTE.to_string());

        apply_requested_sections(&mut disease, DiseaseSections::default(), None)
            .await
            .expect("sections should apply");

        assert!(disease.funding.is_none());
        assert!(disease.funding_note.is_none());
    }

    #[test]
    fn resolver_queries_adds_cml_fallback_variant() {
        let queries = resolver_queries("chronic myeloid leukemia");
        assert!(
            queries
                .iter()
                .any(|query| query == "chronic myelogenous leukemia")
        );
        assert!(
            queries
                .iter()
                .any(|query| query == "chronic myelogenous leukemia, bcr-abl1 positive")
        );
    }

    #[test]
    fn resolver_queries_adds_hodgkin_alias_variants() {
        let queries = resolver_queries("Hodgkin lymphoma");
        assert!(queries.iter().any(|query| query == "hodgkins lymphoma"));
        assert!(queries.iter().any(|query| query == "hodgkin disease"));
    }

    #[tokio::test]
    async fn add_survival_section_sets_truthful_note_for_unmapped_disease() {
        let _lock = lock_env().await;
        let server = MockServer::start().await;
        mock_seer_catalog(&server).await;
        let _seer_base = set_env_var("BIOMCP_SEER_BASE", Some(&server.uri()));

        let mut disease = test_disease("MONDO:0007947", "Marfan syndrome");
        add_survival_section(&mut disease)
            .await
            .expect("survival section");

        assert!(disease.survival.is_none());
        assert_eq!(
            disease.survival_note.as_deref(),
            Some(SURVIVAL_NO_DATA_NOTE)
        );
    }

    #[tokio::test]
    async fn add_survival_section_sets_unavailable_note_when_catalog_fails() {
        let _lock = lock_env().await;
        let server = MockServer::start().await;
        let _seer_base = set_env_var("BIOMCP_SEER_BASE", Some(&server.uri()));

        Mock::given(method("GET"))
            .and(path("/get_var_formats.php"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let mut disease = test_disease("MONDO:0004952", "Hodgkin's lymphoma");
        add_survival_section(&mut disease)
            .await
            .expect("survival section");

        assert!(disease.survival.is_none());
        assert_eq!(
            disease.survival_note.as_deref(),
            Some(SURVIVAL_UNAVAILABLE_NOTE)
        );
    }

    #[tokio::test]
    async fn resolve_disease_hit_by_name_direct_rejects_weak_contains_only_match() {
        let _guard = lock_env().await;
        let server = MockServer::start().await;
        let _env = set_env_var(
            "BIOMCP_MYDISEASE_BASE",
            Some(&format!("{}/v1", server.uri())),
        );

        Mock::given(method("GET"))
            .and(path("/v1/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "total": 1,
                "hits": [{
                    "_id": "MONDO:0015760",
                    "mondo": {"name": "T-cell non-Hodgkin lymphoma"}
                }]
            })))
            .mount(&server)
            .await;

        let client = MyDiseaseClient::new().expect("client");
        let best = resolve_disease_hit_by_name_direct(&client, "Hodgkin lymphoma")
            .await
            .expect("weak direct match should not error");

        assert!(best.is_none());
    }

    #[test]
    fn parse_hpo_query_terms_requires_valid_ids() {
        let parsed = parse_hpo_query_terms("HP:0001250 HP:0001263").expect("valid terms");
        assert_eq!(parsed, vec!["HP:0001250", "HP:0001263"]);
        let comma_separated = parse_hpo_query_terms("hp:0001250, HP:0001263").expect("comma terms");
        assert_eq!(comma_separated, vec!["HP:0001250", "HP:0001263"]);
        assert!(parse_hpo_query_terms("NOT_AN_HPO").is_err());
    }

    #[test]
    fn split_phenotype_queries_preserves_single_phrase_and_splits_commas() {
        assert_eq!(
            split_phenotype_queries("developmental delay"),
            vec!["developmental delay"]
        );
        assert_eq!(
            split_phenotype_queries("seizure, developmental delay,  hypotonia "),
            vec!["seizure", "developmental delay", "hypotonia"]
        );
    }

    #[tokio::test]
    async fn resolve_phenotype_query_terms_empty_input_mentions_hpo_ids_and_symptom_phrases() {
        let err = resolve_phenotype_query_terms("   ")
            .await
            .expect_err("empty phenotype query should fail");

        match err {
            BioMcpError::InvalidArgument(message) => {
                assert!(message.contains("Use HPO IDs or symptom phrases"));
                assert!(message.contains("HP:0001250 HP:0001263"));
                assert!(message.contains("seizure, developmental delay"));
            }
            other => panic!("expected InvalidArgument, got: {other}"),
        }
    }

    #[test]
    fn disease_candidate_score_prefers_canonical_colorectal_match_over_subtype() {
        let broad = disease_candidate_score("colorectal cancer", "colorectal carcinoma");
        let subtype = disease_candidate_score(
            "colorectal cancer",
            "hereditary nonpolyposis colorectal cancer type 6",
        );
        assert!(broad > subtype);
    }

    #[test]
    fn scored_best_candidate_for_queries_prefers_hodgkin_alias_over_non_hodgkin_contains_match() {
        let queries = resolver_queries("Hodgkin lymphoma");
        let best = scored_best_candidate_for_queries(
            &queries,
            vec![
                test_disease_hit("MONDO:0015760", "T-cell non-Hodgkin lymphoma", &[], &[]),
                test_disease_hit(
                    "MONDO:0004952",
                    "Hodgkins lymphoma",
                    &["Hodgkin disease"],
                    &[],
                ),
            ],
        )
        .expect("a best hit should be selected");

        assert_eq!(best.id, "MONDO:0004952");
    }

    fn test_disease_hit(
        id: &str,
        disease_name: &str,
        mondo_synonyms: &[&str],
        do_synonyms: &[&str],
    ) -> MyDiseaseHit {
        serde_json::from_value(serde_json::json!({
            "_id": id,
            "mondo": {
                "name": disease_name,
                "synonym": mondo_synonyms,
            },
            "disease_ontology": {
                "name": disease_name,
                "synonyms": do_synonyms,
            }
        }))
        .expect("valid disease hit")
    }

    #[test]
    fn rerank_disease_search_hits_prefers_canonical_exact_candidate_across_query_variants() {
        let canonical = test_disease_hit(
            "MONDO:0024331",
            "colorectal carcinoma",
            &["colorectal cancer"],
            &["colorectal cancer"],
        );

        let ranked = rerank_disease_search_hits(
            "colorectal cancer",
            vec![
                (
                    0,
                    vec![test_disease_hit(
                        "MONDO:0101010",
                        "hereditary nonpolyposis colorectal cancer type 6",
                        &[],
                        &[],
                    )],
                ),
                (
                    1,
                    vec![
                        canonical,
                        test_disease_hit(
                            "MONDO:0101010",
                            "hereditary nonpolyposis colorectal cancer type 6",
                            &[],
                            &[],
                        ),
                    ],
                ),
            ],
        );

        let ids = ranked.iter().map(|hit| hit.id.as_str()).collect::<Vec<_>>();
        assert_eq!(ids, vec!["MONDO:0024331", "MONDO:0101010"]);
    }

    fn test_discover_disease_concept(
        label: &str,
        primary_id: Option<&str>,
        synonyms: &[&str],
        xrefs: &[(&str, &str)],
        match_tier: crate::entities::discover::MatchTier,
        confidence: crate::entities::discover::DiscoverConfidence,
    ) -> crate::entities::discover::DiscoverConcept {
        crate::entities::discover::DiscoverConcept {
            label: label.to_string(),
            primary_id: primary_id.map(str::to_string),
            primary_type: crate::entities::discover::DiscoverType::Disease,
            synonyms: synonyms.iter().map(|value| (*value).to_string()).collect(),
            xrefs: xrefs
                .iter()
                .map(|(source, id)| crate::entities::discover::ConceptXref {
                    source: (*source).to_string(),
                    id: (*id).to_string(),
                })
                .collect(),
            sources: Vec::new(),
            match_tier,
            confidence,
        }
    }

    #[test]
    fn fallback_candidates_rank_specific_crosswalkable_disease_ahead_of_generic_rows() {
        let candidates = rank_disease_fallback_candidates(
            "Arnold Chiari syndrome",
            &[
                test_discover_disease_concept(
                    "syndromic disease",
                    Some("UMLS:C0039082"),
                    &[],
                    &[("OMIM", "607208")],
                    crate::entities::discover::MatchTier::Exact,
                    crate::entities::discover::DiscoverConfidence::CanonicalId,
                ),
                test_discover_disease_concept(
                    "Arnold-Chiari malformation",
                    Some("MESH:D001139"),
                    &["Arnold Chiari syndrome"],
                    &[("MESH", "D001139"), ("OMIM", "207950")],
                    crate::entities::discover::MatchTier::Contains,
                    crate::entities::discover::DiscoverConfidence::CanonicalId,
                ),
            ],
        );

        assert_eq!(candidates[0].label, "Arnold-Chiari malformation");
        assert_eq!(
            candidates[0].source_ids[0],
            DiseaseFallbackId::Crosswalk(DiseaseXrefKind::Mesh, "D001139".into())
        );
    }

    #[test]
    fn fallback_candidate_source_ids_prefer_primary_then_ranked_xrefs() {
        let candidate = rank_disease_fallback_candidates(
            "Arnold Chiari syndrome",
            &[test_discover_disease_concept(
                "Arnold-Chiari malformation",
                Some("OMIM:207950"),
                &[],
                &[
                    ("ICD10CM", "Q07.0"),
                    ("MESH", "D001139"),
                    ("OMIM", "207950"),
                ],
                crate::entities::discover::MatchTier::Exact,
                crate::entities::discover::DiscoverConfidence::CanonicalId,
            )],
        );

        assert_eq!(
            candidate[0].source_ids,
            vec![
                DiseaseFallbackId::Crosswalk(DiseaseXrefKind::Mesh, "D001139".into()),
                DiseaseFallbackId::Crosswalk(DiseaseXrefKind::Omim, "207950".into()),
                DiseaseFallbackId::Crosswalk(DiseaseXrefKind::Icd10Cm, "Q07.0".into())
            ]
        );
    }

    #[test]
    fn fallback_rows_dedupe_by_resolved_disease_id() {
        let mut seen = HashSet::new();
        let rows = [
            DiseaseSearchResult {
                id: "MONDO:0000115".into(),
                name: "Arnold-Chiari malformation".into(),
                synonyms_preview: None,
                resolved_via: Some("MESH crosswalk".into()),
                source_id: Some("MESH:D001139".into()),
            },
            DiseaseSearchResult {
                id: "MONDO:0000115".into(),
                name: "Arnold-Chiari malformation".into(),
                synonyms_preview: None,
                resolved_via: Some("OMIM crosswalk".into()),
                source_id: Some("OMIM:207950".into()),
            },
        ];

        let deduped = rows
            .into_iter()
            .filter(|row| seen.insert(row.id.clone()))
            .collect::<Vec<_>>();

        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].source_id.as_deref(), Some("MESH:D001139"));
    }

    #[tokio::test]
    async fn fallback_search_page_swallows_discover_errors() {
        let candidates = rank_disease_fallback_candidates(
            "Arnold Chiari syndrome",
            &[test_discover_disease_concept(
                "Arnold-Chiari malformation",
                Some("MESH:D001139"),
                &["Arnold Chiari syndrome"],
                &[("MESH", "D001139")],
                crate::entities::discover::MatchTier::Exact,
                crate::entities::discover::DiscoverConfidence::CanonicalId,
            )],
        );

        let page = collect_fallback_search_page(
            "Arnold Chiari syndrome",
            10,
            0,
            candidates,
            |_source_id| async {
                Err(BioMcpError::Api {
                    api: "mydisease.info".into(),
                    message: "lookup failed".into(),
                })
            },
        )
        .await
        .expect("fallback should degrade to no rows");

        assert!(page.is_none());
    }

    #[tokio::test]
    async fn fallback_search_page_applies_offset_and_limit_after_dedupe() {
        let candidates = rank_disease_fallback_candidates(
            "Arnold Chiari syndrome",
            &[
                test_discover_disease_concept(
                    "Arnold-Chiari malformation",
                    Some("MESH:D001139"),
                    &["Arnold Chiari syndrome"],
                    &[("MESH", "D001139")],
                    crate::entities::discover::MatchTier::Exact,
                    crate::entities::discover::DiscoverConfidence::CanonicalId,
                ),
                test_discover_disease_concept(
                    "Arnold Chiari syndrome",
                    Some("OMIM:207950"),
                    &[],
                    &[("OMIM", "207950")],
                    crate::entities::discover::MatchTier::Exact,
                    crate::entities::discover::DiscoverConfidence::CanonicalId,
                ),
                test_discover_disease_concept(
                    "Chiari malformation type II",
                    Some("ICD10CM:Q07.0"),
                    &["Arnold Chiari syndrome"],
                    &[("ICD10CM", "Q07.0")],
                    crate::entities::discover::MatchTier::Contains,
                    crate::entities::discover::DiscoverConfidence::CanonicalId,
                ),
            ],
        );

        let page = collect_fallback_search_page(
            "Arnold Chiari syndrome",
            1,
            1,
            candidates,
            |source_id| async move {
                let row = match source_id {
                    DiseaseFallbackId::Crosswalk(DiseaseXrefKind::Mesh, value)
                        if value == "D001139" =>
                    {
                        DiseaseSearchResult {
                            id: "MONDO:0000115".into(),
                            name: "Arnold-Chiari malformation".into(),
                            synonyms_preview: None,
                            resolved_via: Some("MESH crosswalk".into()),
                            source_id: Some("MESH:D001139".into()),
                        }
                    }
                    DiseaseFallbackId::Crosswalk(DiseaseXrefKind::Omim, value)
                        if value == "207950" =>
                    {
                        DiseaseSearchResult {
                            id: "MONDO:0000115".into(),
                            name: "Arnold-Chiari malformation".into(),
                            synonyms_preview: None,
                            resolved_via: Some("OMIM crosswalk".into()),
                            source_id: Some("OMIM:207950".into()),
                        }
                    }
                    DiseaseFallbackId::Crosswalk(DiseaseXrefKind::Icd10Cm, value)
                        if value == "Q07.0" =>
                    {
                        DiseaseSearchResult {
                            id: "MONDO:0002115".into(),
                            name: "Chiari malformation type II".into(),
                            synonyms_preview: None,
                            resolved_via: Some("ICD10CM crosswalk".into()),
                            source_id: Some("ICD10CM:Q07.0".into()),
                        }
                    }
                    other => panic!("unexpected source id: {other:?}"),
                };
                Ok(Some(row))
            },
        )
        .await
        .expect("fallback page should build")
        .expect("offset page should retain remaining unique rows");

        assert_eq!(page.total, Some(2));
        assert_eq!(page.results.len(), 1);
        assert_eq!(page.results[0].id, "MONDO:0002115");
        assert_eq!(page.results[0].source_id.as_deref(), Some("ICD10CM:Q07.0"));
    }

    #[tokio::test]
    async fn resolve_fallback_row_ignores_not_found_canonical_ids() {
        let _guard = lock_env().await;
        let server = MockServer::start().await;
        let _env = set_env_var(
            "BIOMCP_MYDISEASE_BASE",
            Some(&format!("{}/v1", server.uri())),
        );

        Mock::given(method("GET"))
            .and(path("/v1/disease/DOID:8552"))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&server)
            .await;

        let client = MyDiseaseClient::new().expect("client");
        let row = resolve_fallback_row(
            &client,
            false,
            &DiseaseFallbackId::CanonicalOntology("DOID:8552".into()),
        )
        .await
        .expect("canonical not-found should degrade cleanly");

        assert!(row.is_none());
    }

    #[test]
    fn disease_exact_rank_prefers_exact_then_prefix_then_contains() {
        assert!(
            disease_exact_rank("colorectal cancer", "colorectal cancer")
                > disease_exact_rank("colorectal cancer syndrome", "colorectal cancer")
        );
        assert!(
            disease_exact_rank("colorectal cancer syndrome", "colorectal cancer")
                > disease_exact_rank("metastatic colorectal cancer", "colorectal cancer")
        );
    }

    #[test]
    fn resolver_queries_adds_carcinoma_fallback_for_cancer_terms() {
        let queries = resolver_queries("breast cancer");
        assert!(queries.iter().any(|q| q == "breast cancer"));
        assert!(queries.iter().any(|q| q == "breast carcinoma"));
    }

    #[tokio::test]
    async fn get_disease_preserves_canonical_mondo_lookup_path() {
        let _guard = lock_env().await;
        let server = MockServer::start().await;
        let _env = set_env_var(
            "BIOMCP_MYDISEASE_BASE",
            Some(&format!("{}/v1", server.uri())),
        );

        let body = r#"{
          "_id": "MONDO:0005105",
          "mondo": {
            "name": "melanoma",
            "definition": "Example disease."
          }
        }"#;

        Mock::given(method("GET"))
            .and(path("/v1/disease/MONDO:0005105"))
            .and(query_param(
                "fields",
                "_id,mondo.name,mondo.definition,mondo.parents,mondo.synonym,mondo.xrefs,disease_ontology.name,disease_ontology.doid,disease_ontology.def,disease_ontology.parents,disease_ontology.synonyms,disease_ontology.xrefs,umls.mesh,umls.nci,umls.snomed,umls.icd10am,disgenet.genes_related_to_disease,hpo.phenotype_related_to_disease.hpo_id,hpo.phenotype_related_to_disease.evidence,hpo.phenotype_related_to_disease.hp_freq,hpo.inheritance.hpo_id",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
            .expect(1)
            .mount(&server)
            .await;

        let disease = get("MONDO:0005105", &[])
            .await
            .expect("canonical get should resolve");
        assert_eq!(disease.id, "MONDO:0005105");
        assert_eq!(disease.name, "melanoma");
    }

    #[tokio::test]
    async fn get_disease_resolves_mesh_and_omim_crosswalk_ids_before_fetch() {
        let _guard = lock_env().await;
        let server = MockServer::start().await;
        let _env = set_env_var(
            "BIOMCP_MYDISEASE_BASE",
            Some(&format!("{}/v1", server.uri())),
        );

        let melanoma_get = r#"{
          "_id": "MONDO:0005105",
          "mondo": {
            "name": "melanoma",
            "definition": "Example disease."
          },
          "disease_ontology": {
            "name": "melanoma"
          }
        }"#;
        let marfan_get = r#"{
          "_id": "MONDO:0007947",
          "mondo": {
            "name": "Marfan syndrome",
            "definition": "Example syndrome."
          },
          "disease_ontology": {
            "name": "Marfan syndrome"
          }
        }"#;

        Mock::given(method("GET"))
            .and(path("/v1/query"))
            .and(query_param(
                "q",
                "(mondo.xrefs.mesh:\"D008545\" OR disease_ontology.xrefs.mesh:\"D008545\" OR umls.mesh:\"D008545\")",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{"total":2,"hits":[{"_id":"DOID:1909","disease_ontology":{"name":"melanoma"}},{"_id":"MONDO:0005105","mondo":{"name":"melanoma"}}]}"#,
                "application/json",
            ))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/v1/query"))
            .and(query_param(
                "q",
                "(mondo.xrefs.omim:\"154700\" OR disease_ontology.xrefs.omim:\"154700\")",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{"total":1,"hits":[{"_id":"MONDO:0007947","mondo":{"name":"Marfan syndrome"}}]}"#,
                "application/json",
            ))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/v1/disease/MONDO:0005105"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(melanoma_get, "application/json"))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/v1/disease/MONDO:0007947"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(marfan_get, "application/json"))
            .expect(1)
            .mount(&server)
            .await;

        let mesh = get("MESH:D008545", &[])
            .await
            .expect("mesh crosswalk should resolve");
        assert_eq!(mesh.id, "MONDO:0005105");
        assert_eq!(mesh.name, "melanoma");

        let omim = get("OMIM:154700", &[])
            .await
            .expect("omim crosswalk should resolve");
        assert_eq!(omim.id, "MONDO:0007947");
        assert_eq!(omim.name, "Marfan syndrome");
    }

    #[tokio::test]
    async fn get_disease_returns_not_found_for_unresolved_crosswalk_without_name_fallback() {
        let _guard = lock_env().await;
        let server = MockServer::start().await;
        let _env = set_env_var(
            "BIOMCP_MYDISEASE_BASE",
            Some(&format!("{}/v1", server.uri())),
        );

        Mock::given(method("GET"))
            .and(path("/v1/query"))
            .and(query_param(
                "q",
                "(mondo.xrefs.omim:\"000000\" OR disease_ontology.xrefs.omim:\"000000\")",
            ))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_raw(r#"{"total":0,"hits":[]}"#, "application/json"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let err = get("OMIM:000000", &[])
            .await
            .expect_err("unresolved crosswalk should return not found");
        match err {
            BioMcpError::NotFound {
                entity,
                id,
                suggestion,
            } => {
                assert_eq!(entity, "disease");
                assert_eq!(id, "OMIM:000000");
                assert!(suggestion.contains("biomcp discover"));
            }
            other => panic!("expected not found, got {other:?}"),
        }
    }

    #[test]
    fn civic_gene_symbol_extraction_ignores_protein_change_tokens() {
        assert_eq!(
            civic_gene_symbol_from_profile("BRAF V600E").as_deref(),
            Some("BRAF")
        );
        assert_eq!(
            civic_gene_symbol_from_profile("V600E BRAF").as_deref(),
            Some("BRAF")
        );
        assert_eq!(civic_gene_symbol_from_profile("V600E"), None);
    }

    pub(crate) async fn proof_augment_genes_with_opentargets_merges_sources_without_duplicates() {
        let mut disease = test_disease("MONDO:0003864", "chronic lymphocytic leukemia");
        disease.associated_genes = vec!["TP53".into(), "BCL2".into()];
        disease.gene_associations = vec![
            DiseaseGeneAssociation {
                gene: "TP53".into(),
                relationship: Some("causal".into()),
                source: Some("Monarch".into()),
                opentargets_score: None,
            },
            DiseaseGeneAssociation {
                gene: "BCL2".into(),
                relationship: Some("associated with disease".into()),
                source: Some("CIViC".into()),
                opentargets_score: None,
            },
        ];
        disease.top_gene_scores = vec![
            DiseaseTargetScore {
                symbol: "TP53".into(),
                summary: DiseaseAssociationScoreSummary {
                    overall_score: 0.99,
                    gwas_score: None,
                    rare_variant_score: None,
                    somatic_mutation_score: Some(0.88),
                },
            },
            DiseaseTargetScore {
                symbol: "BCL2".into(),
                summary: DiseaseAssociationScoreSummary {
                    overall_score: 0.91,
                    gwas_score: None,
                    rare_variant_score: None,
                    somatic_mutation_score: Some(0.72),
                },
            },
            DiseaseTargetScore {
                symbol: "ATM".into(),
                summary: DiseaseAssociationScoreSummary {
                    overall_score: 0.84,
                    gwas_score: None,
                    rare_variant_score: None,
                    somatic_mutation_score: Some(0.67),
                },
            },
        ];

        augment_genes_with_opentargets(&mut disease)
            .await
            .expect("augmentation should succeed");
        attach_opentargets_scores(&mut disease);

        assert_eq!(disease.gene_associations.len(), 3);
        assert_eq!(
            disease.gene_associations[0].source.as_deref(),
            Some("Monarch; OpenTargets")
        );
        assert_eq!(
            disease.gene_associations[1].source.as_deref(),
            Some("CIViC; OpenTargets")
        );
        assert_eq!(disease.gene_associations[2].gene, "ATM");
        assert_eq!(
            disease.gene_associations[2].source.as_deref(),
            Some("OpenTargets")
        );
    }

    #[tokio::test]
    async fn augment_genes_with_opentargets_merges_sources_without_duplicates() {
        proof_augment_genes_with_opentargets_merges_sources_without_duplicates().await;
    }

    pub(crate) async fn proof_augment_genes_with_opentargets_respects_twenty_gene_cap() {
        let mut disease = test_disease("MONDO:0003864", "chronic lymphocytic leukemia");
        disease.associated_genes = (0..20).map(|index| format!("GENE{index}")).collect();
        disease.gene_associations = (0..20)
            .map(|index| DiseaseGeneAssociation {
                gene: format!("GENE{index}"),
                relationship: Some("associated".into()),
                source: Some("Monarch".into()),
                opentargets_score: None,
            })
            .collect();
        disease.top_gene_scores = vec![DiseaseTargetScore {
            symbol: "TP53".into(),
            summary: DiseaseAssociationScoreSummary {
                overall_score: 0.99,
                gwas_score: None,
                rare_variant_score: None,
                somatic_mutation_score: Some(0.88),
            },
        }];

        augment_genes_with_opentargets(&mut disease)
            .await
            .expect("augmentation should succeed");

        assert_eq!(disease.gene_associations.len(), 20);
        assert!(
            !disease
                .gene_associations
                .iter()
                .any(|row| row.gene == "TP53")
        );
        assert_eq!(disease.associated_genes.len(), 20);
    }

    #[tokio::test]
    async fn augment_genes_with_opentargets_respects_twenty_gene_cap() {
        proof_augment_genes_with_opentargets_respects_twenty_gene_cap().await;
    }

    pub(crate) async fn proof_enrich_sparse_disease_identity_prefers_exact_ols4_match() {
        let _guard = lock_env().await;
        let ols4 = MockServer::start().await;
        let _ols4_env = set_env_var("BIOMCP_OLS4_BASE", Some(&ols4.uri()));

        Mock::given(method("GET"))
            .and(path("/api/search"))
            .and(query_param("q", "MONDO:0019468"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "response": {
                    "docs": [
                        {
                            "iri": "http://purl.obolibrary.org/obo/MONDO_0019469",
                            "ontology_name": "mondo",
                            "ontology_prefix": "mondo",
                            "short_form": "MONDO_0019469",
                            "obo_id": "MONDO:0019469",
                            "label": "wrong disease",
                            "description": [],
                            "exact_synonyms": ["Wrong"],
                            "type": "class"
                        },
                        {
                            "iri": "http://purl.obolibrary.org/obo/MONDO_0019468",
                            "ontology_name": "mondo",
                            "ontology_prefix": "mondo",
                            "short_form": "MONDO_0019468",
                            "obo_id": "MONDO:0019468",
                            "label": "T-cell prolymphocytic leukemia",
                            "description": [],
                            "exact_synonyms": ["T-PLL"],
                            "type": "class"
                        }
                    ]
                }
            })))
            .mount(&ols4)
            .await;

        let mut disease = test_disease("MONDO:0019468", "MONDO:0019468");
        enrich_sparse_disease_identity(&mut disease)
            .await
            .expect("identity repair should succeed");

        assert_eq!(disease.name, "T-cell prolymphocytic leukemia");
        assert_eq!(disease.synonyms, vec!["T-PLL".to_string()]);
    }

    #[tokio::test]
    async fn enrich_sparse_disease_identity_prefers_exact_ols4_match() {
        proof_enrich_sparse_disease_identity_prefers_exact_ols4_match().await;
    }

    pub(crate) async fn proof_get_disease_genes_promotes_opentargets_rows_for_cll() {
        let _guard = lock_env().await;
        let mydisease = MockServer::start().await;
        let opentargets = MockServer::start().await;
        let monarch = MockServer::start().await;
        let civic = MockServer::start().await;
        let mychem = MockServer::start().await;
        let ctgov = MockServer::start().await;
        let _mydisease_env = set_env_var(
            "BIOMCP_MYDISEASE_BASE",
            Some(&format!("{}/v1", mydisease.uri())),
        );
        let _opentargets_env = set_env_var("BIOMCP_OPENTARGETS_BASE", Some(&opentargets.uri()));
        let _monarch_env = set_env_var("BIOMCP_MONARCH_BASE", Some(&monarch.uri()));
        let _civic_env = set_env_var("BIOMCP_CIVIC_BASE", Some(&civic.uri()));
        let _mychem_env = set_env_var("BIOMCP_MYCHEM_BASE", Some(&format!("{}/v1", mychem.uri())));
        let _ctgov_env = set_env_var(
            "BIOMCP_CTGOV_BASE",
            Some(&format!("{}/api/v2", ctgov.uri())),
        );

        Mock::given(method("GET"))
            .and(path("/v1/disease/MONDO:0003864"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "_id": "MONDO:0003864",
                "mondo": {
                    "name": "chronic lymphocytic leukemia",
                    "synonym": ["CLL"]
                },
                "disease_ontology": {
                    "name": "chronic lymphocytic leukemia"
                }
            })))
            .mount(&mydisease)
            .await;

        Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(body_string_contains("SearchDisease"))
            .and(body_string_contains("\"query\":\"chronic lymphocytic leukemia\""))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "search": {
                        "hits": [
                            {"id": "EFO_0000095", "name": "chronic lymphocytic leukemia", "entity": "disease"}
                        ]
                    }
                }
            })))
            .mount(&opentargets)
            .await;

        Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(body_string_contains("DiseaseGenes"))
            .and(body_string_contains("\"efoId\":\"EFO_0000095\""))
            .and(body_string_contains("\"size\":20"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "disease": {
                        "associatedTargets": {
                            "rows": [
                                {
                                    "score": 0.99,
                                    "datatypeScores": [{"id": "somatic_mutation", "score": 0.88}],
                                    "datasourceScores": [],
                                    "target": {"approvedSymbol": "TP53"}
                                },
                                {
                                    "score": 0.94,
                                    "datatypeScores": [{"id": "somatic_mutation", "score": 0.71}],
                                    "datasourceScores": [],
                                    "target": {"approvedSymbol": "ATM"}
                                },
                                {
                                    "score": 0.91,
                                    "datatypeScores": [{"id": "somatic_mutation", "score": 0.69}],
                                    "datasourceScores": [],
                                    "target": {"approvedSymbol": "NOTCH1"}
                                },
                                {
                                    "score": 0.89,
                                    "datatypeScores": [{"id": "somatic_mutation", "score": 0.66}],
                                    "datasourceScores": [],
                                    "target": {"approvedSymbol": "XPO1"}
                                },
                                {
                                    "score": 0.86,
                                    "datatypeScores": [{"id": "somatic_mutation", "score": 0.62}],
                                    "datasourceScores": [],
                                    "target": {"approvedSymbol": "MYD88"}
                                },
                                {
                                    "score": 0.85,
                                    "datatypeScores": [{"id": "somatic_mutation", "score": 0.61}],
                                    "datasourceScores": [],
                                    "target": {"approvedSymbol": "SF3B1"}
                                },
                                {
                                    "score": 0.82,
                                    "datatypeScores": [{"id": "somatic_mutation", "score": 0.58}],
                                    "datasourceScores": [],
                                    "target": {"approvedSymbol": "FBXW7"}
                                },
                                {
                                    "score": 0.81,
                                    "datatypeScores": [{"id": "somatic_mutation", "score": 0.57}],
                                    "datasourceScores": [],
                                    "target": {"approvedSymbol": "BCL2"}
                                }
                            ]
                        }
                    }
                }
            })))
            .mount(&opentargets)
            .await;

        mock_empty_monarch(&monarch).await;

        Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(body_string_contains("CivicContext"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "evidenceItems": {
                        "totalCount": 1,
                        "nodes": [
                            {
                                "id": 1,
                                "name": "BCL2 evidence",
                                "status": "ACCEPTED",
                                "evidenceType": "PREDICTIVE",
                                "evidenceLevel": "A",
                                "significance": "SUPPORTS",
                                "molecularProfile": {"name": "BCL2 amplification"},
                                "disease": {"displayName": "chronic lymphocytic leukemia"},
                                "therapies": [],
                                "source": {
                                    "citation": "PMID:1",
                                    "sourceType": "PUBMED",
                                    "publicationYear": 2024
                                }
                            }
                        ]
                    },
                    "assertions": {
                        "totalCount": 0,
                        "nodes": []
                    }
                }
            })))
            .mount(&civic)
            .await;

        mock_empty_mychem(&mychem).await;
        mock_empty_ctgov(&ctgov).await;

        let disease = get("MONDO:0003864", &["genes".to_string()])
            .await
            .expect("CLL should resolve");

        let genes = disease
            .gene_associations
            .iter()
            .map(|row| row.gene.as_str())
            .collect::<Vec<_>>();
        let cll_gold = [
            "TP53", "ATM", "NOTCH1", "XPO1", "MYD88", "SF3B1", "FBXW7", "BCL2",
        ];
        let matched = cll_gold.iter().filter(|gene| genes.contains(gene)).count();
        assert!(
            matched >= 8,
            "expected >=8 CLL gold genes, got {matched}: {genes:?}"
        );
        assert!(disease.gene_associations.iter().any(|row| {
            row.gene == "TP53"
                && row.source.as_deref() == Some("OpenTargets")
                && row.opentargets_score.is_some()
        }));
        assert!(disease.gene_associations.iter().any(|row| {
            row.gene == "BCL2"
                && row.source.as_deref() == Some("CIViC; OpenTargets")
                && row.opentargets_score.is_some()
        }));
    }

    #[tokio::test]
    async fn get_disease_genes_promotes_opentargets_rows_for_cll() {
        proof_get_disease_genes_promotes_opentargets_rows_for_cll().await;
    }

    pub(crate) async fn proof_get_disease_genes_uses_ols4_label_fallback_for_sparse_mondo_identity()
    {
        let _guard = lock_env().await;
        let mydisease = MockServer::start().await;
        let opentargets = MockServer::start().await;
        let monarch = MockServer::start().await;
        let civic = MockServer::start().await;
        let ols4 = MockServer::start().await;
        let mychem = MockServer::start().await;
        let ctgov = MockServer::start().await;
        let _mydisease_env = set_env_var(
            "BIOMCP_MYDISEASE_BASE",
            Some(&format!("{}/v1", mydisease.uri())),
        );
        let _opentargets_env = set_env_var("BIOMCP_OPENTARGETS_BASE", Some(&opentargets.uri()));
        let _monarch_env = set_env_var("BIOMCP_MONARCH_BASE", Some(&monarch.uri()));
        let _civic_env = set_env_var("BIOMCP_CIVIC_BASE", Some(&civic.uri()));
        let _ols4_env = set_env_var("BIOMCP_OLS4_BASE", Some(&ols4.uri()));
        let _mychem_env = set_env_var("BIOMCP_MYCHEM_BASE", Some(&format!("{}/v1", mychem.uri())));
        let _ctgov_env = set_env_var(
            "BIOMCP_CTGOV_BASE",
            Some(&format!("{}/api/v2", ctgov.uri())),
        );

        Mock::given(method("GET"))
            .and(path("/v1/disease/MONDO:0019468"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "_id": "MONDO:0019468",
                "mondo": {
                    "name": "MONDO:0019468"
                }
            })))
            .mount(&mydisease)
            .await;

        Mock::given(method("GET"))
            .and(path("/api/search"))
            .and(query_param("q", "MONDO:0019468"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "response": {
                    "docs": [
                        {
                            "iri": "http://purl.obolibrary.org/obo/MONDO_0019468",
                            "ontology_name": "mondo",
                            "ontology_prefix": "mondo",
                            "short_form": "MONDO_0019468",
                            "obo_id": "MONDO:0019468",
                            "label": "T-cell prolymphocytic leukemia",
                            "description": [],
                            "exact_synonyms": ["T-PLL"],
                            "type": "class"
                        }
                    ]
                }
            })))
            .mount(&ols4)
            .await;

        Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(body_string_contains("SearchDisease"))
            .and(body_string_contains("\"query\":\"T-cell prolymphocytic leukemia\""))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "search": {
                        "hits": [
                            {"id": "EFO_1000560", "name": "T-cell prolymphocytic leukemia", "entity": "disease"}
                        ]
                    }
                }
            })))
            .expect(1)
            .mount(&opentargets)
            .await;

        Mock::given(method("POST"))
            .and(path("/graphql"))
            .and(body_string_contains("DiseaseGenes"))
            .and(body_string_contains("\"efoId\":\"EFO_1000560\""))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "disease": {
                        "associatedTargets": {
                            "rows": [
                                {
                                    "score": 0.95,
                                    "datatypeScores": [{"id": "somatic_mutation", "score": 0.82}],
                                    "datasourceScores": [],
                                    "target": {"approvedSymbol": "ATM"}
                                },
                                {
                                    "score": 0.88,
                                    "datatypeScores": [{"id": "somatic_mutation", "score": 0.77}],
                                    "datasourceScores": [],
                                    "target": {"approvedSymbol": "JAK3"}
                                },
                                {
                                    "score": 0.81,
                                    "datatypeScores": [{"id": "somatic_mutation", "score": 0.72}],
                                    "datasourceScores": [],
                                    "target": {"approvedSymbol": "STAT5B"}
                                }
                            ]
                        }
                    }
                }
            })))
            .mount(&opentargets)
            .await;

        mock_empty_monarch(&monarch).await;
        mock_empty_civic(&civic).await;
        mock_empty_mychem(&mychem).await;
        mock_empty_ctgov(&ctgov).await;

        let disease = get("MONDO:0019468", &["genes".to_string()])
            .await
            .expect("T-PLL should resolve");

        assert_eq!(disease.name, "T-cell prolymphocytic leukemia");
        assert!(disease.synonyms.iter().any(|value| value == "T-PLL"));
        let genes = disease
            .gene_associations
            .iter()
            .map(|row| row.gene.as_str())
            .collect::<Vec<_>>();
        assert!(genes.contains(&"ATM"));
        assert!(genes.contains(&"JAK3"));
        assert!(genes.contains(&"STAT5B"));
    }

    #[tokio::test]
    async fn get_disease_genes_uses_ols4_label_fallback_for_sparse_mondo_identity() {
        proof_get_disease_genes_uses_ols4_label_fallback_for_sparse_mondo_identity().await;
    }
}
