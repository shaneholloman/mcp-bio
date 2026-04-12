use super::*;
use crate::entities::disease::DiseaseVariantAssociation;

#[test]
fn disease_markdown_renders_opentargets_scores_in_summary_and_genes_table() {
    let disease = Disease {
        id: "MONDO:0005105".to_string(),
        name: "melanoma".to_string(),
        definition: None,
        synonyms: Vec::new(),
        parents: Vec::new(),
        associated_genes: vec!["BRAF".to_string(), "NRAS".to_string()],
        gene_associations: vec![
            crate::entities::disease::DiseaseGeneAssociation {
                gene: "BRAF".to_string(),
                relationship: Some("causal".to_string()),
                source: Some("Monarch".to_string()),
                opentargets_score: Some(crate::entities::disease::DiseaseAssociationScoreSummary {
                    overall_score: 0.912,
                    gwas_score: Some(0.321),
                    rare_variant_score: Some(0.654),
                    somatic_mutation_score: Some(0.876),
                }),
            },
            crate::entities::disease::DiseaseGeneAssociation {
                gene: "NRAS".to_string(),
                relationship: Some("associated".to_string()),
                source: Some("CIViC".to_string()),
                opentargets_score: None,
            },
        ],
        top_genes: vec!["BRAF".to_string(), "NRAS".to_string()],
        top_gene_scores: vec![
            crate::entities::disease::DiseaseTargetScore {
                symbol: "BRAF".to_string(),
                summary: crate::entities::disease::DiseaseAssociationScoreSummary {
                    overall_score: 0.912,
                    gwas_score: Some(0.321),
                    rare_variant_score: Some(0.654),
                    somatic_mutation_score: Some(0.876),
                },
            },
            crate::entities::disease::DiseaseTargetScore {
                symbol: "NRAS".to_string(),
                summary: crate::entities::disease::DiseaseAssociationScoreSummary {
                    overall_score: 0.701,
                    gwas_score: None,
                    rare_variant_score: None,
                    somatic_mutation_score: Some(0.443),
                },
            },
        ],
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
        civic: None,
        disgenet: None,
        funding: None,
        funding_note: None,
        xrefs: std::collections::HashMap::new(),
    };

    let summary = disease_markdown(&disease, &[]).expect("rendered markdown");
    assert!(summary.contains("Genes (Open Targets): BRAF (OT 0.912), NRAS (OT 0.701)"));

    let genes = disease_markdown(&disease, &["genes".to_string()]).expect("rendered markdown");
    assert!(genes.contains("| Gene | Relationship | Source | OpenTargets |"));
    assert!(genes.contains("overall 0.912; GWAS 0.321; rare 0.654; somatic 0.876"));
    assert!(genes.contains("| NRAS | associated | CIViC | - |"));
}

