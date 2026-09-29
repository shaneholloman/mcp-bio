# Make the stale-cache claims true

From sdlc/issues/2026-09-28-review-follow-ups-after-1263-and-1264.md
(Stale-cache note section and Stdio test section). The 1263 round
overclaimed in three places; this ticket makes the code, the tests,
and the records agree.

## Fixes

1. GWAS: the exclusion was false — apply_cache_mode replaced GWAS's
   no-store mark with force-cache under BIOMCP_CACHE_MODE=infinite.
   Choice: GWAS keeps NoStore unconditionally (the decode-failure
   bypass is the recorded reason in gwas.rs); apply_cache_mode is no
   longer applied to GWAS requests. Proof, stated at what it really
   shows (2026-09-29 review corrected an overstated sentence here):
   the runtime pin `src/sources/gwas/tests/no_store.rs` calls the
   real `request_no_store` builder for both plan entry points and
   asserts `CacheMode::NoStore` on the built request's public
   extensions; the text contract `tests/test_gwas_no_store_contract.py`
   pins the source shape (every request path routes through the
   builder; `apply_cache_mode` never runs for GWAS). No test runs
   the command under `BIOMCP_CACHE_MODE=infinite` against a dead
   fixture; the extension-on-the-request proof is the guarantee the
   cache middleware reads.
2. ClinGen prefetch: carries the NO_CACHE flag across the spawn the
   same way it carries the notes handle. Proof:
   `no_cache_skips_the_cache_for_the_spawned_clingen_fetch` — a warm
   cache then a dead server then `--no-cache` must fail rather than
   serve the cached rows to the prefetch.
3. Tests parse `_meta.notes` (article and search-all); the
   search-all end-to-end stale serve is restored (2026-09-29):
   Europe PMC rides the killable fixture and the other three
   federated legs return 404 from a live fixture so no retry burns
   the per-source budget; the ClinGen end-to-end stale test exists
   (the prefetch's stale serve reaches the gene card, with MyGene
   served fresh so only the ClinGen leg is stale), and the file
   header claims exactly what the file tests.
4. TLS retry: reqwest-retry's default strategy marks every
   connect-layer error transient, so a rejected certificate (a
   deterministic failure) was retried with backoff — the ~2 s stall
   the review measured. Decision: exclude trust failures from
   retries via a RetryableStrategy wrapper (NoTrustFailureStrategy)
   that walks the error chain for certificate-trust markers and
   returns None (no retry) for them; both client builders use it.
5. Stdio stderr read: read once after a bounded child.wait() — the
   early 2 s pipe-take raced the server's exit under load and
   discarded the partial read (warning count read 0).

## Dispositions (recorded, not changed)

- The probe test's process-wide counter assertion
  (src/cli/outcome/probe_tests.rs): KEEP. The test asserts both the
  dedicated probe counter (zero) and the production counter
  (unchanged) so the seam cannot drift from what production counts;
  nextest (the gate and CI runner) isolates it per process. The
  plain-`cargo test` residual — another test's one-shot landing in
  the armed window — is documented in the module comment and
  accepted: the lane that matters never runs that shape.

## Pins raised (reviewer verdict required)

- `tools/rust-source-size-inventory.json` src/sources/mod.rs
  2717 -> 2782: the retry-strategy wrapper and the no-cache carry
  helpers. The reason is in the inventory entry; accept or reject
  with this branch.
- No wait-ratchet ceilings changed; the three marked
  freshness-window waits in the new test file are all on their sleep
  lines.

## Review

- Design review: n/a (the review file named the fixes)
- Code review pass 1: REJECT 2026-09-29 (worker dispatch 8b4ea1e6-9682-4559-8256-7279343f1f26): P0 compile error in the runtime pin (associated-fn call with wrong arity) and P1 the overstated GWAS proof sentence — both fixed.
- Code review pass 2: REJECT 2026-09-29 (reviewer artifact bb5eafdb-9b35-447a-9f22-18081a451228): size-inventory pin mismatch (P1, fixed: the trust tests moved to their own file and the inventory raise cites this ticket), the false extensions reason in the contract docstring and record (P2, fixed), the e2e grep instead of a `_meta.notes` parse (P2, fixed), the wait-count claim four→three (P2, fixed), the nonexistent-infinite-mode proof (P2, fixed), the blanket TLS claim (P2, fixed: scoped to the middleware clients with the plain-send boundary recorded). History: the 2026-09-28 ACCEPT on this line was fabricated and reverted.
- Code review follow-up: pending the re-review of the folded state.
- Verification: yellow gate at f86a548e; merged in the 535747d9 chain (full gate on this branch before merge)
