//! nci_cts tier tests.
//!
//! Tier 2 (`construction`) asserts the pure `RequestPlan` builders; Tier 3 (`parsing`)
//! decodes committed fixture bytes. Both are pure — no MockServer, no env var, no lock.
//! Tier 4 (`live`) hits the real API (needs `NCI_API_KEY`) and is `#[ignore]`d.

mod construction;
mod live;
mod parsing;