pub(crate) fn proof_disease_markdown_renders_ot_only_gene_association_table() {
    let disease = Disease {
        id: "MONDO:0003864".to_string(),
        name: "chronic lymphocytic leukemia".to_string(),
        definition: None,
        synonyms: Vec::new(),
        parents: Vec::new(),
        associated_genes: vec!["TP53".to_string(), "ATM".to_string()],
        gene_associations: vec![
            crate::entities::disease::DiseaseGeneAssociation {
                gene: "TP53".to_string(),
                relationship: Some("associated with disease".to_string()),
                source: Some("OpenTargets".to_string()),
                opentargets_score: Some(crate::entities::disease::DiseaseAssociationScoreSummary {
                    overall_score: 0.991,
                    gwas_score: None,
                    rare_variant_score: None,
                    somatic_mutation_score: Some(0.881),
                }),
            },
            crate::entities::disease::DiseaseGeneAssociation {
                gene: "ATM".to_string(),
                relationship: Some("associated with disease".to_string()),
                source: Some("OpenTargets".to_string()),
                opentargets_score: Some(crate::entities::disease::DiseaseAssociationScoreSummary {
                    overall_score: 0.942,
                    gwas_score: None,
                    rare_variant_score: None,
                    somatic_mutation_score: Some(0.731),
                }),
            },
        ],
        top_genes: vec!["TP53".to_string(), "ATM".to_string()],
        top_gene_scores: vec![
            crate::entities::disease::DiseaseTargetScore {
                symbol: "TP53".to_string(),
                summary: crate::entities::disease::DiseaseAssociationScoreSummary {
                    overall_score: 0.991,
                    gwas_score: None,
                    rare_variant_score: None,
                    somatic_mutation_score: Some(0.881),
                },
            },
            crate::entities::disease::DiseaseTargetScore {
                symbol: "ATM".to_string(),
                summary: crate::entities::disease::DiseaseAssociationScoreSummary {
                    overall_score: 0.942,
                    gwas_score: None,
                    rare_variant_score: None,
                    somatic_mutation_score: Some(0.731),
                },
            },
        ],
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
        civic: None,
        disgenet: None,
        funding: None,
        funding_note: None,
        xrefs: std::collections::HashMap::new(),
    };

    let genes = disease_markdown(&disease, &["genes".to_string()]).expect("rendered markdown");
    assert!(genes.contains("| Gene | Relationship | Source | OpenTargets |"));
    assert!(genes.contains(
        "| TP53 | associated with disease | OpenTargets | overall 0.991; somatic 0.881 |"
    ));
    assert!(!genes.contains("TP53, ATM"));
}

#[test]
fn disease_markdown_renders_ot_only_gene_association_table() {
    proof_disease_markdown_renders_ot_only_gene_association_table();
}

#[test]
fn disease_markdown_preserves_full_definition_text() {
    let full_definition = concat!(
        "A rare hypomyelinating leukodystrophy disorder in which the cause of the disease is ",
        "a variation in any of the POLR genes, including POLR1C, POLR3A or POLR3B. ",
        "It is characterized by the association of hypomyelination, hypodontia, ",
        "hypogonadotropic hypogonadism, and neurodevelopmental delay or regression."
    );
    let disease = Disease {
        id: "MONDO:0100605".to_string(),
        name: "4H leukodystrophy".to_string(),
        definition: Some(full_definition.to_string()),
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
        civic: None,
        disgenet: None,
        funding: None,
        funding_note: None,
        xrefs: std::collections::HashMap::new(),
    };

    let markdown = disease_markdown(&disease, &[]).expect("rendered markdown");
    assert!(markdown.contains(full_definition));
    assert!(markdown.contains("hypogonadotropic hypogonadism"));
    assert!(markdown.contains("neurodevelopmental delay or regression"));
    assert!(!markdown.contains("It is characterized by the association of…"));
}

#[test]
fn disease_markdown_phenotypes_section_renders_key_features() {
    let disease = Disease {
            id: "MONDO:0008222".to_string(),
            name: "Andersen-Tawil syndrome".to_string(),
            definition: Some(
                "A potassium channel disorder characterized by periodic paralysis, prolonged QT interval, and ventricular arrhythmias."
                    .to_string(),
            ),
            synonyms: Vec::new(),
            parents: Vec::new(),
            associated_genes: Vec::new(),
            gene_associations: Vec::new(),
            top_genes: Vec::new(),
            top_gene_scores: Vec::new(),
            treatment_landscape: Vec::new(),
            recruiting_trial_count: None,
            pathways: Vec::new(),
            phenotypes: vec![crate::entities::disease::DiseasePhenotype {
                hpo_id: "HP:0000001".to_string(),
                name: Some("Periodic paralysis".to_string()),
                evidence: None,
                frequency: None,
                frequency_qualifier: Some("Very frequent (80-99%)".to_string()),
                onset_qualifier: None,
                sex_qualifier: None,
                stage_qualifier: None,
                qualifiers: Vec::new(),
                source: None,
            }],
            key_features: vec![
                "periodic paralysis".to_string(),
                "prolonged QT interval".to_string(),
                "ventricular arrhythmias".to_string(),
            ],
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
            xrefs: std::collections::HashMap::new(),
        };

    let markdown = disease_markdown(&disease, &["phenotypes".to_string()]).expect("markdown");
    assert!(markdown.contains("### Key Features"));
    assert!(markdown.contains("- periodic paralysis"));
    assert!(markdown.contains("These summarize the classic presentation; the table below is the comprehensive HPO annotation list."));
    assert!(markdown.contains("source-backed"));
}

