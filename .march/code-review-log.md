# Code Review Log - Ticket 209

## Critique

Reviewed `.march/ticket.md`, `.march/design-draft.md`, `.march/design-final.md`,
`.march/code-log.md`, the full `git diff main..HEAD`, and the two ticket
commits:

- `d7d13696` `Define disease/gene discovery contract`
- `091521f5` `Surface disease and gene section follow-ups`

Re-ran the relevant local gates during review:

- `cargo test --lib && cargo clippy --lib --tests -- -D warnings`
- `BIOMCP_BIN="$PWD/target/debug/biomcp" XDG_CACHE_HOME="$PWD/.cache" PATH="$PWD/target/debug:$PATH" RUST_LOG=error .venv/bin/pytest spec/02-gene.md spec/07-disease.md spec/11-evidence-urls.md spec/21-cross-entity-see-also.md --mustmatch-lang bash --mustmatch-timeout 60 -v`

### Design Completeness Audit

All in-scope design items have matching implementation or proof after repair:

- Shared source of truth for disease/gene next commands:
  - `src/render/markdown/sections.rs` adds `visible_section_commands(...)`,
    `disease_next_commands(...)`, and `gene_next_commands(...)`.
  - `src/render/markdown/mod.rs` exports the new helpers.
  - `src/cli/disease/dispatch.rs` and `src/cli/gene/dispatch.rs` now feed JSON
    `_meta.next_commands` from the shared helpers instead of `related_*()` only.
- Discovery order and visible limits:
  - `src/render/markdown/sections.rs` adds
    `DISEASE_DISCOVERY_SECTION_NAMES`, `GENE_DISCOVERY_SECTION_NAMES`, and
    entity-specific limits of 5 for disease and 4 for gene.
- Requested-section filtering and no-self-loop behavior:
  - the shared helper path still relies on `sections_for(...)` rather than
    dispatch-local filtering.
  - regression proof exists in
    `src/cli/tests/next_commands_json_property/disease_trial.rs` and
    `src/cli/tests/next_commands_json_property/gene_article.rs`.
- Missing survival description:
  - `src/render/markdown/sections.rs` now maps
    `("disease", "survival")` to `SEER Explorer cancer survival rates`.
- Docs and contract surfaces:
  - `docs/user-guide/disease.md`
  - `docs/user-guide/gene.md`
  - `docs/reference/data-sources.md`
  - `spec/07-disease.md`
  - `spec/02-gene.md`
  - `spec/11-evidence-urls.md`
- Execution order:
  - docs/spec/tests landed in `d7d13696`
  - runtime wiring landed afterward in `091521f5`
  - this matches both `.march/code-log.md` and the actual commit history.

Acceptance criteria 7 and 8 intentionally rely on unchanged runtime behavior:

- disease `survival` staying in `all` is still implemented by
  `src/entities/disease/get.rs`
- disease/gene `funding` staying opt-in is still enforced by
  `src/entities/disease/get.rs` and `src/entities/gene.rs`

The diff correctly adds proof for those invariants rather than rewriting the
runtime.

No unmatched `Needs change` markers were present in `design-final.md`.

### Test-Design Traceability

Proof-matrix coverage after repair:

- Disease base-card `More:` order and survival description:
  - `spec/07-disease.md::Getting Disease Details`
  - `src/render/markdown/sections/tests.rs::sections_disease_base_card_surfaces_survival_and_funding_before_all`
- Gene base-card `More:` order and preserved trio:
  - `spec/02-gene.md::Getting Gene Details`
  - `spec/21-cross-entity-see-also.md::Gene More Ordering`
  - `src/render/markdown/sections/tests.rs::sections_gene_base_card_surfaces_funding_as_fourth_command`
- Disease/gene JSON `_meta.next_commands` ordering:
  - `spec/07-disease.md::Getting Disease Details`
  - `spec/02-gene.md::Getting Gene Details`
  - property coverage in
    `src/cli/tests/next_commands_json_property/disease_trial.rs` and
    `src/cli/tests/next_commands_json_property/gene_article.rs`
- Shared JSON contract surface for the new disease/gene follow-ups:
  - `spec/11-evidence-urls.md::JSON Metadata for Repaired Gaps`
- Requested section omission:
  - `disease_json_next_commands_omit_requested_section_follow_up`
  - `gene_json_next_commands_omit_requested_section_follow_up`
- Parser validity:
  - `src/cli/tests/next_commands_validity.rs`
- Disease `survival` remains in `all`:
  - `spec/07-disease.md::Disease Funding Stays Opt-In`
