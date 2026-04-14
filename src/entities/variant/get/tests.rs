//! Sidecar tests for variant detail and enrichment helpers.

use super::super::test_support::*;
use super::*;

#[test]
fn variant_json_omits_legacy_name_when_absent() {
    let variant = gwas_only_variant_stub("rs7903146");
    let json = serde_json::to_value(&variant).expect("variant should serialize");
    assert!(json.get("legacy_name").is_none());
}

#[test]
fn parse_sections_supports_new_variant_sections() {
    let flags = parse_sections(&[
        "conservation".to_string(),
        "predictions".to_string(),
        "cosmic".to_string(),
        "cgi".to_string(),
        "civic".to_string(),
        "cbioportal".to_string(),
        "gwas".to_string(),
    ])
    .expect("sections should parse");

    assert!(flags.include_conservation);
    assert!(flags.include_expanded_predictions);
    assert!(flags.include_cosmic);
    assert!(flags.include_cgi);
    assert!(flags.include_civic);
    assert!(flags.include_cbioportal);
    assert!(flags.include_gwas);
}

#[test]
fn gwas_only_request_detection_matches_section_flags() {
    let gwas_only = parse_sections(&["gwas".to_string()]).expect("sections should parse");
    assert!(is_gwas_only_request(&gwas_only));

    let gwas_plus_clinvar = parse_sections(&["gwas".to_string(), "clinvar".to_string()])
        .expect("sections should parse");
    assert!(!is_gwas_only_request(&gwas_plus_clinvar));
}

#[test]
fn gwas_only_variant_stub_keeps_requested_rsid() {
    let variant = gwas_only_variant_stub("rs7903146");
    assert_eq!(variant.id, "rs7903146");
    assert_eq!(variant.rsid.as_deref(), Some("rs7903146"));
    assert!(variant.gwas.is_empty());
    assert_eq!(variant.gwas_unavailable_reason, None);
}

#[test]
fn civic_molecular_profile_name_prefers_gene_and_hgvs_p() {
    let variant = Variant {
        gene: "BRAF".into(),
        id: "chr7:g.140453136A>T".into(),
        hgvs_p: Some("p.V600E".into()),
        legacy_name: None,
        hgvs_c: None,
        rsid: None,
        cosmic_id: None,
        significance: None,
        clinvar_id: None,
        clinvar_review_status: None,
        clinvar_review_stars: None,
        conditions: Vec::new(),
        gnomad_af: None,
        allele_frequency_raw: None,
        allele_frequency_percent: None,
        consequence: None,
        cadd_score: None,
        sift_pred: None,
        polyphen_pred: None,
        conservation: None,
        expanded_predictions: Vec::new(),
        population_breakdown: None,
        cosmic_context: None,
        cgi_associations: Vec::new(),
        civic: None,
        clinvar_conditions: Vec::new(),
        clinvar_condition_reports: None,
        top_disease: None,
        cancer_frequencies: Vec::new(),
        cancer_frequency_source: None,
        gwas: Vec::new(),
        gwas_unavailable_reason: None,
        supporting_pmids: None,
        prediction: None,
    };

    assert_eq!(
        civic_molecular_profile_name(&variant).as_deref(),
        Some("BRAF V600E")
    );
}

#[tokio::test]
async fn gwas_only_request_returns_variant_when_gwas_is_unavailable() {
    let _env = lock_env().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(
            "/singleNucleotidePolymorphisms/rs7903146/associations",
        ))
        .and(query_param("projection", "associationByStudy"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_string("{bad-json"),
        )
        .mount(&server)
        .await;

    let _base = set_env_var("BIOMCP_GWAS_BASE", Some(&server.uri()));
    let variant = get("rs7903146", &["gwas".to_string()])
        .await
        .expect("GWAS-only request should degrade");

    assert_eq!(variant.id, "rs7903146");
    assert!(variant.gwas.is_empty());
    assert_eq!(
        variant.gwas_unavailable_reason.as_deref(),
        Some("GWAS association data temporarily unavailable.")
    );
    assert_eq!(variant.supporting_pmids, None);
}

#[test]
fn therapies_from_oncokb_truncation_shows_count() {
    let annotation: OncoKBAnnotation = serde_json::from_value(serde_json::json!({
        "treatments": [
            {"level": "LEVEL_1", "drugs": [{"drugName": "osimertinib"}], "cancerType": {"name": "Lung"}},
            {"level": "LEVEL_2", "drugs": [{"drugName": "afatinib"}], "cancerType": {"name": "Lung"}},
            {"level": "LEVEL_3A", "drugs": [{"drugName": "erlotinib"}], "cancerType": {"name": "Lung"}},
            {"level": "LEVEL_3B", "drugs": [{"drugName": "gefitinib"}], "cancerType": {"name": "Lung"}},
            {"level": "LEVEL_4", "drugs": [{"drugName": "dacomitinib"}], "cancerType": {"name": "Lung"}},
            {"level": "LEVEL_R1", "drugs": [{"drugName": "poziotinib"}], "cancerType": {"name": "Lung"}},
            {"level": "LEVEL_R2", "drugs": [{"drugName": "mobocertinib"}], "cancerType": {"name": "Lung"}}
        ]
    }))
    .expect("valid OncoKB annotation");

    let therapies = therapies_from_oncokb(&annotation);
    assert_eq!(therapies.len(), 6);
    assert!(
        therapies
            .last()
            .and_then(|row| row.note.as_deref())
            .is_some_and(|note| note.contains("(and 1 more)"))
    );
}
