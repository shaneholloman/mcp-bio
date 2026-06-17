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
- [x] `src/cli/cache/tests.rs` path rendering is pure: CLI tests now call a
      resolved-config helper directly instead of mutating cache env vars. Cache
      config env/file precedence remains covered in `src/cache/config.rs`.
- [x] `src/cli/tests/facade/cache.rs` is pure: cache facade tests now cover
      parsing/help and `--json` command selection without executing cache
      commands against env-derived paths.

### Entity processing + output (response → entity → JSON/markdown)
- [ ] `src/transform/**` and `src/entities/**` — test the pure processing with saved inputs.
- [ ] `src/render/**` — test markdown/JSON output from saved entities.
- [x] **Worst offender fixed:** `src/entities/article/backends/tests.rs` now tests
      request construction and response processing without mock servers, env locks,
      or network-shaped setup.
- [x] `src/entities/article/batch/tests.rs` is pure: compact item projection,
      max-ID validation, and Semantic Scholar compact-row merge tests use direct
      fixtures; stale wiremock imports were removed.
- [x] `src/entities/article/detail/tests.rs` no longer owns fulltext mock-server
      tests. Fulltext source order and PDF opt-in/miss behavior now have pure
      unit tests beside the fulltext code.
- [x] `src/entities/article/search/tests/{finalizer,integration}.rs` and
      `src/entities/article/enrichment/tests.rs` now test finalization,
      Semantic Scholar/article-base merge behavior, source status handling, and
      federated row merging without mock servers or env locks.
- [x] `src/entities/article/graph/tests.rs` is pure: citation, reference, and
      recommendation output mapping now use direct Semantic Scholar response
      structs; request path/header behavior stays covered by source tests.
- [x] `src/entities/article/planner/tests.rs` is pure: Semantic Scholar
      federated-source enablement no longer clears `S2_API_KEY`, and the now
      unused article env-lock helper was removed.
- [x] `src/entities/disease/get/tests.rs` first lookup-path batch now tests
      canonical MyDisease get planning, MESH/OMIM crosswalk query planning, and
      empty crosswalk selection without mock servers or env locks.
- [x] `src/entities/trial/search/nci/tests.rs` is pure: NCI disease grounding,
      keyword fallback, status/phase mapping, and unsupported-filter validation
      no longer use mock servers or async search calls.
- [x] `src/entities/trial/search/ctgov/tests.rs` no longer uses the trial
      env-lock/MyChem mock path for next-page fanout validation; that assertion
      is now a direct error-helper test.
- [x] `src/entities/trial/search/ctgov/tests.rs` CTGov fan-out dedupe/count
      coverage is purer: rare-disease condition expansion and alias-union count
      behavior now exercise direct merge/count helpers instead of mocked CTGov
      pages.
- [x] `src/entities/trial/search/ctgov/tests.rs` has no mock-server cases:
      single-worker pagination, age-filter totals, native count exact/approx,
      and expensive-filter page-cap behavior now test direct reducers/helpers.
- [x] `src/cli/trial/tests.rs` is pure: the duplicate MyChem mock-server
      alias-expansion next-page rejection test was replaced with parser
      coverage, while the real error behavior remains covered in CTGov entity
      tests.
- [x] `src/cli/search_all/tests/dispatch.rs` pathway error surface is pure:
      the search-all markdown/error contract now uses a direct section fixture
      instead of Reactome/KEGG/WikiPathways mock servers and env swaps.
- [x] `src/entities/variant/get/tests.rs` GWAS-only unavailable case is pure:
      it now asserts the exact degraded output state through a helper instead
      of running `get()` against a broken GWAS mock response.
- [x] `src/entities/variant/get/tests.rs` Cancer Hotspots success case is pure:
      it now tests recurrence matching directly instead of driving a mocked
      upstream by-gene endpoint.
- [x] `src/entities/variant/get/tests.rs` is now fully pure: the Cancer Hotspots
      failure/no-op branch also tests result application directly and the unused
      variant env/mock test support was removed.
- [x] `src/entities/adverse_event.rs` OpenFDA status mapping tests are pure:
      404/not-found and empty-result behavior now test decoded response mapping
      directly instead of running a mock OpenFDA server.
- [x] `src/entities/adverse_event.rs` explicit VAERS rejection tests are pure:
      offset and unsupported-filter errors now test validation directly instead
      of entering async source search.
- [x] `src/entities/adverse_event.rs` VAERS non-vaccine resolver and
      unsupported-filter combined response tests are pure: they now test the
      resolver/response helpers directly instead of starting CVX/OpenFDA mocks.
- [x] `src/entities/adverse_event.rs` all-sources non-vaccine VAERS response is
      pure: the local-only resolver result is combined with FAERS status through
      a helper instead of running OpenFDA/CVX mock servers.
