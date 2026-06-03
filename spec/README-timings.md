# Spec Lane Audit

## Canary Lane Contract

| Target | Run when | Timeout | Scope | Cache contract |
|---|---|---|---|---|
| `make spec-contracts` | March `spec-only`, `release-gate`, and routine pre-merge proof | `180s` per heading | deterministic validation-lane docs and static surface contracts, including `spec/surface/cli.md` and `spec/surface/test_parallel_isolation_contract.py` | uses the release binary selected by `PATH` and `BIOMCP_BIN`; no live-smoke commands run in this lane |
| `make release-live-smoke` | explicit opt-in operator confidence before releases | n/a | small live public-upstream matrix for discover/OLS4, disease, article source status, variant normalization, and the pathway assertions | every command goes through `tools/biomcp-ci`, which owns cache/XDG roots and optional-key stripping |
| `make spec-pr` | PR CI canary and repo-local debugging of the executable corpus that is not live-smoke-only | `180s` per heading | the active v2 corpus under `spec/entity/` and `spec/surface/`, excluding the pathway live-smoke spec | CI restores `.cache/biomcp-specs/`; cache hits export `BIOMCP_SPEC_CACHE_HIT=1`, which makes `tools/biomcp-ci` replay the warm HTTP cache with `BIOMCP_CACHE_MODE=infinite` |
| `make spec` | repo-local canary reruns and spec debugging | `120s` per heading | the same non-pathway canary tree as `make spec-pr` | uses the same wrapper/cache root, but cold local runs leave `BIOMCP_CACHE_MODE` unset so the cache can refill |
| `make test-contracts` | PR contracts lane and local docs/Python validation | n/a | Rust release build plus Python/docs contract checks | independent of the executable-spec wrapper |

Routine validation now uses the deterministic `make spec-contracts` lane, and
public upstream confidence is live and opt-in through `make release-live-smoke`.
Ticket 379 pruned representative live public-upstream assertions from
`spec/entity/article.md`, `spec/entity/variant.md`, `spec/entity/disease.md`,
and `spec/surface/discover.md`: deterministic request, source, fixture, and
renderer contracts own routine proof, while `release-live-smoke` owns the small
operator live matrix.
Legacy canary targets remain available for corpus debugging: upstream-heavy
canaries leave the main xdist pool and rerun in a serialized leg:
`spec/entity/protein.md` for the ComplexPortal canary plus
`spec/entity/disease.md` and `spec/surface/discover.md` for OLS4-heavy
disease/discover headings such as synonym rescue, alias routing, and symptom
mapping. `spec/entity/pathway.md` is live-source-dependent across KEGG,
Reactome, and WikiPathways, so ticket 390 removes it from routine `make spec`
and `make spec-pr` and runs it only through `release-live-smoke`. The protein
ComplexPortal section is fixture-backed rather than a live upstream canary; live
ComplexPortal availability belongs to `biomcp health`/operator inspection,
while OLS4 public confidence belongs to `release-live-smoke`. FAQ #14 is
absorbed by the serial OLS4 parallel-isolation contract rather than by a new
OLS4 fixture server. The
executable docs themselves call `tools/biomcp-ci`; `make spec` and `make
spec-pr` choose timeout plus that upstream-heavy partitioning. The spec targets
install Python dev dependencies with
`uv sync --extra dev --no-install-project`, then invoke pytest with
`uv run --no-sync ...` so uv does not install or rebuild the maturin-backed
current project. The binary under test remains `target/release/biomcp` via
`PATH` and `BIOMCP_BIN`.

## Active Corpus

