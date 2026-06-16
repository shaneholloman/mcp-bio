# BioMCP Test-Ecosystem Rebuild — Plan & Checklist

Status: **planning / foundations**  ·  Worktree: `worktrees/biomcp-test-rebuild`
(branch `test-ecosystem-rebuild`, off `main@43d6fb4e`)  ·  Author: Ian + agent, 2026-06-16

## 0. TL;DR

A BioMCP build/SDLC step spends **~90% of its wall-clock re-running the test/spec
gates**, not on the model. `make test` is ~15 min because ~500 tests serialize on a
global env-var mutex and ~400 spin up in-process mock HTTP servers. We are going to
**stand up a brand-new, parallel test ecosystem** in this worktree — pure,
HTTP-free, lock-free Tier 1–3 tests organized in a new folder structure — **prove it
matches old coverage via `cargo-llvm-cov`, then delete the old tests.** Agents also
**hunt for and file bugs** as they decompose. Goal: `make test` from ~15 min → a
couple of minutes, fully parallel, which turns 4–6 hour tickets into well-under-an-hour.

The build-cache root cause (every build recompiled the whole crate) is already fixed
and merged (`3dc693dc`); proven on ticket 426 (~18% faster end-to-end, light steps
−36/−41%). This project attacks the remaining ~90%: **test runtime.**

---

## 1. Findings — where the time actually goes

Parsed the real Pi session transcripts (per-message timestamps), two steps:

| | 420 implement (84 min) | 426 03-code (117 min) |
|---|---|---|
| **Tool execution** | **73 min (88%)** | **108 min (92%)** |
| Model (gpt-5.5) | 10 min (12%) | 9 min (8%) |
| `make test` | 55 min (~4 runs) | 30 min |
| `make spec` | 13 min | 56 min |

- The model is **not** the bottleneck. ~90% of a step is re-running `make test`
  (~15 min) + `make spec` (~8–13 min), 3–5× per step after each edit.
- `make test` ≈ `cargo nextest` over 2079 unit tests. **0 use `#[serial]`, but ~516
  acquire a single global env mutex (`env_lock()` in `src/test_support.rs`)** because
  source clients read their base URL from a **process-global env var**
  (`BIOMCP_*_BASE`). Those ~516 run one-at-a-time (~13 min serial). ~408 tests also
  start an in-process `wiremock` server.
- `make spec` is dominated by `spec/entity/article.md` (~6 min): it restarts a
  fixture HTTP server 15× and exercises real retry/timeout waits.
- The issues folder corroborates the chronic pattern: converted issues
  `331-wikipathways-nextest-parallel-flake`, `351-gene-all-warm-budget-xdist-outlier`,
  `389-pathway-filter-test-flake`, `328-disease-nih-funding-context-flake`, plus open
  `415-slow-ctgov-alias-fanout-tests`, `420-mcp-spec-fixed-port-flake`. All symptoms
  of HTTP-server-spinning, env-locked, port-bound tests.

---

## 2. Target test taxonomy (what we SHOULD have)

| Tier | Tests | HTTP? | Lives in | Speed |
|---|---|---|---|---|
| **1. CLI parse/route** | args → parsed command + request struct (routing, defaults, validation, exit codes) | none | in-crate `src/cli/**` | instant, parallel |
| **2. Request construction** | request/config → the **exact outbound HTTP** (method, full URL, query params, headers, body) — *built and asserted, never sent* | none | in-crate `src/sources/<src>/tests/construction.rs` | instant, parallel |
| **3. Response parse/render** | a **committed fixture** of response bytes → parsed entities + rendered markdown/JSON | none | in-crate `src/sources/<src>/tests/parsing.rs` + `src/entities/**` | instant, parallel |
| **4. End-to-end (a few)** | hit the real source **once** → store the Tier-3 fixture → assert the round-trip; catches upstream drift | real, sparingly | mustmatch `spec/` **live lane (`make verify`)**, ~1 per source | slow — NOT in the routine gate |