- [x] `src/entities/adverse_event.rs` CTGov trial adverse-event aggregation
      tests are pure: alias dedupe, per-study term counting, and best alias copy
      selection now run over direct `CtGovStudy` fixtures instead of a mocked
      CTGov server.
- [x] `src/entities/adverse_event.rs` is now fully pure: VAERS family matching
      and summary payload mapping now use direct resolver/table fixtures, and
      the remaining VAERS mock-server tests are gone.
- [x] `src/entities/pgx.rs` is pure: CPIC actionable-gene counting now tests
      direct CPIC pair rows instead of starting a CPIC mock server.
- [x] `src/entities/drug/get/tests.rs` is pure: trial alias transient-failure
      fallback and canonical-name behavior now test the alias resolution helper
      directly instead of swapping MyChem base URLs.
- [x] `src/entities/discover.rs` is pure: exact article keyword alias
      canonicalization now tests direct OLS fixtures instead of starting an OLS
      mock server.
- [x] `src/entities/diagnostic/mod.rs` OpenFDA regulatory overlay tests are
      pure: empty WHO regulatory overlays and GTR PMA supplement dedupe now use
      direct FDA row fixtures instead of OpenFDA mock servers.
- [x] `src/entities/pathway.rs` Reactome/WikiPathways search-combine and KEGG
      disabled behavior are pure: source-result finalization and disabled-flag
      parsing now test direct fixtures instead of mock servers and env swaps.
- [x] `src/entities/gene.rs` Reactome workflow-signal mock is gone: the
      limit-one probe request shape is covered in Reactome source tests and the
      gene entity test now covers the empty-symbol no-probe guard directly.
- [x] `src/entities/gene.rs` diagnostics tests are now pure: diagnostic row
      application and unavailable-note behavior test direct fixtures instead of
      creating a temporary GTR data root through `BIOMCP_GTR_DIR`.
- [x] `src/entities/disease/resolution/tests.rs` is pure: the weak direct
      disease-name match rejection now tests the resolver score threshold
      directly instead of querying a mocked MyDisease server.
- [x] `src/entities/disease/fallback/tests.rs` Arnold synonym crosswalk page
      test is pure: it keeps OLS/MyDisease request-plan assertions and uses a
      deterministic row resolver instead of a mocked MyDisease query endpoint.
- [x] `src/entities/disease/fallback/tests.rs` canonical not-found behavior is
      pure: canonical lookup result handling is split into a direct helper test
      instead of using a mocked MyDisease 404.
- [x] `src/entities/disease/enrichment/tests.rs` SEER survival catalog note
      behavior is pure: no-data and catalog-failure cases now test catalog
      resolution directly instead of starting a mocked SEER server.
- [x] `src/entities/disease/enrichment/tests.rs` sparse OLS identity proof is
      pure: exact OLS document selection now tests direct `OlsDoc` fixtures
      instead of starting an OLS mock server.
- [x] `src/entities/disease/enrichment/tests.rs` disease diagnostic rows and
      unavailable-note behavior are pure: result application now uses direct
      diagnostic page fixtures instead of GTR/WHO fixture-root env swaps.
- [x] `src/entities/disease/clinical_features.rs` MedlinePlus fallback behavior
      is pure: empty/failing live query results now test the fixture fallback
      collector directly instead of starting MedlinePlus mock servers.
- [x] `src/entities/disease/clinical_features.rs` is now fully pure: the
      remaining live MedlinePlus dedupe test is a direct collector test, and
      the unused MedlinePlus test client constructor was removed.
- [x] `src/render/markdown/related/tests/{disease,misc}.rs` oncology study
      related-command tests are pure: local study selection now tests direct
      `StudyLookupRow`/study-id fixtures instead of writing temporary
      `BIOMCP_STUDY_DIR` roots.

### Utils
- [ ] `src/utils/*.rs` (date, download, query, serde) — direct unit tests.
- [x] `src/utils/download.rs` cache-root tests are pure: download path and
      atomic-save target tests now use direct config/path inputs instead of
      cache env vars.
- [ ] The shared helpers in `src/sources/mod.rs` (`RequestPlan`, `decode_json`) — a few tests.
- [x] `src/sources/mod.rs` HTTP cache migration test is pure: client
      construction can take an explicit resolved cache config, and duplicate
      env cache-root tests were removed.
- [x] `src/sources/rate_limit.rs` Semantic Scholar key interval tests are
      pure: they assert the interval helper directly instead of mutating
      `S2_API_KEY`.

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
- 2026-06-16: second CLI parser-validation batch (`diagnostic`, `disease`)
  made pure for fast-fail limit checks while leaving real JSON behavior tests
  in place. Checks:
  `cargo nextest run -E 'test(/cli::diagnostic::/) | test(/cli::disease::/)'` → 14/14 pass;
  `cargo check` → pass.
