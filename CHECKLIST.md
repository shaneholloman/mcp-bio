# BioMCP test rebuild — checklist

**Goal (plain):** replace the slow/hanging tests with fast unit tests. For every API
endpoint, two unit tests with **no network** — one that checks we **build the call** right,
one that checks we **parse the result** right. Plus CLI tests, util tests, and a few real
**smoke tests**. We do **not** test the network call itself.

**Where to work:** the worktree `worktrees/biomcp-test-rebuild`, branch
`test-ecosystem-rebuild`. Edit ONLY here — never `repos/biomcp`.

**Copy these — they're done and are the template:** `src/sources/mygene.rs` +
`src/sources/mygene/tests/`, plus `nci_cts` and `myvariant`. The step-by-step recipe with
examples is `PATTERN.md` (read it once).

---

## Before you start
- The worktree is ready at commit `d47d6ce4`. If `main` has advanced since, update this
  branch onto it first.
- **biomcp March is paused** (`march worker pause biomcp`). Leave it paused; resume with
  `march worker resume biomcp` only when Ian says the machine is stable. Don't spawn
  background agents — work this list yourself, in the foreground.
- 3 endpoints are already done (see Inventory). Start with the BioThings pair
  (`mychem`, `mydisease`) — they're near-copies of `mygene`/`myvariant`.

## The system you're testing
CLI parses the args → picks a service call with params → the service **builds an HTTP
request, sends it, gets a response, turns it into JSON or markdown**. ~40 endpoints, plus
shared utils. Today each endpoint glues build + send + parse into one function — so your one
real task per endpoint is to **split build and parse out into their own callable functions**
so they can be tested on their own.

## What to write for each endpoint (the repeating unit of work)
1. **Build test** — given the inputs, assert the request we'd send: method, path, query
   params, headers, body. Nothing is sent.
2. **Parse test** — given a saved real response (a fixture file), assert we get the right
   entity and the right JSON/markdown. Nothing is sent.

How (copy the three examples):
- Split each public method so a pure `*_plan()` function builds the request as data (use the
  `RequestPlan` helper in `src/sources/mod.rs`), and the response decode is a callable
  function on the response bytes (use the `decode_json` helper). The async method then just
  does: build plan → send → decode.
- Put the tests in `src/sources/<endpoint>/tests/construction.rs` (build) and `parsing.rs`
  (parse). The source `.rs` file keeps its own subdir — do NOT rename it.
- Grab one real response with `curl --compressed` into `testdata/sources/<endpoint>/`. Trim
  huge payloads (pass-through `serde_json::Value` fields only need ~1 element).

## Safety rule (do NOT skip — this is how we prove we didn't break anything)
1. Refactor the production code (split build/parse). Keep the public methods the same.
2. **Run the endpoint's OLD tests — they must still pass.** That's the proof the refactor
   didn't change behavior.
3. Add the new build + parse tests; confirm they pass.
4. Old pass + new pass → **then delete the old tests** for that endpoint (and any now-unused
   `new_for_test` / mock-server scaffolding, unless something outside the file still uses it).
5. The **downstream tests** (the entity-layer and CLI tests that call this client) must stay
   green the whole way through — they're the real "didn't break anything" check. Keep them.

## Go fast (minimize compile + test time)
- **Never run the whole suite** (`make test` or unfiltered `cargo nextest`) — it hangs 15+
  min on the old leaky tests. Always scope: `cargo nextest run -E 'test(/sources::<endpoint>::/)'`.
- **Batch:** edit 3-5 endpoints, then compile/test once. One compile covers all of them (the
  crate is a single compile unit), so batching is the main way to save time.
- `cargo fmt` before every commit (the pre-commit hook runs `cargo fmt --check` + `cargo
  check` and rejects the commit otherwise). The hook does NOT run tests, so it's quick.
- Don't measure coverage per endpoint (slow). The build + parse tests cover the logic; if you
  want a coverage number, take it once over a batch at the end.
- Quick purity check (optional): `bash scripts/check-no-server-tests.sh` fails if a normal
  test starts a mock server or reads a base-URL env var.

---

## Inventory (check off as you go)

### DONE
- [x] mygene · [x] nci_cts · [x] myvariant · [x] mychem · [x] mydisease

