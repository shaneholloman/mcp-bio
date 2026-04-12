//! Cross-cutting markdown tests pending full sidecar extraction.

use super::*;
use crate::entities::adverse_event::DeviceEvent;
use crate::entities::article::{
    AnnotationCount, Article, ArticleAnnotations, ArticleRankingOptions, ArticleSearchFilters,
    ArticleSearchResult, ArticleSource,
};
use crate::entities::disease::{Disease, DiseaseVariantAssociation};
use crate::entities::drug::Drug;
use crate::entities::gene::Gene;
use crate::entities::pathway::Pathway;
use crate::entities::pgx::Pgx;
use crate::entities::study::{
    CnaDistributionResult as StudyCnaDistributionResult, CoOccurrencePair as StudyCoOccurrencePair,
    CoOccurrenceResult as StudyCoOccurrenceResult, CohortResult as StudyCohortResult,
    ExpressionComparisonResult as StudyExpressionComparisonResult,
    ExpressionDistributionResult as StudyExpressionDistributionResult,
    ExpressionGroupStats as StudyExpressionGroupStats,
    MutationComparisonResult as StudyMutationComparisonResult,
    MutationFrequencyResult as StudyMutationFrequencyResult,
    MutationGroupStats as StudyMutationGroupStats, SampleUniverseBasis as StudySampleUniverseBasis,
    StudyDownloadCatalog, StudyDownloadResult, StudyInfo, StudyQueryResult,
    SurvivalEndpoint as StudySurvivalEndpoint, SurvivalGroupResult as StudySurvivalGroupResult,
    SurvivalResult as StudySurvivalResult, TopMutatedGeneRow as StudyTopMutatedGeneRow,
    TopMutatedGenesResult as StudyTopMutatedGenesResult,
};
use crate::entities::variant::{TreatmentImplication, Variant, VariantOncoKbResult};

fn article_filters_for_test(sort: ArticleSort) -> ArticleSearchFilters {
    ArticleSearchFilters {
        gene: None,
        gene_anchored: false,
        disease: None,
        drug: None,
        author: None,
        keyword: None,
        date_from: None,
        date_to: None,
        article_type: None,
        journal: None,
        open_access: false,
        no_preprints: true,
        exclude_retracted: true,
        max_per_source: None,
        sort,
        ranking: ArticleRankingOptions::default(),
    }
}

#[test]
fn proof_markdown_module_layout_uses_directory_module() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let markdown_dir = root.join("src/render/markdown");

    assert!(
        markdown_dir.is_dir(),
        "expected src/render/markdown/ to exist"
    );
    assert!(
        markdown_dir.join("mod.rs").is_file(),
        "expected src/render/markdown/mod.rs to exist"
    );
    assert!(
        markdown_dir.join("tests.rs").is_file(),
        "expected src/render/markdown/tests.rs to exist"
    );
    assert!(
        markdown_dir.join("root_tests.rs").is_file(),
        "expected src/render/markdown/root_tests.rs to exist"
    );
    assert!(
        markdown_dir.join("test_support.rs").is_file(),
        "expected src/render/markdown/test_support.rs to exist"
    );
    assert!(
        !root.join("src/render/markdown.rs").exists(),
        "expected src/render/markdown.rs to be removed"
    );
}

#[test]
fn trial_search_markdown_with_footer_shows_scoped_zero_result_nickname_hint() {
    let markdown = trial_search_markdown_with_footer(
        "condition=CodeBreaK 300",
        &[],
        Some(0),
        "",
        true,
        Some("CodeBreaK 300"),
    )
    .expect("markdown");

    assert!(markdown.contains("ClinicalTrials.gov does not index trial nicknames."));
    assert!(markdown.contains("biomcp search trial -i \"<drug>\" -c \"<condition>\""));
    assert!(markdown.contains("biomcp search article \"CodeBreaK 300\" to find the NCT ID"));
}

#[test]
fn trial_search_markdown_with_footer_omits_zero_result_nickname_hint_without_flag() {
    let markdown =
        trial_search_markdown_with_footer("condition=melanoma", &[], Some(0), "", false, None)
            .expect("markdown");

    assert!(!markdown.contains("ClinicalTrials.gov does not index trial nicknames."));
}