| Path | Purpose |
|---|---|
| `spec/entity/gene.md` | gene search/get canary for identity, tissue-expression context, druggability, and funding/diagnostics pivots |
| `spec/entity/variant.md` | variant canary for gene-scoped search, protein-filter normalization, residue aliases, and clinical/population context |
| `spec/entity/article.md` | article canary for typed vs keyword search, source-aware result structure, annotations, and fulltext fallback |
| `spec/entity/trial.md` | trial canary for condition/status search, alias normalization, age-count transparency, and eligibility/location detail |
| `spec/entity/drug.md` | drug canary for multi-region search, brand bridging, structured-indication truthfulness, and regulatory/target pivots |
| `spec/entity/disease.md` | disease canary for MONDO grounding, synonym rescue, genes/diagnostics gating, funding, and executable pivots |
| `spec/entity/protein.md` | protein canary for reviewed search defaults, UniProt identity, complexes/structures, and JSON follow-up contracts |
| `spec/entity/pathway.md` | live-smoke-only pathway canary for alias normalization, exact-title ranking, concise KEGG defaults, and source-aware section rejection |
| `spec/entity/study.md` | study canary for local cBioPortal discovery, typed analytics validation, comparison summaries, and chart output |
| `spec/entity/pgx.md` | pgx canary for gene/drug CPIC interaction search, opt-in recommendations, and population-frequency detail |
| `spec/entity/phenotype.md` | phenotype canary for HPO/symptom inputs, similarity-ranked disease output, and typed disease follow-ups |
| `spec/entity/diagnostic.md` | diagnostic canary for source-aware search, gene-first GTR guidance, compact discovery rows, and WHO detail paths |
| `spec/entity/vaers.md` | vaers canary for vaccine-first CDC aggregation, aggregate-only reporting, and explicit source limitations/combined output |
| `spec/surface/cli.md` | CLI surface canary for top-level help/list discovery, operator commands, cache-mode exceptions, and health/admin guidance |
| `spec/surface/mcp.md` | MCP surface canary for stdio/HTTP entrypoints, probe routes, and streamable-HTTP tool execution |
| `spec/surface/discover.md` | onboarding-surface canary for discover resolution, suggest routing, skill guidance, and fallback behavior |

## Bash Mustmatch Lint Rule

Every `##` spec section with at least one non-skipped `bash` block must include
at least one `| mustmatch` line unless the section explicitly opts out with
`<!-- mustmatch-lint: skip -->`.

This rule exists because the mustmatch pytest plugin silently does not collect
bash blocks that never pipe to `mustmatch`. A section that only uses `jq -e` or
other exit-code checks can disappear from pytest output instead of passing,
failing, or skipping.

Prefer adding a meaningful `mustmatch` assertion on user-visible output or a
stable JSON anchor even when the section also uses `jq -e` for structured
validation. Reserve the opt-out for genuinely exit-code-only checks or cases
without a stable, meaningful output anchor. For readability, place the opt-out
comment immediately after the `##` heading.

## Audit Method

- Measure in the current worktree after `cargo build --release --locked`.
- Keep Python setup project-free: `uv sync --extra dev --no-install-project`,
  then `uv run --no-sync ...` for pytest/spec commands.
- Run `make spec-contracts` for routine deterministic timing. Run `make spec-pr`
  cold once to populate `.cache/biomcp-specs/`, then rerun warm without clearing
  that cache root when measuring the legacy canary lane.
- `tools/biomcp-ci` owns `BIOMCP_CACHE_DIR`, `XDG_CACHE_HOME`,
  `XDG_CONFIG_HOME`, optional-key stripping, and the `BIOMCP_SPEC_CACHE_HIT=1`
  to `BIOMCP_CACHE_MODE=infinite` warm-hit replay switch.
- The routine deterministic lane should stay within the spec-v2 design budget:
  `<=5 minutes` warm and `<=15 minutes` cold per cache schema/version key.
- CI's `spec-stable` job restores `.cache/biomcp-specs/` with the key
  `spec-http-${runner.os}-${biomcp-version}-${spec-cache-schema-version}`.
- `spec-cache-schema-version` is a workflow-local literal so incompatible cache
  layouts stay explicit in review.

## Warm Timing Record

Warm `make spec-pr` measured `56.16s` on beelink on `2026-04-24` after one
untimed warm-up run. `make spec-contracts` measured `386.98s` on beelink on
`2026-05-23` in the code-step worktree, including the release rebuild and the
48 deterministic routine spec assertions; that value is recorded in the
`spec-only` validation-profile comment.

## Per-Section Warm Ceilings

| Section | Lane | Ceiling | Why |
|---|---|---|---|
| `spec/entity/gene.md::All-Section Warm Budget` | quarantined from routine `make spec-pr` by ticket 372 | n/a | This timing-only canary failed twice during routine `make spec-pr` at 45599ms and 43332ms against the former 12000ms ceiling. Per ticket 371's request-contract strategy, restore it only as a deterministic benchmark/ratchet or explicit performance lane, not as a default live-heavy spec blocker. |