### Endpoints TODO — build test + parse test each (`~N` = old test count, a size hint)
`ls src/sources/*.rs` is the source of truth if anything here is stale. Do families
together — siblings share structure, so they go fast.

BioThings (near-copies of mygene/myvariant):
- [x] mychem ~8 · [x] mydisease ~14

NCBI / literature:
- [x] pubmed ~18 · [x] pubtator ~8 · [x] ncbi_efetch ~2 · [x] ncbi_idconv ~3 · [x] pmc_oa ~6
- [x] europepmc ~8 · [x] semantic_scholar ~13 · [x] litsense2 ~4 · [x] nih_reporter ~7

Trials / cancer:
- [x] clinicaltrials ~6 · [x] cbioportal ~2 · [x] cbioportal_download ~9 · [x] cbioportal_study ~35
- [x] cancerhotspots ~5 · [x] oncokb ~4 (no API token available → reuse the existing canned response as the fixture) · [x] seer ~4

Variants / genomics:
- [x] gnomad ~4 · [x] gtex ~4 · [x] gwas ~5 · [x] variantvalidator ~6 · [x] mutalyzer ~6
- [x] clingen ~5 · [x] civic ~3 · [x] gtr ~12

Drugs / chem / regulatory:
- [x] chembl ~3 · [x] dgidb ~3 · [x] ddinter ~5 · [x] openfda ~9 · [x] ema ~11 · [x] pharmgkb ~2
- [x] cpic ~3 · [x] cvx ~11 · [x] vaers ~10 · [x] who_pq ~20 · [x] who_ivd ~6

Ontologies / proteins / pathways / misc:
- [x] uniprot ~10 · [x] interpro ~2 · [x] hpa ~4 · [x] hpo ~4 · [x] monarch ~4 · [x] ols4 ~3
- [x] umls ~1 · [x] reactome ~3 · [x] wikipathways ~8 · [x] kegg ~6 · [x] gprofiler ~7
- [x] enrichr ~4 · [x] quickgo ~3 · [x] complexportal ~3 · [x] string ~3 · [x] disgenet ~10
- [x] alphagenome ~4 · [x] medlineplus ~5 · [x] figshare ~13

(~57 endpoints. Auth keys are present in env for nci_cts/umls/alphagenome/disgenet/s2;
OncoKB has none — harvest its existing stub instead of curling.)

### CLI points (args → right service call + params)
- [ ] One pure test set per CLI command under `src/cli/**` (gene, variant, article, trial,
      drug, disease, protein, pathway, pgx, adverse_event, …). Many already have a `tests.rs`
      — make them pure (parse args → assert request/route) where they aren't.

### Entity processing + output (response → entity → JSON/markdown)
- [ ] `src/transform/**` and `src/entities/**` — test the pure processing with saved inputs.
- [ ] `src/render/**` — test markdown/JSON output from saved entities.
- [ ] **Worst offenders, fix these:** `src/entities/article/backends/tests.rs` — they hang
      15+ min because they hit the real network (they only mock some of their clients).
      Rework them to test the pieces without real calls. This is the single biggest speed win.

### Utils
- [ ] `src/utils/*.rs` (date, download, query, serde) — direct unit tests.
- [ ] The shared helpers in `src/sources/mod.rs` (`RequestPlan`, `decode_json`) — a few tests.

### Smoke tests (a few, real network — the ONLY network tests)
- [ ] gene → gene info · [ ] variant → variant info · [ ] article → article
- [ ] (optional) trial, drug
Keep these `#[ignore]` so they stay out of the normal gate; run them in the verify lane.

### Final — prove we didn't break anything
- [ ] Once everything's converted and the old leaky tests are gone, run the full gate
      (`make test` — now fast, no hangs) and confirm green.
- [ ] Then delete the leftover old machinery (the global env-lock mutex, the mock-server
      scaffolding) and confirm `make lint` / `make test` / `make spec` are all green.

---

## Pointers
- `PATTERN.md` — the recipe + worked examples + gotchas.
- `src/sources/{mygene,nci_cts,myvariant}.rs` + their `tests/` — copy these.
- `coverage/BASELINE.md` — what "kept coverage" looked like for the done ones.
- `TEST-REBUILD.md` — original detailed write-up (background only; skip if you just want to work).

