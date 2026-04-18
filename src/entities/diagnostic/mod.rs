//! Diagnostic entity models and workflows exposed through the stable diagnostic facade.

mod get;
mod search;

pub use self::get::get;
#[allow(unused_imports)]
pub use self::search::{search, search_page, search_query_summary};

use serde::{Deserialize, Serialize};

use crate::sources::gtr::{GtrIndex, GtrRecord};

pub(crate) const DIAGNOSTIC_SOURCE: &str = "gtr";
const DIAGNOSTIC_SECTION_GENES: &str = "genes";
const DIAGNOSTIC_SECTION_CONDITIONS: &str = "conditions";
const DIAGNOSTIC_SECTION_METHODS: &str = "methods";
const DIAGNOSTIC_SECTION_ALL: &str = "all";

pub const DIAGNOSTIC_SECTION_NAMES: &[&str] = &[
    DIAGNOSTIC_SECTION_GENES,
    DIAGNOSTIC_SECTION_CONDITIONS,
    DIAGNOSTIC_SECTION_METHODS,
    DIAGNOSTIC_SECTION_ALL,
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub source: String,
    pub source_id: String,
    pub accession: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub laboratory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub institution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clia_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_licenses: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_status: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub method_categories: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub methods: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticSearchResult {
    pub accession: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer_or_lab: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub genes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct DiagnosticSearchFilters {
    pub gene: Option<String>,
    pub disease: Option<String>,
    pub test_type: Option<String>,
    pub manufacturer: Option<String>,
}

pub(crate) fn preferred_diagnostic_name(record: &GtrRecord) -> String {
    optional_text(&record.lab_test_name)
        .or_else(|| optional_text(&record.manufacturer_test_name))
        .unwrap_or_else(|| record.accession.clone())
}

pub(crate) fn manufacturer_or_lab_label(record: &GtrRecord) -> Option<String> {
    optional_text(&record.manufacturer_test_name)
        .or_else(|| optional_text(&record.name_of_laboratory))
        .or_else(|| optional_text(&record.lab_test_name))
}

pub(crate) fn optional_text(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

pub(crate) fn search_result(record: &GtrRecord, index: &GtrIndex) -> DiagnosticSearchResult {
    DiagnosticSearchResult {
        accession: record.accession.clone(),
        name: preferred_diagnostic_name(record),
        test_type: optional_text(&record.test_type),
        manufacturer_or_lab: manufacturer_or_lab_label(record),
        genes: index.merged_genes(&record.accession),
        conditions: index.conditions(&record.accession),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::Path;

    use crate::sources::gtr::{GTR_CONDITION_GENE_FILE, GTR_TEST_VERSION_FILE, resolve_gtr_root};
    use crate::test_support::{TempDirGuard, env_lock, set_env_var};

    async fn install_fixture_root(
        label: &str,
    ) -> (
        tokio::sync::MutexGuard<'static, ()>,
        TempDirGuard,
        crate::test_support::EnvVarGuard,
    ) {
        let lock = env_lock().lock().await;
        let root = TempDirGuard::new(label);
        write_fixture(root.path());
        let env = set_env_var(
            "BIOMCP_GTR_DIR",
            Some(root.path().to_str().expect("utf-8 path")),
        );
        (lock, root, env)
    }

    fn write_fixture(root: &Path) {
        std::fs::write(
            root.join(GTR_TEST_VERSION_FILE),
            include_bytes!("../../../spec/fixtures/gtr/test_version.gz"),
        )
        .expect("write test_version.gz");
        std::fs::write(
            root.join(GTR_CONDITION_GENE_FILE),
            include_str!("../../../spec/fixtures/gtr/test_condition_gene.txt"),
        )
        .expect("write test_condition_gene.txt");
    }

    #[tokio::test]
    async fn search_page_requires_at_least_one_filter() {
        let err = search_page(&DiagnosticSearchFilters::default(), 10, 0)
            .await
            .expect_err("missing filters should fail");

        assert_eq!(
            err.to_string(),
            "Invalid argument: diagnostic search requires at least one of --gene, --disease, --type, or --manufacturer"
        );
    }

    #[tokio::test]
    async fn search_page_applies_conjunctive_filters_and_stable_ordering() {
        let (_lock, _root, _env) = install_fixture_root("diagnostic-search").await;

        let page = search_page(
            &DiagnosticSearchFilters {
                gene: Some("EGFR".to_string()),
                disease: Some("melanoma".to_string()),
                test_type: Some("molecular".to_string()),
                manufacturer: Some("Precision".to_string()),
            },
            10,
            0,
        )
        .await
        .expect("search page");

        assert_eq!(page.total, Some(1));
        assert_eq!(page.results.len(), 1);
        assert_eq!(page.results[0].accession, "GTR000000002.1");
        assert_eq!(page.results[0].name, "EGFR Melanoma Molecular Assay");
        assert_eq!(page.results[0].genes, vec!["EGFR".to_string()]);
        assert_eq!(
            page.results[0].conditions,
            vec!["Cutaneous melanoma".to_string()]
        );
        assert_eq!(
            resolve_gtr_root()
                .file_name()
                .and_then(|value| value.to_str()),
            Some(
                _root
                    .path()
                    .file_name()
                    .and_then(|value| value.to_str())
                    .expect("fixture dir")
            )
        );
    }

    #[tokio::test]
    async fn get_keeps_summary_by_default_and_requested_sections_as_options() {
        let (_lock, _root, _env) = install_fixture_root("diagnostic-get").await;

        let summary = get("GTR000000001.1", &[]).await.expect("summary get");
        assert_eq!(summary.source, "gtr");
        assert_eq!(summary.source_id, "GTR000000001.1");
        assert_eq!(summary.accession, "GTR000000001.1");
        assert_eq!(summary.name, "BRCA1 Hereditary Cancer Panel");
        assert!(summary.genes.is_none());
        assert!(summary.conditions.is_none());
        assert!(summary.methods.is_none());
        assert_eq!(
            summary.method_categories,
            vec!["Molecular genetics".to_string()]
        );

        let expanded = get(
            "GTR000000001.1",
            &[
                "genes".to_string(),
                "conditions".to_string(),
                "methods".to_string(),
            ],
        )
        .await
        .expect("expanded get");
        assert_eq!(
            expanded.genes,
            Some(vec!["BRCA1".to_string(), "BARD1".to_string()])
        );
        assert_eq!(
            expanded.conditions,
            Some(vec![
                "Hereditary breast ovarian cancer syndrome".to_string(),
                "Breast cancer".to_string()
            ])
        );
        assert_eq!(
            expanded.methods,
            Some(vec![
                "Sequence analysis".to_string(),
                "Deletion/duplication analysis".to_string()
            ])
        );
    }

    #[test]
    fn search_query_summary_uses_documented_filter_order() {
        let summary = search_query_summary(&DiagnosticSearchFilters {
            gene: Some("BRCA1".to_string()),
            disease: Some("melanoma".to_string()),
            test_type: Some("molecular".to_string()),
            manufacturer: Some("Tempus".to_string()),
        });

        assert_eq!(
            summary,
            "gene=BRCA1, disease=melanoma, type=molecular, manufacturer=Tempus"
        );
    }
}