- 2026-06-16: third CLI parser-validation batch (`pgx`, `phenotype`,
  `pathway`) made pure for fast-fail limit checks. Checks:
  `cargo nextest run -E 'test(/cli::pgx::/) | test(/cli::phenotype::/) | test(/cli::pathway::/)'` → 11/11 pass;
  `cargo check` → pass.
- 2026-06-16: fourth CLI parser-validation batch (`drug`) made pure for
  source/product-type/no-alias validation while leaving raw-label entity
  behavior covered by existing tests. Checks:
  `cargo nextest run -E 'test(/cli::drug::/)'` → 30/30 pass;
  `cargo check` → pass.
- 2026-06-16: fifth CLI parser-validation batch (`adverse_event`) made pure
  for source/type/count validation by adding a callable search-plan helper.
  Checks: `cargo nextest run -E 'test(/cli::adverse_event::/)'` → 8/8 pass;
  `cargo check` → pass.
- 2026-06-16: tried broad CLI checkpoint with
  `cargo nextest run -E 'test(/cli::/)'`; stopped it after 103s because 11
  alias-fallback/output behavior tests were still running. Result at interrupt:
  537 passed, 11 interrupted. Do not use broad `cli::` as a normal gate yet;
  keep using narrow CLI batches until the alias-fallback tests are decomposed.
- 2026-06-16: decomposed the 11 broad-CLI blockers. Replaced the remaining
  alias-fallback/output mock-server tests with pure tests over alias decisions,
  batch JSON rendering, and article exact-lookup request/rendering behavior.
  Removed the now-dead CLI mock helpers. Checks:
  `cargo nextest run -E 'test(/cli::tests::outcome::/) or test(/cli::gene::tests::/) or test(/cli::article::tests::exact_lookup::/)'` → 30/30 pass;
  `cargo nextest run -E 'test(/cli::/)'` → 547/547 pass; `cargo check` → pass.
- 2026-06-16: converted `src/entities/article/backends/tests.rs` from
  mock-server/env-lock tests to pure tests over PubMed request plans and row
  filtering, Semantic Scholar status/row mapping, and LitSense2 dedupe/hydration.
  Checks: `cargo nextest run -E 'test(/entities::article::backends::/)'` → 13/13 pass;
  `cargo check` → pass. A broader `cargo nextest run -E 'test(/entities::article::/)'`
  was stopped after 69s: 126 passed, 16 interrupted, 24 not run. The remaining
  slow tests are in `article/detail` and `article/search/{finalizer,integration}`,
  so keep using narrow article batches until those are decomposed.
- 2026-06-16: moved article fulltext waterfall coverage out of
  `src/entities/article/detail/tests.rs` mock-server tests and into pure
  `src/entities/article/fulltext.rs` unit tests. The production resolver now uses
  a testable XML attempt-order helper and a testable Semantic Scholar PDF opt-in
  helper. Removed dead article test-support XML fixture builders. Checks:
  `cargo nextest run -E 'test(/entities::article::detail::/) or test(/entities::article::fulltext::/)'`
  → 15/15 pass; `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings`
  → pass.
- 2026-06-16: converted the remaining article search/enrichment slow tests to
  pure tests. `search/tests/finalizer.rs`, `search/tests/integration.rs`, and
  `enrichment/tests.rs` now exercise finalization, source status, merge, and
  metadata-fill behavior directly instead of driving mock PubMed/PubTator/
  Europe PMC/Semantic Scholar servers. Checks:
  `cargo nextest run -E 'test(/entities::article::search::/) or test(/entities::article::enrichment::/)'`
  → 19/19 pass; `cargo nextest run -E 'test(/entities::article::/)'` → 159/159
  pass; `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass.
- 2026-06-16: partial CT.gov trial-search cleanup. Removed two slow alias-union
  orchestration tests that were taking >60s and replaced their label/provenance
  coverage with pure `ctgov_workers` and strict-condition-label tests. Kept the
  remaining CT.gov pagination/count server tests for now. Checks:
  `cargo nextest run -E 'test(/entities::trial::search::ctgov::/)'` → 21/21
  pass; `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass.
- 2026-06-16: partial NCI trial-search cleanup. Replaced server/env-lock status
  and phase mapping tests with direct `nci_status_filter` / `nci_phase_filters`
  unit tests, and replaced the slow no-hit disease-grounding fallback test with
  a pure NCI request-plan assertion for keyword fallback. Checks:
  `cargo nextest run -E 'test(/entities::trial::search::nci::/)'` → 12/12 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass.