## Batch log
- 2026-06-16: `mychem` + `mydisease` converted. Checks:
  `cargo nextest run -E 'test(/sources::mychem::/) | test(/sources::mydisease::/)'` → 27/27 pass;
  `bash scripts/check-no-server-tests.sh` → pass;
  `cargo nextest run -E 'test(/entities::disease/) | test(/entities::trial::search::nci/)'` → 84/84 pass.
- 2026-06-16: `ncbi_idconv` converted. Checks:
  `cargo nextest run -E 'test(/sources::ncbi_idconv::/)'` → 10/10 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `ncbi_efetch` converted. Checks:
  `cargo nextest run -E 'test(/sources::ncbi_efetch::/)'` → 9/9 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `pmc_oa` converted. Checks:
  `cargo nextest run -E 'test(/sources::pmc_oa::/)'` → 11/11 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `litsense2` converted. Checks:
  `cargo nextest run -E 'test(/sources::litsense2::/)'` → 10/10 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `nih_reporter` converted. Checks:
  `cargo nextest run -E 'test(/sources::nih_reporter::/)'` → 10/10 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `europepmc` converted. Checks:
  `cargo nextest run -E 'test(/sources::europepmc::/)'` → 12/12 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `pubtator` converted. Checks:
  `cargo nextest run -E 'test(/sources::pubtator::/)'` → 11/11 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `semantic_scholar` converted. Checks:
  `cargo nextest run -E 'test(/sources::semantic_scholar::/)'` → 15/15 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `pubmed` converted. Checks:
  `cargo nextest run -E 'test(/sources::pubmed::/)'` → 11/11 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: NCBI/literature group check. Checks:
  `cargo nextest run -E '<pubmed|pubtator|europepmc|semantic_scholar|litsense2|nih_reporter|pmc_oa|ncbi_efetch|ncbi_idconv source filters>'`
  → 99/99 pass.
- 2026-06-16: `cbioportal` converted. Checks:
  `cargo nextest run -E 'test(/sources::cbioportal::/)'` → 7/7 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `clinicaltrials` converted. Checks:
  `cargo nextest run -E 'test(/sources::clinicaltrials::/)'` → 8/8 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `cbioportal_download` converted. Checks:
  `cargo nextest run -E 'test(/sources::cbioportal_download::/)'` → 13/13 pass.
- 2026-06-16: `cbioportal_study` reviewed and kept as-is because it is already a
  pure local file parser/statistics test set, not an HTTP source. Checks:
  `cargo nextest run -E 'test(/sources::cbioportal_study::/)'` → 35/35 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `cancerhotspots` converted. Checks:
  `cargo nextest run -E 'test(/sources::cancerhotspots::/)'` → 8/8 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `oncokb` converted using a committed canned annotation fixture; no
  API token or network needed. Checks:
  `cargo nextest run -E 'test(/sources::oncokb::/)'` → 7/7 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `seer` converted. Checks:
  `cargo nextest run -E 'test(/sources::seer::/)'` → 7/7 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `gnomad` converted. Checks:
  `cargo nextest run -E 'test(/sources::gnomad::/)'` → 7/7 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `gtex` converted. Checks:
  `cargo nextest run -E 'test(/sources::gtex::/)'` → 8/8 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `gwas` converted. Checks:
  `cargo nextest run -E 'test(/sources::gwas::/)'` → 10/10 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `variantvalidator` converted. Checks:
  `cargo nextest run -E 'test(/sources::variantvalidator::/)'` → 7/7 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `mutalyzer` converted. Checks:
  `cargo nextest run -E 'test(/sources::mutalyzer::/)'` → 7/7 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `clingen` converted. Checks:
  `cargo nextest run -E 'test(/sources::clingen::/)'` → 9/9 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `civic` converted. Checks:
  `cargo nextest run -E 'test(/sources::civic::/)'` → 6/6 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `gtr` converted to standard source test layout and kept pure local
  file/parser coverage. Checks:
  `cargo nextest run -E 'test(/sources::gtr::/)'` → 12/12 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `chembl` converted. Checks:
  `cargo nextest run -E 'test(/sources::chembl::/)'` → 6/6 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `dgidb` converted. Checks:
  `cargo nextest run -E 'test(/sources::dgidb::/)'` → 5/5 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `ddinter` converted to standard source test layout and kept pure
  local-data coverage. Checks:
  `cargo nextest run -E 'test(/sources::ddinter::/)'` → 7/7 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `openfda` converted. Checks:
  `cargo nextest run -E 'test(/sources::openfda::/)'` → 12/12 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `ema` converted to standard source test layout and kept pure
  local-feed coverage. Checks:
  `cargo nextest run -E 'test(/sources::ema::/)'` → 10/10 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `pharmgkb` converted. Checks:
  `cargo nextest run -E 'test(/sources::pharmgkb::/)'` → 6/6 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `cpic` converted. Checks:
  `cargo nextest run -E 'test(/sources::cpic::/)'` → 7/7 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `cvx` converted to standard source test layout and removed the
  source-level env mutation test. Checks:
  `cargo nextest run -E 'test(/sources::cvx::/)'` → 11/11 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `vaers` converted. Checks:
  `cargo nextest run -E 'test(/sources::vaers::/)'` → 10/10 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `who_pq` converted to standard source test layout and kept pure
  local CSV/feed coverage. Checks:
  `cargo nextest run -E 'test(/sources::who_pq::/)'` → 16/16 pass;
  `bash scripts/check-no-server-tests.sh` → pass.
