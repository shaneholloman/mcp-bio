# Code Review Log — Ticket 211

Date: 2026-04-15
Ticket: `211-fix-documentation-drift-readme-sources-changelog-who-root-help-count`

## Phase 1 — Critique

### Design Completeness Audit

I read `.march/design-draft.md`, `.march/design-final.md`, `.march/code-log.md`,
`.march/ticket.md`, and the full `git diff main..HEAD`, then traced every
design-final contract surface to the actual change set.

| Design / acceptance item | Matching code change | Matching proof surface | Result |
|---|---|---|---|
| Landing-page source tables updated in both `README.md` and `docs/index.md` | `README.md`, `docs/index.md` | `tests/test_public_search_all_docs_contract.py` | Implemented |
| 0.8.20 changelog backfills WHO Prequalification + `who sync` | `CHANGELOG.md` | `tests/test_docs_changelog_refresh.py` | Implemented |
| Skill routing guidance mentions `biomcp ema sync` / `biomcp who sync` | `skills/SKILL.md` | `src/cli/skill.rs`, `tests/test_public_skill_docs_contract.py` | Implemented |
| Root CLI help removes the stale source count | `src/cli/types.rs` | `src/cli/tests/facade/help.rs`, `spec/15-mcp-runtime.md` | Implemented |
| MCP description / initialize instructions remove the stale source count | `build.rs`, `src/mcp/shell.rs` | `tests/test_mcp_contract.py`, `tests/test_mcp_http_transport.py` | Implemented |
| Contract-first execution order | `.march/code-log.md` step order and proof notes | N/A | Confirmed |

No design-final item was missing a corresponding code change.

One design-draft detail differed from design-final: the draft placed the
`ema sync` / `who sync` note under `## Output and evidence rules`, while the
final design moved it to `## Routing rules`. I treated `design-final.md` as
the controlling contract and verified the implementation against that version.

### Test-Design Traceability

Every proof-matrix entry had a corresponding test/spec owner:

| Proof-matrix contract | Test / spec found | Result |
|---|---|---|
| Landing-page source tables reflect the expansion batch | `tests/test_public_search_all_docs_contract.py::test_entities_and_sources_tables_list_current_source_expansion_rows` | Present |
| 0.8.20 changelog mentions WHO Prequalification and `who sync` | `tests/test_docs_changelog_refresh.py::test_changelog_has_backfilled_releases_and_release_header` | Present but too weak |
| Embedded skill routing mentions `ema sync` / `who sync` | `src/cli/skill.rs::embedded_skill_overview_is_routing_first_and_points_to_worked_examples`, `tests/test_public_skill_docs_contract.py::test_public_skill_docs_match_current_cli_contract` | Present |
| Root CLI help is count-free | `src/cli/tests/facade/help.rs::top_level_help_uses_count_free_source_phrase`, `spec/15-mcp-runtime.md::Top-Level Discovery` | Present |
| MCP initialize instructions and tool description are count-free | `tests/test_mcp_contract.py`, `tests/test_mcp_http_transport.py` | Present but initialize assertion was too weak |
| Full Rust library gate stays green | `cargo test --lib` | Present |

Blocking proof-quality defects found during traceability:

1. The changelog test only asserted within the full `0.8.20` release block, so
   the WHO backfill could have been moved out of `### New features` and still
   passed.
2. The MCP initialize tests only banned `"15 sources"`, so a regression to
   `"15 biomedical sources"` would not have been caught.

### Other Quality Checks

- Security: no untrusted-input flow, shell execution path, auth boundary, or
  data-exposure behavior changed. The ticket remained doc/help/test-only.
- Duplication: no new production abstraction was introduced. I also checked for
  an existing subsection helper before adding one to the changelog test; none
  existed in this file.
- Implementation quality: the production changes follow adjacent conventions and
  stay within the documented ticket scope. I found no runtime logic defect, no
  stale production docs, and no missing contract owner beyond the weak proofs
  above.

## Phase 2 — Fix Plan

1. Tighten the changelog contract test so the WHO backfill is required inside
   `## 0.8.20` -> `### New features`, not merely somewhere in the release block.
2. Tighten both MCP initialize proof surfaces so they reject both stale count
   phrasings.
3. Re-run the targeted docs/MCP proofs and the repo's `focused` validation
   profile after the review edits.

## Phase 3 — Repair

### Fixes Applied

- Updated `tests/test_docs_changelog_refresh.py`:
  - added `_markdown_subsection_block(...)`
  - scoped the WHO assertions to the `### New features` subsection
  - required `WHO Prequalification`, `--region who`, and `who sync` there
- Updated `tests/test_mcp_contract.py`:
  - initialize instructions must not contain `"15 biomedical sources"`
- Updated `tests/test_mcp_http_transport.py`:
  - Streamable HTTP initialize instructions must not contain
    `"15 biomedical sources"`

### Post-Fix Collateral Damage Scan

After each edit I checked the touched files for:

- dead code or orphaned helpers/imports: none introduced
- shadowed variables: none introduced
- stale assertion text: none introduced; the new wording matches the intended
  contracts
- resource-cleanup conflicts: not applicable; no resource-management code was
  touched

### Validation

Passed:

- `cargo test --lib`
- `cargo build --release --locked`
- `uv run --no-project --with pytest --with pytest-asyncio --with mcp pytest tests/test_public_search_all_docs_contract.py tests/test_docs_changelog_refresh.py tests/test_public_skill_docs_contract.py tests/test_mcp_contract.py tests/test_mcp_http_transport.py -q --mcp-cmd "./target/release/biomcp serve"`
- `PATH="$(pwd)/target/release:$PATH" uv run --no-project --with pytest --with mustmatch pytest spec/15-mcp-runtime.md --mustmatch-lang bash --mustmatch-timeout 60 -k "Top and Level and Discovery" -v`
- focused profile equivalent from `.march/validation-profiles.toml`:
  - `cargo test --lib`
  - `cargo clippy --lib --tests -- -D warnings`

No out-of-scope follow-up issue was filed.

## Residual Concerns

None in the reviewed ticket scope. The only issues found were proof weaknesses,
and those are now repaired.

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| 1 | weak-assertion | no | `tests/test_docs_changelog_refresh.py` only checked the full `0.8.20` block, so the WHO backfill could move out of `### New features` without failing proof |
| 2 | weak-assertion | no | The MCP initialize tests only rejected `"15 sources"`, so a regression to `"15 biomedical sources"` would have escaped detection |
