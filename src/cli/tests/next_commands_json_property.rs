use crate::cli::Cli;
use clap::Parser;
use serde::Serialize;

use crate::entities::adverse_event::{AdverseEvent, AdverseEventReport, DeviceEvent};
use crate::entities::article::{AnnotationCount, Article, ArticleAnnotations};
use crate::entities::disease::Disease;
use crate::entities::drug::Drug;
use crate::entities::gene::Gene;
use crate::entities::pathway::Pathway;
use crate::entities::pgx::Pgx;
use crate::entities::protein::Protein;
use crate::entities::trial::Trial;
use crate::entities::variant::Variant;

fn collect_next_commands(json: &str) -> Vec<String> {
    let value: serde_json::Value = serde_json::from_str(json).expect("valid json");
    value["_meta"]["next_commands"]
        .as_array()
        .expect("next_commands array")
        .iter()
        .map(|cmd| cmd.as_str().expect("command string").to_string())
        .collect()
}

fn assert_json_next_commands_parse(label: &str, json: &str) {
    let value: serde_json::Value =
        serde_json::from_str(json).unwrap_or_else(|e| panic!("{label}: invalid json: {e}"));
    let cmds = value["_meta"]["next_commands"]
        .as_array()
        .unwrap_or_else(|| panic!("{label}: missing _meta.next_commands"));
    assert!(
        !cmds.is_empty(),
        "{label}: expected at least one next_command"
    );
    for cmd in cmds {
        let cmd = cmd
            .as_str()
            .unwrap_or_else(|| panic!("{label}: next_command was not a string"));
        let argv = shlex::split(cmd).unwrap_or_else(|| panic!("{label}: shlex failed on: {cmd}"));
        Cli::try_parse_from(argv)
            .unwrap_or_else(|e| panic!("{label}: failed to parse '{cmd}': {e}"));
    }
}

