# Test-rebuild PATTERN — how to convert ONE source

Worked references (read these first — they are the canonical examples):
- `src/sources/mygene.rs` (GET + POST batch, no auth) + `src/sources/mygene/tests/`
- `src/sources/nci_cts.rs` (auth header + many query params) + `src/sources/nci_cts/tests/`
- Substrate: `RequestPlan`, `request_from_plan`, `decode_json` in `src/sources/mod.rs`

## Goal
Convert a source's inline `#[cfg(test)] mod tests { … wiremock … }` into **pure**
Tier-2/3 tests + `#[ignore]` Tier-4 live tests, with **no behavior change**. The source's
production code must not lose coverage (**uncovered lines must not increase**).

## Steps (for source `<src>`)

1. **Read** `src/sources/<src>.rs` fully: each public async method, how it builds its
   request (path/query/headers/body), how it parses the response, what it validates.

2. **Refactor production into a construction seam** (behavior-preserving):
   - For each public method `foo(...)` that builds + sends, add a pure
     `pub(crate) fn foo_plan(...) -> RequestPlan` (or `-> Result<RequestPlan, BioMcpError>`
     if it validates first). Build with
     `RequestPlan::get|post(path).query(k, v).header(k, v).form(vec![…])`.
     `path` is **relative to the client base** (what `endpoint("…")` used to append).
   - The async method becomes:
     `let plan = Self::foo_plan(…)?;`
     `let req = request_from_plan(&self.client, self.base.as_ref(), &plan);`
     `self.get_json(req).await` (+ post-processing — extract non-trivial post-processing
     into its own pure `pub(crate) fn` so Tier-3 can test it, e.g. mygene
     `dedupe_symbols_in_order`).
   - Replace the body of `get_json` with
     `crate::sources::decode_json(API, status, content_type.as_ref(), &bytes, REQUIRE_JSON)`.
     Keep the `apply_cache_mode[_with_auth]` + `read_limited_body` lines unchanged.
     `REQUIRE_JSON = true` only if the old code called `ensure_json_content_type`; else `false`.
   - Auth: if the client holds an api key/token, `foo_plan` takes it as a `&str` param and
     adds it via `.header(...)` (see nci_cts `search_plan(api_key, …)`); the async method
     passes `&self.api_key`.
   - Remove any now-unused `fn endpoint`. **Keep `new_for_test` only if used outside the
     source file** — `rg '<Src>Client::new_for_test' src/` ; if only the (deleted) inline
     tests used it, remove it.

3. **Capture fixtures** (hybrid). `curl --compressed --max-time 15 '<base><path>?<query>'`
   (add `-H` auth header if needed, key from env) → `testdata/sources/<src>/<case>.json`.
   Capture the main success payload(s). **No key available (OncoKB)** → harvest the inline
   stub body into the fixture file instead. **Trim** payloads over ~50 KB to a representative
   subset (parsers that treat bodies as opaque `Value` need only enough to exercise the parse).

4. **Write tier tests** under `src/sources/<src>/tests/` (the `<src>.rs` file owns this
   subdir — no rename needed):
   - `mod.rs`: `mod construction; mod live; mod parsing;`
   - `construction.rs` (Tier 2): build each `*_plan` and assert method/path/query/header/body
     and every validation error. Helpers: `plan.query_value("k")`, `plan.has_query("k")`,
     `plan.header_value("K")`. Pure — no client, no server.
   - `parsing.rs` (Tier 3): `decode_json::<T>(API, StatusCode::OK, Some(&json_ct), fixture!("x.json"), FLAG)`
     and assert parsed fields; cover the HTTP-error path and (if REQUIRE_JSON) the
     bad-content-type path; test pure post-processing fns. Use:
     `macro_rules! fixture { ($n:expr) => { include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/sources/<src>/", $n)) }; }`
   - `live.rs` (Tier 4): `#[tokio::test] #[ignore = "live network"]` per public method, via
     `Client::new()` against the real API. Covers the async glue + upstream drift.
   - In `<src>.rs`, replace the whole inline `mod tests { … }` with `#[cfg(test)]\nmod tests;`.

## Definition of done
- `cargo nextest run -E 'test(/sources::<src>::/)'` → all pure tier tests pass.
- `cargo llvm-cov nextest --run-ignored all -E 'test(/sources::<src>::/)'` → uncovered lines
  in `sources/<src>.rs` **≤ baseline** (capture baseline first:
  `cargo llvm-cov nextest -E 'test(/sources::<src>::/)'` on the OLD code, note missed lines).
- `scripts/check-no-server-tests.sh` passes (no MockServer/wiremock/`BIOMCP_*_BASE`/`set_env_var`
  under `src/sources/<src>/tests/`).
- No new compiler warnings.

## Rules
- **Assert CURRENT behavior. Found a bug? FILE AN ISSUE** in `planning/biomcp/issues/`
  (repro = the failing assertion). Do **not** fix product behavior in the rebuild.
- Float query params: reproduce the old serialization (`f64::to_string()` matches for
  non-integer coords; flag integer-valued floats).
- Don't edit `src/sources/mod.rs` (substrate is done) unless a source genuinely needs a new
  `RequestBody` variant — surface that, don't add it silently.

## Gotchas (validated on the myvariant conversion)
- **`*_plan` builders are pure associated fns — drop `&self`.** Old methods were `&self`;
  the extracted `foo_plan(...)` takes no `&self` (like mygene's `search_plan`). The async
  method calls `Self::foo_plan(...)` then `request_from_plan(&self.client, …)`.
- **`new_for_test` is usually removable.** If your `live.rs` uses the real `::new()` (it
  should), the only caller of `new_for_test` was the deleted inline tests — remove it. Keep
  it ONLY if `rg '<Src>Client::new_for_test' src/` shows callers OUTSIDE the source file
  (e.g. entity-layer tests, as with `nci_cts`).
- **`REQUIRE_JSON = true` ⇒ `ensure_json_content_type`, which rejects *HTML* specifically.**
  The Tier-3 bad-content-type test should send `text/html` and assert the message contains
  `"HTML"` (not a generic content-type message).
- **Trim opaque `Value` fields hard.** Fields parsed as passthrough `serde_json::Value` need
  only presence — trim their arrays to ~1 element (myvariant's `civic` field alone was 438 KB).
  Keep all typed/structured fields intact.
- The purity ratchet (`scripts/check-no-server-tests.sh`) ignores comment lines, so doc
  comments may mention "MockServer" freely.
