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
        who_reference_number: reference.to_string(),
        inn: inn.to_string(),
        presentation: format!("{inn} Tablet 100mg"),
        dosage_form: "Tablet".to_string(),
        product_type: "Finished Pharmaceutical Product".to_string(),
        therapeutic_area: "Malaria".to_string(),
        applicant: "Example Applicant".to_string(),
        listing_basis: "Prequalification - Abridged".to_string(),
        alternative_listing_basis: None,
        prequalification_date: Some("2024-01-01".to_string()),
    }
}
