use super::*;

#[test]
fn related_gene_prioritizes_localization_deepening_when_supported() {
    let gene = Gene {
        symbol: "OPA1".to_string(),
        name: "OPA1 mitochondrial dynamin like GTPase".to_string(),
        entrez_id: "4976".to_string(),
        ensembl_id: Some("ENSG00000198836".to_string()),
        location: Some("3q29".to_string()),
        genomic_coordinates: None,
        omim_id: None,
        uniprot_id: Some("O60313".to_string()),
        summary: Some(
            "Mitochondrial inner membrane fusion GTPase required for cristae organization."
                .to_string(),
        ),
        gene_type: Some("protein-coding".to_string()),
        aliases: vec!["large GTPase 1".to_string()],
        clinical_diseases: Vec::new(),
        clinical_drugs: Vec::new(),
        pathways: None,
        ontology: None,
        diseases: None,
        protein: Some(crate::entities::gene::GeneProtein {
            accession: "O60313".to_string(),
            name: "Dynamin-like 120 kDa protein, mitochondrial".to_string(),
            function: None,
            length: None,
            isoforms: Vec::new(),
            alternative_names: Vec::new(),
        }),
        go: None,
        interactions: None,
        civic: None,
        expression: None,
        hpa: Some(crate::sources::hpa::GeneHpa {
            tissues: vec![crate::sources::hpa::HpaTissueExpression {
                tissue: "Retina".to_string(),
                level: "High".to_string(),
            }],
            subcellular_main_location: vec!["Mitochondria".to_string()],
            subcellular_additional_location: Vec::new(),
            reliability: Some("Approved".to_string()),
            protein_summary: None,
            rna_summary: None,
        }),
        druggability: None,
        clingen: None,
        constraint: None,
        disgenet: None,
        funding: None,
        funding_note: None,
    };

    let related = related_gene(&gene);
    assert_eq!(related[0], "biomcp get gene OPA1 protein");
    assert_eq!(related[1], "biomcp get gene OPA1 hpa");
    assert!(related.contains(&"biomcp search pgx -g OPA1".to_string()));
    assert!(related.contains(&"biomcp search variant -g OPA1".to_string()));
}

#[test]
fn related_gene_promotes_clingen_trial_search_before_generic_pivots() {
    let gene = Gene {
        symbol: "OPA1".to_string(),
        name: "OPA1 mitochondrial dynamin like GTPase".to_string(),
        entrez_id: "4976".to_string(),
        ensembl_id: Some("ENSG00000198836".to_string()),
        location: Some("3q29".to_string()),
        genomic_coordinates: None,
        omim_id: None,
        uniprot_id: Some("O60313".to_string()),
        summary: Some(
            "Mitochondrial inner membrane fusion GTPase required for cristae organization."
                .to_string(),
        ),
        gene_type: Some("protein-coding".to_string()),
        aliases: vec!["large GTPase 1".to_string()],
        clinical_diseases: Vec::new(),
        clinical_drugs: Vec::new(),
        pathways: None,
        ontology: None,
        diseases: None,
        protein: Some(crate::entities::gene::GeneProtein {
            accession: "O60313".to_string(),
            name: "Dynamin-like 120 kDa protein, mitochondrial".to_string(),
            function: None,
            length: None,
            isoforms: Vec::new(),
            alternative_names: Vec::new(),
        }),
        go: None,
        interactions: None,
        civic: None,
        expression: None,
        hpa: Some(crate::sources::hpa::GeneHpa {
            tissues: vec![crate::sources::hpa::HpaTissueExpression {
                tissue: "Retina".to_string(),
                level: "High".to_string(),
            }],
            subcellular_main_location: vec!["Mitochondria".to_string()],
            subcellular_additional_location: Vec::new(),
            reliability: Some("Approved".to_string()),
            protein_summary: None,
            rna_summary: None,
        }),
        druggability: None,
        clingen: Some(crate::sources::clingen::GeneClinGen {
            validity: vec![crate::sources::clingen::ClinGenValidity {
                disease: "dominant optic atrophy".to_string(),
                classification: "Definitive".to_string(),
                review_date: Some("2024-01-01".to_string()),
                moi: Some("AD".to_string()),
            }],
            haploinsufficiency: None,
            triplosensitivity: None,
        }),
        constraint: None,
        disgenet: None,
        funding: None,
        funding_note: None,
    };

    let related = related_gene(&gene);
    assert_eq!(related[0], "biomcp get gene OPA1 protein");
    assert_eq!(related[1], "biomcp get gene OPA1 hpa");
    assert_eq!(
        related[2],
        "biomcp search trial -c \"dominant optic atrophy\" -s recruiting"
    );
    assert_eq!(
        related_command_description(&related[2]),
        Some("recruiting trials for the top ClinGen disease on this gene card")
    );
    assert!(related.contains(&"biomcp search pgx -g OPA1".to_string()));
}