#[test]
fn disease_markdown_phenotypes_section_renders_definition_hint_when_key_features_missing() {
    let disease = Disease {
        id: "MONDO:0001111".to_string(),
        name: "Example syndrome".to_string(),
        definition: Some("Example syndrome is a rare inherited condition.".to_string()),
        synonyms: Vec::new(),
        parents: Vec::new(),
        associated_genes: Vec::new(),
        gene_associations: Vec::new(),
        top_genes: Vec::new(),
        top_gene_scores: Vec::new(),
        treatment_landscape: Vec::new(),
        recruiting_trial_count: None,
        pathways: Vec::new(),
        phenotypes: vec![crate::entities::disease::DiseasePhenotype {
            hpo_id: "HP:0001250".to_string(),
            name: Some("Seizure".to_string()),
            evidence: None,
            frequency: None,
            frequency_qualifier: None,
            onset_qualifier: None,
            sex_qualifier: None,
            stage_qualifier: None,
            qualifiers: Vec::new(),
            source: None,
        }],
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
        xrefs: std::collections::HashMap::new(),
    };

    let markdown = disease_markdown(&disease, &["phenotypes".to_string()]).expect("markdown");
    assert!(markdown.contains(
            "Classic features are best summarized in the disease definition. Run `biomcp get disease MONDO:0001111` to review the definition; the table below is the comprehensive HPO annotation list."
        ));
    assert!(markdown.contains("source-backed"));
    assert!(!markdown.contains("### Key Features"));
}

#[test]
fn disease_markdown_phenotypes_section_without_definition_only_shows_completeness_note() {
    let disease = Disease {
        id: "MONDO:0002222".to_string(),
        name: "Undocumented syndrome".to_string(),
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
        phenotypes: vec![crate::entities::disease::DiseasePhenotype {
            hpo_id: "HP:0001250".to_string(),
            name: Some("Seizure".to_string()),
            evidence: None,
            frequency: None,
            frequency_qualifier: None,
            onset_qualifier: None,
            sex_qualifier: None,
            stage_qualifier: None,
            qualifiers: Vec::new(),
            source: None,
        }],
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
        xrefs: std::collections::HashMap::new(),
    };

    let markdown = disease_markdown(&disease, &["phenotypes".to_string()]).expect("markdown");
    assert!(markdown.contains("source-backed"));
    assert!(!markdown.contains("Classic features are best summarized"));
    assert!(!markdown.contains("### Key Features"));
}

#[test]
fn disease_markdown_renders_top_variant_summary() {
    let disease = Disease {
        id: "MONDO:0005105".to_string(),
        name: "melanoma".to_string(),
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
        variants: vec![DiseaseVariantAssociation {
            variant: "BRAF V600E".to_string(),
            relationship: Some("associated with disease".to_string()),
            source: Some("CIViC".to_string()),
            evidence_count: Some(3),
        }],
        top_variant: Some(DiseaseVariantAssociation {
            variant: "BRAF V600E".to_string(),
            relationship: Some("associated with disease".to_string()),
            source: Some("CIViC".to_string()),
            evidence_count: Some(3),
        }),
        models: Vec::new(),
        prevalence: Vec::new(),
        prevalence_note: None,
        survival: None,
        survival_note: None,
        civic: None,
        disgenet: None,
        funding: None,
        funding_note: None,
        xrefs: std::collections::HashMap::new(),
    };

    let markdown = disease_markdown(&disease, &["variants".to_string()]).expect("markdown");
    assert!(
        markdown.contains(
            "Top Variant: BRAF V600E - associated with disease (CIViC, 3 evidence items)"
        )
    );
}

