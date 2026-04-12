//! Top-level disease proof-hook facade preserved for src/lib.rs tests.

pub(crate) async fn proof_augment_genes_with_opentargets_merges_sources_without_duplicates() {
    super::associations::proof_augment_genes_with_opentargets_merges_sources_without_duplicates()
        .await;
}

pub(crate) async fn proof_augment_genes_with_opentargets_respects_twenty_gene_cap() {
    super::associations::proof_augment_genes_with_opentargets_respects_twenty_gene_cap().await;
}

pub(crate) async fn proof_enrich_sparse_disease_identity_prefers_exact_ols4_match() {
    super::enrichment::proof_enrich_sparse_disease_identity_prefers_exact_ols4_match().await;
}

pub(crate) async fn proof_get_disease_genes_promotes_opentargets_rows_for_cll() {
    super::get::proof_get_disease_genes_promotes_opentargets_rows_for_cll().await;
}

pub(crate) async fn proof_get_disease_genes_uses_ols4_label_fallback_for_sparse_mondo_identity() {
    super::get::proof_get_disease_genes_uses_ols4_label_fallback_for_sparse_mondo_identity().await;
}