#[test]
fn related_drug_includes_pgx_search() {
    let drug = Drug {
        name: "warfarin".to_string(),
        drugbank_id: None,
        chembl_id: None,
        unii: None,
        drug_type: None,
        mechanism: None,
        mechanisms: Vec::new(),
        approval_date: None,
        approval_date_raw: None,
        approval_date_display: None,
        approval_summary: None,
        brand_names: Vec::new(),
        route: None,
        targets: Vec::new(),
        variant_targets: Vec::new(),
        target_family: None,
        target_family_name: None,
        indications: Vec::new(),
        interactions: Vec::new(),
        interaction_text: None,
        pharm_classes: Vec::new(),
        top_adverse_events: Vec::new(),
        faers_query: None,
        label: None,
        label_set_id: None,
        shortage: None,
        approvals: None,
        us_safety_warnings: None,
        ema_regulatory: None,
        ema_safety: None,
        ema_shortage: None,
        who_prequalification: None,
        civic: None,
    };

    let related = related_drug(&drug);
    assert!(related.contains(&"biomcp search pgx -d warfarin".to_string()));
}

#[test]
fn related_disease_suggests_review_when_phenotypes_are_sparse() {
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
        phenotypes: vec![
            crate::entities::disease::DiseasePhenotype {
                hpo_id: "HP:0001166".to_string(),
                name: Some("Arachnodactyly".to_string()),
                evidence: None,
                frequency: None,
                frequency_qualifier: None,
                onset_qualifier: None,
                sex_qualifier: None,
                stage_qualifier: None,
                qualifiers: Vec::new(),
                source: None,
            },
            crate::entities::disease::DiseasePhenotype {
                hpo_id: "HP:0002616".to_string(),
                name: Some("Aortic root dilatation".to_string()),
                evidence: None,
                frequency: None,
                frequency_qualifier: None,
                onset_qualifier: None,
                sex_qualifier: None,
                stage_qualifier: None,
                qualifiers: Vec::new(),
                source: None,
            },
        ],
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

    let related = related_disease(&disease);
    assert_eq!(
        related[0],
        "biomcp search article -d \"Marfan syndrome\" --type review --limit 5"
    );
    assert!(related.contains(&"biomcp search trial -c \"Marfan syndrome\"".to_string()));
    assert!(related.contains(&"biomcp search drug --indication \"Marfan syndrome\"".to_string()));
}

