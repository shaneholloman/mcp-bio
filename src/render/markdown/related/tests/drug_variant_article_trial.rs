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

