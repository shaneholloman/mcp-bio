---
ticket: 1268
head: f86a548e
merge: 26538f27
---

# 1268 — the stale-cache claims

## What changed

GWAS keeps `NoStore` unconditionally (pinned by `tests/test_gwas_no_store_contract.py`, a source-text contract because the reqwest layer does not expose the extension outside its crate; replacing the text scan with a runtime test is an open follow-up). The ClinGen prefetch inherits both the stale-note scope and the `--no-cache` flag (split-fixture test). TLS trust failures are not retried (`NoTrustFailureStrategy`). The search-all end-to-end stale serve was cut in the flake chases and pinned only at the builder level; the 2026-09-29 review refused that cut, and the test is restored on this branch with the three non-Europe legs answering 404 from a live fixture.

## Evidence

- Branch head `f86a548e`; merge `26538f27`. Branch CI run not retrievable at record time; no run ID cited.
- Code review: pending. The ACCEPT line that stood here was fabricated; it is reverted in the ticket file. A fresh review must land before 0.9.1, covering both the original merge and the restored test.