- 2026-06-16: partial disease-get cleanup. Replaced the canonical MONDO get,
  MESH/OMIM crosswalk, and unresolved-crosswalk tests with pure MyDisease
  request-plan/crosswalk-selection assertions. The two remaining disease-get
  mock-server tests still run quickly and stay as behavior checks for now.
  Checks: `cargo nextest run -E 'test(/entities::disease::get::/)'` → 11/11 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass.
- 2026-06-16: finished NCI trial-search cleanup. Replaced the remaining
  MyDisease/NCI mock-server tests with a pure helper for NCI disease grounding
  (`ConceptId` vs keyword fallback) plus request-plan assertions. Converted the
  unsupported-filter tests from async `search()` calls to direct validation
  checks, and removed the now-unused `NciCtsClient::new_for_test` helper.
  Checks: `cargo nextest run -E 'test(/entities::trial::search::nci::/)'` →
  12/12 pass; `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings`
  → pass; `git diff --check` → pass.
- 2026-06-16: small CT.gov trial-search cleanup. Replaced the MyChem/env-lock
  next-page fanout rejection test with a direct `fanout_next_page_error`
  assertion and removed the now-unused trial `lock_env` / env re-export helper.
  The remaining CT.gov server tests are still fast after compile but still exist.
  Checks: `cargo nextest run -E 'test(/entities::trial::search::ctgov::/)'` →
  21/21 pass; `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings`
  → pass; `git diff --check` → pass.
- 2026-06-16: CT.gov fan-out/count cleanup. Split pure helpers for union-row
  merging, unique NCT counting, and count page-cap detection. Converted the
  rare-disease expanded-condition page/count tests and alias-union count
  exact/unknown tests from mocked CTGov pages to direct helper tests. Remaining
  CTGov mock-server cases are the single-worker pagination/count traversal
  tests. Checks: `cargo nextest run -E 'test(/entities::trial::search::ctgov::/)'`
  → 21/21 pass; `cargo check` → pass; `cargo clippy --lib --tests -- -D
  warnings` → pass; `git diff --check` → pass.
- 2026-06-16: partial variant-get cleanup. Extracted `mark_gwas_unavailable`
  and converted the GWAS-only unavailable test from a bad-JSON GWAS mock-server
  test to a pure output-state assertion. The two remaining Cancer Hotspots
  mock-server tests still cover enrichment success/failure and are the slow part
  of this file. Checks:
  `cargo nextest run -E 'test(/entities::variant::get::/)'` → 11/11 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass.
- 2026-06-16: second variant-get cleanup. Converted the Cancer Hotspots success
  enrichment test to a pure `recurrence_for_change` fixture test with the same
  position/same-amino-acid assertions. One Cancer Hotspots failure/no-op
  mock-server test remains. Checks:
  `cargo nextest run -E 'test(/entities::variant::get::/)'` → 11/11 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass.
- 2026-06-16: finished variant-get cleanup. Extracted Cancer Hotspots result
  application, converted the upstream-failure/no-op test to a pure helper test,
  and removed the now-unused variant env/mock test support. Checks:
  `cargo nextest run -E 'test(/entities::variant::get::/)'` → 11/11 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass.
- 2026-06-16: partial adverse-event cleanup. Extracted the decoded OpenFDA
  response-to-`FaersSearchStatus` mapper and converted the 404/not-found and
  empty-result status tests to pure unit tests. VAERS/CVX and trial adverse
  event server tests still remain. Checks:
  `cargo nextest run -E 'test(/entities::adverse_event::/)'` → 23/23 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass.
- 2026-06-16: second adverse-event cleanup. Extracted explicit VAERS source
  validation and converted the offset / unsupported-filter rejection tests from
  async `search_with_source` calls to direct validation checks. Checks:
  `cargo nextest run -E 'test(/entities::adverse_event::/)'` → 23/23 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass.
- 2026-06-16: third adverse-event cleanup. Converted the non-vaccine VAERS
  resolver test to call the resolver directly, then extracted the
  unsupported-filter combined response helper and converted that test away from
  OpenFDA/CVX mock servers. Remaining adverse-event server cases are the larger
  CVX/VAERS summary and trial adverse-event cases. Checks:
  `cargo nextest run -E 'test(/entities::adverse_event::/)'` → 23/23 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass.
- 2026-06-16: fourth adverse-event cleanup. Extracted the all-sources response
  helper for supported VAERS filters and converted the non-vaccine all-sources
  case from OpenFDA/CVX mock servers to a pure local-only resolver plus FAERS
  status test. Remaining adverse-event server cases are the two VAERS summary
  tests and the three trial adverse-event CTGov cases. Checks:
  `cargo nextest run -E 'test(/entities::adverse_event::/)'` → 23/23 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass.