#[test]
fn gene_markdown_includes_evidence_links() {
    let gene = Gene {
        symbol: "BRAF".to_string(),
        name: "B-Raf proto-oncogene".to_string(),
        entrez_id: "673".to_string(),
        ensembl_id: Some("ENSG00000157764".to_string()),
        location: Some("7q34".to_string()),
        genomic_coordinates: None,
        omim_id: None,
        uniprot_id: Some("P15056".to_string()),
        summary: Some("Kinase involved in MAPK signaling.".to_string()),
        gene_type: Some("protein-coding".to_string()),
        aliases: vec!["BRAF1".to_string()],
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

    let markdown = gene_markdown(&gene, &[]).expect("rendered markdown");
    assert!(markdown.contains("BRAF"));
    assert!(markdown.contains("[NCBI Gene](https://www.ncbi.nlm.nih.gov/gene/673)"));
    assert!(markdown.contains("[UniProt](https://www.uniprot.org/uniprot/P15056)"));
}

#[test]
fn markdown_detail_outputs_label_gene_drug_and_disease_sources() {
    let gene = Gene {
        symbol: "CFTR".to_string(),
        name: "CF transmembrane conductance regulator".to_string(),
        entrez_id: "1080".to_string(),
        ensembl_id: None,
        location: Some("7q31.2".to_string()),
        genomic_coordinates: None,
        omim_id: None,
        uniprot_id: Some("P13569".to_string()),
        summary: Some("Chloride channel.".to_string()),
        gene_type: Some("protein-coding".to_string()),
        aliases: vec!["ABCC7".to_string()],
        clinical_diseases: Vec::new(),
        clinical_drugs: Vec::new(),
        pathways: None,
        ontology: Some(vec![crate::entities::gene::EnrichmentResult {
            library: "GO_Biological_Process_2025".to_string(),
            terms: vec![crate::entities::gene::EnrichmentTerm {
                name: "ion transport".to_string(),
                p_value: 0.001,
                genes: "CFTR".to_string(),
            }],
        }]),
        diseases: Some(vec![crate::entities::gene::EnrichmentResult {
            library: "DisGeNET".to_string(),
            terms: vec![crate::entities::gene::EnrichmentTerm {
                name: "cystic fibrosis".to_string(),
                p_value: 0.002,
                genes: "CFTR".to_string(),
            }],
        }]),
        protein: Some(crate::entities::gene::GeneProtein {
            accession: "P13569".to_string(),
            name: "CFTR".to_string(),
            function: Some("ATP-binding cassette transporter.".to_string()),
            length: Some(1480),
            isoforms: Vec::new(),
            alternative_names: Vec::new(),
        }),
        go: Some(vec![crate::entities::gene::GeneGoTerm {
            id: "GO:0006811".to_string(),
            name: "ion transport".to_string(),
            aspect: Some("BP".to_string()),
            evidence: Some("EXP".to_string()),
        }]),
        interactions: Some(vec![crate::entities::gene::GeneInteraction {
            partner: "SLC26A9".to_string(),
            score: Some(0.83),
        }]),
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
    let gene_markdown = gene_markdown(&gene, &[]).expect("gene markdown");
    assert!(gene_markdown.contains("Source: NCBI Gene / MyGene.info"));
    assert!(gene_markdown.contains("## Summary (NCBI Gene)"));
    assert!(gene_markdown.contains("## Aliases (NCBI Gene / MyGene.info)"));
    assert!(gene_markdown.contains("## Ontology (Enrichr)"));
    assert!(gene_markdown.contains("## Diseases (Enrichr)"));
    assert!(gene_markdown.contains("## Protein (UniProt)"));
    assert!(gene_markdown.contains("## GO Terms (QuickGO)"));
    assert!(gene_markdown.contains("## Interactions (STRING)"));
    assert!(gene_markdown.contains("See also:"));
    assert!(gene_markdown.contains("biomcp search variant -g CFTR"));

    let drug = Drug {
        name: "ivacaftor".to_string(),
        drugbank_id: Some("DB08820".to_string()),
        chembl_id: Some("CHEMBL1200749".to_string()),
        unii: None,
        drug_type: Some("small molecule".to_string()),
        mechanism: Some("CFTR potentiator".to_string()),
        mechanisms: vec!["Potentiates CFTR chloride transport.".to_string()],
        approval_date: Some("2012-01-31".to_string()),
        approval_date_raw: Some("2012-01-31".to_string()),
        approval_date_display: Some("January 31, 2012".to_string()),
        approval_summary: Some("FDA approved on January 31, 2012".to_string()),
        brand_names: vec!["Kalydeco".to_string()],
        route: Some("Oral".to_string()),
        targets: vec!["CFTR".to_string()],
        variant_targets: Vec::new(),
        target_family: None,
        target_family_name: None,
        indications: vec!["Cystic fibrosis".to_string()],
        interactions: vec![crate::entities::drug::DrugInteraction {
            drug: "rifampin".to_string(),
            description: Some("May reduce ivacaftor exposure.".to_string()),
        }],
        interaction_text: None,
        pharm_classes: Vec::new(),
        top_adverse_events: vec!["Cough".to_string()],
        faers_query: None,
        label: None,
        label_set_id: None,
        shortage: Some(Vec::new()),
        approvals: Some(Vec::new()),
        us_safety_warnings: None,
        ema_regulatory: None,
        ema_safety: None,
        ema_shortage: None,
        who_prequalification: None,
        civic: None,
    };
    let drug_markdown = drug_markdown(&drug, &["all".to_string()]).expect("drug markdown");
    assert!(drug_markdown.contains("Type (MyChem.info): small molecule"));
    assert!(drug_markdown.contains("FDA Approved (DrugCentral): January 31, 2012"));
    assert!(drug_markdown.contains("Brand Names (DrugBank): Kalydeco"));
    assert!(drug_markdown.contains("Safety (OpenFDA FAERS): Cough"));
    assert!(drug_markdown.contains("## Mechanisms (MyChem.info / ChEMBL)"));
    assert!(drug_markdown.contains("## Targets (ChEMBL / Open Targets)"));
    assert!(drug_markdown.contains("## Indications (Open Targets)"));
    assert!(drug_markdown.contains("## Interactions (DrugBank)"));
    assert!(drug_markdown.contains("## Shortage (US - OpenFDA Drug Shortages)"));
    assert!(drug_markdown.contains("## Regulatory (US - Drugs@FDA)"));

    let disease = crate::entities::disease::Disease {
        id: "MONDO:0009061".to_string(),
        name: "cystic fibrosis".to_string(),
        definition: Some("Inherited disease affecting chloride transport.".to_string()),
        synonyms: vec!["CF".to_string()],
        parents: vec!["autosomal recessive disease".to_string()],
        associated_genes: vec!["CFTR".to_string()],
        gene_associations: Vec::new(),
        top_genes: vec!["CFTR".to_string()],
        top_gene_scores: Vec::new(),
        treatment_landscape: vec!["ivacaftor".to_string()],
        recruiting_trial_count: Some(4),
        pathways: vec![crate::entities::disease::DiseasePathway {
            id: "R-HSA-5673001".to_string(),
            name: "Ion channel transport".to_string(),
        }],
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
    let disease_markdown =
        disease_markdown(&disease, &["all".to_string()]).expect("disease markdown");
    assert!(disease_markdown.contains("## Definition (MyDisease.info)"));
    assert!(disease_markdown.contains("Inherited disease affecting chloride transport."));
    assert!(disease_markdown.contains("Genes (Open Targets): CFTR"));
    assert!(disease_markdown.contains("Treatments (MyChem.info indication search): ivacaftor"));
    assert!(disease_markdown.contains("Recruiting Trials (ClinicalTrials.gov): 4"));
    assert!(disease_markdown.contains("## Synonyms (MONDO / Disease Ontology via MyDisease.info)"));
    assert!(disease_markdown.contains("## Parents (MONDO / Disease Ontology via MyDisease.info)"));
    assert!(disease_markdown.contains("## Pathways (Reactome)"));
}

#[test]
fn markdown_detail_outputs_label_article_trial_and_pathway_sources() {
    let article = Article {
        pmid: Some("22663011".to_string()),
        pmcid: Some("PMC9984800".to_string()),
        doi: Some("10.1000/example".to_string()),
        title: "Example article".to_string(),
        authors: vec!["A. Author".to_string()],
        journal: Some("Example Journal".to_string()),
        date: Some("2012-05-31".to_string()),
        citation_count: Some(12),
        publication_type: Some("Journal Article".to_string()),
        open_access: Some(true),
        abstract_text: Some("Abstract text.".to_string()),
        full_text_path: None,
        full_text_note: Some("Saved full text unavailable.".to_string()),
        annotations: Some(ArticleAnnotations {
            genes: vec![AnnotationCount {
                text: "CFTR".to_string(),
                count: 1,
            }],
            diseases: Vec::new(),
            chemicals: Vec::new(),
            mutations: Vec::new(),
        }),
        semantic_scholar: Some(crate::entities::article::ArticleSemanticScholar {
            paper_id: Some("paper-1".to_string()),
            tldr: Some("TLDR".to_string()),
            citation_count: Some(10),
            influential_citation_count: Some(2),
            reference_count: Some(5),
            is_open_access: Some(true),
            open_access_pdf: None,
        }),
        pubtator_fallback: false,
    };
    let article_markdown = article_markdown(&article, &["all".to_string()]).expect("article");
    assert!(article_markdown.contains("Source: PubMed / Europe PMC"));
    assert!(article_markdown.contains("## Authors (PubMed / Europe PMC)"));
    assert!(article_markdown.contains("## Abstract (PubMed / Europe PMC)"));
    assert!(article_markdown.contains("## PubTator Annotations"));
    assert!(article_markdown.contains("## Full Text (PMC OA)"));
    assert!(article_markdown.contains("## Semantic Scholar"));

    let trial = crate::entities::trial::Trial {
        nct_id: "NCT06668103".to_string(),
        source: Some("ClinicalTrials.gov".to_string()),
        title: "Example trial".to_string(),
        status: "Recruiting".to_string(),
        phase: Some("Phase 2".to_string()),
        study_type: Some("Interventional".to_string()),
        age_range: Some("18 Years and older".to_string()),
        conditions: vec!["cystic fibrosis".to_string()],
        interventions: vec!["ivacaftor".to_string()],
        sponsor: Some("Example Sponsor".to_string()),
        enrollment: Some(42),
        summary: Some("Trial summary.".to_string()),
        start_date: Some("2025-01-01".to_string()),
        completion_date: None,
        eligibility_text: Some("Eligibility text.".to_string()),
        locations: Some(vec![crate::entities::trial::TrialLocation {
            facility: "Example Hospital".to_string(),
            city: "Boston".to_string(),
            state: Some("MA".to_string()),
            country: "United States".to_string(),
            status: Some("Recruiting".to_string()),
            contact_name: None,
            contact_phone: None,
        }]),
        outcomes: Some(crate::entities::trial::TrialOutcomes {
            primary: vec![crate::entities::trial::TrialOutcome {
                measure: "FEV1".to_string(),
                description: None,
                time_frame: None,
            }],
            secondary: Vec::new(),
        }),
        arms: Some(vec![crate::entities::trial::TrialArm {
            label: "Arm A".to_string(),
            arm_type: Some("Experimental".to_string()),
            description: Some("Description".to_string()),
            interventions: vec!["ivacaftor".to_string()],
        }]),
        references: Some(vec![crate::entities::trial::TrialReference {
            pmid: Some("22663011".to_string()),
            citation: "Example citation".to_string(),
            reference_type: Some("background".to_string()),
        }]),
    };
    let trial_markdown = trial_markdown(&trial, &["all".to_string()]).expect("trial");
    assert!(trial_markdown.contains("Source: ClinicalTrials.gov"));
    assert!(trial_markdown.contains("## Conditions (ClinicalTrials.gov)"));
    assert!(trial_markdown.contains("## Interventions (ClinicalTrials.gov)"));
    assert!(trial_markdown.contains("## Summary (ClinicalTrials.gov)"));
    assert!(trial_markdown.contains("## Eligibility (ClinicalTrials.gov)"));
    assert!(trial_markdown.contains("## Locations (ClinicalTrials.gov)"));
    assert!(trial_markdown.contains("## Outcomes (ClinicalTrials.gov)"));
    assert!(trial_markdown.contains("## Arms (ClinicalTrials.gov)"));
    assert!(trial_markdown.contains("## References (ClinicalTrials.gov)"));

    let pathway = Pathway {
        source: "Reactome".to_string(),
        id: "R-HSA-5358351".to_string(),
        name: "Signal transduction".to_string(),
        species: Some("Homo sapiens".to_string()),
        summary: Some("Reactome summary.".to_string()),
        genes: vec!["CFTR".to_string()],
        events: vec!["Channel gating".to_string()],
        enrichment: vec![crate::entities::pathway::PathwayEnrichment {
            source: "Reactome".to_string(),
            id: "R-HSA-1234".to_string(),
            name: "Transport".to_string(),
            p_value: Some(0.001),
        }],
    };
    let pathway_markdown = pathway_markdown(&pathway, &["all".to_string()]).expect("pathway");
    assert!(pathway_markdown.contains("## Summary (Reactome)"));
    assert!(pathway_markdown.contains("## Genes (Reactome)"));
    assert!(pathway_markdown.contains("## Events (Reactome)"));
    assert!(pathway_markdown.contains("## Enrichment (g:Profiler)"));
}

#[test]
fn markdown_detail_outputs_label_variant_protein_pgx_and_openfda_sources() {
    let variant: Variant = serde_json::from_value(serde_json::json!({
        "id": "rs334",
        "gene": "HBB",
        "hgvs_p": "p.Glu7Val",
        "rsid": "rs334",
        "prediction": {
            "expression_lfc": 0.3,
            "splice_score": 0.1,
            "chromatin_score": 0.2,
            "top_gene": "HBB"
        },
        "gnomad_af": 0.01,
        "conservation": {
            "phylop_100way_vertebrate": 0.8
        },
        "expanded_predictions": [
            {"tool": "REVEL", "score": 0.91, "prediction": "Damaging"}
        ],
        "cgi_associations": [
            {"drug": "hydroxyurea", "association": "responsive"}
        ]
    }))
    .expect("variant");
    let variant_markdown = variant_markdown(&variant, &["all".to_string()]).expect("variant");
    assert!(variant_markdown.contains("Source: MyVariant.info / ClinVar"));
    assert!(variant_markdown.contains("## AlphaGenome Prediction"));
    assert!(variant_markdown.contains("## Population (gnomAD via MyVariant.info)"));
    assert!(variant_markdown.contains("## Conservation (MyVariant.info)"));
    assert!(variant_markdown.contains("## Expanded Predictions (MyVariant.info)"));
    assert!(variant_markdown.contains("## CGI Drug Associations (Cancer Genome Interpreter)"));

    let protein = crate::entities::protein::Protein {
        accession: "P15056".to_string(),
        entry_id: Some("BRAF_HUMAN".to_string()),
        name: "Serine/threonine-protein kinase B-raf".to_string(),
        gene_symbol: Some("BRAF".to_string()),
        organism: Some("Homo sapiens".to_string()),
        length: Some(766),
        function: Some("Kinase function.".to_string()),
        structures: vec!["6V34".to_string()],
        structure_count: Some(1),
        domains: vec![crate::entities::protein::ProteinDomain {
            accession: "IPR000719".to_string(),
            name: Some("Protein kinase domain".to_string()),
            domain_type: Some("domain".to_string()),
        }],
        interactions: vec![crate::entities::protein::ProteinInteraction {
            partner: "MEK1".to_string(),
            score: Some(0.92),
        }],
        complexes: vec![crate::entities::protein::ProteinComplex {
            accession: "CPX-1".to_string(),
            name: "BRAF complex".to_string(),
            description: None,
            curation: crate::entities::protein::ProteinComplexCuration::Curated,
            components: vec![crate::entities::protein::ProteinComplexComponent {
                accession: "P15056".to_string(),
                name: "BRAF".to_string(),
                stoichiometry: None,
            }],
        }],
    };
    let protein_markdown = protein_markdown(&protein, &["all".to_string()]).expect("protein");
    assert!(protein_markdown.contains("Source: UniProt"));
    assert!(protein_markdown.contains("## Function (UniProt)"));
    assert!(protein_markdown.contains("## Structures (PDB / AlphaFold via UniProt)"));
    assert!(protein_markdown.contains("## Domains (InterPro)"));
    assert!(protein_markdown.contains("## Interactions (STRING)"));
    assert!(protein_markdown.contains("## Complexes (ComplexPortal)"));

    let pgx = Pgx {
        query: "CYP2D6".to_string(),
        gene: Some("CYP2D6".to_string()),
        drug: Some("codeine".to_string()),
        interactions: vec![crate::entities::pgx::PgxInteraction {
            genesymbol: "CYP2D6".to_string(),
            drugname: "codeine".to_string(),
            cpiclevel: Some("A".to_string()),
            pgxtesting: Some("Recommended".to_string()),
            guidelinename: None,
            guidelineurl: None,
        }],
        recommendations: vec![crate::entities::pgx::PgxRecommendation {
            drugname: "codeine".to_string(),
            phenotype: Some("Poor metabolizer".to_string()),
            activity_score: None,
            implication: None,
            recommendation: Some("Avoid codeine".to_string()),
            classification: Some("Strong".to_string()),
            population: None,
            guidelinename: None,
            guidelineurl: None,
        }],
        frequencies: vec![crate::entities::pgx::PgxFrequency {
            genesymbol: "CYP2D6".to_string(),
            allele: "*4".to_string(),
            population_group: Some("European".to_string()),
            subject_count: None,
            frequency: None,
            min_frequency: None,
            max_frequency: None,
        }],
        guidelines: vec![crate::entities::pgx::PgxGuideline {
            name: "CPIC Guideline".to_string(),
            url: Some("https://example.org/guideline".to_string()),
            genes: vec!["CYP2D6".to_string()],
            drugs: vec!["codeine".to_string()],
        }],
        annotations: Vec::new(),
        annotations_note: Some("PharmGKB note.".to_string()),
    };
    let pgx_markdown = pgx_markdown(&pgx, &["all".to_string()]).expect("pgx");
    assert!(pgx_markdown.contains("Source: CPIC"));
    assert!(pgx_markdown.contains("## Interactions (CPIC)"));
    assert!(pgx_markdown.contains("## Recommendations (CPIC)"));
    assert!(pgx_markdown.contains("## Population Frequencies (CPIC)"));
    assert!(pgx_markdown.contains("| CYP2D6 | *4 | European | - | - |"));
    assert!(pgx_markdown.contains("## Guidelines (CPIC)"));

    let faers = crate::entities::adverse_event::AdverseEvent {
        report_id: "10329882".to_string(),
        drug: "ivacaftor".to_string(),
        reactions: vec!["Cough".to_string()],
        outcomes: vec!["Hospitalization".to_string()],
        patient: None,
        concomitant_medications: vec!["azithromycin".to_string()],
        reporter_type: None,
        reporter_country: None,
        indication: None,
        serious: true,
        date: Some("2024-01-01".to_string()),
    };
    let faers_markdown = adverse_event_markdown(&faers, &["all".to_string()]).expect("faers");
    assert!(faers_markdown.contains("Source: OpenFDA"));
    assert!(faers_markdown.contains("## Reactions (OpenFDA)"));
    assert!(faers_markdown.contains("## Outcomes (OpenFDA)"));
    assert!(faers_markdown.contains("## Concomitant Drugs (OpenFDA)"));

    let device = DeviceEvent {
        report_id: "MDR-123".to_string(),
        report_number: None,
        device: "Infusion Pump".to_string(),
        manufacturer: Some("Example".to_string()),
        event_type: Some("Malfunction".to_string()),
        date: Some("2024-02-01".to_string()),
        description: Some("Description text.".to_string()),
    };
    let device_markdown = device_event_markdown(&device).expect("device");
    assert!(device_markdown.contains("Source: OpenFDA"));
    assert!(device_markdown.contains("## Description (OpenFDA)"));
}

#[test]
fn gene_markdown_section_only_shows_new_gene_enrichment_sections() {
    let gene = Gene {
        symbol: "BRAF".to_string(),
        name: "B-Raf proto-oncogene".to_string(),
        entrez_id: "673".to_string(),
        ensembl_id: Some("ENSG00000157764".to_string()),
        location: Some("7q34".to_string()),
        genomic_coordinates: None,
        omim_id: None,
        uniprot_id: Some("P15056".to_string()),
        summary: Some("Kinase involved in MAPK signaling.".to_string()),
        gene_type: Some("protein-coding".to_string()),
        aliases: vec!["BRAF1".to_string()],
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

    let markdown = gene_markdown(
        &gene,
        &[
            "expression".to_string(),
            "hpa".to_string(),
            "druggability".to_string(),
            "clingen".to_string(),
        ],
    )
    .expect("rendered markdown");

    assert!(markdown.contains("# BRAF - expression, hpa, druggability, clingen"));
    assert!(markdown.contains("## Expression (GTEx)"));
    assert!(markdown.contains("## Human Protein Atlas"));
    assert!(markdown.contains("## Druggability"));
    assert!(markdown.contains("## ClinGen"));
    assert!(markdown.contains("No GTEx expression records returned"));
    assert!(markdown.contains("No Human Protein Atlas records returned"));
    assert!(markdown.contains("No DGIdb interactions returned"));
    assert!(markdown.contains("No ClinGen records returned"));
}

#[test]
fn gene_markdown_renders_combined_dgidb_and_opentargets_druggability() {
    let gene = Gene {
        symbol: "EGFR".to_string(),
        name: "epidermal growth factor receptor".to_string(),
        entrez_id: "1956".to_string(),
        ensembl_id: Some("ENSG00000146648".to_string()),
        location: Some("7p11.2".to_string()),
        genomic_coordinates: None,
        omim_id: None,
        uniprot_id: Some("P00533".to_string()),
        summary: None,
        gene_type: Some("protein-coding".to_string()),
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
        druggability: Some(crate::sources::dgidb::GeneDruggability {
            categories: vec!["Kinase".to_string()],
            interactions: Vec::new(),
            tractability: vec![
                crate::sources::dgidb::GeneTractabilityModality {
                    modality: "small molecule".to_string(),
                    tractable: true,
                    evidence_labels: vec!["Approved Drug".to_string()],
                },
                crate::sources::dgidb::GeneTractabilityModality {
                    modality: "antibody".to_string(),
                    tractable: true,
                    evidence_labels: vec!["Clinical Precedence".to_string()],
                },
            ],
            safety_liabilities: vec![crate::sources::dgidb::GeneSafetyLiability {
                event: "Skin rash".to_string(),
                datasource: Some("ForceGenetics".to_string()),
                effect_direction: Some("activation".to_string()),
                biosample: Some("Skin".to_string()),
            }],
        }),
        clingen: None,
        constraint: None,
        disgenet: None,
        funding: None,
        funding_note: None,
    };

    let markdown = gene_markdown(&gene, &["druggability".to_string()]).expect("rendered markdown");

    assert!(markdown.contains("## Druggability"));
    assert!(!markdown.contains("## Druggability (DGIdb)"));
    assert!(markdown.contains("OpenTargets tractability"));
    assert!(markdown.contains("| Modality | Tractable | Evidence |"));
    assert!(markdown.contains("| small molecule | yes | Approved Drug |"));
    assert!(markdown.contains("| antibody | yes | Clinical Precedence |"));
    assert!(markdown.contains("OpenTargets safety liabilities"));
    assert!(markdown.contains("Skin rash"));
    assert!(markdown.contains("No DGIdb interactions returned for this gene query."));
}

#[test]
fn gene_markdown_renders_dgidb_interaction_table_alongside_opentargets_data() {
    let gene = Gene {
        symbol: "BRAF".to_string(),
        name: "B-Raf proto-oncogene".to_string(),
        entrez_id: "673".to_string(),
        ensembl_id: None,
        location: None,
        genomic_coordinates: None,
        omim_id: None,
        uniprot_id: None,
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
        druggability: Some(crate::sources::dgidb::GeneDruggability {
            categories: vec!["Kinase".to_string()],
            interactions: vec![crate::sources::dgidb::DrugInteraction {
                drug: "Dabrafenib".to_string(),
                interaction_types: vec!["inhibitor".to_string()],
                score: Some(1.2),
                approved: Some(true),
                source_count: 2,
            }],
            tractability: vec![crate::sources::dgidb::GeneTractabilityModality {
                modality: "small molecule".to_string(),
                tractable: true,
                evidence_labels: vec!["Approved Drug".to_string()],
            }],
            safety_liabilities: Vec::new(),
        }),
        clingen: None,
        constraint: None,
        disgenet: None,
        funding: None,
        funding_note: None,
    };

    let markdown = gene_markdown(&gene, &["druggability".to_string()]).expect("rendered markdown");

    assert!(markdown.contains("## Druggability"));
    assert!(markdown.contains("OpenTargets tractability"));
    assert!(markdown.contains("| small molecule | yes | Approved Drug |"));
    // DGIdb interaction table renders (not replaced by empty-state message)
    assert!(markdown.contains("| Drug | Interaction Types | Score | Approved | Sources |"));
    assert!(markdown.contains("| Dabrafenib | inhibitor | 1.200 | yes | 2 |"));
    assert!(!markdown.contains("No DGIdb interactions returned for this gene query."));
}

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
fn gene_markdown_renders_hpa_section_details() {
    let gene = Gene {
        symbol: "BRAF".to_string(),
        name: "B-Raf proto-oncogene".to_string(),
        entrez_id: "673".to_string(),
        ensembl_id: Some("ENSG00000157764".to_string()),
        location: Some("7q34".to_string()),
        genomic_coordinates: None,
        omim_id: None,
        uniprot_id: Some("P15056".to_string()),
        summary: Some("Kinase involved in MAPK signaling.".to_string()),
        gene_type: Some("protein-coding".to_string()),
        aliases: vec!["BRAF1".to_string()],
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
        hpa: Some(crate::sources::hpa::GeneHpa {
            tissues: vec![
                crate::sources::hpa::HpaTissueExpression {
                    tissue: "Liver".to_string(),
                    level: "High".to_string(),
                },
                crate::sources::hpa::HpaTissueExpression {
                    tissue: "Kidney".to_string(),
                    level: "Medium".to_string(),
                },
            ],
            subcellular_main_location: vec!["cytosol".to_string(), "vesicles".to_string()],
            subcellular_additional_location: vec!["plasma membrane".to_string()],
            reliability: Some("Supported".to_string()),
            protein_summary: Some("Ubiquitous cytoplasmic expression.".to_string()),
            rna_summary: Some("Low tissue specificity; Detected in all".to_string()),
        }),
        druggability: None,
        clingen: None,
        constraint: None,
        disgenet: None,
        funding: None,
        funding_note: None,
    };

    let summary = gene_markdown(&gene, &[]).expect("rendered markdown");
    assert!(summary.contains(
            "Aliases are alternate names used in literature and databases when a paper does not use the HGNC symbol."
        ));

    let markdown = gene_markdown(&gene, &["hpa".to_string()]).expect("rendered markdown");

    assert!(markdown.contains("# BRAF - hpa"));
    assert!(markdown.contains("## Human Protein Atlas"));
    assert!(markdown.contains("Reliability: Supported"));
    assert!(markdown.contains("Protein summary: Ubiquitous cytoplasmic expression."));
    assert!(markdown.contains("RNA summary: Low tissue specificity; Detected in all"));
    assert!(markdown.contains("Subcellular main locations: cytosol, vesicles"));
    assert!(markdown.contains("Subcellular additional locations: plasma membrane"));
    assert!(markdown.contains("| Tissue | Level |"));
    assert!(markdown.contains("| Liver | High |"));
    assert!(markdown.contains("| Kidney | Medium |"));
    assert!(
        markdown
            .find("| Tissue | Level |")
            .expect("tissue table should render")
            < markdown
                .find("RNA summary:")
                .expect("rna summary should render")
    );
}

#[test]
fn gene_markdown_renders_protein_isoforms_with_count_and_displayed_length() {
    let gene = Gene {
        symbol: "KRAS".to_string(),
        name: "KRAS proto-oncogene, GTPase".to_string(),
        entrez_id: "3845".to_string(),
        ensembl_id: Some("ENSG00000133703".to_string()),
        location: Some("12p12.1".to_string()),
        genomic_coordinates: None,
        omim_id: None,
        uniprot_id: Some("P01116".to_string()),
        summary: None,
        gene_type: Some("protein-coding".to_string()),
        aliases: vec!["K-RAS2A".to_string(), "K-RAS2B".to_string()],
        clinical_diseases: Vec::new(),
        clinical_drugs: Vec::new(),
        pathways: None,
        ontology: None,
        diseases: None,
        protein: Some(crate::entities::gene::GeneProtein {
            accession: "P01116".to_string(),
            name: "GTPase KRas".to_string(),
            function: Some("Small GTPase involved in signal transduction.".to_string()),
            length: Some(189),
            isoforms: vec![
                crate::entities::gene::GeneProteinIsoform {
                    name: "K-Ras4A".to_string(),
                    length: Some(189),
                },
                crate::entities::gene::GeneProteinIsoform {
                    name: "K-Ras4B".to_string(),
                    length: None,
                },
            ],
            alternative_names: Vec::new(),
        }),
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

    let markdown = gene_markdown(&gene, &["protein".to_string()]).expect("gene markdown");
    assert!(markdown.contains("## Protein (UniProt)"));
    assert!(markdown.contains("- Length: 189 aa"));
    assert!(markdown.contains("- Isoforms (2): K-Ras4A (189 aa), K-Ras4B"));
    assert!(markdown.contains("- Function: Small GTPase involved in signal transduction."));
}

#[test]
fn gene_markdown_without_isoforms_keeps_protein_lines_contiguous() {
    let gene = Gene {
        symbol: "BRAF".to_string(),
        name: "B-Raf proto-oncogene, serine/threonine kinase".to_string(),
        entrez_id: "673".to_string(),
        ensembl_id: Some("ENSG00000157764".to_string()),
        location: Some("7q34".to_string()),
        genomic_coordinates: None,
        omim_id: None,
        uniprot_id: Some("P15056".to_string()),
        summary: None,
        gene_type: Some("protein-coding".to_string()),
        aliases: Vec::new(),
        clinical_diseases: Vec::new(),
        clinical_drugs: Vec::new(),
        pathways: None,
        ontology: None,
        diseases: None,
        protein: Some(crate::entities::gene::GeneProtein {
            accession: "P15056".to_string(),
            name: "Serine/threonine-protein kinase B-raf".to_string(),
            function: Some("Kinase function.".to_string()),
            length: Some(766),
            isoforms: Vec::new(),
            alternative_names: Vec::new(),
        }),
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

    let markdown = gene_markdown(&gene, &["protein".to_string()]).expect("gene markdown");
    assert!(markdown.contains("- Length: 766 aa\n- Function: Kinase function."));
    assert!(!markdown.contains("- Isoforms ("));
    assert!(!markdown.contains("- Length: 766 aa\n\n- Function:"));
}

#[test]
fn gene_markdown_renders_protein_alternative_names() {
    let gene = Gene {
        symbol: "PLIN2".to_string(),
        name: "perilipin 2".to_string(),
        entrez_id: "123".to_string(),
        ensembl_id: Some("ENSG00000147889".to_string()),
        location: Some("9p22.1".to_string()),
        genomic_coordinates: None,
        omim_id: None,
        uniprot_id: Some("Q99541".to_string()),
        summary: None,
        gene_type: Some("protein-coding".to_string()),
        aliases: Vec::new(),
        clinical_diseases: Vec::new(),
        clinical_drugs: Vec::new(),
        pathways: None,
        ontology: None,
        diseases: None,
        protein: Some(crate::entities::gene::GeneProtein {
            accession: "Q99541".to_string(),
            name: "Perilipin-2".to_string(),
            function: None,
            length: Some(437),
            isoforms: Vec::new(),
            alternative_names: vec![
                "Adipophilin".to_string(),
                "ADRP".to_string(),
                "Adipose differentiation-related protein".to_string(),
            ],
        }),
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

    let markdown = gene_markdown(&gene, &["protein".to_string()]).expect("gene markdown");
    assert!(
        markdown.contains(
            "- Also known as: Adipophilin, ADRP, Adipose differentiation-related protein"
        )
    );
}

#[test]
fn gene_markdown_preserves_full_protein_function_text() {
    let long_function = "Mitochondrial dynamin-like GTPase required for fusion. Localizes to the intermembrane space where it helps organize cristae architecture and mitochondrial DNA maintenance.".to_string();
    let gene = Gene {
        symbol: "OPA1".to_string(),
        name: "OPA1 mitochondrial dynamin like GTPase".to_string(),
        entrez_id: "4976".to_string(),
        ensembl_id: Some("ENSG00000198836".to_string()),
        location: Some("3q29".to_string()),
        genomic_coordinates: None,
        omim_id: None,
        uniprot_id: Some("O60313".to_string()),
        summary: None,
        gene_type: Some("protein-coding".to_string()),
        aliases: Vec::new(),
        clinical_diseases: Vec::new(),
        clinical_drugs: Vec::new(),
        pathways: None,
        ontology: None,
        diseases: None,
        protein: Some(crate::entities::gene::GeneProtein {
            accession: "O60313".to_string(),
            name: "Dynamin-like 120 kDa protein, mitochondrial".to_string(),
            function: Some(long_function.clone()),
            length: Some(960),
            isoforms: Vec::new(),
            alternative_names: Vec::new(),
        }),
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

    let markdown = gene_markdown(&gene, &["protein".to_string()]).expect("gene markdown");
    assert!(markdown.contains(&long_function));
    assert!(markdown.contains("intermembrane space"));
    assert!(!markdown.contains("architecture and mitochondrial DNA…"));
}

#[test]
fn gene_markdown_omits_protein_alternative_names_when_absent() {
    let gene = Gene {
        symbol: "BRAF".to_string(),
        name: "B-Raf proto-oncogene, serine/threonine kinase".to_string(),
        entrez_id: "673".to_string(),
        ensembl_id: Some("ENSG00000157764".to_string()),
        location: Some("7q34".to_string()),
        genomic_coordinates: None,
        omim_id: None,
        uniprot_id: Some("P15056".to_string()),
        summary: None,
        gene_type: Some("protein-coding".to_string()),
        aliases: Vec::new(),
        clinical_diseases: Vec::new(),
        clinical_drugs: Vec::new(),
        pathways: None,
        ontology: None,
        diseases: None,
        protein: Some(crate::entities::gene::GeneProtein {
            accession: "P15056".to_string(),
            name: "Serine/threonine-protein kinase B-raf".to_string(),
            function: Some("Kinase function.".to_string()),
            length: Some(766),
            isoforms: Vec::new(),
            alternative_names: Vec::new(),
        }),
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

    let markdown = gene_markdown(&gene, &["protein".to_string()]).expect("gene markdown");
    assert!(!markdown.contains("- Also known as:"));
}

#[test]
fn study_top_mutated_markdown_renders_ranked_table() {
    let markdown = study_top_mutated_markdown(&StudyTopMutatedGenesResult {
        study_id: "msk_impact_2017".to_string(),
        total_samples: 3,
        rows: vec![
            StudyTopMutatedGeneRow {
                gene: "TP53".to_string(),
                mutated_samples: 2,
                mutation_events: 2,
                mutation_rate: 2.0 / 3.0,
            },
            StudyTopMutatedGeneRow {
                gene: "KRAS".to_string(),
                mutated_samples: 2,
                mutation_events: 2,
                mutation_rate: 2.0 / 3.0,
            },
        ],
    });

    assert!(markdown.contains("# Study Top Mutated Genes: msk_impact_2017"));
    assert!(
        markdown.contains(
            "| Gene | Mutated Samples | Mutation Events | Total Samples | Mutation Rate |"
        )
    );
    assert!(markdown.contains("| TP53 | 2 | 2 | 3 |"));
    assert!(markdown.contains("| KRAS | 2 | 2 | 3 |"));
}

#[test]
fn gene_markdown_section_only_shows_constraint_section() {
    let gene = Gene {
        symbol: "TP53".to_string(),
        name: "tumor protein p53".to_string(),
        entrez_id: "7157".to_string(),
        ensembl_id: Some("ENSG00000141510".to_string()),
        location: Some("17p13.1".to_string()),
        genomic_coordinates: None,
        omim_id: None,
        uniprot_id: Some("P04637".to_string()),
        summary: Some("Tumor suppressor.".to_string()),
        gene_type: Some("protein-coding".to_string()),
        aliases: vec!["P53".to_string()],
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
        constraint: Some(crate::entities::gene::GeneConstraint {
            pli: None,
            loeuf: None,
            mis_z: None,
            syn_z: None,
            transcript: Some("ENST00000269305".to_string()),
            source: "gnomAD".to_string(),
            source_version: "v4".to_string(),
            reference_genome: "GRCh38".to_string(),
        }),
        disgenet: None,
        funding: None,
        funding_note: None,
    };

    let markdown = gene_markdown(&gene, &["constraint".to_string()]).expect("rendered markdown");

    assert!(markdown.contains("# TP53 - constraint"));
    assert!(markdown.contains("## Constraint (gnomAD)"));
    assert!(markdown.contains("Source: gnomAD"));
    assert!(markdown.contains("Version: v4"));
    assert!(markdown.contains("Reference genome: GRCh38"));
    assert!(markdown.contains("Transcript: ENST00000269305"));
    assert!(markdown.contains("No gnomAD constraint metrics returned for this gene query."));
}

#[test]
fn gene_markdown_section_only_shows_disgenet_section() {
    let gene = Gene {
        symbol: "TP53".to_string(),
        name: "tumor protein p53".to_string(),
        entrez_id: "7157".to_string(),
        ensembl_id: None,
        location: None,
        genomic_coordinates: None,
        omim_id: None,
        uniprot_id: None,
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
        disgenet: Some(crate::entities::gene::GeneDisgenet {
            associations: vec![crate::entities::gene::GeneDisgenetAssociation {
                disease_name: "Breast Carcinoma".to_string(),
                disease_cui: "C0678222".to_string(),
                score: 0.91,
                publication_count: Some(1234),
                clinical_trial_count: Some(4),
                evidence_index: Some(0.72),
                evidence_level: Some("Definitive".to_string()),
            }],
        }),
        funding: None,
        funding_note: None,
    };

    let markdown = gene_markdown(&gene, &["disgenet".to_string()]).expect("rendered markdown");

    assert!(markdown.contains("# TP53 - disgenet"));
    assert!(markdown.contains("## DisGeNET"));
    assert!(markdown.contains("| Disease | UMLS CUI | Score | PMIDs | Trials | EL | EI |"));
    assert!(
        markdown
            .contains("| Breast Carcinoma | C0678222 | 0.910 | 1234 | 4 | Definitive | 0.720 |")
    );
}

#[test]
fn gene_markdown_disgenet_renders_sparse_optional_fields() {
    let gene = Gene {
        symbol: "KYNU".to_string(),
        name: "kynureninase".to_string(),
        entrez_id: "8942".to_string(),
        ensembl_id: None,
        location: None,
        genomic_coordinates: None,
        omim_id: None,
        uniprot_id: None,
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
        disgenet: Some(crate::entities::gene::GeneDisgenet {
            associations: vec![crate::entities::gene::GeneDisgenetAssociation {
                disease_name: "Sparse Disease".to_string(),
                disease_cui: "C1234567".to_string(),
                score: 0.23,
                publication_count: None,
                clinical_trial_count: None,
                evidence_index: None,
                evidence_level: None,
            }],
        }),
        funding: None,
        funding_note: None,
    };

    let markdown = gene_markdown(&gene, &["disgenet".to_string()]).expect("rendered markdown");

    assert!(markdown.contains("| Disease | UMLS CUI | Score | PMIDs | Trials | EL | EI |"));
    assert!(markdown.contains("| Sparse Disease | C1234567 | 0.230 | - | - | - | - |"));
}

#[test]
fn gene_markdown_funding_renders_linked_rows_and_currency() {
    let gene = Gene {
        symbol: "ERBB2".to_string(),
        name: "erb-b2 receptor tyrosine kinase 2".to_string(),
        entrez_id: "2064".to_string(),
        ensembl_id: None,
        location: None,
        genomic_coordinates: None,
        omim_id: None,
        uniprot_id: None,
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
        funding: Some(crate::sources::nih_reporter::NihReporterFundingSection {
            query: "ERBB2".to_string(),
            fiscal_years: vec![2022, 2023, 2024, 2025, 2026],
            matching_project_years: 176,
            grants: vec![crate::sources::nih_reporter::NihReporterGrant {
                project_title: "Regulation Of Epidermal Differentiation".to_string(),
                project_num: "1ZIAAR041124-23".to_string(),
                core_project_num: Some("ZIAAR041124".to_string()),
                project_detail_url: Some(
                    "https://reporter.nih.gov/project-details/10697688".to_string(),
                ),
                pi_name: Some("MORASSO, MARIA".to_string()),
                organization: Some(
                    "NATIONAL INSTITUTE OF ARTHRITIS AND MUSCULOSKELETAL AND SKIN DISEASES"
                        .to_string(),
                ),
                fiscal_year: 2022,
                award_amount: 2_219_287,
            }],
        }),
        funding_note: None,
    };

    let markdown = gene_markdown(&gene, &["funding".to_string()]).expect("funding markdown");
    let summary = "Showing top 1 unique grants from 176 matching NIH project-year records across FY2022-FY2026.";
    let row = "| [Regulation Of Epidermal Differentiation](https://reporter.nih.gov/project-details/10697688) | MORASSO, MARIA | NATIONAL INSTITUTE OF ARTHRITIS AND MUSCULOSKELETAL AND SKIN DISEASES | 2022 | $2,219,287 |";

    assert!(markdown.contains("# ERBB2 - funding"));
    assert!(markdown.contains("## Funding (NIH Reporter)"));
    assert!(markdown.contains(summary));
    assert!(markdown.contains("| Project | PI | Organization | FY | Amount |"));
    assert!(markdown.contains("[Regulation Of Epidermal Differentiation](https://reporter.nih.gov/project-details/10697688)"));
    assert!(markdown.contains("| MORASSO, MARIA |"));
    assert!(markdown.contains("| 2022 | $2,219,287 |"));
    assert!(
        markdown
            .find(summary)
            .expect("funding summary should render")
            > markdown.find(row).expect("funding row should render")
    );
}

#[test]
fn gene_markdown_all_keeps_opt_in_sections_hidden() {
    let gene = Gene {
        symbol: "ERBB2".to_string(),
        name: "erb-b2 receptor tyrosine kinase 2".to_string(),
        entrez_id: "2064".to_string(),
        ensembl_id: None,
        location: None,
        genomic_coordinates: None,
        omim_id: None,
        uniprot_id: None,
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

    let markdown = gene_markdown(&gene, &["all".to_string()]).expect("all markdown");

    assert!(!markdown.contains("## Funding (NIH Reporter)"));
    assert!(!markdown.contains("## DisGeNET"));
}

#[test]
fn gene_markdown_pathways_show_source_labels() {
    let gene = Gene {
        symbol: "BRAF".to_string(),
        name: "B-Raf proto-oncogene".to_string(),
        entrez_id: "673".to_string(),
        ensembl_id: None,
        location: Some("7q34".to_string()),
        genomic_coordinates: None,
        omim_id: None,
        uniprot_id: None,
        summary: None,
        gene_type: None,
        aliases: Vec::new(),
        clinical_diseases: Vec::new(),
        clinical_drugs: Vec::new(),
        pathways: Some(vec![
            crate::entities::gene::GenePathway {
                source: "KEGG".to_string(),
                id: "hsa05200".to_string(),
                name: "Pathways in cancer".to_string(),
            },
            crate::entities::gene::GenePathway {
                source: "Reactome".to_string(),
                id: "R-HSA-5673001".to_string(),
                name: "RAF/MAP kinase cascade".to_string(),
            },
        ]),
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

    let markdown = gene_markdown(&gene, &[]).expect("rendered markdown");
    assert!(markdown.contains("| Source | ID | Name |"));
    assert!(markdown.contains("| KEGG | hsa05200 | Pathways in cancer |"));
    assert!(markdown.contains("| Reactome | R-HSA-5673001 | RAF/MAP kinase cascade |"));
    assert!(!markdown.contains("Showing pathway rows from Reactome search results."));
}

#[test]
fn pathway_markdown_uses_source_and_source_specific_evidence_url() {
    let pathway = Pathway {
        source: "KEGG".to_string(),
        id: "hsa05200".to_string(),
        name: "Pathways in cancer".to_string(),
        species: Some("Homo sapiens".to_string()),
        summary: Some("Cancer pathway overview.".to_string()),
        genes: vec!["BRAF".to_string(), "EGFR".to_string()],
        events: Vec::new(),
        enrichment: Vec::new(),
    };

    let markdown = pathway_markdown(&pathway, &[]).expect("rendered markdown");
    assert!(markdown.contains("Source: KEGG"));
    assert!(markdown.contains("[KEGG](https://www.kegg.jp/entry/hsa05200)"));

    let wikipathways = Pathway {
        source: "WikiPathways".to_string(),
        id: "WP254".to_string(),
        name: "Apoptosis".to_string(),
        species: Some("Homo sapiens".to_string()),
        summary: None,
        genes: vec!["TP53".to_string()],
        events: Vec::new(),
        enrichment: Vec::new(),
    };

    let markdown = pathway_markdown(&wikipathways, &[]).expect("rendered markdown");
    assert!(markdown.contains("Source: WikiPathways"));
    assert!(markdown.contains("[WikiPathways](https://www.wikipathways.org/pathways/WP254.html)"));
}

#[test]
fn pathway_markdown_hides_genes_section_when_genes_are_empty() {
    let pathway = Pathway {
        source: "KEGG".to_string(),
        id: "hsa05200".to_string(),
        name: "Pathways in cancer".to_string(),
        species: Some("Homo sapiens".to_string()),
        summary: Some("Cancer pathway overview.".to_string()),
        genes: Vec::new(),
        events: Vec::new(),
        enrichment: Vec::new(),
    };

    let markdown = pathway_markdown(&pathway, &[]).expect("rendered markdown");
    assert!(!markdown.contains("## Genes"));
    assert!(!markdown.contains("BRAF"));
}

#[test]
fn pathway_search_markdown_shows_source_column() {
    let results = vec![
        PathwaySearchResult {
            source: "Reactome".to_string(),
            id: "R-HSA-5673001".to_string(),
            name: "RAF/MAP kinase cascade".to_string(),
        },
        PathwaySearchResult {
            source: "KEGG".to_string(),
            id: "hsa04010".to_string(),
            name: "MAPK signaling pathway".to_string(),
        },
    ];

    let markdown =
        pathway_search_markdown("MAPK", &results, Some(results.len())).expect("markdown");
    assert!(markdown.contains("| Source | ID | Name |"));
    assert!(markdown.contains("| Reactome | R-HSA-5673001 | RAF/MAP kinase cascade |"));
    assert!(markdown.contains("| KEGG | hsa04010 | MAPK signaling pathway |"));
}

#[test]
fn markdown_render_variant_entity() {
    let variant: Variant = serde_json::from_value(serde_json::json!({
        "id": "chr7:g.55259515T>G",
        "gene": "EGFR",
        "hgvs_p": "p.L858R",
        "legacy_name": "EGFR L858R",
        "significance": "Pathogenic"
    }))
    .expect("variant should deserialize");

    let markdown = variant_markdown(&variant, &[]).expect("rendered markdown");
    assert!(markdown.contains("EGFR"));
    assert!(markdown.contains("p.L858R"));
    assert!(markdown.contains("Legacy Name: EGFR L858R"));
}

#[test]
fn variant_markdown_renders_compact_clinvar_and_population_fields() {
    let variant: Variant = serde_json::from_value(serde_json::json!({
        "id": "chr7:g.140453136A>T",
        "gene": "BRAF",
        "gnomad_af": 0.0001,
        "allele_frequency_percent": "0.0100%",
        "top_disease": {"condition": "Melanoma", "reports": 2},
        "clinvar_conditions": [{"condition": "Melanoma", "reports": 2}]
    }))
    .expect("variant should deserialize");

    let markdown = variant_markdown(&variant, &["all".to_string()]).expect("rendered markdown");
    assert!(markdown.contains("Top disease (ClinVar): Melanoma (2 reports)"));
    assert!(markdown.contains("gnomAD AF:"));
    assert!(markdown.contains("(0.0100%)"));
}

#[test]
fn variant_markdown_renders_gwas_unavailable_message() {
    let variant: Variant = serde_json::from_value(serde_json::json!({
        "id": "rs7903146",
        "gene": "TCF7L2",
        "rsid": "rs7903146",
        "gwas": [],
        "gwas_unavailable_reason": "GWAS association data temporarily unavailable."
    }))
    .expect("variant should deserialize");

    let markdown = variant_markdown(&variant, &["gwas".to_string()]).expect("rendered markdown");
    assert!(markdown.contains("GWAS association data temporarily unavailable."));
    assert!(!markdown.contains("No GWAS associations found for this variant."));
}

#[test]
fn variant_search_markdown_renders_legacy_name_column_and_fallback() {
    let results = vec![
        VariantSearchResult {
            id: "chr6:g.118880200T>G".to_string(),
            gene: "PLN".to_string(),
            hgvs_p: Some("p.L39X".to_string()),
            legacy_name: Some("PLN L39stop".to_string()),
            significance: Some("Pathogenic".to_string()),
            clinvar_stars: Some(2),
            gnomad_af: None,
            revel: Some(0.935),
            gerp: Some(5.12),
        },
        VariantSearchResult {
            id: "chr6:g.118880100A>G".to_string(),
            gene: "PLN".to_string(),
            hgvs_p: Some("p.K3R".to_string()),
            legacy_name: None,
            significance: None,
            clinvar_stars: None,
            gnomad_af: None,
            revel: None,
            gerp: None,
        },
    ];

    let markdown =
        variant_search_markdown("gene=PLN, hgvsp=L39X", &results).expect("rendered markdown");
    assert!(markdown.contains("| ID | Gene | Protein | Legacy Name | Significance |"));
    assert!(markdown.contains("| chr6:g.118880200T>G | PLN | p.L39X | PLN L39stop |"));
    assert!(markdown.contains("| chr6:g.118880100A>G | PLN | p.K3R | - |"));
}

#[test]
fn variant_search_markdown_renders_related_commands_from_context() {
    let results = vec![
        VariantSearchResult {
            id: "rs199473688".to_string(),
            gene: "SCN5A".to_string(),
            hgvs_p: Some("p.Arg282His".to_string()),
            legacy_name: None,
            significance: Some("Pathogenic".to_string()),
            clinvar_stars: Some(2),
            gnomad_af: None,
            revel: Some(0.91),
            gerp: Some(5.7),
        },
        VariantSearchResult {
            id: "rs7626962".to_string(),
            gene: "SCN5A".to_string(),
            hgvs_p: Some("p.Gly514Cys".to_string()),
            legacy_name: None,
            significance: Some("Likely pathogenic".to_string()),
            clinvar_stars: Some(1),
            gnomad_af: None,
            revel: Some(0.88),
            gerp: Some(5.1),
        },
    ];

    let markdown = variant_search_markdown_with_context(
        "gene=SCN5A, condition=Brugada",
        &results,
        "",
        Some("SCN5A"),
        Some("Brugada"),
    )
    .expect("rendered markdown");

    assert!(markdown.contains("See also:"));
    assert!(markdown.contains("biomcp get variant rs199473688"));
    assert!(markdown.contains("biomcp get gene SCN5A"));
    assert!(markdown.contains("biomcp search disease --query Brugada"));
}

#[test]
fn phenotype_search_markdown_renders_top_disease_follow_up() {
    let results = vec![
        crate::entities::disease::PhenotypeSearchResult {
            disease_id: "MONDO:0100135".to_string(),
            disease_name: "Dravet syndrome".to_string(),
            score: 15.036,
        },
        crate::entities::disease::PhenotypeSearchResult {
            disease_id: "MONDO:0000032".to_string(),
            disease_name: "febrile seizures, familial".to_string(),
            score: 15.036,
        },
    ];

    let markdown = phenotype_search_markdown_with_footer(
        "HP:0002373 HP:0001250",
        &results,
        "Showing 1-2 of 2 results.",
    )
    .expect("rendered markdown");

    assert!(markdown.contains("See also:"));
    assert!(markdown.contains("biomcp get disease \"Dravet syndrome\" genes phenotypes"));
    assert_eq!(
        related_command_description("biomcp get disease \"Dravet syndrome\" genes phenotypes"),
        Some("open the top phenotype-match disease with genes and phenotypes")
    );
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
fn variant_oncokb_markdown_shows_truncation_note() {
    let result = VariantOncoKbResult {
        gene: "EGFR".to_string(),
        alteration: "L858R".to_string(),
        oncogenic: Some("Oncogenic".to_string()),
        level: Some("Level 1".to_string()),
        effect: Some("Gain-of-function".to_string()),
        therapies: vec![
            TreatmentImplication {
                level: "Level 1".to_string(),
                drugs: vec!["osimertinib".to_string()],
                cancer_type: Some("Lung adenocarcinoma".to_string()),
                note: None,
            },
            TreatmentImplication {
                level: "Level 2".to_string(),
                drugs: vec!["afatinib".to_string()],
                cancer_type: Some("Lung adenocarcinoma".to_string()),
                note: Some("(and 2 more)".to_string()),
            },
        ],
    };

    let markdown = variant_oncokb_markdown(&result);
    assert!(markdown.contains("| Drug | Level | Cancer Type | Note |"));
    assert!(markdown.contains("(and 2 more)"));
}

#[test]
fn pagination_footer_offset_suppresses_more_when_complete_single_result() {
    let footer = pagination_footer(PaginationFooterMode::Offset, 0, 10, 1, Some(1), None);
    assert!(footer.contains("Showing 1 of 1 results."));
    assert!(!footer.contains("Use --offset"));
}

#[test]
fn pagination_footer_offset_keeps_more_when_additional_rows_exist() {
    let footer = pagination_footer(PaginationFooterMode::Offset, 0, 2, 2, Some(10), None);
    assert!(footer.contains("Showing 1-2 of 10 results."));
    assert!(footer.contains("Use --offset 2 for more."));
}

#[test]
fn pagination_footer_offset_suppresses_more_on_last_page() {
    let footer = pagination_footer(PaginationFooterMode::Offset, 8, 2, 2, Some(10), None);
    assert!(footer.contains("Showing 9-10 of 10 results."));
    assert!(!footer.contains("Use --offset"));
}

#[test]
fn pagination_footer_cursor_prefers_offset_guidance_without_placeholder() {
    let footer = pagination_footer(
        PaginationFooterMode::Cursor,
        0,
        1,
        1,
        Some(20),
        Some("abc123"),
    );
    assert!(footer.contains("Use --offset 1 for more."));
    assert!(footer.contains("--next-page is also supported"));
    assert!(!footer.contains("<TOKEN>"));
}

#[test]
fn article_entities_markdown_uses_safe_gene_search_commands() {
    let annotations = ArticleAnnotations {
        genes: vec![
            AnnotationCount {
                text: "BRAF".to_string(),
                count: 5,
            },
            AnnotationCount {
                text: "serine-threonine protein kinase".to_string(),
                count: 1,
            },
        ],
        diseases: Vec::new(),
        chemicals: Vec::new(),
        mutations: vec![AnnotationCount {
            text: "V600E".to_string(),
            count: 2,
        }],
    };

    let markdown =
        article_entities_markdown("22663011", Some(&annotations), Some(5)).expect("markdown");
    assert!(markdown.contains("`biomcp search gene -q BRAF`"));
    assert!(markdown.contains("`biomcp search gene -q \"serine-threonine protein kinase\"`"));
    assert!(!markdown.contains("`biomcp get gene serine-threonine protein kinase`"));
    assert!(markdown.contains("`biomcp get variant V600E`"));
}

#[test]
fn article_markdown_renders_semantic_scholar_section() {
    let article = Article {
        pmid: Some("22663011".to_string()),
        pmcid: None,
        doi: Some("10.1000/example".to_string()),
        title: "Example".to_string(),
        authors: Vec::new(),
        journal: Some("Example Journal".to_string()),
        date: Some("2024-01-01".to_string()),
        citation_count: Some(12),
        publication_type: None,
        open_access: Some(true),
        abstract_text: None,
        full_text_path: None,
        full_text_note: None,
        annotations: None,
        semantic_scholar: Some(crate::entities::article::ArticleSemanticScholar {
            paper_id: Some("paper-1".to_string()),
            tldr: Some("A concise summary.".to_string()),
            citation_count: Some(20),
            influential_citation_count: Some(4),
            reference_count: Some(10),
            is_open_access: Some(true),
            open_access_pdf: Some(crate::entities::article::ArticleSemanticScholarPdf {
                url: "https://example.org/paper.pdf".to_string(),
                status: Some("GREEN".to_string()),
                license: Some("CC-BY".to_string()),
            }),
        }),
        pubtator_fallback: false,
    };

    let markdown =
        article_markdown(&article, &["tldr".to_string()]).expect("markdown should render");
    assert!(markdown.contains("## Semantic Scholar"));
    assert!(markdown.contains("TLDR: A concise summary."));
    assert!(markdown.contains("Influential citations: 4"));
    assert!(markdown.contains("Open-access PDF: https://example.org/paper.pdf"));
}

#[test]
fn article_graph_markdown_renders_expected_table_headers() {
    let result = crate::entities::article::ArticleGraphResult {
        article: crate::entities::article::ArticleRelatedPaper {
            paper_id: Some("paper-1".to_string()),
            pmid: Some("22663011".to_string()),
            doi: None,
            arxiv_id: None,
            title: "Seed".to_string(),
            journal: None,
            year: Some(2012),
        },
        edges: vec![crate::entities::article::ArticleGraphEdge {
            paper: crate::entities::article::ArticleRelatedPaper {
                paper_id: Some("paper-2".to_string()),
                pmid: Some("24200969".to_string()),
                doi: None,
                arxiv_id: None,
                title: "Related paper".to_string(),
                journal: Some("Nature".to_string()),
                year: Some(2014),
            },
            intents: vec!["Background".to_string()],
            contexts: vec!["Important supporting context".to_string()],
            is_influential: true,
        }],
    };

    let markdown = article_graph_markdown("Citations", &result).expect("graph markdown");
    assert!(markdown.contains("# Citations for PMID 22663011"));
    assert!(markdown.contains("| PMID | Title | Intents | Influential | Context |"));
    assert!(markdown.contains(
        "| 24200969 | Related paper | Background | yes | Important supporting context |"
    ));
}

#[test]
fn article_batch_markdown_renders_compact_rows() {
    let rows = vec![
        crate::entities::article::ArticleBatchItem {
            requested_id: "22663011".to_string(),
            pmid: Some("22663011".to_string()),
            pmcid: None,
            doi: Some("10.1056/NEJMoa1203421".to_string()),
            title: "Improved survival with vemurafenib".to_string(),
            journal: Some("NEJM".to_string()),
            year: Some(2012),
            entity_summary: Some(crate::entities::article::ArticleBatchEntitySummary {
                genes: vec![crate::entities::article::AnnotationCount {
                    text: "BRAF".to_string(),
                    count: 4,
                }],
                diseases: vec![crate::entities::article::AnnotationCount {
                    text: "melanoma".to_string(),
                    count: 2,
                }],
                chemicals: Vec::new(),
                mutations: Vec::new(),
            }),
            tldr: Some("BRAF inhibitor benefit in melanoma.".to_string()),
            citation_count: Some(120),
            influential_citation_count: Some(18),
        },
        crate::entities::article::ArticleBatchItem {
            requested_id: "PMC9984800".to_string(),
            pmid: Some("24200969".to_string()),
            pmcid: Some("PMC9984800".to_string()),
            doi: None,
            title: "Follow-up trial".to_string(),
            journal: Some("Nature".to_string()),
            year: Some(2014),
            entity_summary: None,
            tldr: None,
            citation_count: None,
            influential_citation_count: None,
        },
    ];

    let markdown = article_batch_markdown(&rows).expect("batch markdown");
    assert!(markdown.contains("# Article Batch (2)"));
    assert!(markdown.contains("## 1. Improved survival with vemurafenib"));
    assert!(markdown.contains("PMID: 22663011"));
    assert!(markdown.contains("Entities: Genes: BRAF (4); Diseases: melanoma (2)"));
    assert!(markdown.contains("TLDR: BRAF inhibitor benefit in melanoma."));
    assert!(markdown.contains("Citations: 120 (influential: 18)"));
    assert!(markdown.contains("## 2. Follow-up trial"));
    assert!(markdown.contains("PMID: 24200969"));
    // Absent optional fields are omitted, not printed as placeholders
    assert!(!markdown.contains("TLDR: -"));
    assert!(!markdown.contains("Entities: -"));
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
fn disease_markdown_links_source_cells_and_footer_evidence_urls() {
    let disease = Disease {
        id: "MONDO:0009061".to_string(),
        name: "cystic fibrosis".to_string(),
        definition: None,
        synonyms: Vec::new(),
        parents: Vec::new(),
        associated_genes: Vec::new(),
        gene_associations: vec![crate::entities::disease::DiseaseGeneAssociation {
            gene: "CFTR".to_string(),
            relationship: Some("gene associated with condition".to_string()),
            source: Some("infores:orphanet".to_string()),
            opentargets_score: None,
        }],
        top_genes: Vec::new(),
        top_gene_scores: Vec::new(),
        treatment_landscape: Vec::new(),
        recruiting_trial_count: None,
        pathways: Vec::new(),
        phenotypes: vec![crate::entities::disease::DiseasePhenotype {
            hpo_id: "HP:0001945".to_string(),
            name: Some("Dehydration".to_string()),
            evidence: None,
            frequency: None,
            frequency_qualifier: None,
            onset_qualifier: None,
            sex_qualifier: None,
            stage_qualifier: None,
            qualifiers: Vec::new(),
            source: Some("infores:omim".to_string()),
        }],
        key_features: Vec::new(),
        variants: Vec::new(),
        top_variant: None,
        models: vec![crate::entities::disease::DiseaseModelAssociation {
            model: "Cftr tm1Unc".to_string(),
            model_id: Some("MGI:3698752".to_string()),
            organism: Some("Mus musculus".to_string()),
            relationship: Some("model of".to_string()),
            source: Some("infores:mgi".to_string()),
            evidence_count: Some(3),
        }],
        prevalence: Vec::new(),
        prevalence_note: None,
        survival: None,
        survival_note: None,
        civic: None,
        disgenet: None,
        funding: None,
        funding_note: None,
        xrefs: std::collections::HashMap::from([
            ("Orphanet".to_string(), "586".to_string()),
            ("OMIM".to_string(), "219700".to_string()),
        ]),
    };

    let markdown = disease_markdown(&disease, &["all".to_string()]).expect("markdown");
    assert!(markdown.contains("[infores:orphanet](https://www.orpha.net/en/disease/detail/586)"));
    assert!(markdown.contains("[infores:omim](https://www.omim.org/entry/219700)"));
    assert!(markdown.contains("[infores:mgi](https://www.informatics.jax.org/accession/MGI:"));
    assert!(markdown.contains("[Orphanet](https://www.orpha.net/en/disease/detail/586)"));
    assert!(markdown.contains("[OMIM](https://www.omim.org/entry/219700)"));
    assert!(markdown.contains("[MGI](https://www.informatics.jax.org/accession/MGI:"));
}

#[test]
fn drug_markdown_uses_label_interaction_text_before_public_unavailable_fallback() {
    let drug = Drug {
        name: "warfarin".to_string(),
        drugbank_id: Some("DB00682".to_string()),
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
        interaction_text: Some("DRUG INTERACTIONS\n\nWarfarin interacts with aspirin.".to_string()),
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

    let markdown = drug_markdown(&drug, &["interactions".to_string()]).expect("markdown");
    assert!(markdown.contains("## Interactions"));
    assert!(markdown.contains("DRUG INTERACTIONS"));
    assert!(!markdown.contains("No known drug-drug interactions found."));
}

#[test]
fn drug_markdown_uses_truthful_public_unavailable_interactions_message() {
    let drug = Drug {
        name: "pembrolizumab".to_string(),
        drugbank_id: Some("DB09037".to_string()),
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

    let markdown = drug_markdown(&drug, &["interactions".to_string()]).expect("markdown");
    assert!(markdown.contains("Interaction details not available from public sources."));
    assert!(!markdown.contains("No known drug-drug interactions found."));
}

#[test]
fn drug_markdown_shows_target_family_and_members_when_present() {
    let drug = Drug {
        name: "olaparib".to_string(),
        drugbank_id: Some("DB09074".to_string()),
        chembl_id: Some("CHEMBL1789941".to_string()),
        unii: None,
        drug_type: Some("small molecule".to_string()),
        mechanism: Some("PARP inhibitor".to_string()),
        mechanisms: vec!["PARP inhibitor".to_string()],
        approval_date: None,
        approval_date_raw: None,
        approval_date_display: None,
        approval_summary: None,
        brand_names: Vec::new(),
        route: None,
        targets: vec![
            "PARP1".to_string(),
            "PARP2".to_string(),
            "PARP3".to_string(),
        ],
        variant_targets: Vec::new(),
        target_family: Some("PARP".to_string()),
        target_family_name: Some("poly(ADP-ribose) polymerase".to_string()),
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

    let markdown = drug_markdown(&drug, &["targets".to_string()]).expect("markdown");
    assert!(markdown.contains("Family: PARP (poly(ADP-ribose) polymerase)"));
    assert!(markdown.contains("Members: PARP1, PARP2, PARP3"));
}

#[test]
fn drug_markdown_renders_variant_targets_as_additive_line() {
    let drug = Drug {
        name: "rindopepimut".to_string(),
        drugbank_id: None,
        chembl_id: Some("CHEMBL2108508".to_string()),
        unii: None,
        drug_type: Some("vaccine".to_string()),
        mechanism: None,
        mechanisms: Vec::new(),
        approval_date: None,
        approval_date_raw: None,
        approval_date_display: None,
        approval_summary: None,
        brand_names: Vec::new(),
        route: None,
        targets: vec!["EGFR".to_string()],
        variant_targets: vec!["EGFRvIII".to_string()],
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

    let markdown = drug_markdown(&drug, &["targets".to_string()]).expect("markdown");
    assert!(markdown.contains("## Targets (ChEMBL / Open Targets)"));
    assert!(markdown.contains("EGFR"));
    assert!(markdown.contains("Variant Targets (CIViC): EGFRvIII"));
}

#[test]
fn drug_markdown_omits_target_family_for_mixed_targets() {
    let drug = Drug {
        name: "imatinib".to_string(),
        drugbank_id: Some("DB00619".to_string()),
        chembl_id: Some("CHEMBL941".to_string()),
        unii: None,
        drug_type: Some("small-molecule".to_string()),
        mechanism: Some("Inhibitor of BCR-ABL".to_string()),
        mechanisms: vec!["Inhibitor of BCR-ABL".to_string()],
        approval_date: None,
        approval_date_raw: None,
        approval_date_display: None,
        approval_summary: None,
        brand_names: Vec::new(),
        route: None,
        targets: vec!["ABL1".to_string(), "KIT".to_string(), "PDGFRB".to_string()],
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

    let markdown = drug_markdown(&drug, &["targets".to_string()]).expect("markdown");
    assert!(!markdown.contains("Family:"));
    assert!(!markdown.contains("Members:"));
    assert!(markdown.contains("ABL1, KIT, PDGFRB"));
}

#[test]
fn drug_markdown_with_region_all_keeps_us_and_eu_blocks_separate() {
    let drug = Drug {
        name: "pembrolizumab".to_string(),
        drugbank_id: Some("DB09037".to_string()),
        chembl_id: None,
        unii: None,
        drug_type: None,
        mechanism: None,
        mechanisms: Vec::new(),
        approval_date: None,
        approval_date_raw: None,
        approval_date_display: None,
        approval_summary: None,
        brand_names: vec!["Keytruda".to_string()],
        route: None,
        targets: Vec::new(),
        variant_targets: Vec::new(),
        target_family: None,
        target_family_name: None,
        indications: Vec::new(),
        interactions: Vec::new(),
        interaction_text: None,
        pharm_classes: Vec::new(),
        top_adverse_events: vec!["Rash".to_string()],
        faers_query: None,
        label: None,
        label_set_id: None,
        shortage: Some(vec![crate::entities::drug::DrugShortageEntry {
            status: Some("Current".to_string()),
            availability: Some("Limited".to_string()),
            company_name: Some("Example Pharma".to_string()),
            generic_name: Some("pembrolizumab".to_string()),
            related_info: Some("https://example.org/us-shortage".to_string()),
            update_date: Some("2026-01-13".to_string()),
            initial_posting_date: None,
        }]),
        approvals: Some(vec![DrugApproval {
            application_number: "BLA125514".to_string(),
            sponsor_name: Some("Merck Sharp & Dohme".to_string()),
            openfda_brand_names: vec!["Keytruda".to_string()],
            openfda_generic_names: vec!["pembrolizumab".to_string()],
            products: Vec::new(),
            submissions: Vec::new(),
        }]),
        us_safety_warnings: Some("Immune-mediated adverse reactions.".to_string()),
        ema_regulatory: Some(vec![EmaRegulatoryRow {
            medicine_name: "Keytruda".to_string(),
            active_substance: "pembrolizumab".to_string(),
            ema_product_number: "EMEA/H/C/003820".to_string(),
            status: "Authorised".to_string(),
            holder: Some("Merck Sharp & Dohme B.V.".to_string()),
            recent_activity: vec![crate::entities::drug::EmaRegulatoryActivity {
                first_published_date: "27/02/2026".to_string(),
                last_updated_date: None,
            }],
        }]),
        ema_safety: Some(EmaSafetyInfo {
            dhpcs: vec![crate::entities::drug::EmaDhpcEntry {
                medicine_name: "Keytruda".to_string(),
                dhpc_type: Some("DHPC".to_string()),
                regulatory_outcome: Some("Updated safety communication".to_string()),
                first_published_date: Some("15/01/2026".to_string()),
                last_updated_date: None,
            }],
            referrals: Vec::new(),
            psusas: Vec::new(),
        }),
        ema_shortage: Some(vec![EmaShortageEntry {
            medicine_affected: "Keytruda".to_string(),
            status: Some("Resolved".to_string()),
            availability_of_alternatives: Some("Yes".to_string()),
            first_published_date: Some("10/01/2026".to_string()),
            last_updated_date: Some("13/01/2026".to_string()),
        }]),
        who_prequalification: None,
        civic: None,
    };

    let markdown = drug_markdown_with_region(&drug, &["all".to_string()], DrugRegion::All, false)
        .expect("markdown");
    assert!(markdown.contains("## Regulatory (US - Drugs@FDA)"));
    assert!(markdown.contains("## Regulatory (EU - EMA)"));
    assert!(markdown.contains("## Safety (US - OpenFDA)"));
    assert!(markdown.contains("## Safety (EU - EMA)"));
    assert!(markdown.contains("## Shortage (US - OpenFDA Drug Shortages)"));
    assert!(markdown.contains("## Shortage (EU - EMA)"));
    assert!(markdown.contains("BLA125514"));
    assert!(markdown.contains("EMEA/H/C/003820"));
    assert!(markdown.contains("Immune-mediated adverse reactions."));
    assert!(markdown.contains("Resolved"));
}

#[test]
fn drug_markdown_with_region_who_renders_regulatory_block() {
    let drug = Drug {
        name: "trastuzumab".to_string(),
        drugbank_id: Some("DB00072".to_string()),
        chembl_id: None,
        unii: None,
        drug_type: None,
        mechanism: None,
        mechanisms: Vec::new(),
        approval_date: None,
        approval_date_raw: None,
        approval_date_display: None,
        approval_summary: None,
        brand_names: vec!["Herceptin".to_string()],
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
        who_prequalification: Some(vec![WhoPrequalificationEntry {
            who_reference_number: "BT-ON001".to_string(),
            inn: "Trastuzumab".to_string(),
            presentation: "Trastuzumab Powder for concentrate for solution for infusion 150 mg"
                .to_string(),
            dosage_form: "Powder for concentrate for solution for infusion".to_string(),
            product_type: "Biotherapeutic Product".to_string(),
            therapeutic_area: "Oncology".to_string(),
            applicant: "Samsung Bioepis NL B.V.".to_string(),
            listing_basis: "Prequalification - Abridged".to_string(),
            alternative_listing_basis: None,
            prequalification_date: Some("2019-12-18".to_string()),
        }]),
        civic: None,
    };

    let markdown =
        drug_markdown_with_region(&drug, &["regulatory".to_string()], DrugRegion::Who, false)
            .expect("markdown");

    assert!(markdown.contains("## Regulatory (WHO Prequalification)"));
    assert!(markdown.contains("| WHO Ref | Presentation | Dosage Form |"));
    assert!(markdown.contains("BT-ON001"));
    assert!(markdown.contains("Samsung Bioepis NL B.V."));
    assert!(markdown.contains("2019-12-18"));
}

#[test]
fn drug_search_all_region_markdown_includes_who_block() {
    let markdown = drug_search_markdown_with_region(
        "trastuzumab",
        DrugRegion::All,
        &[crate::entities::drug::DrugSearchResult {
            name: "trastuzumab".to_string(),
            drugbank_id: None,
            mechanism: None,
            target: Some("ERBB2".to_string()),
            drug_type: None,
        }],
        Some(1),
        &[crate::entities::drug::EmaDrugSearchResult {
            name: "Herzuma".to_string(),
            active_substance: "trastuzumab".to_string(),
            ema_product_number: "EMEA/H/C/004123".to_string(),
            status: "Authorised".to_string(),
        }],
        Some(1),
        &[crate::entities::drug::WhoPrequalificationSearchResult {
            inn: "Trastuzumab".to_string(),
            therapeutic_area: "Oncology".to_string(),
            dosage_form: "Powder for concentrate for solution for infusion".to_string(),
            applicant: "Samsung Bioepis NL B.V.".to_string(),
            who_reference_number: "BT-ON001".to_string(),
            listing_basis: "Prequalification - Abridged".to_string(),
            prequalification_date: Some("2019-12-18".to_string()),
        }],
        Some(1),
        "",
    )
    .expect("markdown");

    assert!(markdown.contains("## US (MyChem.info / OpenFDA)"));
    assert!(markdown.contains("## EU (EMA)"));
    assert!(markdown.contains("## WHO (WHO Prequalification)"));
    assert!(markdown.contains("BT-ON001"));
    assert!(markdown.contains("EMEA/H/C/004123"));
}

#[test]
fn drug_markdown_with_region_eu_all_suppresses_us_header_facts() {
    // Criterion 9: `get drug <name> all --region eu` must not show US-specific
    // header lines (FDA Approved, Safety FAERS) even though the full card is rendered.
    let drug = Drug {
        name: "pembrolizumab".to_string(),
        drugbank_id: Some("DB09037".to_string()),
        chembl_id: None,
        unii: None,
        drug_type: None,
        mechanism: None,
        mechanisms: Vec::new(),
        approval_date: Some("2014-09-04".to_string()),
        approval_date_raw: Some("20140904".to_string()),
        approval_date_display: Some("September 4, 2014".to_string()),
        approval_summary: Some("FDA approved September 4, 2014".to_string()),
        brand_names: vec!["Keytruda".to_string()],
        route: None,
        targets: Vec::new(),
        variant_targets: Vec::new(),
        target_family: None,
        target_family_name: None,
        indications: Vec::new(),
        interactions: Vec::new(),
        interaction_text: None,
        pharm_classes: Vec::new(),
        top_adverse_events: vec!["Fatigue".to_string(), "Rash".to_string()],
        faers_query: None,
        label: None,
        label_set_id: None,
        shortage: None,
        approvals: None,
        us_safety_warnings: None,
        ema_regulatory: Some(vec![EmaRegulatoryRow {
            medicine_name: "Keytruda".to_string(),
            active_substance: "pembrolizumab".to_string(),
            ema_product_number: "EMEA/H/C/003820".to_string(),
            status: "Authorised".to_string(),
            holder: None,
            recent_activity: Vec::new(),
        }]),
        ema_safety: Some(EmaSafetyInfo {
            dhpcs: Vec::new(),
            referrals: Vec::new(),
            psusas: Vec::new(),
        }),
        ema_shortage: Some(Vec::new()),
        who_prequalification: None,
        civic: None,
    };

    let markdown = drug_markdown_with_region(&drug, &["all".to_string()], DrugRegion::Eu, false)
        .expect("markdown");

    // EU EMA section must be present
    assert!(markdown.contains("## Regulatory (EU - EMA)"));
    assert!(markdown.contains("EMEA/H/C/003820"));

    // US-specific header facts must be absent
    assert!(
        !markdown.contains("FDA Approved"),
        "US approval date must not appear in EU-only output"
    );
    assert!(
        !markdown.contains("Safety (OpenFDA FAERS)"),
        "US FAERS safety line must not appear in EU-only output"
    );
}

#[test]
fn drug_markdown_with_region_eu_safety_shows_truthful_empty_subsections() {
    let drug = Drug {
        name: "semaglutide".to_string(),
        drugbank_id: Some("DB13928".to_string()),
        chembl_id: None,
        unii: None,
        drug_type: None,
        mechanism: None,
        mechanisms: Vec::new(),
        approval_date: None,
        approval_date_raw: None,
        approval_date_display: None,
        approval_summary: None,
        brand_names: vec!["Ozempic".to_string()],
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
        ema_safety: Some(EmaSafetyInfo {
            dhpcs: vec![crate::entities::drug::EmaDhpcEntry {
                medicine_name: "Ozempic".to_string(),
                dhpc_type: Some("DHPC".to_string()),
                regulatory_outcome: Some("Medicine shortage".to_string()),
                first_published_date: Some("10/01/2026".to_string()),
                last_updated_date: Some("13/01/2026".to_string()),
            }],
            referrals: Vec::new(),
            psusas: Vec::new(),
        }),
        ema_shortage: None,
        who_prequalification: None,
        civic: None,
    };

    let markdown = drug_markdown_with_region(&drug, &["safety".to_string()], DrugRegion::Eu, false)
        .expect("markdown");
    assert!(markdown.contains("## Safety (EU - EMA)"));
    assert!(markdown.contains("### DHPCs"));
    assert!(markdown.contains("Medicine shortage"));
    assert!(markdown.contains("### Referrals"));
    assert!(markdown.contains("### PSUSAs"));
    assert!(markdown.contains("No data found (EMA)"));
}

#[test]
fn drug_search_empty_state_frames_zero_indication_miss_as_regulatory_signal() {
    let markdown = drug_search_markdown_with_region(
        "indication=Marfan syndrome",
        DrugRegion::Us,
        &[],
        Some(0),
        &[],
        None,
        &[],
        None,
        "",
    )
    .expect("markdown");

    assert!(markdown.contains("U.S. regulatory data"));
    assert!(markdown.contains("This absence is informative"));
    assert!(markdown.contains(
        "biomcp search article -k \"Marfan syndrome treatment\" --type review --limit 5"
    ));
    assert!(markdown.contains("Try: biomcp discover \"Marfan syndrome\""));
    assert!(!markdown.contains("No drugs found\n"));
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

#[test]
fn drug_search_standard_empty_state_includes_discover_hint() {
    let markdown = drug_search_markdown_with_footer("MK-3475", &[], Some(0), "").expect("markdown");

    assert!(markdown.contains("Try: biomcp discover MK-3475"));
}

#[test]
fn drug_search_eu_empty_state_includes_discover_hint() {
    let markdown = drug_search_markdown_with_region(
        "MK-3475",
        DrugRegion::Eu,
        &[],
        None,
        &[],
        Some(0),
        &[],
        None,
        "",
    )
    .expect("markdown");

    assert!(markdown.contains("Try: biomcp discover MK-3475"));
}

#[test]
fn drug_search_all_region_empty_state_calls_out_regulatory_absence() {
    let markdown = drug_search_markdown_with_region(
        "indication=Marfan syndrome",
        DrugRegion::All,
        &[],
        Some(0),
        &[],
        Some(0),
        &[],
        Some(0),
        "",
    )
    .expect("markdown");

    assert!(
        markdown.contains("specific to the structured regulatory portion of the combined search")
    );
    assert!(markdown.contains("## US (MyChem.info / OpenFDA)"));
    assert!(markdown.contains("## EU (EMA)"));
}

#[test]
fn drug_search_all_region_empty_state_includes_discover_only_when_both_regions_are_empty() {
    let empty_markdown = drug_search_markdown_with_region(
        "MK-3475",
        DrugRegion::All,
        &[],
        Some(0),
        &[],
        Some(0),
        &[],
        Some(0),
        "",
    )
    .expect("markdown");
    assert!(empty_markdown.contains("Try: biomcp discover MK-3475"));

    let us_only_markdown = drug_search_markdown_with_region(
        "MK-3475",
        DrugRegion::All,
        &[crate::entities::drug::DrugSearchResult {
            name: "pembrolizumab".to_string(),
            drugbank_id: None,
            mechanism: None,
            target: None,
            drug_type: None,
        }],
        Some(1),
        &[],
        Some(0),
        &[],
        Some(0),
        "",
    )
    .expect("markdown");
    assert!(!us_only_markdown.contains("Try: biomcp discover MK-3475"));

    let eu_only_markdown = drug_search_markdown_with_region(
        "MK-3475",
        DrugRegion::All,
        &[],
        Some(0),
        &[crate::entities::drug::EmaDrugSearchResult {
            name: "Keytruda".to_string(),
            active_substance: "pembrolizumab".to_string(),
            ema_product_number: "EMEA/H/C/003820".to_string(),
            status: "Authorized".to_string(),
        }],
        Some(1),
        &[],
        Some(0),
        "",
    )
    .expect("markdown");
    assert!(!eu_only_markdown.contains("Try: biomcp discover MK-3475"));
}

#[test]
fn drug_search_eu_indication_empty_state_includes_discover_hint() {
    let markdown = drug_search_markdown_with_region(
        "indication=Marfan syndrome",
        DrugRegion::Eu,
        &[],
        None,
        &[],
        Some(0),
        &[],
        None,
        "",
    )
    .expect("markdown");

    assert!(markdown.contains("Try: biomcp discover \"Marfan syndrome\""));
}

#[test]
fn pgx_markdown_includes_evidence_links() {
    let pgx = Pgx {
        query: "CYP2D6".to_string(),
        gene: Some("CYP2D6".to_string()),
        drug: Some("warfarin".to_string()),
        interactions: Vec::new(),
        recommendations: Vec::new(),
        frequencies: Vec::new(),
        guidelines: Vec::new(),
        annotations: Vec::new(),
        annotations_note: None,
    };

    let markdown = pgx_markdown(&pgx, &[]).expect("rendered markdown");
    assert!(markdown.contains("[CPIC](https://cpicpgx.org/genes/cyp2d6/)"));
    assert!(markdown.contains("[PharmGKB](https://www.pharmgkb.org/gene/CYP2D6)"));
    assert!(markdown.contains("[PharmGKB](https://www.pharmgkb.org/chemical/warfarin)"));
}

#[test]
fn protein_markdown_renders_complexes_summary_and_detail_bullets() {
    let protein = Protein {
            accession: "P15056".to_string(),
            entry_id: Some("BRAF_HUMAN".to_string()),
            name: "Serine/threonine-protein kinase B-raf".to_string(),
            gene_symbol: Some("BRAF".to_string()),
            organism: Some("Homo sapiens".to_string()),
            length: Some(766),
            function: None,
            structures: Vec::new(),
            structure_count: None,
            domains: Vec::new(),
            interactions: Vec::new(),
            complexes: vec![
                ProteinComplex {
                    accession: "CPX-1234".to_string(),
                    name: "BRAF signaling complex with an intentionally long display label that should truncate in the summary table".to_string(),
                    description: Some("Signals through MAPK.".to_string()),
                    curation: ProteinComplexCuration::Curated,
                    components: vec![
                        ProteinComplexComponent {
                            accession: "P15056".to_string(),
                            name: "BRAF".to_string(),
                            stoichiometry: Some("1".to_string()),
                        },
                        ProteinComplexComponent {
                            accession: "Q02750".to_string(),
                            name: "MAP2K1".to_string(),
                            stoichiometry: None,
                        },
                        ProteinComplexComponent {
                            accession: "P10398".to_string(),
                            name: "RAF1".to_string(),
                            stoichiometry: None,
                        },
                        ProteinComplexComponent {
                            accession: "P07900".to_string(),
                            name: "HSP90AA1".to_string(),
                            stoichiometry: None,
                        },
                        ProteinComplexComponent {
                            accession: "Q16543".to_string(),
                            name: "CDC37".to_string(),
                            stoichiometry: None,
                        },
                        ProteinComplexComponent {
                            accession: "Q9Y243".to_string(),
                            name: "AKT3".to_string(),
                            stoichiometry: None,
                        },
                        ProteinComplexComponent {
                            accession: "P31749".to_string(),
                            name: "AKT1".to_string(),
                            stoichiometry: None,
                        },
                    ],
                },
                ProteinComplex {
                    accession: "CPX-5678".to_string(),
                    name: "Complex with no listed members".to_string(),
                    description: None,
                    curation: ProteinComplexCuration::Predicted,
                    components: Vec::new(),
                },
            ],
        };

    let markdown = protein_markdown(&protein, &["complexes".to_string()]).expect("markdown");
    assert!(markdown.contains("## Complexes"));
    assert!(markdown.contains("| ID | Name | Members | Curation |"));
    assert!(!markdown.contains("| ID | Name | Components | Curation |"));
    assert!(
        markdown.contains(
            "| CPX-1234 | BRAF signaling complex with an intentionally long di… | 7 | curated |"
        ),
        "expected truncated complex summary row, got:\n{markdown}"
    );
    assert!(markdown.contains(
            "- `CPX-1234` members (7): BRAF (1), MAP2K1, RAF1, HSP90AA1, CDC37, +2 more\n  Description: Signals through MAPK."
        ));
    assert!(markdown.contains("- `CPX-5678` members (0): none listed"));
    assert!(!markdown.contains("- `CPX-1234`: Signals through MAPK."));
    assert!(!markdown.contains("AKT3"));
    assert!(!markdown.contains("AKT1"));
    assert!(!markdown.contains("See also: biomcp get protein P15056 complexes"));
}

#[test]
fn article_search_markdown_preserves_rank_order_and_shows_rationale() {
    let rows = vec![
        ArticleSearchResult {
            pmid: "1".into(),
            title: "Entity-ranked".into(),
            pmcid: Some("PMC1".into()),
            doi: Some("10.1000/one".into()),
            journal: Some("Journal A".into()),
            date: Some("2025-01-01".into()),
            citation_count: Some(10),
            influential_citation_count: Some(4),
            source: ArticleSource::PubTator,
            score: Some(99.1),
            is_retracted: Some(false),
            abstract_snippet: Some("Abstract one".into()),
            ranking: Some(crate::entities::article::ArticleRankingMetadata {
                directness_tier: 3,
                anchor_count: 2,
                title_anchor_hits: 2,
                abstract_anchor_hits: 0,
                combined_anchor_hits: 2,
                all_anchors_in_title: true,
                all_anchors_in_text: true,
                study_or_review_cue: false,
                pubmed_rescue: false,
                pubmed_rescue_kind: None,
                pubmed_source_position: None,
                mode: Some(crate::entities::article::ArticleRankingMode::Lexical),
                semantic_score: None,
                lexical_score: None,
                citation_score: None,
                position_score: None,
                composite_score: None,
                avg_source_rank: None,
            }),
            matched_sources: vec![ArticleSource::PubTator, ArticleSource::SemanticScholar],
            normalized_title: "entity-ranked".into(),
            normalized_abstract: "abstract one".into(),
            publication_type: None,
            source_local_position: 0,
        },
        ArticleSearchResult {
            pmid: "2".into(),
            title: "Field-ranked".into(),
            pmcid: None,
            doi: None,
            journal: Some("Journal B".into()),
            date: Some("2025-01-02".into()),
            citation_count: Some(12),
            influential_citation_count: Some(1),
            source: ArticleSource::EuropePmc,
            score: None,
            is_retracted: Some(false),
            abstract_snippet: Some("Abstract two".into()),
            ranking: Some(crate::entities::article::ArticleRankingMetadata {
                directness_tier: 2,
                anchor_count: 2,
                title_anchor_hits: 1,
                abstract_anchor_hits: 1,
                combined_anchor_hits: 2,
                all_anchors_in_title: false,
                all_anchors_in_text: true,
                study_or_review_cue: true,
                pubmed_rescue: false,
                pubmed_rescue_kind: None,
                pubmed_source_position: None,
                mode: Some(crate::entities::article::ArticleRankingMode::Lexical),
                semantic_score: None,
                lexical_score: None,
                citation_score: None,
                position_score: None,
                composite_score: None,
                avg_source_rank: None,
            }),
            matched_sources: vec![ArticleSource::EuropePmc],
            normalized_title: "field-ranked".into(),
            normalized_abstract: "abstract two".into(),
            publication_type: Some("Review".into()),
            source_local_position: 1,
        },
    ];

    let markdown = article_search_markdown_with_footer_and_context(
            "gene=BRAF",
            &rows,
            "",
            &article_filters_for_test(crate::entities::article::ArticleSort::Relevance),
            true,
            Some(
                "Note: --type restricts article search to Europe PMC and PubMed. PubTator3, LitSense2, and Semantic Scholar do not support publication-type filtering.",
            ),
            None,
        )
        .expect("markdown should render");
    assert!(markdown.contains(
            "> Note: --type restricts article search to Europe PMC and PubMed. PubTator3, LitSense2, and Semantic Scholar do not support publication-type filtering."
        ));
    assert!(markdown.contains("Semantic Scholar: enabled"));
    assert!(markdown.contains("Ranking: calibrated PubMed rescue + lexical directness"));
    assert!(markdown.contains("| PMID | Title | Source(s) | Date | Why | Cit. |"));
    assert!(markdown.contains("PubTator3, Semantic Scholar"));
    assert!(markdown.contains("title 2/2"));
    assert!(markdown.contains("title+abstract 2/2"));
    assert!(
        markdown
            .contains("--date-from/--date-to <YYYY|YYYY-MM|YYYY-MM-DD> (alias: --since/--until)")
    );
    assert!(!markdown.contains("## PubTator3"));
    assert!(!markdown.contains("## Europe PMC"));
    assert!(markdown.find("|1|").unwrap() < markdown.find("|2|").unwrap());
}

#[test]
fn article_ranking_why_tier1_mixed_shows_title_plus_abstract() {
    let row = ArticleSearchResult {
        pmid: "1".into(),
        title: "Partial coverage".into(),
        pmcid: None,
        doi: None,
        journal: None,
        date: None,
        citation_count: None,
        influential_citation_count: None,
        source: ArticleSource::EuropePmc,
        matched_sources: vec![ArticleSource::EuropePmc],
        score: None,
        is_retracted: None,
        abstract_snippet: None,
        ranking: Some(crate::entities::article::ArticleRankingMetadata {
            directness_tier: 1,
            anchor_count: 3,
            title_anchor_hits: 1,
            abstract_anchor_hits: 1,
            combined_anchor_hits: 2,
            all_anchors_in_title: false,
            all_anchors_in_text: false,
            study_or_review_cue: false,
            pubmed_rescue: false,
            pubmed_rescue_kind: None,
            pubmed_source_position: None,
            mode: Some(crate::entities::article::ArticleRankingMode::Lexical),
            semantic_score: None,
            lexical_score: None,
            citation_score: None,
            position_score: None,
            composite_score: None,
            avg_source_rank: None,
        }),
        normalized_title: "partial coverage".into(),
        normalized_abstract: String::new(),
        publication_type: None,
        source_local_position: 0,
    };
    let why = article_ranking_why(&row, &article_filters_for_test(ArticleSort::Relevance));
    assert_eq!(why, "title+abstract 2/3");
}

#[test]
fn article_ranking_why_rescue_composes_with_lexical_reason() {
    let row = ArticleSearchResult {
        pmid: "1".into(),
        title: "Rescued partial coverage".into(),
        pmcid: None,
        doi: None,
        journal: None,
        date: None,
        citation_count: None,
        influential_citation_count: None,
        source: ArticleSource::PubMed,
        matched_sources: vec![ArticleSource::PubMed],
        score: None,
        is_retracted: None,
        abstract_snippet: None,
        ranking: Some(crate::entities::article::ArticleRankingMetadata {
            directness_tier: 1,
            anchor_count: 3,
            title_anchor_hits: 1,
            abstract_anchor_hits: 1,
            combined_anchor_hits: 2,
            all_anchors_in_title: false,
            all_anchors_in_text: false,
            study_or_review_cue: false,
            pubmed_rescue: true,
            pubmed_rescue_kind: Some(crate::entities::article::ArticlePubMedRescueKind::Unique),
            pubmed_source_position: Some(0),
            mode: Some(crate::entities::article::ArticleRankingMode::Lexical),
            semantic_score: None,
            lexical_score: None,
            citation_score: None,
            position_score: None,
            composite_score: None,
            avg_source_rank: None,
        }),
        normalized_title: "rescued partial coverage".into(),
        normalized_abstract: String::new(),
        publication_type: None,
        source_local_position: 0,
    };

    let why = article_ranking_why(&row, &article_filters_for_test(ArticleSort::Relevance));
    assert_eq!(why, "pubmed-rescue + title+abstract 2/3");
}

#[test]
fn article_ranking_why_semantic_includes_score_and_lexical_context() {
    let row = ArticleSearchResult {
        pmid: "1".into(),
        title: "Semantic lead".into(),
        pmcid: None,
        doi: None,
        journal: None,
        date: None,
        citation_count: None,
        influential_citation_count: None,
        source: ArticleSource::EuropePmc,
        matched_sources: vec![ArticleSource::EuropePmc],
        score: Some(0.81234),
        is_retracted: None,
        abstract_snippet: None,
        ranking: Some(crate::entities::article::ArticleRankingMetadata {
            directness_tier: 2,
            anchor_count: 3,
            title_anchor_hits: 2,
            abstract_anchor_hits: 0,
            combined_anchor_hits: 2,
            all_anchors_in_title: true,
            all_anchors_in_text: false,
            study_or_review_cue: false,
            pubmed_rescue: false,
            pubmed_rescue_kind: None,
            pubmed_source_position: None,
            mode: Some(crate::entities::article::ArticleRankingMode::Semantic),
            semantic_score: Some(0.81234),
            lexical_score: None,
            citation_score: None,
            position_score: None,
            composite_score: None,
            avg_source_rank: None,
        }),
        normalized_title: "semantic lead".into(),
        normalized_abstract: String::new(),
        publication_type: None,
        source_local_position: 0,
    };

    let why = article_ranking_why(&row, &article_filters_for_test(ArticleSort::Relevance));
    assert_eq!(why, "semantic 0.812 + title 2/3");
}

#[test]
fn article_ranking_why_hybrid_includes_score_and_lexical_context() {
    let row = ArticleSearchResult {
        pmid: "1".into(),
        title: "Hybrid lead".into(),
        pmcid: None,
        doi: None,
        journal: None,
        date: None,
        citation_count: None,
        influential_citation_count: None,
        source: ArticleSource::EuropePmc,
        matched_sources: vec![ArticleSource::EuropePmc],
        score: Some(0.9),
        is_retracted: None,
        abstract_snippet: None,
        ranking: Some(crate::entities::article::ArticleRankingMetadata {
            directness_tier: 1,
            anchor_count: 3,
            title_anchor_hits: 1,
            abstract_anchor_hits: 1,
            combined_anchor_hits: 2,
            all_anchors_in_title: false,
            all_anchors_in_text: false,
            study_or_review_cue: false,
            pubmed_rescue: false,
            pubmed_rescue_kind: None,
            pubmed_source_position: None,
            mode: Some(crate::entities::article::ArticleRankingMode::Hybrid),
            semantic_score: Some(0.9),
            lexical_score: Some(1.0 / 3.0),
            citation_score: Some(0.1),
            position_score: Some(0.4),
            composite_score: Some(0.61234),
            avg_source_rank: Some(1.0),
        }),
        normalized_title: "hybrid lead".into(),
        normalized_abstract: String::new(),
        publication_type: None,
        source_local_position: 0,
    };

    let why = article_ranking_why(&row, &article_filters_for_test(ArticleSort::Relevance));
    assert_eq!(why, "hybrid 0.612 + title+abstract 2/3");
}

#[test]
fn search_all_markdown_renders_section_note() {
    let results = crate::cli::search_all::SearchAllResults {
        query: "gene=EGFR disease=non-small cell lung cancer".to_string(),
        sections: vec![crate::cli::search_all::SearchAllSection {
            entity: "variant".to_string(),
            label: "Variants".to_string(),
            count: 1,
            total: Some(1),
            error: None,
            note: Some(
                "No disease-filtered variants found; showing top gene variants.".to_string(),
            ),
            results: vec![serde_json::json!({
                "id": "rs121434568",
                "gene": "EGFR",
                "hgvs_p": "L858R",
                "significance": "Pathogenic",
            })],
            links: Vec::new(),
        }],
        searches_dispatched: 1,
        searches_with_results: 1,
        wall_time_ms: 42,
        debug_plan: None,
    };

    let markdown = search_all_markdown(&results, false).expect("markdown should render");
    assert!(markdown.contains("> No disease-filtered variants found; showing top gene variants."));
}

#[test]
fn article_search_markdown_prepends_debug_plan_block() {
    let debug_plan = DebugPlan {
        surface: "search_article",
        query: "gene=BRAF".to_string(),
        anchor: None,
        legs: vec![crate::cli::debug_plan::DebugPlanLeg {
            leg: "article".to_string(),
            entity: "article".to_string(),
            filters: vec!["gene=BRAF".to_string()],
            routing: vec!["planner=federated".to_string()],
            sources: vec!["PubTator3".to_string(), "Europe PMC".to_string()],
            matched_sources: vec!["PubTator3".to_string()],
            count: 1,
            total: Some(1),
            note: None,
            error: None,
        }],
    };
    let rows = vec![ArticleSearchResult {
        pmid: "1".into(),
        title: "Entity-ranked".into(),
        pmcid: None,
        doi: None,
        journal: Some("Journal A".into()),
        date: Some("2025-01-01".into()),
        citation_count: Some(10),
        influential_citation_count: Some(4),
        source: ArticleSource::PubTator,
        score: Some(99.1),
        is_retracted: Some(false),
        abstract_snippet: Some("Abstract one".into()),
        ranking: None,
        matched_sources: vec![ArticleSource::PubTator],
        normalized_title: "entity-ranked".into(),
        normalized_abstract: "abstract one".into(),
        publication_type: None,
        source_local_position: 0,
    }];

    let markdown = article_search_markdown_with_footer_and_context(
        "gene=BRAF",
        &rows,
        "",
        &article_filters_for_test(crate::entities::article::ArticleSort::Relevance),
        true,
        None,
        Some(&debug_plan),
    )
    .expect("markdown should render");

    assert!(markdown.starts_with("## Debug plan"));
    assert!(markdown.contains("\"surface\": \"search_article\""));
    assert!(markdown.contains("# Articles: gene=BRAF"));
}

#[test]
fn study_list_markdown_renders_study_table() {
    let markdown = study_list_markdown(&[StudyInfo {
        study_id: "msk_impact_2017".to_string(),
        name: "MSK-IMPACT".to_string(),
        cancer_type: Some("mixed".to_string()),
        citation: Some("Zehir et al.".to_string()),
        sample_count: Some(10945),
        available_data: vec!["mutations".to_string(), "cna".to_string()],
    }]);

    assert!(markdown.contains("# Study Datasets"));
    assert!(markdown.contains("| Study ID | Name | Cancer Type | Samples | Available Data |"));
    assert!(markdown.contains("msk_impact_2017"));
    assert!(markdown.contains("mutations, cna"));
}

#[test]
fn study_query_markdown_renders_mutation_shape() {
    let markdown = study_query_markdown(&StudyQueryResult::MutationFrequency(
        StudyMutationFrequencyResult {
            study_id: "msk_impact_2017".to_string(),
            gene: "TP53".to_string(),
            mutation_count: 10,
            unique_samples: 9,
            total_samples: 100,
            frequency: 0.09,
            top_variant_classes: vec![("Missense_Mutation".to_string(), 8)],
            top_protein_changes: vec![("p.R175H".to_string(), 3)],
        },
    ));

    assert!(markdown.contains("# Study Mutation Frequency: TP53 (msk_impact_2017)"));
    assert!(markdown.contains("| Mutation records | 10 |"));
    assert!(markdown.contains("## Top Variant Classes"));
    assert!(markdown.contains("## Top Protein Changes"));
}

#[test]
fn study_query_markdown_renders_cna_and_expression_shapes() {
    let cna = study_query_markdown(&StudyQueryResult::CnaDistribution(
        StudyCnaDistributionResult {
            study_id: "brca_tcga_pan_can_atlas_2018".to_string(),
            gene: "ERBB2".to_string(),
            total_samples: 20,
            deep_deletion: 1,
            shallow_deletion: 2,
            diploid: 10,
            gain: 4,
            amplification: 3,
        },
    ));
    assert!(cna.contains("# Study CNA Distribution: ERBB2 (brca_tcga_pan_can_atlas_2018)"));
    assert!(cna.contains("| Amplification (2) | 3 |"));

    let expression = study_query_markdown(&StudyQueryResult::ExpressionDistribution(
        StudyExpressionDistributionResult {
            study_id: "paad_qcmg_uq_2016".to_string(),
            gene: "KRAS".to_string(),
            file: "data_mrna_seq_v2_rsem_zscores_ref_all_samples.txt".to_string(),
            sample_count: 50,
            mean: 0.2,
            median: 0.1,
            min: -2.0,
            max: 2.5,
            q1: -0.4,
            q3: 0.5,
        },
    ));
    assert!(expression.contains("# Study Expression Distribution: KRAS (paad_qcmg_uq_2016)"));
    assert!(expression.contains("| Sample count | 50 |"));
}

#[test]
fn study_filter_markdown_renders_tables_and_samples() {
    let markdown = study_filter_markdown(&StudyFilterResult {
        study_id: "brca_tcga_pan_can_atlas_2018".to_string(),
        criteria: vec![
            crate::entities::study::FilterCriterionSummary {
                description: "mutated TP53".to_string(),
                matched_count: 3,
            },
            crate::entities::study::FilterCriterionSummary {
                description: "amplified ERBB2".to_string(),
                matched_count: 2,
            },
        ],
        total_study_samples: Some(4),
        matched_count: 2,
        matched_sample_ids: vec!["S2".to_string(), "S3".to_string()],
    });

    assert!(markdown.contains("# Study Filter: brca_tcga_pan_can_atlas_2018"));
    assert!(markdown.contains("## Criteria"));
    assert!(markdown.contains("| Filter | Matching Samples |"));
    assert!(markdown.contains("| mutated TP53 | 3 |"));
    assert!(markdown.contains("## Result"));
    assert!(markdown.contains("| Study Total Samples | 4 |"));
    assert!(markdown.contains("| Intersection | 2 |"));
    assert!(markdown.contains("## Matched Samples"));
    assert!(markdown.contains("S2"));
    assert!(markdown.contains("S3"));
}

#[test]
fn study_filter_markdown_renders_empty_results_and_unknown_totals() {
    let markdown = study_filter_markdown(&StudyFilterResult {
        study_id: "demo_study".to_string(),
        criteria: vec![crate::entities::study::FilterCriterionSummary {
            description: "expression > 1.5 for MYC".to_string(),
            matched_count: 0,
        }],
        total_study_samples: None,
        matched_count: 0,
        matched_sample_ids: Vec::new(),
    });

    assert!(markdown.contains("| Study Total Samples | - |"));
    assert!(markdown.contains("| Intersection | 0 |"));
    assert!(markdown.contains("## Matched Samples"));
    assert!(markdown.contains("\nNone\n"));
}

#[test]
fn study_filter_markdown_truncates_long_sample_lists() {
    let markdown = study_filter_markdown(&StudyFilterResult {
        study_id: "long_study".to_string(),
        criteria: vec![crate::entities::study::FilterCriterionSummary {
            description: "mutated TP53".to_string(),
            matched_count: 55,
        }],
        total_study_samples: Some(100),
        matched_count: 55,
        matched_sample_ids: (1..=55).map(|idx| format!("S{idx}")).collect(),
    });

    assert!(markdown.contains("S1"));
    assert!(markdown.contains("S50"));
    assert!(!markdown.contains("S51\n"));
    assert!(markdown.contains("... and 5 more (use --json for full list)"));
}

#[test]
fn study_co_occurrence_markdown_renders_pair_table() {
    let markdown = study_co_occurrence_markdown(&StudyCoOccurrenceResult {
        study_id: "msk_impact_2017".to_string(),
        genes: vec!["TP53".to_string(), "KRAS".to_string()],
        total_samples: 100,
        sample_universe_basis: StudySampleUniverseBasis::ClinicalSampleFile,
        pairs: vec![StudyCoOccurrencePair {
            gene_a: "TP53".to_string(),
            gene_b: "KRAS".to_string(),
            both_mutated: 10,
            a_only: 20,
            b_only: 15,
            neither: 55,
            log_odds_ratio: Some(0.1234),
            p_value: Some(6.0e-22),
        }],
    });

    assert!(markdown.contains("# Study Co-occurrence: msk_impact_2017"));
    assert!(markdown.contains("Sample universe: clinical sample file"));
    assert!(markdown.contains(
        "| Gene A | Gene B | Both | A only | B only | Neither | Log Odds Ratio | p-value |"
    ));
    assert!(markdown.contains("| TP53 | KRAS | 10 | 20 | 15 | 55 | 0.123400 | 6.000e-22 |"));
}

#[test]
fn study_co_occurrence_markdown_marks_mutation_observed_fallback() {
    let markdown = study_co_occurrence_markdown(&StudyCoOccurrenceResult {
        study_id: "fallback_study".to_string(),
        genes: vec!["TP53".to_string(), "KRAS".to_string()],
        total_samples: 3,
        sample_universe_basis: StudySampleUniverseBasis::MutationObserved,
        pairs: vec![],
    });

    assert!(markdown.contains(
        "Sample universe: mutation-observed samples only (clinical sample file unavailable)"
    ));
}

#[test]
fn study_cohort_markdown_renders_group_counts() {
    let markdown = study_cohort_markdown(&StudyCohortResult {
        study_id: "brca_tcga_pan_can_atlas_2018".to_string(),
        gene: "TP53".to_string(),
        stratification: "mutation".to_string(),
        mutant_samples: 348,
        wildtype_samples: 736,
        mutant_patients: 348,
        wildtype_patients: 736,
        total_samples: 1084,
        total_patients: 1084,
    });

    assert!(markdown.contains("# Study Cohort: TP53 (brca_tcga_pan_can_atlas_2018)"));
    assert!(markdown.contains("Stratification: mutation status"));
    assert!(markdown.contains("| Group | Samples | Patients |"));
    assert!(markdown.contains("| TP53-mutant | 348 | 348 |"));
    assert!(markdown.contains("| Total | 1084 | 1084 |"));
}

#[test]
fn study_survival_markdown_renders_group_table() {
    let markdown = study_survival_markdown(&StudySurvivalResult {
        study_id: "brca_tcga_pan_can_atlas_2018".to_string(),
        gene: "TP53".to_string(),
        endpoint: StudySurvivalEndpoint::Os,
        groups: vec![
            StudySurvivalGroupResult {
                group_name: "TP53-mutant".to_string(),
                n_patients: 340,
                n_events: 48,
                n_censored: 292,
                km_median_months: Some(85.2),
                survival_1yr: Some(0.91),
                survival_3yr: Some(0.72),
                survival_5yr: None,
                event_rate: 0.141176,
                km_curve_points: Vec::new(),
            },
            StudySurvivalGroupResult {
                group_name: "TP53-wildtype".to_string(),
                n_patients: 720,
                n_events: 64,
                n_censored: 656,
                km_median_months: None,
                survival_1yr: Some(0.97),
                survival_3yr: Some(0.88),
                survival_5yr: Some(0.74),
                event_rate: 0.088889,
                km_curve_points: Vec::new(),
            },
        ],
        log_rank_p: Some(0.0042),
    });

    assert!(markdown.contains("# Study Survival: TP53 (brca_tcga_pan_can_atlas_2018)"));
    assert!(markdown.contains("Endpoint: Overall Survival (OS)"));
    assert!(
        markdown.contains(
            "| Group | N | Events | Censored | Event Rate | KM Median | 1yr | 3yr | 5yr |"
        )
    );
    assert!(
        markdown.contains("| TP53-mutant | 340 | 48 | 292 | 0.141176 | 85.2 | 0.910 | 0.720 | - |")
    );
    assert!(markdown.contains("Log-rank p-value: 0.004"));
}

#[test]
fn study_download_markdown_renders_result_table() {
    let markdown = study_download_markdown(&StudyDownloadResult {
        study_id: "msk_impact_2017".to_string(),
        path: "/tmp/studies/msk_impact_2017".to_string(),
        downloaded: true,
    });

    assert!(markdown.contains("# Study Download: msk_impact_2017"));
    assert!(markdown.contains("| Study ID | msk_impact_2017 |"));
    assert!(markdown.contains("| Downloaded | yes |"));
}

#[test]
fn study_download_catalog_markdown_renders_remote_ids() {
    let markdown = study_download_catalog_markdown(&StudyDownloadCatalog {
        study_ids: vec![
            "msk_impact_2017".to_string(),
            "brca_tcga_pan_can_atlas_2018".to_string(),
        ],
    });

    assert!(markdown.contains("# Downloadable cBioPortal Studies"));
    assert!(markdown.contains("| Study ID |"));
    assert!(markdown.contains("| msk_impact_2017 |"));
    assert!(markdown.contains("| brca_tcga_pan_can_atlas_2018 |"));
}

#[test]
fn study_compare_expression_markdown_renders_distribution_table() {
    let markdown = study_compare_expression_markdown(&StudyExpressionComparisonResult {
        study_id: "brca_tcga_pan_can_atlas_2018".to_string(),
        stratify_gene: "TP53".to_string(),
        target_gene: "ERBB2".to_string(),
        groups: vec![
            StudyExpressionGroupStats {
                group_name: "TP53-mutant".to_string(),
                sample_count: 345,
                mean: 0.234,
                median: 0.112,
                min: -2.1,
                max: 4.5,
                q1: -0.45,
                q3: 0.78,
            },
            StudyExpressionGroupStats {
                group_name: "TP53-wildtype".to_string(),
                sample_count: 730,
                mean: -0.089,
                median: -0.156,
                min: -3.2,
                max: 5.1,
                q1: -0.67,
                q3: 0.34,
            },
        ],
        mann_whitney_u: Some(9821.0),
        mann_whitney_p: Some(0.003),
    });

    assert!(markdown.contains("# Study Group Comparison: Expression"));
    assert!(markdown.contains(
        "Stratify gene: TP53 | Target gene: ERBB2 | Study: brca_tcga_pan_can_atlas_2018"
    ));
    assert!(markdown.contains("| Group | N | Mean | Median | Q1 | Q3 | Min | Max |"));
    assert!(markdown.contains("Mann-Whitney U: 9821.000"));
    assert!(markdown.contains("Mann-Whitney p-value: 0.003"));
    assert!(
        markdown.contains(
            "| TP53-wildtype | 730 | -0.089 | -0.156 | -0.670 | 0.340 | -3.200 | 5.100 |"
        )
    );
}

#[test]
fn study_compare_mutations_markdown_renders_rate_table() {
    let markdown = study_compare_mutations_markdown(&StudyMutationComparisonResult {
        study_id: "brca_tcga_pan_can_atlas_2018".to_string(),
        stratify_gene: "TP53".to_string(),
        target_gene: "PIK3CA".to_string(),
        groups: vec![
            StudyMutationGroupStats {
                group_name: "TP53-mutant".to_string(),
                sample_count: 348,
                mutated_count: 120,
                mutation_rate: 0.344828,
            },
            StudyMutationGroupStats {
                group_name: "TP53-wildtype".to_string(),
                sample_count: 736,
                mutated_count: 220,
                mutation_rate: 0.298913,
            },
        ],
    });

    assert!(markdown.contains("# Study Group Comparison: Mutation Rate"));
    assert!(markdown.contains(
        "Stratify gene: TP53 | Target gene: PIK3CA | Study: brca_tcga_pan_can_atlas_2018"
    ));
    assert!(markdown.contains("| Group | N | Mutated | Mutation Rate |"));
    assert!(markdown.contains("| TP53-mutant | 348 | 120 | 0.344828 |"));
}
