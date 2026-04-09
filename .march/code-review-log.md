# Code Review Log

## Critique

- Read `.march/design-draft.md`, `.march/design-final.md`, `.march/code-log.md`, and `git diff main..HEAD`.
- Ran the design-completeness audit against the final design:
  - Help-query formulation contract mapped to [src/cli/mod.rs](/home/ian/workspace/worktrees/157-document-article-search-query-decomposition-for-agent-skills/src/cli/mod.rs) and [spec/06-article.md](/home/ian/workspace/worktrees/157-document-article-search-query-decomposition-for-agent-skills/spec/06-article.md).
  - `biomcp list article` guidance mapped to [src/cli/list.rs](/home/ian/workspace/worktrees/157-document-article-search-query-decomposition-for-agent-skills/src/cli/list.rs), [spec/01-overview.md](/home/ian/workspace/worktrees/157-document-article-search-query-decomposition-for-agent-skills/spec/01-overview.md), and [spec/06-article.md](/home/ian/workspace/worktrees/157-document-article-search-query-decomposition-for-agent-skills/spec/06-article.md).
  - Top-level compact hint / MCP description contract mapped to [src/cli/list_reference.md](/home/ian/workspace/worktrees/157-document-article-search-query-decomposition-for-agent-skills/src/cli/list_reference.md) and [tests/test_mcp_contract.py](/home/ian/workspace/worktrees/157-document-article-search-query-decomposition-for-agent-skills/tests/test_mcp_contract.py).
  - Public docs alignment mapped to [docs/user-guide/article.md](/home/ian/workspace/worktrees/157-document-article-search-query-decomposition-for-agent-skills/docs/user-guide/article.md), [docs/how-to/find-articles.md](/home/ian/workspace/worktrees/157-document-article-search-query-decomposition-for-agent-skills/docs/how-to/find-articles.md), [docs/reference/article-keyword-search.md](/home/ian/workspace/worktrees/157-document-article-search-query-decomposition-for-agent-skills/docs/reference/article-keyword-search.md), [tests/test_public_skill_docs_contract.py](/home/ian/workspace/worktrees/157-document-article-search-query-decomposition-for-agent-skills/tests/test_public_skill_docs_contract.py), and [tests/test_upstream_planning_analysis_docs.py](/home/ian/workspace/worktrees/157-document-article-search-query-decomposition-for-agent-skills/tests/test_upstream_planning_analysis_docs.py).
  - Confirmed no runtime planner/ranking edits in `src/entities/article.rs`.
- Findings:
  - [docs/how-to/find-articles.md](/home/ian/workspace/worktrees/157-document-article-search-query-decomposition-for-agent-skills/docs/how-to/find-articles.md) still claimed `--source pubmed` gives direct “MeSH/title/abstract” behavior, which the implementation and final design do not promise.
  - Top-level list/MCP proofs were too loose: they checked for the new compact hint but not for the required absence of the full article tutorial on those surfaces.
  - `list article` proofs checked the worked examples but missed the explicit keyword-only and unknown-entity guidance rows required by the design.
  - Docs-contract tests did not prevent future reintroduction of the banned PubMed wording or missing unknown-entity guidance on the keyword reference page.

## Fixes Applied

- Reworded [docs/how-to/find-articles.md](/home/ian/workspace/worktrees/157-document-article-search-query-decomposition-for-agent-skills/docs/how-to/find-articles.md) to describe `--source pubmed` as PubMed-only article search on the compatible filter set, removing the unsupported MeSH/title/abstract claim.
- Strengthened root-list proofs in [src/cli/list.rs](/home/ian/workspace/worktrees/157-document-article-search-query-decomposition-for-agent-skills/src/cli/list.rs) and [spec/01-overview.md](/home/ian/workspace/worktrees/157-document-article-search-query-decomposition-for-agent-skills/spec/01-overview.md) so `biomcp list` must contain the compact article-routing hint and must not contain the full `## Query formulation` tutorial or detailed worked-example text.
- Strengthened article-list proofs in [src/cli/list.rs](/home/ian/workspace/worktrees/157-document-article-search-query-decomposition-for-agent-skills/src/cli/list.rs), [spec/01-overview.md](/home/ian/workspace/worktrees/157-document-article-search-query-decomposition-for-agent-skills/spec/01-overview.md), and [spec/06-article.md](/home/ian/workspace/worktrees/157-document-article-search-query-decomposition-for-agent-skills/spec/06-article.md) so the explicit keyword-only and unknown-entity guidance rows are asserted, not just implied by examples.
- Strengthened contract tests in [tests/test_mcp_contract.py](/home/ian/workspace/worktrees/157-document-article-search-query-decomposition-for-agent-skills/tests/test_mcp_contract.py), [tests/test_public_skill_docs_contract.py](/home/ian/workspace/worktrees/157-document-article-search-query-decomposition-for-agent-skills/tests/test_public_skill_docs_contract.py), and [tests/test_upstream_planning_analysis_docs.py](/home/ian/workspace/worktrees/157-document-article-search-query-decomposition-for-agent-skills/tests/test_upstream_planning_analysis_docs.py) to reject top-level tutorial leakage and MeSH/title/abstract drift.

## Verification

- `cargo fmt --check`
- `cargo test list_root_includes_routing_table_and_quickstart`
- `cargo test list_trial_and_article_include_missing_flags`
- `uv run --extra dev pytest tests/test_public_skill_docs_contract.py::test_public_skill_docs_match_current_cli_contract tests/test_upstream_planning_analysis_docs.py::test_technical_and_ux_docs_match_current_cli_and_workflow_contracts tests/test_mcp_contract.py::test_biomcp_description_matches_list_contract -q --mcp-cmd "./target/release/biomcp serve"`
- `uv run --extra dev pytest spec/01-overview.md spec/06-article.md --mustmatch-lang bash --mustmatch-timeout 60 -v`
- `uv run --extra dev mkdocs build --strict`
- `make check < /dev/null` — passed
- `make spec` — did not stabilize; full-suite runs timed out in existing live smoke specs rather than failing on deterministic assertion mismatches

## Residual Concerns

- `make spec` remains flaky because existing live mustmatch specs exceeded the 60s timeout budget on repeated full-suite runs. Observed failing nodes:
  - `spec/06-article.md::Keyword Anchors Tokenize In JSON Ranking Metadata`
  - `spec/06-article.md::Article Debug Plan`
  - `spec/06-article.md::Sort Behavior`
  - `spec/17-cross-entity-pivots.md::Variant pivots`
  - `spec/18-source-labels.md::Markdown Source Labels`
- Isolated reruns with larger timeout budgets passed, which points to suite/runtime variability rather than a stable regression from this review patch.
- Filed out-of-scope reliability follow-up: [/home/ian/workspace/planning/biomcp/issues/157-live-spec-mustmatch-timeouts.md](/home/ian/workspace/planning/biomcp/issues/157-live-spec-mustmatch-timeouts.md).

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| 1 | stale-doc | no | `docs/how-to/find-articles.md` claimed unsupported PubMed “MeSH/title/abstract” behavior |
| 2 | weak-assertion | no | Top-level `biomcp list` / MCP description proofs did not enforce the “compact hint, not full tutorial” contract |
| 3 | missing-test | no | `list article` proofs did not assert the explicit keyword-only and unknown-entity guidance rows required by the design |
| 4 | weak-assertion | no | Docs-contract tests did not guard against reintroducing banned PubMed wording or missing keyword-reference unknown-entity guidance |