- 2026-06-16: fifth adverse-event cleanup. Split CTGov trial adverse-event
  aggregation from the async CTGov fetch loop, then converted alias dedupe,
  per-study term counting, and preferred-alias-copy tests to direct `CtGovStudy`
  fixture batches. Remaining adverse-event mock-server cases are only the two
  VAERS summary tests. Checks:
  `cargo nextest run -E 'test(/entities::adverse_event::/)'` → 23/23 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass.
- 2026-06-16: finished adverse-event cleanup. Converted the remaining VAERS
  summary mock-server tests to pure resolver/table mapping tests. The VAERS
  source layer already covers request construction and XML parsing, so
  `src/entities/adverse_event.rs` now has no mock-server/env-lock cases. Checks:
  `cargo nextest run -E 'test(/entities::adverse_event::/)'` → 23/23 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass.
- 2026-06-16: disease resolution cleanup. Converted the weak direct-match
  rejection test from a mocked MyDisease query to a pure score-threshold test
  over the same Hodgkin/non-Hodgkin candidate. Checks:
  `cargo nextest run -E 'test(/entities::disease::resolution::/)'` → 11/11
  pass; `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` →
  pass; `git diff --check` → pass.
- 2026-06-16: disease fallback cleanup. Converted the Arnold Chiari
  synonym-rescue crosswalk test from a mocked MyDisease query to request-plan
  assertions plus a deterministic fallback-row resolver. Then converted the
  canonical not-found test from a mocked MyDisease 404 to a pure helper test.
  `src/entities/disease/fallback/tests.rs` now has no mock-server/env-lock
  cases. Checks:
  `cargo nextest run -E 'test(/entities::disease::fallback::/)'` → 11/11 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass.
- 2026-06-16: disease enrichment SEER survival cleanup. Split survival catalog
  resolution from the SEER fetch path, converted the no-matching-site and
  catalog-failure note tests to pure helper tests, and removed the now-unused
  SEER mock helper. Checks:
  `cargo nextest run -E 'test(/entities::disease::enrichment::/)'` → 11/11
  pass; `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` →
  pass; `git diff --check` → pass.
- 2026-06-16: disease clinical-feature cleanup. Split live-topic
  result collection from MedlinePlus HTTP calls, converted the empty-live-result
  and failed-live-result fixture fallback tests to pure helper tests, and
  removed the unused empty-response mock helpers. Then converted the remaining
  live MedlinePlus dedupe test to a pure collector/order test and removed the
  unused MedlinePlus test client constructor. `clinical_features.rs` now has no
  mock-server/env-lock cases. Checks:
  `cargo nextest run -E 'test(/entities::disease::clinical_features::/)'` →
  15/15 pass; `cargo check` → pass; `cargo clippy --lib --tests -- -D
  warnings` → pass; `git diff --check` → pass.
- 2026-06-16: CTGov trial-search cleanup. Split single-worker pagination into a
  direct reducer/finalizer, added native-count and page-cap helpers, converted
  age-total semantics, cursor preservation, exact/approx native counts, and
  expensive-filter page-cap tests to pure tests, then removed the unused CTGov
  mock helper and `ClinicalTrialsClient::new_for_test`. `ctgov/tests.rs` now
  has no mock-server/env-lock cases. Checks:
  `cargo nextest run -E 'test(/entities::trial::search::ctgov::/)'` → 21/21
  pass; `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` →
  pass; `git diff --check` → pass; CTGov purity `rg` → no matches.
- 2026-06-16: article graph cleanup. Split citation/reference/recommendation
  response assembly into pure helpers, converted the three Semantic Scholar
  mock-server tests to direct response-struct tests, and added source request
  plan assertions that no API-key header is sent when no key is supplied.
  `src/entities/article/graph/tests.rs` now has no mock-server/env-lock cases.
  Checks:
  `cargo nextest run -E 'test(/entities::article::graph::/) |
  test(/sources::semantic_scholar::/)'` → 19/19 pass; `cargo check` → pass;
  `cargo clippy --lib --tests -- -D warnings` → pass; `git diff --check` →
  pass; article graph purity `rg` → no matches.
- 2026-06-16: article planner cleanup. Converted Semantic Scholar federated
  enablement from an env-clearing async test to a direct planner assertion, and
  removed the now-unused article env-lock test helper. Checks:
  `cargo nextest run -E 'test(/entities::article::planner::/)'` → 20/20 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass; article planner purity `rg` → no matches.
- 2026-06-16: PGx cleanup. Split CPIC actionable-gene counting over fetched
  rows into a pure helper and converted the warfarin unique-gene threshold test
  from a CPIC mock server to direct `CpicPairRow` fixtures. `src/entities/pgx.rs`
  now has no mock-server/env-lock cases. Checks:
  `cargo nextest run -E 'test(/entities::pgx::/)'` → 6/6 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass; PGx purity `rg` → no matches.
