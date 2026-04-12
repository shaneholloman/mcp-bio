use super::super::test_support::*;
use super::*;

#[test]
fn civic_gene_symbol_extraction_ignores_protein_change_tokens() {
    assert_eq!(
        civic_gene_symbol_from_profile("BRAF V600E").as_deref(),
        Some("BRAF")
    );
    assert_eq!(
        civic_gene_symbol_from_profile("V600E BRAF").as_deref(),
        Some("BRAF")
    );
    assert_eq!(civic_gene_symbol_from_profile("V600E"), None);
}

pub(crate) async fn proof_augment_genes_with_opentargets_merges_sources_without_duplicates() {
    let mut disease = test_disease("MONDO:0003864", "chronic lymphocytic leukemia");
    disease.associated_genes = vec!["TP53".into(), "BCL2".into()];
    disease.gene_associations = vec![
        DiseaseGeneAssociation {
            gene: "TP53".into(),
            relationship: Some("causal".into()),
            source: Some("Monarch".into()),
            opentargets_score: None,
        },
        DiseaseGeneAssociation {
            gene: "BCL2".into(),
            relationship: Some("associated with disease".into()),
            source: Some("CIViC".into()),
            opentargets_score: None,
        },
    ];
    disease.top_gene_scores = vec![
        DiseaseTargetScore {
            symbol: "TP53".into(),
            summary: DiseaseAssociationScoreSummary {
                overall_score: 0.99,
                gwas_score: None,
                rare_variant_score: None,
                somatic_mutation_score: Some(0.88),
            },
        },
        DiseaseTargetScore {
            symbol: "BCL2".into(),
            summary: DiseaseAssociationScoreSummary {
                overall_score: 0.91,
                gwas_score: None,
                rare_variant_score: None,
                somatic_mutation_score: Some(0.72),
            },
        },
        DiseaseTargetScore {
            symbol: "ATM".into(),
            summary: DiseaseAssociationScoreSummary {
                overall_score: 0.84,
                gwas_score: None,
                rare_variant_score: None,
                somatic_mutation_score: Some(0.67),
            },
        },
    ];

    augment_genes_with_opentargets(&mut disease)
        .await
        .expect("augmentation should succeed");
    attach_opentargets_scores(&mut disease);

    assert_eq!(disease.gene_associations.len(), 3);
    assert_eq!(
        disease.gene_associations[0].source.as_deref(),
        Some("Monarch; OpenTargets")
    );
    assert_eq!(
        disease.gene_associations[1].source.as_deref(),
        Some("CIViC; OpenTargets")
    );
    assert_eq!(disease.gene_associations[2].gene, "ATM");
    assert_eq!(
        disease.gene_associations[2].source.as_deref(),
        Some("OpenTargets")
    );
}

#[tokio::test]
async fn augment_genes_with_opentargets_merges_sources_without_duplicates() {
    proof_augment_genes_with_opentargets_merges_sources_without_duplicates().await;
}

pub(crate) async fn proof_augment_genes_with_opentargets_respects_twenty_gene_cap() {
    let mut disease = test_disease("MONDO:0003864", "chronic lymphocytic leukemia");
    disease.associated_genes = (0..20).map(|index| format!("GENE{index}")).collect();
    disease.gene_associations = (0..20)
        .map(|index| DiseaseGeneAssociation {
            gene: format!("GENE{index}"),
            relationship: Some("associated".into()),
            source: Some("Monarch".into()),
            opentargets_score: None,
        })
        .collect();
    disease.top_gene_scores = vec![DiseaseTargetScore {
        symbol: "TP53".into(),
        summary: DiseaseAssociationScoreSummary {
            overall_score: 0.99,
            gwas_score: None,
            rare_variant_score: None,
            somatic_mutation_score: Some(0.88),
        },
    }];

    augment_genes_with_opentargets(&mut disease)
        .await
        .expect("augmentation should succeed");

    assert_eq!(disease.gene_associations.len(), 20);
    assert!(
        !disease
            .gene_associations
            .iter()
            .any(|row| row.gene == "TP53")
    );
    assert_eq!(disease.associated_genes.len(), 20);
}

#[tokio::test]
async fn augment_genes_with_opentargets_respects_twenty_gene_cap() {
    proof_augment_genes_with_opentargets_respects_twenty_gene_cap().await;
}