#[test]
fn related_disease_promotes_top_gene_context_before_generic_pivots() {
    let disease = Disease {
        id: "MONDO:0100135".to_string(),
        name: "Dravet syndrome".to_string(),
        definition: None,
        synonyms: Vec::new(),
        parents: Vec::new(),
        associated_genes: vec!["SCN1A".to_string()],
        gene_associations: Vec::new(),
        top_genes: vec!["SCN1A".to_string()],
        top_gene_scores: vec![crate::entities::disease::DiseaseTargetScore {
            symbol: "SCN1A".to_string(),
            summary: crate::entities::disease::DiseaseAssociationScoreSummary {
                overall_score: 0.872,
                gwas_score: None,
                rare_variant_score: Some(0.997),
                somatic_mutation_score: None,
            },
        }],
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

    let related = related_disease(&disease);
    assert_eq!(related[0], "biomcp get gene SCN1A clingen constraint");
    assert_eq!(related[1], "biomcp search trial -c \"Dravet syndrome\"");
    assert_eq!(
        related_command_description(&related[0]),
        Some("review ClinGen validity and constraint evidence for the top disease gene")
    );
}

#[test]
fn related_disease_falls_back_to_unscored_top_gene_context() {
    let disease = Disease {
        id: "MONDO:0100135".to_string(),
        name: "Dravet syndrome".to_string(),
        definition: None,
        synonyms: Vec::new(),
        parents: Vec::new(),
        associated_genes: vec!["SCN1A".to_string()],
        gene_associations: Vec::new(),
        top_genes: vec!["SCN1A".to_string()],
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

    let related = related_disease(&disease);
    assert_eq!(related[0], "biomcp get gene SCN1A clingen constraint");
}

#[test]
fn related_disease_uses_synonym_when_name_is_raw_id() {
    let disease = Disease {
        id: "MONDO:0100605".to_string(),
        name: "MONDO:0100605".to_string(),
        definition: None,
        synonyms: vec!["4H leukodystrophy".to_string()],
        parents: Vec::new(),
        associated_genes: Vec::new(),
        gene_associations: Vec::new(),
        top_genes: Vec::new(),
        top_gene_scores: Vec::new(),
        treatment_landscape: Vec::new(),
        recruiting_trial_count: None,
        pathways: Vec::new(),
        phenotypes: vec![crate::entities::disease::DiseasePhenotype {
            hpo_id: "HP:0001252".to_string(),
            name: Some("Hypomyelination".to_string()),
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

    let related = related_disease(&disease);
    assert_eq!(
        related[0],
        "biomcp search article -d \"4H leukodystrophy\" --type review --limit 5"
    );
}

#[test]
fn related_disease_non_oncology_skips_study_hints() {
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

    let related = related_disease(&disease);
    assert!(!related.iter().any(|cmd| cmd.contains("biomcp study ")));
}

#[test]
fn related_disease_quotes_single_word_indication_search() {
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

    let related = related_disease(&disease);
    assert!(related.contains(&"biomcp search drug --indication \"melanoma\"".to_string()));
}

#[test]
fn related_disease_oncology_without_local_match_falls_back_to_download_list() {
    let _guard = crate::test_support::env_lock().blocking_lock();
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
        variants: Vec::new(),
        top_variant: None,
        models: Vec::new(),
        prevalence: Vec::new(),
        prevalence_note: None,
        survival: None,
        survival_note: None,
        civic: Some(crate::sources::civic::CivicContext::default()),
        disgenet: None,
        funding: None,
        funding_note: None,
        xrefs: std::collections::HashMap::new(),
    };

    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "biomcp-render-study-empty-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create empty study root");
    let original = std::env::var_os("BIOMCP_STUDY_DIR");
    unsafe { std::env::set_var("BIOMCP_STUDY_DIR", &root) };
    let related = related_disease(&disease);
    match original {
        Some(value) => unsafe { std::env::set_var("BIOMCP_STUDY_DIR", value) },
        None => unsafe { std::env::remove_var("BIOMCP_STUDY_DIR") },
    }
    let _ = std::fs::remove_dir_all(&root);

    assert!(related.contains(&"biomcp study download --list".to_string()));
}

#[test]
fn related_disease_oncology_with_local_match_prefers_top_mutated() {
    let _guard = crate::test_support::env_lock().blocking_lock();
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "biomcp-render-study-match-{}-{unique}",
        std::process::id()
    ));
    let study_dir = root.join("brca_tcga_pan_can_atlas_2018");
    std::fs::create_dir_all(&study_dir).expect("create study dir");
    std::fs::write(
            study_dir.join("meta_study.txt"),
            "cancer_study_identifier: brca_tcga_pan_can_atlas_2018\nname: BRCA TCGA PanCan Atlas 2018\ntype_of_cancer: brca\n",
        )
        .expect("write meta");
    std::fs::write(
            study_dir.join("data_mutations.txt"),
            "Hugo_Symbol\tTumor_Sample_Barcode\tVariant_Classification\tHGVSp_Short\nTP53\tS1\tMissense_Mutation\tp.R175H\n",
        )
        .expect("write mutations");
    std::fs::write(
            study_dir.join("data_clinical_sample.txt"),
            "# comment\nPATIENT_ID\tSAMPLE_ID\tCANCER_TYPE\tCANCER_TYPE_DETAILED\tONCOTREE_CODE\nP1\tS1\tBreast Cancer\tBreast Invasive Carcinoma\tBRCA\n",
        )
        .expect("write clinical sample");

    let disease = Disease {
        id: "MONDO:0007254".to_string(),
        name: "breast cancer".to_string(),
        definition: None,
        synonyms: vec!["mammary carcinoma".to_string()],
        parents: Vec::new(),
        associated_genes: Vec::new(),
        gene_associations: Vec::new(),
        top_genes: Vec::new(),
        top_gene_scores: vec![crate::entities::disease::DiseaseTargetScore {
            symbol: "TP53".to_string(),
            summary: crate::entities::disease::DiseaseAssociationScoreSummary {
                overall_score: 0.8,
                gwas_score: None,
                rare_variant_score: None,
                somatic_mutation_score: Some(0.4),
            },
        }],
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

    let original = std::env::var_os("BIOMCP_STUDY_DIR");
    unsafe { std::env::set_var("BIOMCP_STUDY_DIR", &root) };
    let related = related_disease(&disease);
    match original {
        Some(value) => unsafe { std::env::set_var("BIOMCP_STUDY_DIR", value) },
        None => unsafe { std::env::remove_var("BIOMCP_STUDY_DIR") },
    }
    let _ = std::fs::remove_dir_all(&root);

    assert!(
        related
            .contains(&"biomcp study top-mutated --study brca_tcga_pan_can_atlas_2018".to_string())
    );
    assert!(!related.contains(&"biomcp study download --list".to_string()));
}

#[test]
fn related_disease_oncology_matches_noncontiguous_carcinoma_study_labels() {
    let _guard = crate::test_support::env_lock().blocking_lock();
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "biomcp-render-study-carcinoma-{}-{unique}",
        std::process::id()
    ));
    let study_dir = root.join("brca_tcga_pan_can_atlas_2018");
    std::fs::create_dir_all(&study_dir).expect("create study dir");
    std::fs::write(
            study_dir.join("meta_study.txt"),
            "cancer_study_identifier: brca_tcga_pan_can_atlas_2018\nname: BRCA TCGA PanCan Atlas 2018\ntype_of_cancer: brca\n",
        )
        .expect("write meta");
    std::fs::write(
            study_dir.join("data_mutations.txt"),
            "Hugo_Symbol\tTumor_Sample_Barcode\tVariant_Classification\tHGVSp_Short\nTP53\tS1\tMissense_Mutation\tp.R175H\n",
        )
        .expect("write mutations");
    std::fs::write(
            study_dir.join("data_clinical_sample.txt"),
            "# comment\nPATIENT_ID\tSAMPLE_ID\tCANCER_TYPE\tCANCER_TYPE_DETAILED\tONCOTREE_CODE\nP1\tS1\tBreast Cancer\tBreast Invasive Carcinoma\tBRCA\n",
        )
        .expect("write clinical sample");

    let disease = Disease {
        id: "MONDO:0004989".to_string(),
        name: "breast carcinoma".to_string(),
        definition: None,
        synonyms: Vec::new(),
        parents: Vec::new(),
        associated_genes: Vec::new(),
        gene_associations: Vec::new(),
        top_genes: Vec::new(),
        top_gene_scores: vec![crate::entities::disease::DiseaseTargetScore {
            symbol: "TP53".to_string(),
            summary: crate::entities::disease::DiseaseAssociationScoreSummary {
                overall_score: 0.8,
                gwas_score: None,
                rare_variant_score: None,
                somatic_mutation_score: Some(0.4),
            },
        }],
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

    let original = std::env::var_os("BIOMCP_STUDY_DIR");
    unsafe { std::env::set_var("BIOMCP_STUDY_DIR", &root) };
    let related = related_disease(&disease);
    match original {
        Some(value) => unsafe { std::env::set_var("BIOMCP_STUDY_DIR", value) },
        None => unsafe { std::env::remove_var("BIOMCP_STUDY_DIR") },
    }
    let _ = std::fs::remove_dir_all(&root);

    assert!(
        related
            .contains(&"biomcp study top-mutated --study brca_tcga_pan_can_atlas_2018".to_string())
    );
    assert!(!related.contains(&"biomcp study download --list".to_string()));
}

#[test]
fn related_drug_suggests_review_when_label_and_indications_are_sparse() {
    let drug = Drug {
        name: "orteronel".to_string(),
        drugbank_id: None,
        chembl_id: None,
        unii: None,
        drug_type: None,
        mechanism: None,
        mechanisms: Vec::new(),
        approval_date: None,
        approval_date_raw: None,
        approval_date_display: None,
        approval_summary: None,
        brand_names: Vec::new(),
        route: None,
        targets: Vec::new(),
        variant_targets: Vec::new(),
        target_family: None,
        target_family_name: None,
        indications: Vec::new(),
        interactions: Vec::new(),
        interaction_text: None,
        pharm_classes: Vec::new(),
        top_adverse_events: Vec::new(),
        faers_query: None,
        label: None,
        label_set_id: None,
        shortage: None,
        approvals: None,
        us_safety_warnings: None,
        ema_regulatory: None,
        ema_safety: None,
        ema_shortage: None,
        who_prequalification: None,
        civic: None,
    };

    let related = related_drug(&drug);
    assert_eq!(
        related[0],
        "biomcp search article --drug orteronel --type review --limit 5"
    );
    assert!(related.contains(&"biomcp search pgx -d orteronel".to_string()));
    assert!(related.contains(&"biomcp drug adverse-events orteronel".to_string()));
}

#[test]
fn related_variant_vus_promotes_literature_before_drug_target() {
    let variant: Variant = serde_json::from_value(serde_json::json!({
        "id": "chr2:g.166848047C>G",
        "gene": "SCN1A",
        "hgvs_p": "p.T1174S",
        "legacy_name": "SCN1A T1174S",
        "significance": "Uncertain significance",
        "top_disease": {"condition": "Dravet syndrome", "reports": 7}
    }))
    .expect("variant should deserialize");

    let related = related_variant(&variant);
    assert_eq!(related[0], "biomcp get gene SCN1A");
    assert_eq!(
        related[1],
        "biomcp search article -g SCN1A -d \"Dravet syndrome\" -k \"T1174S\" --limit 5"
    );
    assert_eq!(related[2], "biomcp search drug --target SCN1A");
    assert_eq!(
        related_command_description(&related[1]),
        Some("literature follow-up for an uncertain-significance variant")
    );
}

#[test]
fn related_variant_vus_keyword_only_follow_up_keeps_description() {
    let variant: Variant = serde_json::from_value(serde_json::json!({
        "id": "chr2:g.166848047C>G",
        "gene": "",
        "hgvs_p": "p.T1174S",
        "significance": "VUS"
    }))
    .expect("variant should deserialize");

    let related = related_variant(&variant);
    assert_eq!(related[0], "biomcp search article -k \"T1174S\" --limit 5");
    assert_eq!(
        related_command_description(&related[0]),
        Some("literature follow-up for an uncertain-significance variant")
    );

    let rendered = format_related_block(related);
    assert!(rendered.contains("literature follow-up for an uncertain-significance variant"));
}

#[test]
fn related_variant_pathogenic_keeps_drug_target_without_vus_literature_pivot() {
    let variant: Variant = serde_json::from_value(serde_json::json!({
        "id": "chr7:g.140453136A>T",
        "gene": "BRAF",
        "hgvs_p": "p.V600E",
        "legacy_name": "BRAF V600E",
        "significance": "Likely pathogenic",
        "top_disease": {"condition": "Melanoma", "reports": 5}
    }))
    .expect("variant should deserialize");

    let related = related_variant(&variant);
    assert_eq!(related[0], "biomcp get gene BRAF");
    assert_eq!(related[1], "biomcp search drug --target BRAF");
    assert!(
        !related
            .iter()
            .any(|cmd| cmd.starts_with("biomcp search article -g BRAF"))
    );
}

#[test]
fn related_article_uses_article_entities_helper_command() {
    let article = Article {
        pmid: Some("22663011".to_string()),
        pmcid: None,
        doi: None,
        title: "Improved survival with MEK inhibition in BRAF-mutated melanoma.".to_string(),
        authors: Vec::new(),
        journal: None,
        date: None,
        citation_count: None,
        publication_type: None,
        open_access: None,
        abstract_text: None,
        full_text_path: None,
        full_text_note: None,
        annotations: Some(ArticleAnnotations {
            genes: vec![
                AnnotationCount {
                    text: "serine-threonine protein kinase".to_string(),
                    count: 7,
                },
                AnnotationCount {
                    text: "BRAF".to_string(),
                    count: 5,
                },
                AnnotationCount {
                    text: "MEK".to_string(),
                    count: 3,
                },
                AnnotationCount {
                    text: "B-RAF".to_string(),
                    count: 1,
                },
            ],
            diseases: vec![
                AnnotationCount {
                    text: "melanoma".to_string(),
                    count: 2,
                },
                AnnotationCount {
                    text: "metastatic melanoma".to_string(),
                    count: 1,
                },
            ],
            chemicals: vec![AnnotationCount {
                text: "trametinib".to_string(),
                count: 8,
            }],
            mutations: Vec::new(),
        }),
        semantic_scholar: None,
        pubtator_fallback: false,
    };

    let related = related_article(&article);
    assert_eq!(related[0], "biomcp article entities 22663011");
    let braf = related
        .iter()
        .position(|cmd| cmd == "biomcp search gene -q BRAF")
        .expect("curated BRAF pivot should be promoted");
    let mek = related
        .iter()
        .position(|cmd| cmd == "biomcp search gene -q MEK")
        .expect("curated MEK pivot should be promoted");
    let melanoma = related
        .iter()
        .position(|cmd| cmd == "biomcp search disease --query melanoma")
        .expect("disease pivot should be promoted");
    let trametinib = related
        .iter()
        .position(|cmd| cmd == "biomcp get drug trametinib")
        .expect("drug pivot should be promoted");
    let references = related
        .iter()
        .position(|cmd| cmd == "biomcp article references 22663011 --limit 3")
        .expect("references command should remain available");
    let citations = related
        .iter()
        .position(|cmd| cmd == "biomcp article citations 22663011 --limit 3")
        .expect("citations command should remain available");
    let recommendations = related
        .iter()
        .position(|cmd| cmd == "biomcp article recommendations 22663011 --limit 3")
        .expect("recommendations command should remain available");

    assert!(braf < references);
    assert!(mek < references);
    assert!(melanoma < citations);
    assert!(trametinib < recommendations);
    assert!(references < citations);
    assert!(citations < recommendations);
    assert!(
        !related
            .iter()
            .any(|cmd| cmd == "biomcp get gene serine-threonine protein kinase")
    );
    assert!(
        !related
            .iter()
            .any(|cmd| cmd == "biomcp search gene -q \"serine-threonine protein kinase\"")
    );
    assert!(!related.iter().any(|cmd| cmd.contains("biomcp get article")));

    let rendered = format_related_block(related);
    assert!(rendered.contains("standardized entity extraction"));
    assert!(rendered.contains(
        "background evidence this paper builds on; use if the primary paper lacks context"
    ));
    assert!(rendered.contains(
        "later papers that cite this article; use only if the primary paper lacks your answer"
    ));
    assert!(rendered.contains(
        "related papers to broaden coverage; use only if the primary paper lacks your answer"
    ));
}

#[test]
fn related_trial_promotes_results_search_for_completed_or_terminated_studies() {
    let trial = crate::entities::trial::Trial {
            nct_id: "NCT02576665".to_string(),
            source: None,
            title: "A Study of Toca 511, a Retroviral Replicating Vector, Combined With Toca FC in Patients With Solid Tumors or Lymphoma (Toca 6)".to_string(),
            status: "TERMINATED".to_string(),
            phase: None,
            study_type: None,
            age_range: None,
            conditions: vec!["Colorectal Cancer".to_string()],
            interventions: vec!["Toca 511".to_string()],
            sponsor: None,
            enrollment: None,
            summary: None,
            start_date: None,
            completion_date: None,
            eligibility_text: None,
            locations: None,
            outcomes: None,
            arms: None,
            references: None,
        };

    let related = related_trial(&trial);
    assert_eq!(
        related[0],
        "biomcp search article --drug \"Toca 511\" -q \"NCT02576665 A Study of Toca 511, a\" --limit 5"
    );
    assert_eq!(
        related[1],
        "biomcp search disease --query \"Colorectal Cancer\""
    );

    let rendered = format_related_block(related.clone());
    assert!(
        rendered.contains(
            "find publications or conference reports from this completed/terminated trial"
        )
    );
    assert_eq!(
        related_command_description(&related[0]),
        Some("find publications or conference reports from this completed/terminated trial")
    );
    assert_eq!(
        related_command_description("biomcp search article --drug pembrolizumab --limit 5"),
        None
    );
}

#[test]
fn related_trial_keeps_recruiting_order_without_results_search() {
    let trial = crate::entities::trial::Trial {
        nct_id: "NCT01234567".to_string(),
        source: None,
        title: "Example trial".to_string(),
        status: "Recruiting".to_string(),
        phase: None,
        study_type: None,
        age_range: None,
        conditions: vec!["melanoma".to_string()],
        interventions: vec!["dabrafenib".to_string()],
        sponsor: None,
        enrollment: None,
        summary: None,
        start_date: None,
        completion_date: None,
        eligibility_text: None,
        locations: None,
        outcomes: None,
        arms: None,
        references: None,
    };

    let related = related_trial(&trial);
    assert_eq!(related[0], "biomcp search disease --query melanoma");
    assert!(!related.iter().any(|cmd| {
        cmd.starts_with("biomcp search article --drug ") && cmd.contains(" --limit 5")
    }));
}

#[test]
fn related_trial_completed_promotes_results_search_before_condition_pivots() {
    let trial = crate::entities::trial::Trial {
        nct_id: "NCT01234567".to_string(),
        source: None,
        title: "Example completed trial".to_string(),
        status: "Completed".to_string(),
        phase: None,
        study_type: None,
        age_range: None,
        conditions: vec!["melanoma".to_string()],
        interventions: vec!["dabrafenib".to_string()],
        sponsor: None,
        enrollment: None,
        summary: None,
        start_date: None,
        completion_date: None,
        eligibility_text: None,
        locations: None,
        outcomes: None,
        arms: None,
        references: None,
    };

    let related = related_trial(&trial);
    assert_eq!(
        related[0],
        "biomcp search article --drug dabrafenib -q \"NCT01234567 Example completed trial\" --limit 5"
    );
    assert_eq!(related[1], "biomcp search disease --query melanoma");
}

#[test]
fn related_trial_results_search_without_intervention_keeps_seed_quoted() {
    let trial = crate::entities::trial::Trial {
        nct_id: "NCT09999999".to_string(),
        source: None,
        title: "   ".to_string(),
        status: "Completed".to_string(),
        phase: None,
        study_type: None,
        age_range: None,
        conditions: vec!["melanoma".to_string()],
        interventions: Vec::new(),
        sponsor: None,
        enrollment: None,
        summary: None,
        start_date: None,
        completion_date: None,
        eligibility_text: None,
        locations: None,
        outcomes: None,
        arms: None,
        references: None,
    };

    let related = related_trial(&trial);
    assert_eq!(
        related[0],
        "biomcp search article -q \"NCT09999999\" --limit 5"
    );
    assert_eq!(
        related_command_description(&related[0]),
        Some("find publications or conference reports from this completed/terminated trial")
    );
}

#[test]
fn related_pgx_uses_search_flags() {
    let pgx = Pgx {
        query: "CYP2D6".to_string(),
        gene: Some("CYP2D6".to_string()),
        drug: Some("warfarin sodium".to_string()),
        interactions: Vec::new(),
        recommendations: Vec::new(),
        frequencies: Vec::new(),
        guidelines: Vec::new(),
        annotations: Vec::new(),
        annotations_note: None,
    };

    let related = related_pgx(&pgx);
    assert!(related.contains(&"biomcp search pgx -g CYP2D6".to_string()));
    assert!(related.contains(&"biomcp search pgx -d \"warfarin sodium\"".to_string()));
}

#[test]
fn related_disease_malformed_study_lookup_falls_back_to_download_list() {
    let _guard = crate::test_support::env_lock().blocking_lock();
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
        variants: Vec::new(),
        top_variant: None,
        models: Vec::new(),
        prevalence: Vec::new(),
        prevalence_note: None,
        survival: None,
        survival_note: None,
        civic: Some(crate::sources::civic::CivicContext::default()),
        disgenet: None,
        funding: None,
        funding_note: None,
        xrefs: std::collections::HashMap::new(),
    };

    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "biomcp-render-study-malformed-{}-{unique}",
        std::process::id()
    ));
    let study_dir = root.join("broken-study");
    std::fs::create_dir_all(&study_dir).expect("create malformed study dir");
    std::fs::write(
        study_dir.join("meta_study.txt"),
        "name: Missing identifier\n",
    )
    .expect("write malformed meta");
    std::fs::write(
            study_dir.join("data_mutations.txt"),
            "Hugo_Symbol\tTumor_Sample_Barcode\tVariant_Classification\tHGVSp_Short\nTP53\tS1\tMissense_Mutation\tp.R175H\n",
        )
        .expect("write mutations");

    let original = std::env::var_os("BIOMCP_STUDY_DIR");
    unsafe { std::env::set_var("BIOMCP_STUDY_DIR", &root) };
    let related = related_disease(&disease);
    match original {
        Some(value) => unsafe { std::env::set_var("BIOMCP_STUDY_DIR", value) },
        None => unsafe { std::env::remove_var("BIOMCP_STUDY_DIR") },
    }
    let _ = std::fs::remove_dir_all(&root);

    assert!(related.contains(&"biomcp study download --list".to_string()));
}