- 2026-06-16: `who_ivd` converted to standard source test layout and removed the
  source-level env mutation test. Checks:
  `cargo nextest run -E 'test(/sources::who_ivd::/)'` → 7/7 pass;
  `bash scripts/check-no-server-tests.sh` → pass; `cargo check` → pass.
- 2026-06-16: `uniprot` converted to pure request construction and response
  parsing tests, replacing the source-level mock server test. Checks:
  `cargo nextest run -E 'test(/sources::uniprot::/)'` → 17/17 pass;
  `bash scripts/check-no-server-tests.sh` → pass; `cargo check` → pass.
- 2026-06-16: `interpro` converted to pure request construction and response
  parsing tests, replacing source-level mock server tests. Checks:
  `cargo nextest run -E 'test(/sources::interpro::/)'` → 3/3 pass;
  `bash scripts/check-no-server-tests.sh` → pass; `cargo check` → pass.
- 2026-06-16: `hpa` converted to pure request construction and XML response
  parsing tests, replacing source-level mock server tests. Checks:
  `cargo nextest run -E 'test(/sources::hpa::/)'` → 6/6 pass;
  `bash scripts/check-no-server-tests.sh` → pass; `cargo check` → pass.
- 2026-06-16: `hpo` converted to pure request construction and JSON response
  parsing tests, replacing source-level mock server tests. Checks:
  `cargo nextest run -E 'test(/sources::hpo::/)'` → 8/8 pass;
  `bash scripts/check-no-server-tests.sh` → pass; `cargo check` → pass.
- 2026-06-16: `monarch` converted to pure request construction and response
  mapper tests for associations and phenotype similarity, replacing source-level
  mock server tests. Checks:
  `cargo nextest run -E 'test(/sources::monarch::/)'` → 10/10 pass;
  `bash scripts/check-no-server-tests.sh` → pass; `cargo check` → pass.
- 2026-06-16: `ols4` moved to standard source test layout with pure request-plan
  and search-response parsing tests, replacing the source-level mock server test.
  Checks: `cargo nextest run -E 'test(/sources::ols4::/)'` → 4/4 pass;
  `bash scripts/check-no-server-tests.sh` → pass; `cargo check` → pass.
- 2026-06-16: `umls` converted to pure authenticated search/atoms request
  construction and JSON parsing tests, replacing the source-level mock server
  test. Checks: `cargo nextest run -E 'test(/sources::umls::/)'` → 6/6 pass;
  `bash scripts/check-no-server-tests.sh` → pass; `cargo check` → pass.
- 2026-06-16: `reactome` converted to pure request construction and response
  mapper tests for search and pathway events, replacing source-level mock server
  tests. Checks: `cargo nextest run -E 'test(/sources::reactome::/)'` → 7/7 pass;
  `bash scripts/check-no-server-tests.sh` → pass; `cargo check` → pass.
