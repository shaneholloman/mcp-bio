//! Shared test-only helpers and re-exports for nested drug module tests.

#[allow(unused_imports)]
pub(super) use crate::entities::SearchPage;
#[allow(unused_imports)]
pub(super) use crate::error::BioMcpError;
#[allow(unused_imports)]
pub(super) use crate::sources::mychem::MyChemHit;

#[allow(unused_imports)]
pub(super) use super::{DrugRegion, DrugSearchFilters, DrugSearchResult, WhoPrequalificationEntry};

pub(super) fn mychem_row(name: &str) -> DrugSearchResult {
    DrugSearchResult {
        name: name.to_string(),
        drugbank_id: None,
        drug_type: None,
        mechanism: None,
        target: None,
    }
}

pub(super) fn who_row(reference: &str, inn: &str) -> WhoPrequalificationEntry {
    WhoPrequalificationEntry {
        who_reference_number: Some(reference.to_string()),
        inn: inn.to_string(),
        presentation: Some(format!("{inn} Tablet 100mg")),
        dosage_form: Some("Tablet".to_string()),
        product_type: "Finished Pharmaceutical Product".to_string(),
        therapeutic_area: "Malaria".to_string(),
        applicant: "Example Applicant".to_string(),
        listing_basis: Some("Prequalification - Abridged".to_string()),
        alternative_listing_basis: None,
        prequalification_date: Some("2024-01-01".to_string()),
        who_product_id: None,
        grade: None,
        confirmation_document_date: None,
    }
}

pub(super) fn who_api_row(product_id: &str, inn: &str) -> WhoPrequalificationEntry {
    WhoPrequalificationEntry {
        who_reference_number: None,
        inn: inn.to_string(),
        presentation: None,
        dosage_form: None,
        product_type: "Active Pharmaceutical Ingredient".to_string(),
        therapeutic_area: "Malaria".to_string(),
        applicant: "Example API Applicant".to_string(),
        listing_basis: None,
        alternative_listing_basis: None,
        prequalification_date: Some("2024-01-01".to_string()),
        who_product_id: Some(product_id.to_string()),
        grade: Some("Standard".to_string()),
        confirmation_document_date: Some("2024-02-01".to_string()),
    }
}