#[test]
fn disease_markdown_renders_survival_summary_and_note() {
    let disease = Disease {
        id: "MONDO:0001234".to_string(),
        name: "chronic myeloid leukemia".to_string(),
        definition: None,
        synonyms: vec!["CML".to_string()],
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
        survival: Some(crate::entities::disease::DiseaseSurvival {
            site_code: 97,
            site_label: "Chronic Myeloid Leukemia (CML)".to_string(),
            series: vec![
                crate::entities::disease::DiseaseSurvivalSeries {
                    sex: "Both Sexes".to_string(),
                    latest_observed: Some(crate::entities::disease::DiseaseSurvivalPoint {
                        year: 2017,
                        relative_survival_rate: Some(69.4),
                        standard_error: Some(1.1),
                        lower_ci: Some(67.2),
                        upper_ci: Some(71.3),
                        modeled_relative_survival_rate: Some(70.4),
                        case_count: Some(471),
                    }),
                    latest_modeled: Some(crate::entities::disease::DiseaseSurvivalPoint {
                        year: 2018,
                        relative_survival_rate: None,
                        standard_error: None,
                        lower_ci: None,
                        upper_ci: None,
                        modeled_relative_survival_rate: Some(70.0),
                        case_count: None,
                    }),
                    points: vec![
                        crate::entities::disease::DiseaseSurvivalPoint {
                            year: 2016,
                            relative_survival_rate: Some(67.1),
                            standard_error: Some(1.2),
                            lower_ci: Some(64.8),
                            upper_ci: Some(69.5),
                            modeled_relative_survival_rate: Some(67.1),
                            case_count: Some(450),
                        },
                        crate::entities::disease::DiseaseSurvivalPoint {
                            year: 2017,
                            relative_survival_rate: Some(69.4),
                            standard_error: Some(1.1),
                            lower_ci: Some(67.2),
                            upper_ci: Some(71.3),
                            modeled_relative_survival_rate: Some(70.4),
                            case_count: Some(471),
                        },
                        crate::entities::disease::DiseaseSurvivalPoint {
                            year: 2018,
                            relative_survival_rate: None,
                            standard_error: None,
                            lower_ci: None,
                            upper_ci: None,
                            modeled_relative_survival_rate: Some(70.0),
                            case_count: None,
                        },
                    ],
                },
                crate::entities::disease::DiseaseSurvivalSeries {
                    sex: "Male".to_string(),
                    latest_observed: Some(crate::entities::disease::DiseaseSurvivalPoint {
                        year: 2017,
                        relative_survival_rate: Some(63.9),
                        standard_error: Some(1.6),
                        lower_ci: Some(60.8),
                        upper_ci: Some(66.9),
                        modeled_relative_survival_rate: Some(64.8),
                        case_count: Some(284),
                    }),
                    latest_modeled: Some(crate::entities::disease::DiseaseSurvivalPoint {
                        year: 2018,
                        relative_survival_rate: None,
                        standard_error: None,
                        lower_ci: None,
                        upper_ci: None,
                        modeled_relative_survival_rate: Some(64.2),
                        case_count: None,
                    }),
                    points: vec![
                        crate::entities::disease::DiseaseSurvivalPoint {
                            year: 2016,
                            relative_survival_rate: Some(61.3),
                            standard_error: Some(1.7),
                            lower_ci: Some(58.1),
                            upper_ci: Some(64.4),
                            modeled_relative_survival_rate: Some(61.3),
                            case_count: Some(273),
                        },
                        crate::entities::disease::DiseaseSurvivalPoint {
                            year: 2017,
                            relative_survival_rate: Some(63.9),
                            standard_error: Some(1.6),
                            lower_ci: Some(60.8),
                            upper_ci: Some(66.9),
                            modeled_relative_survival_rate: Some(64.8),
                            case_count: Some(284),
                        },
                        crate::entities::disease::DiseaseSurvivalPoint {
                            year: 2018,
                            relative_survival_rate: None,
                            standard_error: None,
                            lower_ci: None,
                            upper_ci: None,
                            modeled_relative_survival_rate: Some(64.2),
                            case_count: None,
                        },
                    ],
                },
            ],
        }),
        survival_note: None,
        civic: None,
        disgenet: None,
        funding: None,
        funding_note: None,
        xrefs: std::collections::HashMap::new(),
    };

    let markdown = disease_markdown(&disease, &["survival".to_string()]).expect("markdown");
    assert!(markdown.contains("## Survival (SEER Explorer)"));
    assert!(markdown.contains("site code 97"));
    assert!(markdown.contains("All Ages"));
    assert!(markdown.contains("All Races / Ethnicities"));
    assert!(markdown.contains(
            "| Sex | Latest observed year | 5-year relative survival | 95% CI | Cases | Latest modeled |",
        ));
    assert!(markdown.contains("| Both Sexes | 2017 | 69.4% | 67.2%-71.3% | 471 | 2018: 70.0% |",));
    assert!(markdown.contains("### Recent History"));
    assert!(markdown.contains("| Male | 2017 | 63.9% | 60.8%-66.9% | 284 |"));

    let mut note_disease = disease.clone();
    note_disease.survival = None;
    note_disease.survival_note =
        Some("SEER survival data not available for this condition.".to_string());

    let note_markdown =
        disease_markdown(&note_disease, &["survival".to_string()]).expect("note markdown");
    assert!(note_markdown.contains("## Survival (SEER Explorer)"));
    assert!(note_markdown.contains("SEER survival data not available for this condition."));
    assert!(!note_markdown.contains("| Sex | Latest observed year |"));
}