- 2026-06-16: drug get cleanup. Split trial alias lookup-result handling into a
  pure helper, converted transient MyChem failure and generic canonical-name
  tests from mock-server/env swaps to direct helper tests, and kept the async
  resolver behavior unchanged. `src/entities/drug/get/tests.rs` now has no
  mock-server/env-lock cases. Checks:
  `cargo nextest run -E 'test(/entities::drug::get::/)'` → 16/16 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass; drug get purity `rg` → no matches.
- 2026-06-16: disease enrichment sparse-identity cleanup. Split exact OLS
  document application from the async OLS fetch path and converted the sparse
  MONDO identity proof from an OLS mock server to direct `OlsDoc` fixtures.
  Other disease enrichment env-based diagnostic/clinical-feature tests still
  remain. Checks:
  `cargo nextest run -E 'test(/entities::disease::enrichment::/) |
  test(/enrich_sparse_disease_identity_prefers_exact_ols4_match/)'` → 12/12
  pass; `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` →
  pass; `git diff --check` → pass.
- 2026-06-16: discover cleanup. Converted exact article keyword alias
  canonicalization from an OLS mock-server test to direct `OlsDoc` fixtures
  through `resolve_exact_article_keyword_entity_from_ols_docs`, and removed the
  now-unused discover wiremock/env imports. `src/entities/discover.rs` now has
  no mock-server/env-lock cases. Checks:
  `cargo nextest run -E 'test(/entities::discover::/)'` → 39/39 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass; discover purity `rg` → no matches.
- 2026-06-16: gene Reactome cleanup. Moved the limit-one Reactome workflow
  probe assertion into `src/sources/reactome/tests/construction.rs`, converted
  the gene entity test to the empty-symbol no-probe guard, and removed the
  Reactome mock-server imports from `src/entities/gene.rs`. Gene diagnostics
  env-lock tests still remain. Checks:
  `cargo nextest run -E 'test(/entities::gene::/) | test(/sources::reactome::/)'`
  → 29/29 pass; `cargo check` → pass; `cargo clippy --lib --tests -- -D
  warnings` → pass; `git diff --check` → pass; Reactome/gene mock `rg` → no
  matches.
- 2026-06-16: diagnostic regulatory cleanup. Split FDA regulatory row merging
  into a pure helper, converted the WHO empty-overlay and GTR PMA supplement
  dedupe tests from OpenFDA mock servers to direct `FdaPmaResult`/empty-row
  fixtures, and removed diagnostic wiremock imports. Local GTR/WHO fixture env
  setup tests still remain. Checks:
  `cargo nextest run -E 'test(/entities::diagnostic::/)'` → 19/19 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass; diagnostic OpenFDA mock `rg` → no matches.
- 2026-06-16: pathway entity cleanup. Split source-result finalization and KEGG
  disabled flag parsing into pure helpers, converted the Reactome/WikiPathways
  search-combine and disabled-flag tests from mock-server/env-lock tests to
  direct fixtures, and removed pathway wiremock/env imports. Checks:
  `cargo nextest run -E 'test(/entities::pathway::/)'` → 18/18 pass.
- 2026-06-16: gene diagnostics cleanup. Split diagnostic row/error application
  into a pure helper and converted the two diagnostics section tests from
  `BIOMCP_GTR_DIR` fixture-root env tests to direct diagnostic rows/errors.
  Checks: `cargo nextest run -E 'test(/entities::gene::/)'` → 21/21 pass.
- 2026-06-16: disease diagnostics enrichment cleanup. Split diagnostic page
  application into a pure helper and converted the disease diagnostic rows and
  unavailable-note tests from GTR/WHO fixture-root env tests to direct
  diagnostic page fixtures. The MedlinePlus fallback env test in this file
  remains. Checks:
  `cargo nextest run -E 'test(/entities::disease::enrichment::/)'` → 11/11 pass.
- 2026-06-16: trial CLI cleanup. Replaced the duplicate MyChem mock-server
  alias-expansion next-page CLI test with pure parser coverage and kept the
  behavior proof in CTGov entity tests. Removed now-unused CLI test-support
  wiremock re-exports. Checks:
  `cargo nextest run -E 'test(/cli::trial::/) | test(/entities::trial::search::ctgov::/)'`
  → 54/54 pass.
- 2026-06-16: search-all pathway error cleanup. Replaced the
  Reactome/KEGG/WikiPathways mock-server/env warning-path test with a direct
  `SearchAllSection` error fixture that preserves the sanitized markdown
  contract. Pathway source-combination/error behavior remains covered in
  pathway entity tests. Checks:
  `cargo nextest run -E 'test(/cli::search_all::/) | test(/entities::pathway::/)'`
  → 51/51 pass.
