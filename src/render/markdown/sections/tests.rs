use super::*;

#[test]
fn sections_pathway_for_kegg_excludes_unsupported_sections() {
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

    let sections = sections_pathway(&pathway, &[]);
    assert_eq!(sections, vec!["genes".to_string()]);
}

#[test]
fn sections_pathway_for_reactome_keeps_full_supported_set() {
    let pathway = Pathway {
        source: "Reactome".to_string(),
        id: "R-HSA-5673001".to_string(),
        name: "RAF/MAP kinase cascade".to_string(),
        species: None,
        summary: None,
        genes: Vec::new(),
        events: Vec::new(),
        enrichment: Vec::new(),
    };

    let sections = sections_pathway(&pathway, &[]);
    assert_eq!(
        sections,
        vec![
            "genes".to_string(),
            "events".to_string(),
            "enrichment".to_string()
        ]
    );
}

#[test]
fn format_sections_block_renders_described_executable_commands() {
    let block = format_sections_block(
        "gene",
        "TP53",
        vec![
            "pathways".to_string(),
            "hpa".to_string(),
            "diseases".to_string(),
            "protein".to_string(),
        ],
    );

    assert!(block.contains("More:"));
    assert!(block.contains("biomcp get gene TP53 pathways"));
    assert!(block.contains("Reactome/KEGG pathway context"));
    assert!(block.contains("biomcp get gene TP53 hpa"));
    assert!(block.contains("Human Protein Atlas tissue expression and localization"));
    assert!(block.contains("biomcp get gene TP53 diseases"));
    assert!(block.contains("disease associations"));
    assert!(block.contains("All:"));
    assert!(block.contains("biomcp get gene TP53 all"));
}

#[test]
fn format_sections_block_keeps_gene_ontology_in_top_more_entries() {
    let block = format_sections_block(
        "gene",
        "NANOG",
        vec![
            "pathways".to_string(),
            "ontology".to_string(),
            "diseases".to_string(),
            "protein".to_string(),
        ],
    );

    let pathways = block
        .find("biomcp get gene NANOG pathways")
        .expect("pathways command");
    let ontology = block
        .find("biomcp get gene NANOG ontology")
        .expect("ontology command");
    let diseases = block
        .find("biomcp get gene NANOG diseases")
        .expect("diseases command");
    assert!(pathways < ontology);
    assert!(ontology < diseases);
}

#[test]
fn format_sections_block_describes_guardrailed_drug_and_trial_sections() {
    let drug_block = format_sections_block(
        "drug",
        "pembrolizumab",
        vec![
            "label".to_string(),
            "regulatory".to_string(),
            "safety".to_string(),
        ],
    );

    assert!(drug_block.contains(
            "biomcp get drug pembrolizumab label   - approved-indication and FDA label detail beyond the base card"
        ));
    assert!(drug_block.contains(
            "biomcp get drug pembrolizumab regulatory   - approval and supplement history; use only if the base card lacks approval context"
        ));
    assert!(drug_block.contains(
            "biomcp get drug pembrolizumab safety   - regulatory safety detail; use `biomcp drug adverse-events <name>` first when you want post-marketing signal"
        ));

    let terminated = crate::entities::trial::Trial {
        nct_id: "NCT02576665".to_string(),
        source: None,
        title: "Completed trial".to_string(),
        status: "TERMINATED".to_string(),
        phase: None,
        study_type: None,
        age_range: None,
        conditions: vec!["melanoma".to_string()],
        interventions: vec!["trametinib".to_string()],
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
    let terminated_sections = sections_trial(&terminated, &[]);
    assert_eq!(terminated_sections[0], "outcomes");
    assert_eq!(terminated_sections[1], "references");
    assert_eq!(terminated_sections[2], "arms");

    let trial_block =
        format_sections_block("trial", &terminated.nct_id, terminated_sections.clone());
    assert!(
        trial_block.contains(
            "biomcp get trial NCT02576665 outcomes   - endpoint measures and time frames"
        )
    );
    assert!(trial_block.contains(
        "biomcp get trial NCT02576665 references   - linked publications and PMID citations"
    ));
    assert!(
        trial_block.contains(
            "biomcp get trial NCT02576665 arms   - study arms and assigned interventions"
        )
    );

    let recruiting = crate::entities::trial::Trial {
        status: "Recruiting".to_string(),
        ..terminated
    };
    let recruiting_sections = sections_trial(&recruiting, &[]);
    assert_eq!(recruiting_sections[0], "eligibility");
    assert_eq!(recruiting_sections[1], "locations");
    assert_eq!(recruiting_sections[2], "outcomes");
}