- 2026-06-16: `wikipathways` converted to pure request construction and response
  mapping/error tests, replacing source-level mock server and env-mutation cache
  tests. Checks: `cargo nextest run -E 'test(/sources::wikipathways::/)'` → 8/8 pass;
  `bash scripts/check-no-server-tests.sh` → pass; `cargo check` → pass.
- 2026-06-16: `kegg` converted to pure path-segment construction and text
  parsing tests, replacing source-level mock server tests. Checks:
  `cargo nextest run -E 'test(/sources::kegg::/)'` → 8/8 pass;
  `bash scripts/check-no-server-tests.sh` → pass; `cargo check` → pass.
- 2026-06-16: `gprofiler` converted to pure POST body construction, response
  parsing, and transient-error remap tests, replacing source-level mock server
  tests. Checks: `cargo nextest run -E 'test(/sources::gprofiler::/)'` → 5/5 pass;
  `bash scripts/check-no-server-tests.sh` → pass; `cargo check` → pass.
- 2026-06-16: `enrichr` converted to pure add-list body construction, enrich
  query construction, and response decoding tests, replacing source-level mock
  server tests. Checks: `cargo nextest run -E 'test(/sources::enrichr::/)'` → 6/6 pass;
  `bash scripts/check-no-server-tests.sh` → pass; `cargo check` → pass.
- 2026-06-16: `quickgo` converted to pure request construction and JSON
  response parsing tests, replacing source-level mock server tests. Checks:
  `cargo nextest run -E 'test(/sources::quickgo::/)'` → 5/5 pass;
  `bash scripts/check-no-server-tests.sh` → pass; `cargo check` → pass.
- 2026-06-16: `complexportal` converted to pure request construction and
  response mapping tests, replacing source-level mock server tests. Checks:
  `cargo nextest run -E 'test(/sources::complexportal::/)'` → 4/4 pass;
  `bash scripts/check-no-server-tests.sh` → pass; `cargo check` → pass.
- 2026-06-16: `string` converted to pure request construction and JSON parsing
  tests, replacing source-level mock server tests. Checks:
  `cargo nextest run -E 'test(/sources::string::/)'` → 4/4 pass;
  `bash scripts/check-no-server-tests.sh` → pass; `cargo check` → pass.
- 2026-06-16: `disgenet` converted to pure authenticated request
  construction, response decoding, association mapping, and disease resolution
  tests, replacing source-level mock server tests. Checks:
  `cargo nextest run -E 'test(/sources::disgenet::/)'` → 17/17 pass;
  `bash scripts/check-no-server-tests.sh` → pass; `cargo check` → pass.
- 2026-06-16: `alphagenome` moved to standard source test layout with pure
  gRPC request construction, tensor parsing, and helper tests. Checks:
  `cargo nextest run -E 'test(/sources::alphagenome::/)'` → 6/6 pass;
  `bash scripts/check-no-server-tests.sh` → pass; `cargo check` → pass.
- 2026-06-16: `medlineplus` converted to pure request construction and XML
  response parsing tests, replacing source-level mock server tests. Kept the
  test-only constructor because downstream entity tests still use it. Checks:
  `cargo nextest run -E 'test(/sources::medlineplus::/)'` → 10/10 pass;
  `bash scripts/check-no-server-tests.sh` → pass; `cargo check` → pass.
- 2026-06-16: `figshare` converted to pure article/search request
  construction, JSON response parsing, download response decision, and URL
  validation tests, replacing source-level mock server tests. Checks:
  `cargo nextest run -E 'test(/sources::figshare::/)'` → 18/18 pass;
  `bash scripts/check-no-server-tests.sh` → pass; `cargo check` → pass.
- 2026-06-16: source endpoint inventory checkpoint. Checks:
  `cargo nextest run -E 'test(/sources::/)'` → 669/669 pass.
- 2026-06-16: first CLI parser-validation batch (`gwas`, `protein`) made
  pure by moving fast-fail validation into callable helpers instead of async
  handler tests. Checks:
  `cargo nextest run -E 'test(/cli::gwas::/) | test(/cli::protein::/)'` → 4/4 pass;
  `cargo check` → pass.