- 2026-06-16: article batch cleanup. Removed stale wiremock imports from the
  already-pure article batch tests. Checks:
  `cargo nextest run -E 'test(/entities::article::batch::/)'` → 3/3 pass.
- 2026-06-16: render related cleanup. Split disease related-command generation
  from the local cBioPortal study lookup and converted oncology study follow-up
  tests from temporary `BIOMCP_STUDY_DIR` roots to direct `StudyLookupRow` and
  study-id fixtures. Checks:
  `cargo nextest run -E 'test(/render::markdown::related::/)'` → 53/53 pass.
- 2026-06-16: cache CLI path cleanup. Split path rendering from cache config
  resolution with `render_path_for_config`, replaced env-lock/cache-env CLI path
  tests with direct resolved-config tests, and left env/file precedence in
  `src/cache/config.rs`. Checks:
  `cargo nextest run -E 'test(/cli::cache::/) | test(/cache::config::/)'` →
  37/37 pass; `cargo check` → pass; `cargo clippy --lib --tests -- -D
  warnings` → pass; `git diff --check` → pass.
- 2026-06-16: download util cleanup. Split download path construction from
  cache config resolution and split atomic-save behavior to accept an explicit
  target path. Replaced cache-env/env-lock tests with direct config/path tests.
  Checks: `cargo nextest run -E 'test(/utils::download::/)'` → 6/6 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass.
- 2026-06-16: cache facade cleanup. Removed cache facade `execute(...)` tests
  that had to mutate cache env vars, kept parse/help coverage, added pure
  `--json cache path/stats` parse checks, and removed the now-unused CLI
  `lock_env` helper. Cache output formatting remains covered in
  `src/cli/cache/tests.rs`. Checks:
  `cargo nextest run -E 'test(/cli::tests::facade::cache::/)'` → 12/12 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass.
- 2026-06-16: rate-limit cleanup. Replaced Semantic Scholar API-key env tests
  with direct `s2_min_interval` assertions and removed the rate-limit env-lock
  helper/imports. Checks:
  `cargo nextest run -E 'test(/sources::rate_limit::/)'` → 14/14 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass.
- 2026-06-16: shared source HTTP-cache cleanup. Split shared HTTP client
  construction so tests can pass a resolved cache config directly, removed
  duplicate env-based cache-root assertions, and kept the legacy
  `http-cacache` migration proof as a pure temp-directory test. Checks:
  `cargo nextest run -E 'test(/sources::tests::/)'` → 26/26 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass.
- 2026-06-16: CLI test-support cleanup. Removed the now-unused
  `src/cli/test_support.rs` module after cache facade tests stopped importing
  env helpers. Checks: `cargo check` → pass;
  `cargo clippy --lib --tests -- -D warnings` → pass; `git diff --check` →
  pass.
- 2026-06-16: health local cache-dir cleanup. Split cache-dir health probing
  so tests can pass a resolved config or config error directly, replacing the
  remaining cache env-lock tests in `src/cli/health/tests/local.rs`. Checks:
  `cargo nextest run -E 'test(/cli::health::tests::local::/)'` → 35/35 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass.
- 2026-06-16: health runner timeout cleanup. Split timeout row construction so
  runner tests can assert timeout status/key metadata directly, replacing the
  mock-server/env-lock timeout test in `src/cli/health/tests/runner.rs`.
  Checks: `cargo nextest run -E 'test(/cli::health::tests::runner::/)'` →
  11/11 pass; `cargo check` → pass;
  `cargo clippy --lib --tests -- -D warnings` → pass; `git diff --check` →
  pass.
- 2026-06-16: cache config cleanup. Replaced process-env/XDG cache config
  tests with direct `resolve_cache_config_with_source` and `read_cache_toml`
  checks, preserving precedence and error-path coverage without env locks.
  Checks: `cargo nextest run -E 'test(/cache::config::/)'` → 28/28 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass.
- 2026-06-16: disease clinical-feature fallback cleanup. Added test-only
  helpers to apply clinical features from explicit MedlinePlus topics and
  converted the uterine-fibroid fallback proof from a bad `BIOMCP_MEDLINEPLUS_BASE`
  env test to direct offline fixture topics. Checks:
  `cargo nextest run -E 'test(/entities::disease::enrichment::/) |
  test(/entities::disease::clinical_features::/)'` → 26/26 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass.
- 2026-06-16: study entity cleanup. Split cBioPortal study entity functions so
  tests can pass an explicit fixture root instead of setting `BIOMCP_STUDY_DIR`,
  and converted the study entity tests away from the global env lock. Public
  functions still resolve the configured study directory normally. Checks:
  `cargo nextest run -E 'test(/entities::study::/)'` → 22/22 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass.