fn assert_entity_json_next_commands<T: Serialize>(
    label: &str,
    entity: &T,
    evidence_urls: Vec<(&'static str, String)>,
    next_commands: Vec<String>,
    section_sources: Vec<crate::render::provenance::SectionSource>,
) {
    let json =
        crate::render::json::to_entity_json(entity, evidence_urls, next_commands, section_sources)
            .unwrap_or_else(|e| panic!("{label}: failed to render entity json: {e}"));
    assert_json_next_commands_parse(label, &json);
}

#[test]
fn gene_json_next_commands_parse() {
    let gene = Gene {
        symbol: "BRAF".to_string(),
        name: "B-Raf proto-oncogene".to_string(),
        entrez_id: "673".to_string(),
        ensembl_id: Some("ENSG00000157764".to_string()),
        location: Some("7q34".to_string()),
        genomic_coordinates: None,
        omim_id: Some("164757".to_string()),
        uniprot_id: Some("P15056".to_string()),
        summary: None,
        gene_type: None,
        aliases: Vec::new(),
        clinical_diseases: Vec::new(),
        clinical_drugs: Vec::new(),
        pathways: None,
        ontology: None,
        diseases: None,
        protein: None,
        go: None,
        interactions: None,
        civic: None,
        expression: None,
        hpa: None,
        druggability: None,
        clingen: None,
        constraint: None,
        disgenet: None,
        funding: None,
        funding_note: None,
    };

    assert_entity_json_next_commands(
        "gene",
        &gene,
        crate::render::markdown::gene_evidence_urls(&gene),
        crate::render::markdown::related_gene(&gene),
        crate::render::provenance::gene_section_sources(&gene),
    );
}

#[test]
fn gene_json_next_commands_include_clingen_trial_search() {
    let gene = Gene {
        symbol: "SCN1A".to_string(),
        name: "sodium voltage-gated channel alpha subunit 1".to_string(),
        entrez_id: "6323".to_string(),
        ensembl_id: Some("ENSG00000144285".to_string()),
        location: Some("2q24.3".to_string()),
        genomic_coordinates: None,
        omim_id: Some("182389".to_string()),
        uniprot_id: Some("P35498".to_string()),
        summary: None,
        gene_type: None,
        aliases: Vec::new(),
        clinical_diseases: Vec::new(),
        clinical_drugs: Vec::new(),
        pathways: None,
        ontology: None,
        diseases: None,
        protein: None,
        go: None,
        interactions: None,
        civic: None,
        expression: None,
        hpa: None,
        druggability: None,
        clingen: Some(crate::sources::clingen::GeneClinGen {
            validity: vec![crate::sources::clingen::ClinGenValidity {
                disease: "genetic developmental and epileptic encephalopathy".to_string(),
                classification: "Definitive".to_string(),
                review_date: Some("2025-12-16".to_string()),
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

    let next_commands = crate::render::markdown::related_gene(&gene);
    let json = crate::render::json::to_entity_json(
        &gene,
        crate::render::markdown::gene_evidence_urls(&gene),
        next_commands,
        crate::render::provenance::gene_section_sources(&gene),
    )
    .expect("gene json");
    assert_json_next_commands_parse("gene-clingen", &json);
    assert!(collect_next_commands(&json).contains(
        &"biomcp search trial -c \"genetic developmental and epileptic encephalopathy\" -s recruiting"
            .to_string()
    ));
}

#[test]
fn batch_protein_json_omits_requested_section_from_next_commands() {
    let protein = Protein {
        accession: "P00533".to_string(),
        entry_id: Some("EGFR_HUMAN".to_string()),
        name: "Epidermal growth factor receptor".to_string(),
        gene_symbol: Some("EGFR".to_string()),
        organism: None,
        length: None,
        function: None,
        structures: Vec::new(),
        structure_count: None,
        domains: Vec::new(),
        interactions: Vec::new(),
        complexes: Vec::new(),
    };
    let requested_sections = ["complexes".to_string()];
    let json = crate::cli::render_batch_json(std::slice::from_ref(&protein), |item| {
        crate::render::json::to_entity_json_value(
            item,
            crate::render::markdown::protein_evidence_urls(item),
            crate::render::markdown::related_protein(item, &requested_sections),
            crate::render::provenance::protein_section_sources(item),
        )
    })
    .expect("batch json");

    let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    let commands = value[0]["_meta"]["next_commands"]
        .as_array()
        .expect("next_commands array")
        .iter()
        .map(|cmd| cmd.as_str().expect("command string"))
        .collect::<Vec<_>>();

    assert!(
        !commands.contains(&"biomcp get protein P00533 complexes"),
        "requested section should not be suggested again: {value}"
    );
    assert!(
        commands.contains(&"biomcp get protein P00533 structures"),
        "expected structures follow-up: {value}"
    );
    assert!(
        commands.contains(&"biomcp get gene EGFR"),
        "expected linked gene follow-up: {value}"
    );
}

#[test]
fn article_json_next_commands_parse() {
    let article = Article {
        pmid: Some("22663011".to_string()),
        pmcid: Some("PMC9984800".to_string()),
        doi: Some("10.1056/NEJMoa1203421".to_string()),
        title: "Example about melanoma".to_string(),
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
            genes: vec![AnnotationCount {
                text: "serine-threonine protein kinase".to_string(),
                count: 1,
            }],
            diseases: vec![AnnotationCount {
                text: "melanoma".to_string(),
                count: 1,
            }],
            chemicals: vec![AnnotationCount {
                text: "osimertinib".to_string(),
                count: 1,
            }],
            mutations: Vec::new(),
        }),
        semantic_scholar: None,
        pubtator_fallback: false,
    };
    let next_commands = crate::render::markdown::related_article(&article);
    assert!(
        next_commands
            .iter()
            .any(|cmd| { cmd == "biomcp search gene -q \"serine-threonine protein kinase\"" })
    );
    assert!(
        !next_commands
            .iter()
            .any(|cmd| cmd == "biomcp get gene serine-threonine protein kinase")
    );

    assert_entity_json_next_commands(
        "article",
        &article,
        crate::render::markdown::article_evidence_urls(&article),
        next_commands,
        crate::render::provenance::article_section_sources(&article),
    );
}

#[test]
fn disease_json_next_commands_parse() {
    let disease = Disease {
        id: "MONDO:0004992".to_string(),
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

    assert_entity_json_next_commands(
        "disease",
        &disease,
        crate::render::markdown::disease_evidence_urls(&disease),
        crate::render::markdown::related_disease(&disease),
        crate::render::provenance::disease_section_sources(&disease),
    );
}

#[test]
fn disease_json_next_commands_include_top_gene_context() {
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

    let next_commands = crate::render::markdown::related_disease(&disease);
    let json = crate::render::json::to_entity_json(
        &disease,
        crate::render::markdown::disease_evidence_urls(&disease),
        next_commands,
        crate::render::provenance::disease_section_sources(&disease),
    )
    .expect("disease json");
    assert_json_next_commands_parse("disease-top-gene", &json);
    assert!(
        collect_next_commands(&json)
            .contains(&"biomcp get gene SCN1A clingen constraint".to_string())
    );
}

#[test]
fn pgx_json_next_commands_parse() {
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

    assert_entity_json_next_commands(
        "pgx",
        &pgx,
        crate::render::markdown::pgx_evidence_urls(&pgx),
        crate::render::markdown::related_pgx(&pgx),
        crate::render::provenance::pgx_section_sources(&pgx),
    );
}

#[test]
fn trial_json_next_commands_parse() {
    let trial = Trial {
        nct_id: "NCT01234567".to_string(),
        source: None,
        title: "Example trial".to_string(),
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
    let next_commands = crate::render::markdown::related_trial(&trial);
    assert!(next_commands.iter().any(|cmd| {
        cmd == "biomcp search article --drug dabrafenib -q \"NCT01234567 Example trial\" --limit 5"
    }));

    assert_entity_json_next_commands(
        "trial",
        &trial,
        crate::render::markdown::trial_evidence_urls(&trial),
        next_commands,
        crate::render::provenance::trial_section_sources(&trial),
    );
}

#[test]
fn variant_json_next_commands_parse() {
    let variant: Variant = serde_json::from_value(serde_json::json!({
        "id": "rs113488022",
        "gene": "BRAF",
        "hgvs_p": "p.V600E",
        "rsid": "rs113488022"
    }))
    .expect("variant should deserialize");

    assert_entity_json_next_commands(
        "variant",
        &variant,
        crate::render::markdown::variant_evidence_urls(&variant),
        crate::render::markdown::related_variant(&variant),
        crate::render::provenance::variant_section_sources(&variant),
    );
}

#[test]
fn variant_json_next_commands_include_vus_literature_route() {
    let variant: Variant = serde_json::from_value(serde_json::json!({
        "id": "chr2:g.166848047C>G",
        "gene": "SCN1A",
        "hgvs_p": "p.T1174S",
        "legacy_name": "SCN1A T1174S",
        "significance": "Uncertain significance",
        "top_disease": {"condition": "Dravet syndrome", "reports": 7}
    }))
    .expect("variant should deserialize");

    let next_commands = crate::render::markdown::related_variant(&variant);
    let json = crate::render::json::to_entity_json(
        &variant,
        crate::render::markdown::variant_evidence_urls(&variant),
        next_commands,
        crate::render::provenance::variant_section_sources(&variant),
    )
    .expect("variant json");
    assert_json_next_commands_parse("variant-vus", &json);
    assert!(
        collect_next_commands(&json).contains(
            &"biomcp search article -g SCN1A -d \"Dravet syndrome\" -k \"T1174S\" --limit 5"
                .to_string()
        )
    );
}

#[test]
fn drug_json_next_commands_parse() {
    let drug = Drug {
        name: "osimertinib".to_string(),
        drugbank_id: Some("DB09330".to_string()),
        chembl_id: Some("CHEMBL3353410".to_string()),
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
        targets: vec!["EGFR".to_string()],
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

    assert_entity_json_next_commands(
        "drug",
        &drug,
        crate::render::markdown::drug_evidence_urls(&drug),
        crate::render::markdown::related_drug(&drug),
        crate::render::provenance::drug_section_sources(&drug),
    );
}

#[test]
fn pathway_json_next_commands_parse() {
    let pathway = Pathway {
        source: "KEGG".to_string(),
        id: "hsa05200".to_string(),
        name: "Pathways in cancer".to_string(),
        species: None,
        summary: None,
        genes: Vec::new(),
        events: Vec::new(),
        enrichment: Vec::new(),
    };

    let next_commands = crate::render::markdown::related_pathway(&pathway);
    assert_eq!(
        next_commands,
        vec!["biomcp pathway drugs hsa05200".to_string()]
    );
    assert!(
        next_commands
            .iter()
            .all(|cmd| !cmd.contains("get pathway hsa05200")),
        "pathway next_commands should not repeat the current flow"
    );
    assert!(
        next_commands
            .iter()
            .all(|cmd| !cmd.contains("events") && !cmd.contains("enrichment")),
        "pathway next_commands should not suggest unsupported sections"
    );

    assert_entity_json_next_commands(
        "pathway",
        &pathway,
        crate::render::markdown::pathway_evidence_urls(&pathway),
        next_commands,
        crate::render::provenance::pathway_section_sources(&pathway),
    );
}

#[test]
fn protein_json_next_commands_parse() {
    let protein = Protein {
        accession: "P00533".to_string(),
        entry_id: Some("EGFR_HUMAN".to_string()),
        name: "Epidermal growth factor receptor".to_string(),
        gene_symbol: Some("EGFR".to_string()),
        organism: None,
        length: None,
        function: None,
        structures: Vec::new(),
        structure_count: None,
        domains: Vec::new(),
        interactions: Vec::new(),
        complexes: Vec::new(),
    };

    let base_next_commands = crate::render::markdown::related_protein(&protein, &[]);
    assert!(base_next_commands.contains(&"biomcp get protein P00533 structures".to_string()));
    assert!(base_next_commands.contains(&"biomcp get protein P00533 complexes".to_string()));

    let section_next_commands =
        crate::render::markdown::related_protein(&protein, &["complexes".to_string()]);
    assert!(!section_next_commands.contains(&"biomcp get protein P00533 complexes".to_string()));
    assert!(section_next_commands.contains(&"biomcp get protein P00533 structures".to_string()));
    assert!(section_next_commands.contains(&"biomcp get gene EGFR".to_string()));

    assert_entity_json_next_commands(
        "protein",
        &protein,
        crate::render::markdown::protein_evidence_urls(&protein),
        section_next_commands,
        crate::render::provenance::protein_section_sources(&protein),
    );
}

#[test]
fn batch_adverse_event_json_uses_variant_specific_meta() {
    let faers = AdverseEvent {
        report_id: "1001".to_string(),
        drug: "osimertinib".to_string(),
        reactions: Vec::new(),
        outcomes: Vec::new(),
        patient: None,
        concomitant_medications: Vec::new(),
        reporter_type: None,
        reporter_country: None,
        indication: None,
        serious: true,
        date: None,
    };
    let device = DeviceEvent {
        report_id: "MDR-123".to_string(),
        report_number: None,
        device: "HeartValve".to_string(),
        manufacturer: None,
        event_type: None,
        date: None,
        description: None,
    };
    let reports = vec![
        AdverseEventReport::Faers(faers),
        AdverseEventReport::Device(device),
    ];

    let json = crate::cli::render_batch_json(&reports, |item| match item {
        AdverseEventReport::Faers(report) => crate::render::json::to_entity_json_value(
            item,
            crate::render::markdown::adverse_event_evidence_urls(report),
            crate::render::markdown::related_adverse_event(report),
            crate::render::provenance::adverse_event_report_section_sources(item),
        ),
        AdverseEventReport::Device(report) => crate::render::json::to_entity_json_value(
            item,
            crate::render::markdown::device_event_evidence_urls(report),
            crate::render::markdown::related_device_event(report),
            crate::render::provenance::adverse_event_report_section_sources(item),
        ),
    })
    .expect("batch json");

    let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    let items = value.as_array().expect("batch array");
    assert_eq!(items.len(), 2, "json={value}");
    assert_eq!(items[0]["_meta"]["evidence_urls"][0]["label"], "OpenFDA");
    assert_eq!(items[1]["_meta"]["evidence_urls"][0]["label"], "OpenFDA");
    assert!(
        items[0]["_meta"]["evidence_urls"][0]["url"]
            .as_str()
            .is_some_and(|url| url.contains("/drug/event.json")),
        "faers report should use drug event evidence url: {value}"
    );
    assert!(
        items[1]["_meta"]["evidence_urls"][0]["url"]
            .as_str()
            .is_some_and(|url| url.contains("/device/event.json")),
        "device report should use device event evidence url: {value}"
    );
    assert!(
        items.iter().all(|item| item["_meta"]["next_commands"]
            .as_array()
            .is_some_and(|cmds| !cmds.is_empty())),
        "each report should retain next commands: {value}"
    );
}

#[test]
fn faers_json_next_commands_parse() {
    let faers = AdverseEvent {
        report_id: "1001".to_string(),
        drug: "osimertinib".to_string(),
        reactions: Vec::new(),
        outcomes: Vec::new(),
        patient: None,
        concomitant_medications: Vec::new(),
        reporter_type: None,
        reporter_country: None,
        indication: None,
        serious: true,
        date: None,
    };
    let report = AdverseEventReport::Faers(faers.clone());

    assert_entity_json_next_commands(
        "adverse-event-faers",
        &report,
        crate::render::markdown::adverse_event_evidence_urls(&faers),
        crate::render::markdown::related_adverse_event(&faers),
        crate::render::provenance::adverse_event_report_section_sources(&report),
    );
}

#[test]
fn device_event_json_next_commands_parse() {
    let device = DeviceEvent {
        report_id: "MDR-123".to_string(),
        report_number: None,
        device: "HeartValve".to_string(),
        manufacturer: None,
        event_type: None,
        date: None,
        description: None,
    };
    let report = AdverseEventReport::Device(device.clone());

    assert_entity_json_next_commands(
        "adverse-event-device",
        &report,
        crate::render::markdown::device_event_evidence_urls(&device),
        crate::render::markdown::related_device_event(&device),
        crate::render::provenance::adverse_event_report_section_sources(&report),
    );
}
