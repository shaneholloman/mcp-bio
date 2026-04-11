# Code Review Log

## Critique

- Read `.march/ticket.md`, `.march/design-draft.md`, `.march/design-final.md`, `.march/code-log.md`, and the full `git diff main..HEAD`.
- Re-ran the relevant local gates independently during review: `make spec`, `make check`, `cargo build --release`, and `uv run mkdocs build --strict`.
- Design completeness audit:
  - Mapped the disease top-gene routing change to `src/render/markdown.rs`, `docs/user-guide/disease.md`, `spec/21-cross-entity-see-also.md`, and the disease JSON/unit tests in `src/cli/mod.rs` and `src/render/markdown.rs`.
  - Mapped the gene ClinGen-trial routing change to `src/render/markdown.rs`, `docs/user-guide/gene.md`, `spec/21-cross-entity-see-also.md`, and the matching JSON/unit tests.
  - Mapped the phenotype markdown follow-up change to `src/render/markdown.rs`, `templates/phenotype_search.md.j2`, `docs/user-guide/phenotype.md`, and `spec/23-phenotype.md`.
  - Confirmed the intentional no-op boundary for `search phenotype --json`: the CLI still routes that path through generic `search_json(...)` rather than entity `_meta.next_commands`.
  - Mapped the variant significance-aware routing and central description-table changes to `src/render/markdown.rs`, `docs/user-guide/variant.md`, `spec/21-cross-entity-see-also.md`, and the existing variant JSON/unit proofs.
- Test-design traceability audit:
  - Disease top-gene follow-up and JSON mirroring were covered.
  - Gene ClinGen trial follow-up and JSON mirroring were covered.
  - Phenotype markdown follow-up was covered outside-in, but the Rust-side `phenotype_search_json_contract_unchanged` proof required by the design was missing.
  - Variant routing had pathogenic and VUS behavior tests, but `next_commands_validity` only exercised one VUS literature command shape even though the renderer can emit gene+disease, gene-only, disease-only, and keyword-only variants.
  - Keyword-only VUS literature follow-ups were emitted without the central descriptive suffix because `related_command_description()` did not recognize that command shape.

## Fixes Applied

- Updated `src/cli/mod.rs`:
  - added `phenotype_search_json_contract_unchanged` to prove `search phenotype --json` keeps the generic search-response shape and does not grow `_meta`
  - extended `variant_next_commands_parse` to validate all VUS literature command shapes the renderer can emit
- Updated `src/render/markdown.rs`:
  - broadened `is_variant_literature_follow_up_command()` so keyword-only VUS article searches still map through the central description table while excluding unrelated `-q` and `--type` article commands
  - added `related_variant_vus_keyword_only_follow_up_keeps_description` as regression coverage for the previously bare keyword-only route
- Corrected one new test fixture after the first focused rerun:
  - the keyword-only VUS regression fixture needed an explicit empty `gene` field because `Variant` requires that field during deserialization

## Post-Fix Collateral Scan

- After the description-matcher change, rechecked the surrounding article-command branches for overmatching. The matcher now excludes `-q` and `--type`, so it does not steal the existing trial-results or review-literature descriptions.
- After adding the new tests, rechecked the touched modules for dead code, unused imports, stale error messages, cleanup conflicts, and shadowing. No new dead code or cleanup issues were introduced.
- The only collateral issue encountered was the missing `gene` field in the new test fixture; it was fixed immediately and the focused rerun stayed green.

## Verification

- Focused Rust proofs:
  - `cargo test next_commands_validity -- --nocapture`
  - `cargo test phenotype_search_json_contract_unchanged -- --nocapture`
  - `cargo test related_variant_vus -- --nocapture`
- Full gates:
  - `make check` — passed
  - `cargo build --release` — passed
  - `uv run mkdocs build --strict` — passed; only existing navigation / MkDocs compatibility warnings were emitted
  - `make spec` — passed on rerun after one transient Semantic Scholar rate-limit failure in `spec/09-search-all.md::JSON Search All Preserves Article Metadata`; the isolated node passed immediately on retry and the subsequent full rerun passed cleanly

## Residual Concerns

- No blocking defects remain in the ticket scope.
- One live-backed `search all` article-metadata spec node proved transiently rate-limit sensitive during verification, but the final full `make spec` rerun passed without code changes.
- No out-of-scope issues were filed from this review pass.

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| 1 | missing-test | yes | The design required a Rust-side proof that `search phenotype --json` keeps the generic search response shape, but only executable-spec coverage existed |
| 2 | missing-test | yes | `next_commands_validity` covered only one VUS literature command shape even though the renderer can emit gene+disease, gene-only, disease-only, and keyword-only forms |
| 3 | description-gap | no | Keyword-only VUS literature follow-ups rendered as bare commands because the central description matcher did not recognize the `search article -k ... --limit 5` shape |