#[test]
fn related_device_event_uses_supported_search_subcommands() {
    let event = DeviceEvent {
        report_id: "MDR-123".to_string(),
        report_number: None,
        device: "Infusion Pump".to_string(),
        manufacturer: None,
        event_type: None,
        date: None,
        description: None,
    };

    let related = related_device_event(&event);
    assert!(related.contains(
        &"biomcp search adverse-event --type device --device \"Infusion Pump\"".to_string()
    ));
    assert!(related.contains(
        &"biomcp search adverse-event --type recall --classification \"Class I\"".to_string()
    ));
}

#[test]
fn related_protein_includes_complexes_follow_up() {
    let protein = Protein {
        accession: "P15056".to_string(),
        entry_id: Some("BRAF_HUMAN".to_string()),
        name: "Serine/threonine-protein kinase B-raf".to_string(),
        gene_symbol: Some("BRAF".to_string()),
        organism: Some("Homo sapiens".to_string()),
        length: Some(766),
        function: None,
        structures: vec!["6V34".to_string()],
        structure_count: Some(1),
        domains: Vec::new(),
        interactions: Vec::new(),
        complexes: Vec::new(),
    };

    let related = related_protein(&protein, &[]);
    assert!(related.contains(&"biomcp get protein P15056 structures".to_string()));
    assert!(related.contains(&"biomcp get protein P15056 complexes".to_string()));
    assert!(related.contains(&"biomcp get gene BRAF".to_string()));
}

#[test]
fn related_protein_excludes_requested_sections() {
    let protein = Protein {
        accession: "P15056".to_string(),
        entry_id: Some("BRAF_HUMAN".to_string()),
        name: "Serine/threonine-protein kinase B-raf".to_string(),
        gene_symbol: Some("BRAF".to_string()),
        organism: Some("Homo sapiens".to_string()),
        length: Some(766),
        function: None,
        structures: vec!["6V34".to_string()],
        structure_count: Some(1),
        domains: Vec::new(),
        interactions: Vec::new(),
        complexes: Vec::new(),
    };

    let related = related_protein(
        &protein,
        &["complexes".to_string(), "structures".to_string()],
    );
    assert!(!related.contains(&"biomcp get protein P15056 structures".to_string()));
    assert!(!related.contains(&"biomcp get protein P15056 complexes".to_string()));
    assert!(related.contains(&"biomcp get gene BRAF".to_string()));
}
