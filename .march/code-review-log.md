# Code Review Log — Ticket 174

## Critique

### Design Completeness Audit

I checked the acceptance criteria, every proof-matrix row, and the file-level
plan in `.march/design-final.md` against `git diff main..HEAD` and the repaired
tree.

- The core NIH Reporter source/entity/provenance/health/docs surfaces were all
  present in the implementation.
- Acceptance criterion 5 was not fully implemented in markdown: `funding`
  remained opt-in in entity parsing, but the renderer still showed the funding
  block for `get gene ... all` and `get disease ... all`.
- The same markdown gate also leaked `disgenet` on `all`, violating the design
  note that heavier opt-in sections stay out of `all`.
- The funding summary copy and placement did not match the contract. The
  design required `Showing top <N> unique grants from <matching_project_years>
  matching NIH project-year records across FY<start>-FY<end>.` beneath the
  table, but the implementation rendered different copy above the table.
- The disease funding docs/help text drifted from the implemented contract.
  Runtime behavior preserves the requested free-text disease phrase and only
  falls back to the canonical disease name for identifier lookups, but the docs
  said the funding query used the normalized/canonical disease name.
- `CHANGELOG.md` was still missing the user-visible NIH Reporter entry required
  by the design file-level plan.
- The design required correct NIH fiscal-year handling around the October 1
  rollover. The source helper used `now_utc().date()` directly, which can pick
  the wrong fiscal window for local operators near the boundary.

### Test-Design Traceability

- The proof-matrix rows for NIH Reporter request shape, fiscal-year logic,
  de-duplication, PI fallback, provenance, health inventory, rate-limit
  registration, docs inventory sync, and operator smoke all had matching code
  or tests in the changed files.
- The proof-matrix row `Funding markdown handles rows and notes` existed, but
  its assertions were too weak to verify the contract text or placement of the
  funding summary line. That let a copy/order regression pass.
- No executable spec asserted the outside-in requirement that `funding` stays
  out of `get gene ... all` or `get disease ... all`. This was a blocking gap,
  because the live renderer bug shipped despite the baseline spec suite being
  green.

## Fix Plan

- Tighten markdown section gating so `funding` and `disgenet` only render when
  explicitly requested.
- Replace the funding summary helper with contract-exact wording and render it
  after the funding table.
- Strengthen the gene and disease funding proofs so they assert the exact
  summary contract and add outside-in coverage proving `all` excludes funding.
- Update disease help/docs/reference text to reflect free-text disease phrases
  plus canonical fallback for identifier lookups.
- Add the missing `CHANGELOG.md` entry.
- Correct NIH fiscal-year date selection to prefer local time with UTC fallback,
  then keep the surrounding code free of dead branches and stale imports.

## Repair

Applied the following fixes directly in the worktree:

- Updated [src/render/markdown.rs](/home/ian/workspace/worktrees/174-add-nih-reporter-funding-data-integration/src/render/markdown.rs) so gene and disease funding/disgenet blocks render only on explicit section requests, and added a contract-exact funding summary helper.
- Moved the funding summary below the table in
  [templates/gene.md.j2](/home/ian/workspace/worktrees/174-add-nih-reporter-funding-data-integration/templates/gene.md.j2) and
  [templates/disease.md.j2](/home/ian/workspace/worktrees/174-add-nih-reporter-funding-data-integration/templates/disease.md.j2).
- Strengthened markdown tests in
  [src/render/markdown.rs](/home/ian/workspace/worktrees/174-add-nih-reporter-funding-data-integration/src/render/markdown.rs)
  to assert the exact funding summary string and to prove `all` hides opt-in
  funding/disgenet sections.
- Added outside-in executable specs in
  [spec/02-gene.md](/home/ian/workspace/worktrees/174-add-nih-reporter-funding-data-integration/spec/02-gene.md) and
  [spec/07-disease.md](/home/ian/workspace/worktrees/174-add-nih-reporter-funding-data-integration/spec/07-disease.md)
  proving `funding` stays out of `all`, and tightened the funding summary
  assertions from a loose FY substring to the full contract pattern.
- Corrected disease funding wording in
  [src/cli/list.rs](/home/ian/workspace/worktrees/174-add-nih-reporter-funding-data-integration/src/cli/list.rs),
  [docs/user-guide/disease.md](/home/ian/workspace/worktrees/174-add-nih-reporter-funding-data-integration/docs/user-guide/disease.md),
  and
  [docs/reference/data-sources.md](/home/ian/workspace/worktrees/174-add-nih-reporter-funding-data-integration/docs/reference/data-sources.md).
- Added the missing NIH Reporter changelog entry in
  [CHANGELOG.md](/home/ian/workspace/worktrees/174-add-nih-reporter-funding-data-integration/CHANGELOG.md).
- Updated
  [src/sources/nih_reporter.rs](/home/ian/workspace/worktrees/174-add-nih-reporter-funding-data-integration/src/sources/nih_reporter.rs)
  to prefer the local date for the fiscal-year window with a UTC fallback, and
  enabled the required `time` crate feature in
  [Cargo.toml](/home/ian/workspace/worktrees/174-add-nih-reporter-funding-data-integration/Cargo.toml).

### Verification

- `cargo test --quiet funding -- --nocapture` passed.
- `cargo test --quiet list_disease -- --nocapture` passed.
- `make check < /dev/null` passed.
- `cargo build --release --bins` passed.
- `make spec` completed with all repaired NIH Reporter funding specs passing on
  the rebuilt release binary. The only remaining failure was the pre-existing
  live GWAS positional spec outage in `spec/12-search-positionals.md`.

### Residual Concerns

- `make spec` is still vulnerable to unrelated live-source outages. I filed
  [174-live-gwas-positional-spec-upstream-flake.md](/home/ian/workspace/planning/biomcp/issues/174-live-gwas-positional-spec-upstream-flake.md)
  because the existing GWAS positional proof currently fails the full suite when
  GWAS Catalog is unavailable.

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| 1 | validation-gap | no | Markdown rendering used `include_all` for opt-in funding/disgenet sections, so `get gene/disease ... all` leaked behavior the contract explicitly excluded. |
| 2 | weak-assertion | no | Funding markdown proofs only matched a loose FY substring, so summary wording and placement regressed without failing tests. |
| 3 | missing-test | no | Design required outside-in proof that `funding` stays out of `all`, but no executable spec covered that behavior. |
| 4 | stale-doc | no | Disease funding help/reference docs described canonical-name querying instead of the implemented requested-phrase plus identifier-fallback behavior. |
| 5 | stale-doc | no | `CHANGELOG.md` lacked the design-required NIH Reporter user-facing entry. |
| 6 | error-classification | no | NIH fiscal-year window selection used a UTC calendar date directly, which can classify the active NIH fiscal year incorrectly around the local Sep/Oct boundary. |
