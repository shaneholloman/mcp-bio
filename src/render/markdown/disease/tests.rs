use super::*;
use crate::entities::disease::{DiseaseClinicalFeature, DiseaseVariantAssociation};

fn clinical_feature_row() -> DiseaseClinicalFeature {
    DiseaseClinicalFeature {
        rank: 1,
        label: "heavy menstrual bleeding".to_string(),
        feature_type: "symptom".to_string(),
        source: "MedlinePlus".to_string(),
        source_url: Some("https://medlineplus.gov/uterinefibroids.html".to_string()),
        source_native_id: "uterinefibroids".to_string(),
        evidence_tier: "clinical_summary".to_string(),
        evidence_text: "...heavy menstrual bleeding...".to_string(),
        evidence_match: "heavy menstrual bleeding".to_string(),
        body_system: Some("reproductive".to_string()),
        topic_title: Some("Uterine Fibroids".to_string()),
        topic_relation: Some("direct".to_string()),
        topic_selection_score: Some(180.0),
        normalized_hpo_id: Some("HP:0000132".to_string()),
        normalized_hpo_label: Some("Menorrhagia".to_string()),
        mapping_confidence: 0.86,
        mapping_method: "reviewed_fixture_exact_or_synonym".to_string(),
    }
}

fn disease_with_clinical_features() -> Disease {
    Disease {
        id: "MONDO:0004277".to_string(),
        name: "uterine leiomyoma".to_string(),
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
        clinical_features: vec![clinical_feature_row()],
        key_features: Vec::new(),
        variants: Vec::new(),
        top_variant: None,
        models: Vec::new(),
        prevalence: Vec::new(),
        prevalence_note: None,
        survival: None,
        survival_note: None,
        civic: None,
        disgenet: None,
        funding: None,
        funding_note: None,
        diagnostics: None,
        diagnostics_note: None,
        xrefs: std::collections::HashMap::new(),
    }
}

fn disease_without_clinical_features() -> Disease {
    let mut disease = disease_with_clinical_features();
    disease.id = "MONDO:0005105".to_string();
    disease.name = "melanoma".to_string();
    disease.clinical_features.clear();
    disease
}

include!("tests/rendering.rs");
include!("tests/extended.rs");