- Disease/gene `funding` stay opt-in:
  - `spec/07-disease.md::Disease Funding Stays Opt-In`
  - `spec/02-gene.md::Gene Funding Stays Opt-In`
  - unchanged parser proof in `src/entities/gene.rs::parse_sections_all_keeps_disgenet_opt_in`
- Cross-entity follow-ups remain available:
  - `spec/21-cross-entity-see-also.md`

The exact function names proposed in `design-final.md` were not used verbatim,
but every required scenario now has matching proof by name or scenario
description.

Issues found during critique:

1. `spec/11-evidence-urls.md` placed the new disease/gene shared JSON-contract
   assertions inside the OpenFDA-gated `JSON Metadata Contract` heading, so the
   proof-matrix item existed in the diff but was skipped in the default local
   spec lane.
2. `spec/02-gene.md::Gene Funding Stays Opt-In` bundled both markdown and JSON
   `get gene ERBB2 all` calls into one 60-second bash item. The contract was
   correct, but the proof was unstable and timed out locally.

### Implementation Quality Review

- Security:
  - no new shell-injection, path-traversal, auth, or secret-handling defects
    found
  - the new next-command builders still rely on `quote_arg(...)` before
    serializing executable follow-ups
- Duplication:
  - no equivalent disease/gene shared next-command helper already existed in the
    repo; the new render-layer helper is the right place to centralize this
    logic
- Runtime quality:
  - no runtime logic defects were found in the disease/gene implementation
  - the renderer and JSON paths now share the same section-command source as
    designed

## Fix Plan

Repair the two proof-surface defects directly:

1. Move the disease/gene shared JSON-contract proof onto an ungated, collected
   `spec/11` heading so the design-matrix assertion executes without
   `OPENFDA_API_KEY`.
2. Make the gene opt-in proof stable under the 60-second spec budget by keeping
   the markdown assertion in `spec/02-gene.md` and moving the JSON opt-in check
   to the collected `spec/11` repaired-gaps heading.

## Repair

Applied the fixes:

- `spec/11-evidence-urls.md`
  - added disease/gene `_meta.next_commands` order assertions to the collected,
    ungated `JSON Metadata for Repaired Gaps` heading
  - added the JSON proof that `biomcp --json get gene ERBB2 all` keeps
    `funding` absent there as well
- `spec/02-gene.md`
  - reduced `Gene Funding Stays Opt-In` to the markdown proof only, removing
    the deterministic timeout source from the single 60-second bash item
- filed an adjacent performance issue:
  - `/home/ian/workspace/planning/biomcp/issues/209-gene-all-runtime-budget.md`

### Post-Fix Collateral Scan

After each spec repair, checked the surrounding surface for:

- dead or uncollected proof: the new disease/gene assertions were moved onto a
  collected `spec/11` heading
- stale wording: the repaired-gaps prose still matches what the blocks now
  assert
- orphaned checks: the gene JSON opt-in assertion was preserved in `spec/11`
  after it was removed from `spec/02`

No collateral issues were introduced.

## Verification

- Focused Rust profile before repair:
  - `cargo test --lib && cargo clippy --lib --tests -- -D warnings`
  - result: passed
- Targeted repaired proofs:
  - `pytest spec/02-gene.md -k 'Gene and Funding and Stays and Opt and In' ...`
  - result: `1 passed`
  - `pytest spec/11-evidence-urls.md -k 'JSON and Metadata and Repaired' ...`
  - result: `3 passed`
- Full changed spec surface after repair:
  - `pytest spec/02-gene.md spec/07-disease.md spec/11-evidence-urls.md spec/21-cross-entity-see-also.md ...`
  - result: `76 passed, 2 skipped`

## Residual Concerns

- The ticket implementation itself is sound after review.
- `spec/11-evidence-urls.md::JSON Metadata Contract` still stays key-gated
  because it exercises OpenFDA-backed output; the disease/gene assertions now
  run separately in an ungated heading.
- The underlying `get gene <symbol> all` runtime is still slow enough to be a
  future live-spec risk; that was filed separately under the biomcp issues
  directory.

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| 1 | weak-assertion | no | The design proof for shared disease/gene JSON follow-up commands lived in `spec/11-evidence-urls.md::JSON Metadata Contract`, which is skipped without `OPENFDA_API_KEY`, so the proof did not execute in the default local lane. |
| 2 | weak-assertion | no | `spec/02-gene.md::Gene Funding Stays Opt-In` combined two slow `get gene ERBB2 all` probes into one 60-second bash item and timed out during the review gate rerun. |