#[test]
fn disease_markdown_section_only_shows_disgenet_section() {
    let disease = Disease {
        id: "MONDO:0007254".to_string(),
        name: "breast cancer".to_string(),
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
        civic: None,
        disgenet: Some(crate::entities::disease::DiseaseDisgenet {
            associations: vec![crate::entities::disease::DiseaseDisgenetAssociation {
                symbol: "TP53".to_string(),
                entrez_id: Some(7157),
                score: 0.91,
                publication_count: Some(1234),
                clinical_trial_count: Some(4),
                evidence_index: Some(0.72),
                evidence_level: Some("Definitive".to_string()),
            }],
        }),
        funding: None,
        funding_note: None,
        xrefs: std::collections::HashMap::new(),
    };

    let markdown =
        disease_markdown(&disease, &["disgenet".to_string()]).expect("rendered markdown");

    assert!(markdown.contains("# breast cancer - disgenet"));
    assert!(markdown.contains("## DisGeNET"));
    assert!(markdown.contains("| Gene | Entrez ID | Score | PMIDs | Trials | EL | EI |"));
    assert!(markdown.contains("| TP53 | 7157 | 0.910 | 1234 | 4 | Definitive | 0.720 |"));
}

#[test]
fn disease_markdown_disgenet_renders_sparse_optional_fields() {
    let disease = Disease {
        id: "MONDO:0000001".to_string(),
        name: "sparse disease".to_string(),
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
        civic: None,
        disgenet: Some(crate::entities::disease::DiseaseDisgenet {
            associations: vec![crate::entities::disease::DiseaseDisgenetAssociation {
                symbol: "KYNU".to_string(),
                entrez_id: None,
                score: 0.23,
                publication_count: None,
                clinical_trial_count: None,
                evidence_index: None,
                evidence_level: None,
            }],
        }),
        funding: None,
        funding_note: None,
        xrefs: std::collections::HashMap::new(),
    };

    let markdown =
        disease_markdown(&disease, &["disgenet".to_string()]).expect("rendered markdown");

    assert!(markdown.contains("| Gene | Entrez ID | Score | PMIDs | Trials | EL | EI |"));
    assert!(markdown.contains("| KYNU | - | 0.230 | - | - | - | - |"));
}

