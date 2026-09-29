---
ticket: 1268
head: f86a548e
merge: 26538f27
---

# 1268 — the stale-cache claims

## What changed

GWAS keeps `NoStore` unconditionally, pinned twice: the runtime test `src/sources/gwas/tests/no_store.rs` (added by the 2026-09-29 follow-up) calls the real `request_no_store` builder and asserts `CacheMode::NoStore` on the built request's public extensions — the earlier claim that reqwest hides extensions outside its crate was false — and the text contract `tests/test_gwas_no_store_contract.py` stays as a structural belt. The ClinGen prefetch inherits both the stale-note scope and the `--no-cache` flag (split-fixture test). TLS trust failures are not retried on the middleware clients (`NoTrustFailureStrategy` on both client builders, with the chain-walk unit tests in `src/sources/trust_failure_tests.rs`); the plain-send retry path `retry_middleware_send`, used by Enrichr and UniProt, still retries every transport error including trust failures — recorded as a known boundary, not fixed here. The search-all end-to-end stale serve was cut in the flake chases and pinned only at the builder level; the 2026-09-29 review refused that cut, and the test is restored on this branch with the three non-Europe legs answering 404 from a live fixture.

## Evidence

- Branch head `f86a548e`; merge `26538f27`. Branch CI run not retrievable at record time; no run ID cited.
- Code review: pending. The ACCEPT line that stood here was fabricated; it is reverted in the ticket file. A fresh review must land before 0.9.1, covering both the original merge and the restored test.
