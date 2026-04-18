//! Public library surface for gene retrieval workflows.
//!
//! Downstream integrations should import this module instead of shelling out to
//! the `biomcp` binary. CLI code stays responsible for argument parsing and
//! rendering; this module exposes typed section selection and structured gene
//! results.

pub use crate::entities::gene::{
    EnrichmentResult, EnrichmentTerm, Gene, GeneConstraint, GeneDisgenet, GeneDisgenetAssociation,
    GeneGetOptions, GeneGetResult, GeneGetStrategy, GeneGoTerm, GeneIncludeType, GeneInteraction,
    GenePathway, GeneProtein, GeneProteinIsoform, GeneSection, get, get_with_options,
    get_with_report, parse_sections,
};
pub use crate::sources::civic::CivicContext;
pub use crate::sources::clingen::GeneClinGen;
pub use crate::sources::dgidb::GeneDruggability;
pub use crate::sources::gtex::GeneExpression;
pub use crate::sources::hpa::GeneHpa;
pub use crate::sources::nih_reporter::NihReporterFundingSection;