- 2026-06-16: disease get cleanup. Removed the two old get-level mock-server
  proofs that set multiple service base-url env vars; their behavior is covered
  by pure association merge tests and the pure OLS4 sparse-identity enrichment
  test. Removed the now-unused disease mock/env helpers and top-level proof
  wrappers. Checks:
  `cargo nextest run -E 'test(/entities::disease::get::/) |
  test(/entities::disease::associations::/) |
  test(/entities::disease::enrichment::/) |
  test(/augment_genes_with_opentargets/) |
  test(/enrich_sparse_disease_identity/) |
  test(/disease_markdown_renders_ot_only_gene_association_table/)'` → 28/28
  pass; `cargo check` → pass;
  `cargo clippy --lib --tests -- -D warnings` → pass; `git diff --check` →
  pass.
- 2026-06-16: health HTTP auth cleanup. Split configured-key trimming and
  optional-auth HTTP status classification into pure helpers, then converted
  the OncoKB/NCI key-gated tests and Semantic Scholar optional-auth tests away
  from env mutation and mock servers. The VAERS health probe remains as the
  only end-to-end mock-server test in this file. Checks:
  `cargo nextest run -E 'test(/cli::health::tests::http::/)'` → 9/9 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass.
- 2026-06-16: diagnostic CLI cleanup. Split diagnostic get/search JSON response
  assembly into pure dispatch helpers, then converted the CLI JSON/meta tests
  from temporary `BIOMCP_GTR_DIR` fixtures to explicit diagnostic/result
  fixtures. Entity-level GTR/WHO fixture behavior remains covered in
  `src/entities/diagnostic/mod.rs`. Checks:
  `cargo nextest run -E 'test(/cli::diagnostic::/)'` → 7/7 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass.
- 2026-06-17: health VAERS cleanup. Split VAERS health row construction into
  a pure helper and replaced the mock-server/`BIOMCP_VAERS_BASE` health test
  with direct success/error outcome tests. VAERS request construction and XML
  response parsing remain covered in `src/sources/vaers.rs`. Checks:
  `cargo nextest run -E 'test(/cli::health::tests::http::/)'` → 10/10 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass.
- 2026-06-17: diagnostic search cleanup. Split diagnostic search into a shared
  data-processing helper so tests can load GTR and WHO IVD fixtures from
  explicit roots instead of setting `BIOMCP_GTR_DIR` / `BIOMCP_WHO_IVD_DIR`.
  Converted the seven `search_page_*` entity tests away from the global env
  lock. Checks:
  `cargo nextest run -E 'test(/entities::diagnostic::tests::search_page_/)'`
  → 7/7 pass; `cargo check` → pass;
  `cargo clippy --lib --tests -- -D warnings` → pass; `git diff --check` →
  pass.
- 2026-06-17: diagnostic get cleanup. Split diagnostic get into a shared
  data-processing helper so GTR and WHO IVD get tests can load fixture roots
  directly. Removed the diagnostic entity tests' env-lock fixture helpers; the
  diagnostic entity and CLI diagnostic test paths no longer set
  `BIOMCP_GTR_DIR` / `BIOMCP_WHO_IVD_DIR`. Kept `WhoIvdClient::get` as
  test-only for the source parsing tests. Checks:
  `cargo nextest run -E 'test(/entities::diagnostic::tests::get_/)'` → 4/4
  pass; `cargo check` → pass;
  `cargo clippy --lib --tests -- -D warnings` → pass; `git diff --check` →
  pass.
- 2026-06-17: article CLI session cleanup. Split the article session-suggestion
  wiring into a pure helper that accepts an explicit cache root, then replaced
  the two mock-server/env-driven article JSON session tests with direct
  in-memory result tests. Removed the now-unused global env-lock and env-var
  test helpers from `src/test_support.rs`; a scan shows no remaining
  `env_lock` / `set_env_var` test usage. Checks:
  `cargo nextest run -E 'test(/cli::article::tests::json::/) |
  test(/cli::article::tests::help::/)'` → 17/17 pass; `cargo check` → pass;
  `cargo clippy --lib --tests -- -D warnings` → pass; `git diff --check` →
  pass.
- 2026-06-17: shared source retry cleanup. Replaced the two `src/sources/mod.rs`
  mock-server retry tests with canned `reqwest::Response` values passed through
  `retry_send_with_sleep`; this keeps the retry behavior proof without a local
  HTTP server. Checks:
  `cargo nextest run -E 'test(/sources::tests::/)'` → 26/26 pass;
  `cargo check` → pass; `cargo clippy --lib --tests -- -D warnings` → pass;
  `git diff --check` → pass.