**Principle:** only Tier 4 touches a network, and only a handful per source. Tiers 1–3
are pure functions — no mock server, no env var, no lock — so nextest parallelizes all
of them. This matches the team's own 2026-06-04 hard-rule (request-contract + renderer
unit tests; never fake a remote to green a gate) — started in tickets 376–379, never
finished.

---

## 3. Substrate — config injection (kills the env lock)

Root enabler. Today: `std::env::var("BIOMCP_PUBMED_BASE")` inside the client →
tests must mutate global env under a lock.

Target: each source client takes its config (base URL, keys, timeouts) **as a value**:
- A `SourceConfig`/`SourceEndpoints` resolved **once at process start** from env
  (single composition root), then passed down.
- Production behavior unchanged (same env vars at startup).
- Tier 2 tests construct the request from config and **assert the request object**
  (no send, no base URL needed beyond a literal). Tier 3 tests call the parser on
  committed bytes (no client at all).
- Result: **no test mutates global env → the `env_lock()` mutex is deleted → full
  parallelism.**

This is the first thing built and proven on one source before fan-out.

---

## 4. New test folder structure

```
src/sources/<source>/
    mod.rs                     # client (now config-injectable)
    tests/
        mod.rs                 # wires the submodules
        construction.rs        # Tier 2 — build request, assert URL/params/headers/body
        parsing.rs             # Tier 3 — committed-fixture bytes -> parsed/rendered
testdata/                      # NEW committed response fixtures (Tier 3 inputs)
    sources/<source>/
        <case>.json|.xml|...   # captured-once real payloads (named by scenario)
src/cli/<command>/tests/
    routing.rs                 # Tier 1 — args -> command/request struct
spec/entity|surface/*.md       # Tier 4 — ONE real round-trip per source (make verify)
```

Conventions: fixtures are captured from a real Tier-4 run and committed; every Tier-3
test names the fixture it consumes; no test under `src/**/tests/` may start a server
or touch env (enforced by ratchet, §10).

---

## 5. Coverage as the cutover safety net  ✅ tooling already installed

`cargo-llvm-cov 0.6.17` + `cargo-nextest 0.9.132` + `llvm-tools` are present — works today.

- **`make coverage`** → `cargo llvm-cov nextest --lcov --output-path coverage/lcov.info`
  plus `--html` for a browsable report; `--summary-only` for the gate number.
- **Baseline:** run coverage on the OLD suite (current `main`) → record overall +
  per-module line/region %. This is the bar.
- **Parity gate (the safety net):** a source's old tests may be deleted **only** when
  the new Tier 1–3 tests for it hit **≥ the baseline coverage for that module**. So we
  never lose coverage in the cutover.
- **Floor ratchet:** add a `make coverage-check` that fails if overall coverage drops
  below the recorded floor; wire into the gate so it can't regress.
- Coverage also surfaces dead/untested code → feeds the issue-hunt (§7).

---

## 6. Harvest vs rebuild

- **Harvest:** the assertions and the real payloads. Each old wiremock test already
  contains (a) the expected request shape and (b) a canned response body — lift the
  response body into a committed `testdata/` fixture (Tier 3) and the request
  expectations into a Tier-2 construction assertion. Harvest fixtures from Tier-4 runs
  where a real payload is better than the old hand-written mock.
- **Rebuild, don't port:** do not mechanically translate wiremock→wiremock. Re-express
  each behavior as the right Tier (1/2/3). Most "round-trip" wiremock tests collapse to
  one Tier-2 + one Tier-3 test.
- **Then wipe:** once a source's new tests meet the coverage bar, delete its old
  test module(s) and any now-unused mock scaffolding.

---

## 7. Issue discovery & creation — a first-class workstream

Decomposing every source is a forced read of every request/response path — the best
bug-hunt we'll ever get. Agents are **instructed to file issues**, not silently fix:

- **Find:** while writing Tier 2/3 tests, flag anything wrong — incorrect URL/params,
  dropped headers/auth, mis-parsed fields, silent error-swallowing, missing
  pagination/retry bounds, coverage holes on error paths.
- **File:** one issue per finding in `planning/biomcp/issues/` with the standard
  frontmatter (`severity`, `status: open`, `type: bug`), a concrete repro (the failing
  Tier-2/3 assertion), and the source/file. **Do not fix product behavior inside the
  test-rebuild** — tests assert *current* behavior; bugs become issues → tickets.
- **Roll up:** a triage pass merges duplicates and converts real bugs to March tickets.

Existing issues folder triaged 2026-06-16 (see `planning/biomcp/issues/` and the
triage index): 47 converted + 5 closed archived; 5 open — `415-slow-ctgov-alias-fanout`,
`420-mcp-spec-fixed-port-flake`, `416-ctgov-trial-helper-live-latency` fold into THIS
project; `413-live-verify-cpic-nih-red` (live lane) and `419-incomplete-checklist`
(process) kept separate.

---

## 8. Execution plan (phases + sub-agent fan-out)

**Phase 0 — Foundations (do first, single-threaded, in this worktree).**
Coverage baseline, `testdata/` + tier conventions, the config-injection substrate, the
no-server/no-env ratchet, and a **full pilot conversion of ONE source (pubmed — 15
tests, highest count)** to prove coverage parity + the parallelism/time win.

**Phase 1 — Source fan-out (parallel sub-agents).** One agent per source (worktree or
branch isolation), following the pilot pattern: harvest → Tier 2 + Tier 3 → coverage
parity → delete old → file issues found. ~40 sources; batch by mock-count.

**Phase 2 — CLI & entity layers.** Tier 1 routing tests; decompose the ~516 env-locked
CLI/entity tests; renderer Tier-3 tests for entity output.

**Phase 3 — Cutover & cleanup.** Delete residual old tests, remove `env_lock()` +
base-URL env plumbing from tests, collapse `make test` to the fast pure suite, move
heavy/e2e to `make verify`, re-measure.

**Phase 4 — Issue roll-up.** Triage all agent-filed issues, dedupe, convert bugs → tickets.

Orchestration: a coordinating agent (this session) drives per-source sub-agents,
gates each on coverage parity, and aggregates issues. Sources are independent → high
parallelism, low conflict.

---

## 9. COMPREHENSIVE CHECKLIST

### Phase 0 — Foundations
- [ ] `make coverage` target (llvm-cov nextest → lcov + html + summary).
- [ ] Record **baseline coverage** (overall + per-module) of current `main`; commit `coverage/BASELINE.md`.
- [ ] `testdata/` fixture tree + naming convention documented.
- [ ] Tier test-module convention (`tests/construction.rs`, `tests/parsing.rs`, `cli/**/tests/routing.rs`).
- [ ] **Config-injection substrate:** source clients take base URL/keys/timeouts as a value; env resolved once at startup.
- [ ] **Ratchet:** routine test may not start a `MockServer` or read a `BIOMCP_*_BASE` env var (lint/clippy/grep gate).
- [ ] **Pilot:** convert `pubmed` end-to-end (Tier 2 + Tier 3 + 1 verify round-trip); prove coverage ≥ baseline; measure test time + parallelism; delete old pubmed tests.
- [ ] Write the pilot up as the canonical pattern doc for fan-out agents.

