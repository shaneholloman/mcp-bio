use super::*;

#[tokio::test]
async fn study_co_occurrence_requires_2_to_10_genes() {
    let err = execute(vec![
        "biomcp".to_string(),
        "study".to_string(),
        "co-occurrence".to_string(),
        "--study".to_string(),
        "msk_impact_2017".to_string(),
        "--genes".to_string(),
        "TP53".to_string(),
    ])
    .await
    .expect_err("study co-occurrence should validate gene count");
    assert!(err.to_string().contains("--genes must contain 2 to 10"));
}

#[tokio::test]
async fn study_filter_requires_at_least_one_criterion() {
    let err = execute(vec![
        "biomcp".to_string(),
        "study".to_string(),
        "filter".to_string(),
        "--study".to_string(),
        "brca_tcga_pan_can_atlas_2018".to_string(),
    ])
    .await
    .expect_err("study filter should require criteria");
    assert!(
        err.to_string()
            .contains("At least one filter criterion is required")
    );
}

#[tokio::test]
async fn study_filter_rejects_malformed_expression_threshold() {
    let err = execute(vec![
        "biomcp".to_string(),
        "study".to_string(),
        "filter".to_string(),
        "--study".to_string(),
        "brca_tcga_pan_can_atlas_2018".to_string(),
        "--expression-above".to_string(),
        "MYC:not-a-number".to_string(),
    ])
    .await
    .expect_err("study filter should validate threshold format");
    assert!(err.to_string().contains("--expression-above"));
    assert!(err.to_string().contains("GENE:THRESHOLD"));
}

#[tokio::test]
async fn study_survival_rejects_unknown_endpoint() {
    let err = execute(vec![
        "biomcp".to_string(),
        "study".to_string(),
        "survival".to_string(),
        "--study".to_string(),
        "msk_impact_2017".to_string(),
        "--gene".to_string(),
        "TP53".to_string(),
        "--endpoint".to_string(),
        "foo".to_string(),
    ])
    .await
    .expect_err("study survival should validate endpoint");
    assert!(err.to_string().contains("Unknown survival endpoint"));
}

#[tokio::test]
async fn study_compare_rejects_unknown_type() {
    let err = execute(vec![
        "biomcp".to_string(),
        "study".to_string(),
        "compare".to_string(),
        "--study".to_string(),
        "msk_impact_2017".to_string(),
        "--gene".to_string(),
        "TP53".to_string(),
        "--type".to_string(),
        "foo".to_string(),
        "--target".to_string(),
        "ERBB2".to_string(),
    ])
    .await
    .expect_err("study compare should validate type");
    assert!(err.to_string().contains("Unknown comparison type"));
}
