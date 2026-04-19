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
        diagnostics: None,
        diagnostics_note: None,
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