#[test]
fn disease_markdown_funding_renders_truthful_notes_without_table() {
    let mut disease = Disease {
        id: "MONDO:0007947".to_string(),
        name: "Marfan syndrome".to_string(),
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
        civic: None,
        disgenet: None,
        funding: Some(crate::sources::nih_reporter::NihReporterFundingSection {
            query: "Marfan syndrome".to_string(),
            fiscal_years: vec![2022, 2023, 2024, 2025, 2026],
            matching_project_years: 0,
            grants: Vec::new(),
        }),
        funding_note: Some("No NIH funding data found for this query.".to_string()),
        xrefs: std::collections::HashMap::new(),
    };

    let no_hit =
        disease_markdown(&disease, &["funding".to_string()]).expect("no-hit funding markdown");
    assert!(no_hit.contains("## Funding (NIH Reporter)"));
    assert!(no_hit.contains("No NIH funding data found for this query."));
    assert!(!no_hit.contains("| Project | PI | Organization | FY | Amount |"));

    disease.funding = None;
    disease.funding_note =
        Some("NIH Reporter funding data is temporarily unavailable.".to_string());

    let unavailable =
        disease_markdown(&disease, &["funding".to_string()]).expect("unavailable funding markdown");
    assert!(unavailable.contains("## Funding (NIH Reporter)"));
    assert!(unavailable.contains("NIH Reporter funding data is temporarily unavailable."));
    assert!(!unavailable.contains("| Project | PI | Organization | FY | Amount |"));
}

#[test]
fn disease_markdown_all_keeps_opt_in_sections_hidden() {
    let disease = Disease {
        id: "MONDO:0007947".to_string(),
        name: "Marfan syndrome".to_string(),
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
        civic: None,
        disgenet: None,
        funding: None,
        funding_note: None,
        xrefs: std::collections::HashMap::new(),
    };

    let markdown = disease_markdown(&disease, &["all".to_string()]).expect("all markdown");

    assert!(!markdown.contains("## Funding (NIH Reporter)"));
    assert!(!markdown.contains("## DisGeNET"));
}

#[test]
fn disease_search_empty_state_includes_discover_hint() {
    let markdown = disease_search_markdown_with_footer(
        "definitelynotarealdisease",
        "definitelynotarealdisease",
        &[],
        false,
        "",
    )
    .expect("markdown");

    assert!(markdown.contains("Try: biomcp discover definitelynotarealdisease"));
}

#[test]
fn disease_search_empty_state_uses_raw_query_in_discover_hint() {
    let markdown = disease_search_markdown_with_footer(
        "Arnold Chiari syndrome",
        "Arnold Chiari syndrome, offset=5",
        &[],
        false,
        "",
    )
    .expect("markdown");

    assert!(markdown.contains("Try: biomcp discover \"Arnold Chiari syndrome\""));
    assert!(!markdown.contains("offset=5\""));
}

#[test]
fn disease_search_fallback_renders_provenance_columns() {
    let markdown = disease_search_markdown_with_footer(
        "Arnold Chiari syndrome",
        "Arnold Chiari syndrome",
        &[DiseaseSearchResult {
            id: "MONDO:0000115".into(),
            name: "Arnold-Chiari malformation".into(),
            synonyms_preview: Some("Chiari malformation".into()),
            resolved_via: Some("MESH crosswalk".into()),
            source_id: Some("MESH:D001139".into()),
        }],
        true,
        "",
    )
    .expect("markdown");

    assert!(markdown.contains("Resolved via discover + crosswalk"));
    assert!(markdown.contains("| ID | Name | Resolved via | Source ID |"));
    assert!(markdown.contains("MESH crosswalk"));
    assert!(markdown.contains("MESH:D001139"));
}
