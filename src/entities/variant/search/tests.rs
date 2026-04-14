//! Sidecar tests for variant MyVariant search helpers.

use super::super::{VariantProteinAlias, VariantSearchFilters, VariantSearchResult};
use super::*;

#[test]
fn search_query_summary_includes_hgvsc_and_rsid() {
    let summary = search_query_summary(&VariantSearchFilters {
        gene: Some("BRAF".into()),
        hgvsc: Some("c.1799T>A".into()),
        rsid: Some("rs113488022".into()),
        ..Default::default()
    });
    assert_eq!(summary, "gene=BRAF, hgvsc=c.1799T>A, rsid=rs113488022");
}

#[test]
fn search_query_summary_includes_residue_alias_marker() {
    let summary = search_query_summary(&VariantSearchFilters {
        gene: Some("PTPN22".into()),
        protein_alias: Some(VariantProteinAlias {
            position: 620,
            residue: 'W',
        }),
        ..Default::default()
    });
    assert_eq!(summary, "gene=PTPN22, residue_alias=620W");
}

#[test]
fn exon_deletion_fallback_preserves_non_exon_filters() {
    let filters = VariantSearchFilters {
        gene: Some("EGFR".into()),
        significance: Some("pathogenic".into()),
        max_frequency: Some(0.01),
        min_cadd: Some(20.0),
        consequence: Some("inframe_deletion".into()),
        review_status: Some("reviewed_by_expert_panel".into()),
        population: Some("eas".into()),
        revel_min: Some(0.7),
        gerp_min: Some(2.5),
        tumor_site: Some("lung".into()),
        condition: Some("nsclc".into()),
        impact: Some("moderate".into()),
        lof: true,
        has: Some("clinvar".into()),
        missing: Some("cosmic".into()),
        therapy: Some("osimertinib".into()),
        ..Default::default()
    };

    let params = exon_deletion_fallback_params(&filters, 25, 10);
    assert_eq!(params.gene.as_deref(), Some("EGFR"));
    assert!(params.hgvsp.is_none());
    assert!(params.hgvsc.is_none());
    assert!(params.rsid.is_none());
    assert!(params.consequence.is_none());
    assert_eq!(params.significance.as_deref(), Some("pathogenic"));
    assert_eq!(params.max_frequency, Some(0.01));
    assert_eq!(params.min_cadd, Some(20.0));
    assert_eq!(
        params.review_status.as_deref(),
        Some("reviewed_by_expert_panel")
    );
    assert_eq!(params.population.as_deref(), Some("eas"));
    assert_eq!(params.revel_min, Some(0.7));
    assert_eq!(params.gerp_min, Some(2.5));
    assert_eq!(params.tumor_site.as_deref(), Some("lung"));
    assert_eq!(params.condition.as_deref(), Some("nsclc"));
    assert_eq!(params.impact.as_deref(), Some("moderate"));
    assert!(params.lof);
    assert_eq!(params.has.as_deref(), Some("clinvar"));
    assert_eq!(params.missing.as_deref(), Some("cosmic"));
    assert_eq!(params.therapy.as_deref(), Some("osimertinib"));
    assert_eq!(params.limit, 25);
    assert_eq!(params.offset, 10);
}

#[test]
fn quality_score_prioritizes_significance_and_frequency() {
    let rich = VariantSearchResult {
        id: "chr1:g.1A>T".into(),
        gene: "TP53".into(),
        hgvs_p: Some("p.V1A".into()),
        legacy_name: None,
        significance: Some("Pathogenic".into()),
        clinvar_stars: None,
        gnomad_af: Some(0.001),
        revel: None,
        gerp: None,
    };
    let sparse = VariantSearchResult {
        id: "chr1:g.2A>T".into(),
        gene: "TP53".into(),
        hgvs_p: Some("p.V2A".into()),
        legacy_name: None,
        significance: None,
        clinvar_stars: None,
        gnomad_af: None,
        revel: None,
        gerp: None,
    };

    assert!(search_result_quality_score(&rich) > search_result_quality_score(&sparse));
}
