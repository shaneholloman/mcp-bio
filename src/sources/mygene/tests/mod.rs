//! mygene tier tests.
//!
//! Tier 2 (`construction`) asserts the pure `RequestPlan` builders; Tier 3 (`parsing`)
//! decodes committed fixture bytes. Both are pure: no local server, env var, or lock.
//! Tier 4 (`live`) hits the real API and is `#[ignore]`d (verify-lane / parity only).

mod construction;
mod live;
mod parsing;
