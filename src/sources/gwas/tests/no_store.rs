//! The runtime no-store pin for GWAS (ticket 1268 follow-up,
//! 2026-09-29 review: the text-scan contract claimed reqwest hides
//! request extensions outside its crate; that is false —
//! `RequestBuilder::build()` yields a `Request` whose `extensions()`
//! are public — so the invariant is now proven on the real request
//! object, not on the source's text shape).

use super::super::*;
use http_cache_reqwest::CacheMode;

/// Every GWAS request carries `CacheMode::NoStore` on the built
/// request itself, whatever `BIOMCP_CACHE_MODE` says: both plan
/// entry points route through `request_no_store`, and the extension
/// survives onto the `reqwest::Request` the cache middleware reads.
#[test]
fn gwas_requests_carry_no_store_on_the_built_request() {
    let client = GwasClient {
        client: crate::sources::shared_client().expect("shared client"),
        base: std::borrow::Cow::Borrowed("http://gwas.test"),
    };

    let plans = [
        client
            .associations_by_rsid_plan("rs36053993")
            .expect("rsid plan"),
        GwasClient::association_search_plan(Some("BRAF"), None, 5).expect("search plan"),
    ];
    for plan in plans {
        // Call the real builder both request paths use, then inspect
        // the built request the middleware would send.
        let request = client
            .request_no_store(&plan)
            .build()
            .expect("built GWAS request");
        assert_eq!(
            request.extensions().get::<CacheMode>(),
            Some(&CacheMode::NoStore),
            "the GWAS request must carry CacheMode::NoStore onto the wire"
        );
    }
}