### Phase 1 — Source fan-out (per source: harvest → Tier2 → Tier3 → parity → wipe → file issues)
- [ ] pubmed (15)  · [ ] opentargets (13)  · [ ] semantic_scholar (10)  · [ ] disgenet (10)
- [ ] pubtator (7) · [ ] mygene (7) · [ ] wikipathways (6) · [ ] mutalyzer (6) · [ ] figshare (6) · [ ] cbioportal_download (6)
- [ ] mydisease (5) · [ ] europepmc (5) · [ ] clingen (5)
- [ ] variantvalidator (4) · [ ] pmc_oa (4) · [ ] openfda (4) · [ ] oncokb (4) · [ ] nci_cts (4) · [ ] myvariant (4) · [ ] monarch (4) · [ ] gwas (4) · [ ] gtex (4) · [ ] gprofiler (4) · [ ] gnomad (4) · [ ] clinicaltrials (4)
- [ ] seer (3) · [ ] medlineplus (3) · [ ] litsense2 (3) · [ ] hpo (3) · [ ] enrichr (3)
- [ ] remaining `BIOMCP_*_BASE` sources with <3 mocks (cancerhotspots, chembl, civic, cpic, dgidb, ema, gnomad, hpa, interpro, kegg, ncbi_idconv, nih_reporter, ols4, mychem, …) — sweep
- [ ] `make spec` article.md fixture: reuse one server + reset state per section (kills the 15× restart)

### Phase 2 — CLI & entity layers
- [ ] Tier 1 routing tests for each CLI command (args → request struct), pure.
- [ ] Decompose the ~516 env-locked CLI/entity tests into Tier 1/2/3.
- [ ] Entity renderer Tier-3 tests (markdown table / JSON envelope) from committed fixtures.

### Phase 3 — Cutover & cleanup
- [ ] Delete residual old wiremock/env-locked tests once parity proven everywhere.
- [ ] Remove `env_lock()` / `src/test_support.rs` env mutex and base-URL env reads from tests.
- [ ] `make test` = fast pure suite only; heavy/e2e moved to `make verify`.
- [ ] Drop `mkdocs --strict` from the inner-loop `make test` (move to verify/publish).
- [ ] Re-measure `make test` (target: ≤ ~2–3 min, fully parallel) and record before/after.
- [ ] Update `make coverage-check` floor.

### Phase 4 — Issues
- [x] Triage existing `issues/` (2026-06-16): archive converted/closed, keep 5 open, fold test-infra ones into this project.
- [ ] Agents file new issues during decomposition (one per bug, repro = failing assertion).
- [ ] Roll-up triage: dedupe, merge, convert real bugs → March tickets.

---

## 10. Guardrails & ratchets
- Tests assert **current** behavior; bugs found → **issues**, never silent product changes inside the rebuild.
- New ratchet: no routine test starts a server or reads a base-URL env var.
- Coverage parity gate before any old-test deletion; coverage floor in the gate after.
- Keep the live `make verify` lane (Tier 4) — do not delete real round-trips, just thin to ~1 per source.
- Don't touch `[profile.release]`, gpt-5.5 model policy, or the validation-profile→step mapping.

## 11. Done criteria
- `make test` runs in minutes, fully parallel; zero `env_lock()` / routine `MockServer`.
- Coverage ≥ baseline (and floored in the gate).
- Old test modules deleted; new tier structure in place across all sources + CLI + entities.
- Agent-found bugs filed as issues and triaged.
- A build ticket's step time is dominated by model turns again, not gate re-runs.

## Appendix — source inventory (config-injection + decomposition targets)
~40 source clients read `BIOMCP_*_BASE`: alphagenome, cancerhotspots, cbioportal(+datahub),
chembl, civic, clingen, complexportal, cpic, ctgov, dgidb, disgenet, ema, enrichr,
europepmc, figshare, gnomad, gprofiler, gtex, gwas, hpa, hpo, interpro, kegg, litsense2,
medlineplus, monarch, mutalyzer, mychem, mydisease, mygene, myvariant, ncbi_idconv,
nci_cts, nih_reporter, ols4, oncokb, openfda, opentargets, pubmed, pubtator,
semantic_scholar, seer, variantvalidator, … (full list: `grep -rhoE 'BIOMCP_[A-Z0-9_]*BASE' src/sources`).
