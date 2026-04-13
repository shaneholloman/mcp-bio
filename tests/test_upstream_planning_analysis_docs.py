from __future__ import annotations

import os
import re
import tomllib
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_PLANNING_ROOT = REPO_ROOT / "tests" / "fixtures" / "planning" / "biomcp"


def _read_repo(path: str) -> str:
    return (REPO_ROOT / path).read_text(encoding="utf-8")


def _current_release_tag_example() -> str:
    cargo = tomllib.loads(_read_repo("Cargo.toml"))
    return f"v{cargo['package']['version']}"


def _planning_root() -> Path:
    return Path(os.environ.get("BIOMCP_PLANNING_ROOT", DEFAULT_PLANNING_ROOT))


def _read_planning(path: str) -> str:
    return (_planning_root() / path).read_text(encoding="utf-8")


def _normalize_ws(text: str) -> str:
    return re.sub(r"\s+", " ", text).strip()


def _markdown_section(text: str, heading: str, level: int = 2) -> str:
    marker = "#" * level
    match = re.search(
        rf"^{re.escape(marker)} {re.escape(heading)}\n(.*?)(?=^{re.escape(marker)} |\Z)",
        text,
        flags=re.MULTILINE | re.DOTALL,
    )
    assert match is not None, f"missing section {marker} {heading}"
    return match.group(1)


def _workflow_job_block(workflow: str, job_name: str) -> str:
    match = re.search(
        rf"^  {re.escape(job_name)}:\n(.*?)(?=^  [A-Za-z0-9_-]+:\n|\Z)",
        workflow,
        flags=re.MULTILINE | re.DOTALL,
    )
    assert match is not None, f"missing workflow job {job_name}"
    return match.group(1)


def _workflow_run_steps(job_block: str) -> list[str]:
    return re.findall(r"^\s+- run: (.+)$", job_block, flags=re.MULTILINE)


def test_planning_contract_uses_repo_fixture_fallback_by_default(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("BIOMCP_PLANNING_ROOT", raising=False)

    assert _planning_root() == DEFAULT_PLANNING_ROOT
    assert "# BioMCP Strategy" in _read_planning("strategy.md")


def test_planning_contract_reads_explicit_env_override(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    monkeypatch.setenv("BIOMCP_PLANNING_ROOT", str(tmp_path))
    (tmp_path / "strategy.md").write_text("# override strategy\n", encoding="utf-8")

    assert _planning_root() == tmp_path
    assert _read_planning("strategy.md") == "# override strategy\n"


def test_planning_contract_bad_override_fails_loudly(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    missing_root = tmp_path / "missing-planning-root"
    monkeypatch.setenv("BIOMCP_PLANNING_ROOT", str(missing_root))

    with pytest.raises(FileNotFoundError):
        _read_planning("strategy.md")


def test_strategy_and_frontier_capture_upstream_planning_contract() -> None:
    strategy = _read_planning("strategy.md")
    frontier = _read_planning("frontier.md")

    assert "# BioMCP Strategy" in strategy
    assert "Rust core, Python packaging" in strategy
    assert "Rate limiting is process-local" in strategy
    assert "G002" in strategy
    assert "G003" in strategy
    assert len(strategy.splitlines()) <= 80

    assert "# BioMCP Frontier" in frontier
    assert "## G002" in frontier
    assert "## G003" in frontier
    assert "architecture/functional/overview.md" in frontier
    assert "architecture/technical/overview.md" in frontier
    assert "architecture/ux/cli-reference.md" in frontier
    assert "Harvest Guidance" in frontier


def test_functional_overview_preserves_readme_surface_and_study_family() -> None:
    functional = _read_repo("architecture/functional/overview.md")

    assert "# BioMCP Functional Overview" in functional
    assert "## Entity Surface" in functional
    for entity in (
        "| gene |",
        "| variant |",
        "| article |",
        "| trial |",
        "| drug |",
        "| disease |",
        "| pathway |",
        "| protein |",
        "| adverse-event |",
        "| pgx |",
        "| gwas |",
        "| phenotype |",
    ):
        assert entity in functional

    assert "## Study Command Family" in functional
    assert "`study` is a separate local analytics surface" in functional
    assert (
        "`biomcp study list|download|filter|query|co-occurrence|cohort|survival|compare`"
        in functional
    )
    assert "BioMCP ships an embedded agent guide plus worked examples" in functional
    assert "`biomcp skill` shows the BioMCP agent guide" in functional
    assert "`biomcp skill install <dir>` installs that guide" in functional
    assert "`biomcp skill list` shows embedded worked examples" in functional
    assert "`biomcp skill <name>` opens an embedded worked example" in functional
    assert "`biomcp://skill/<slug>`" in functional
    assert "discover <query>" in functional
    assert "free-text concept resolution into typed follow-up commands" in functional
    assert "search all [slot filters]" in functional
    assert "biomcp search all --gene BRAF --disease melanoma" in functional
    assert "biomcp search all BRAF" in functional


def test_technical_and_ux_docs_match_current_cli_and_workflow_contracts() -> None:
    technical = _read_repo("architecture/technical/overview.md")
    ux = _read_repo("architecture/ux/cli-reference.md")
    article_guide = _read_repo("docs/user-guide/article.md")
    find_articles = _read_repo("docs/how-to/find-articles.md")
    article_keyword_reference = _read_repo("docs/reference/article-keyword-search.md")
    data_sources = _read_repo("docs/reference/data-sources.md")
    cli_list_reference = _read_repo("src/cli/list_reference.md")
    article_mod = _read_repo("src/entities/article/mod.rs")
    article_planner = _read_repo("src/entities/article/planner.rs")
    article_graph = _read_repo("src/entities/article/graph.rs")
    article_usage = _read_repo("tests/article_usage_stderr.rs")
    release_workflow = _read_repo(".github/workflows/release.yml")
    install_script = _read_repo("install.sh")
    technical_ws = _normalize_ws(technical)
    ux_ws = _normalize_ws(ux)
    example_tag = _current_release_tag_example()
    article_guide_ws = _normalize_ws(article_guide)
    find_articles_ws = _normalize_ws(find_articles)
    article_keyword_reference_ws = _normalize_ws(article_keyword_reference)
    data_sources_ws = _normalize_ws(data_sources)
    cli_list_reference_ws = _normalize_ws(cli_list_reference)
    article_validation_section = _normalize_ws(
        _markdown_section(technical, "Article Federation and Front-Door Validation")
    )
    release_pipeline_section = _normalize_ws(_markdown_section(technical, "Release Pipeline"))

    assert "## Article Federation and Front-Door Validation" in technical
    assert (
        "`search article --source all` plans PubTator3 plus Europe PMC plus PubMed"
        in article_validation_section
    )
    assert "Keyword-bearing queries also add LitSense2" in article_validation_section
    assert "Semantic Scholar remains an optional compatible search leg" in (
        article_validation_section
    )
    assert (
        "Strict Europe PMC-only filters such as `--open-access` and `--type` "
        "disable the federated planner"
    ) in article_validation_section
    assert (
        "`--source pubtator` with strict Europe PMC-only filters is rejected at the front door"
        in article_validation_section
    )
    assert (
        "`--source` remains `all|pubtator|europepmc|pubmed|litsense2` in v1"
        in article_validation_section
    )
    assert (
        "deduplicate across PMID, PMCID, and DOI where possible, then re-rank locally"
        in article_validation_section
    )
    assert (
        "`search article` rejects missing filters, invalid date values, inverted date ranges, "
        "and unsupported `--type` values before backend calls"
        in article_validation_section
    )
    assert (
        "`get article` accepts PMID, PMCID, and DOI only and rejects unsupported identifiers "
        "such as publisher PIIs with a clean `InvalidArgument`"
        in article_validation_section
    )
    assert (
        "Semantic Scholar helper commands accept PMID, PMCID, DOI, arXiv, and Semantic Scholar paper IDs"
        in article_validation_section
    )
    assert (
        "Keyword-bearing article queries default to `hybrid`, while entity-only article "
        "queries default to `lexical`"
        in technical_ws
    )
    assert (
        "`semantic` sorts the LitSense2-derived semantic signal descending and falls "
        "back to the lexical comparator"
        in technical_ws
    )
    assert "with `semantic=0` when LitSense2 did not match" in technical_ws
    assert (
        "0.4*semantic + 0.3*lexical + 0.2*citations + 0.1*position"
        in technical_ws
    )
    assert (
        "Semantic Scholar article helpers are explicitly limited to 1 request/sec per process and are not part of article search fan-out"
        not in technical
    )
    assert (
        "Article search fans out to PubTator3, Europe PMC, and PubMed by default when the filter set is compatible."
        in article_guide_ws
    )
    assert "MeSH/title/abstract" not in article_guide_ws
    assert (
        "When a non-empty keyword is present, BioMCP also adds LitSense2 to the federated route."
        in article_guide_ws
    )
    assert "LitSense2-derived semantic signal" in article_guide_ws
    assert "Rows without LitSense2 provenance contribute `ranking.semantic_score = 0`" in article_guide_ws
    assert "## Query formulation" in article_guide
    assert (
        "Put a known gene, disease, or drug in `-g/--gene`, `-d/--disease`, or `--drug`."
        in article_guide
    )
    assert (
        'biomcp search article --drug amiodarone -k "photosensitivity mechanism" --limit 5'
        in article_guide
    )
    assert "MeSH/title/abstract" not in find_articles_ws
    assert "LitSense2-derived semantic signal" in find_articles_ws
    assert "semantic=0" in find_articles_ws
    assert "Do not guess `-g`, `-d`, or `--drug` when the question is trying to identify the entity itself." in find_articles_ws
    assert 'biomcp search article -k "TCGA mutation analysis dataset" --type review --limit 5' in find_articles_ws
    assert "MeSH/title/abstract" not in article_keyword_reference_ws
    assert (
        "On the default `--source all` route, adding `-k/--keyword` also brings LitSense2 into compatible federated searches and makes the default relevance mode `hybrid`."
        in article_keyword_reference_ws
    )
    assert "LitSense2-derived semantic signal" in article_keyword_reference_ws
    assert "semantic=0" in article_keyword_reference_ws
    assert "do not guess a disease or drug name just to fill `-d` or `--drug`" in article_keyword_reference_ws
    assert "Turn a literature question into article filters" in cli_list_reference_ws
    assert (
        "known gene/disease/drug anchors go in `-g/-d/--drug`; free-text concepts go in `-k`"
        in cli_list_reference_ws
    )
    assert "LitSense2-derived semantic signal" in cli_list_reference_ws
    assert "semantic=0" in cli_list_reference_ws
    assert "PubTator3 + Europe PMC + PubMed + LitSense2 + optional Semantic Scholar" in (
        data_sources_ws
    )
    assert (
        "PubTator3 + Europe PMC + PubMed for federated search, with LitSense2 added for "
        "keyword-bearing queries and an optional Semantic Scholar leg"
        in data_sources_ws
    )
    assert (
        "fn has_strict_europepmc_filters(filters: &ArticleSearchFilters) -> bool {"
        in article_planner
    )
    assert "fn plan_backends(" in article_planner
    assert "pub(crate) fn semantic_scholar_search_enabled(" in article_planner
    assert "--source pubtator does not support --type." in article_planner
    assert "--source pubtator does not support --open-access." in article_planner
    assert "Unsupported identifier format for Semantic Scholar article helpers:" in article_graph
    assert (
        "Unsupported identifier format. BioMCP resolves PMID (digits only"
        in article_mod
    )
    assert "invalid_article_type_is_clean_usage_error_before_pubtator_route" in article_usage
    assert "missing_article_filters_is_clean_usage_error" in article_usage

    assert "CI (`.github/workflows/ci.yml`) runs five parallel jobs" in technical
    assert (
        "`check` (`cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`)"
        in technical_ws
    )
    assert "`make check` is the required local ticket gate" in technical
    assert "that means `lint`, `test`, and `check-quality-ratchet`" in technical
    assert "`version-sync` (`bash scripts/check-version-sync.sh`)" in technical
    assert "`climb-hygiene` (`bash scripts/check-no-climb-tracked.sh`)" in technical
    assert (
        '`contracts` (`cargo build --release --locked`, `uv sync --extra dev`, '
        '`uv run pytest tests/ -v --mcp-cmd "./target/release/biomcp serve"`, '
        '`uv run mkdocs build --strict`)'
        in technical_ws
    )
    assert "`spec-stable` (`cargo build --release --locked`, then `make spec-pr`)" in technical_ws
    assert (
        "PR CI runs `make spec-pr` via the `spec-stable` job in `.github/workflows/ci.yml`"
        in technical_ws
    )
    assert (
        "Volatile live-network headings run separately in `.github/workflows/spec-smoke.yml`"
        in technical_ws
    )
    assert "Contract smoke checks run in `.github/workflows/contracts.yml`" in technical_ws
    assert "The semver tag is the canonical release/version authority." in release_pipeline_section
    assert (
        "PR CI enforces version parity before release via the `version-sync` job and "
        "`scripts/check-version-sync.sh`"
        in release_pipeline_section
    )
    assert (
        "The release workflow builds binaries, publishes PyPI wheels, and deploys docs "
        "from the tagged source"
        in release_pipeline_section
    )
    assert (
        "`install.sh` resolves the latest release with platform assets, not the latest merge to `main`"
        in release_pipeline_section
    )
    assert (
        "The existing `### Post-tag public proof` block is the live verification step for "
        "tag-to-binary and tag-to-docs parity"
        in release_pipeline_section
    )
    assert f'tag="${{BIOMCP_TAG:?set BIOMCP_TAG to the published release tag, e.g. {example_tag}}}"' in technical
    assert 'version="${tag#v}"' in technical
    assert "`workflow_dispatch` can replay a specified tag" in release_pipeline_section
    assert "Release validation runs the Rust checks again" in technical
    assert "workflow_dispatch:" in release_workflow
    assert "inputs:" in release_workflow
    assert "tag:" in release_workflow
    assert "deploy-docs:" in release_workflow
    assert "uv run mkdocs gh-deploy --force" in release_workflow
    assert 'DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/${ASSET}"' in install_script
    assert "Resolved latest release with assets" in install_script
    assert "Streamable HTTP" in technical
    assert "`/mcp`" in technical
    assert "`/health`" in technical
    assert "`/readyz`" in technical
    assert "connection line and `Command:` markers" in technical
    assert "through the remote" in technical
    assert "`biomcp` tool" in technical
    assert "remote `shell`" not in technical

    assert "`search all` Contract" in ux
    assert "biomcp discover <query>" in ux
    assert "## See Also and Next Commands" in ux
    assert "`_meta.next_commands`" in ux
    assert "`discover_try_line()`" in ux
    assert "degrade by omission, not by emitting dead commands" in ux
    assert (
        "runtime-generated per-record `next_commands` remain outside the static list contract"
        in ux_ws
    )
    assert "typed slots first" in ux
    assert "biomcp search all --gene BRAF --disease melanoma" in ux
    assert 'biomcp search all --keyword "checkpoint inhibitor"' in ux
    assert "biomcp search all BRAF" in ux
    assert "positional alias" in ux
    assert "biomcp skill                  → show the embedded BioMCP agent guide" in ux
    assert "biomcp skill list             → list embedded worked examples" in ux
    assert "biomcp cache path             → print the managed HTTP cache path (plain text; ignores `--json`)" in ux
    assert "biomcp cache stats            → show HTTP cache statistics (JSON supported)" in ux
    assert "biomcp cache clean            → remove orphan blobs and optionally age- or size-evict the HTTP cache (JSON supported)" in ux
    assert "biomcp cache clear [--yes]    → destructively wipe the managed HTTP cache tree (JSON success; TTY or `--yes` required)" in ux
    assert "Overview: `biomcp skill`" in ux
    assert "List: `biomcp skill list`" in ux
    assert "Open: `biomcp skill 01` or `biomcp skill article-follow-up`" in ux
    assert "biomcp://skill/<slug>" in ux
    assert "biomcp serve-http            → run the MCP Streamable HTTP server at `/mcp`" in ux
    assert "biomcp serve-sse             → removed compatibility command; use `biomcp serve-http`" not in ux
    assert (
        "Compatibility note: `biomcp serve-sse` remains available only as a hidden "
        "compatibility command that points users to `biomcp serve-http`."
        in ux
    )
    assert (
        "JSON is the default script contract for query commands, with a documented "
        "plain-text exception for `biomcp cache path`. `biomcp cache stats`, "
        "`biomcp cache clean`, and `biomcp cache clear` support `--json` on "
        "success, while `cache clear` still refuses non-TTY destructive runs "
        "unless `--yes` is present. The cache family remains CLI-only because "
        "revealing workstation-local filesystem paths over MCP would cross the "
        "runtime security boundary."
        in ux_ws
    )


def test_chart_rendering_architecture_doc_matches_repo_contract() -> None:
    technical = _read_repo("architecture/technical/overview.md")
    chart_section = _normalize_ws(_markdown_section(technical, "Chart Rendering"))

    assert "## Chart Rendering" in technical
    assert "`biomcp chart` serves embedded markdown chart docs" in chart_section
    assert "`src/cli/chart.rs`" in chart_section
    assert "`docs/charts/`" in chart_section
    assert "`RustEmbed`" in chart_section
    assert "`biomcp chart` documents the chart surface, but does not render charts" in chart_section
    assert "`ChartArgs`" in chart_section
    assert "`src/cli/mod.rs`" in chart_section
    assert "`src/render/chart.rs`" in chart_section
    assert "`study query`" in chart_section
    assert "`study co-occurrence`" in chart_section
    assert "`study compare`" in chart_section
    assert "`study survival`" in chart_section
    assert "`bar`, `stacked-bar`, `pie`, `waterfall`, `heatmap`, `histogram`, `density`, `box`, `violin`, `ridgeline`, `scatter`, and `survival`" in chart_section
    assert "terminal" in chart_section
    assert "SVG file" in chart_section
    assert "PNG file behind the `charts-png` feature" in chart_section
    assert "MCP inline SVG" in chart_section
    assert "`--cols` and `--rows` size terminal output" in chart_section
    assert "`--width` and `--height` size SVG, PNG, and MCP inline SVG output" in chart_section
    assert "`--scale` is PNG-only" in chart_section
    assert "`--title`, `--theme`, and `--palette` style rendered charts" in chart_section
    assert "Heatmaps reject `--palette`" in chart_section
    assert "`rewrite_mcp_chart_args()`" in chart_section
    assert "text pass plus an SVG pass" in chart_section
    assert "`--terminal` is stripped" in chart_section
    assert "`--output` / `-o` are rejected" in chart_section
    assert "`--cols` / `--rows` and `--scale` are rejected for the SVG pass" in chart_section
    assert "`docs/charts/index.md`" in chart_section
    assert "user-facing chart reference and examples" in chart_section


def test_source_integration_architecture_doc_captures_repo_contract() -> None:
    technical = _read_repo("architecture/technical/overview.md")
    source_integration = _read_repo("architecture/technical/source-integration.md")
    drug_guide = _read_repo("docs/user-guide/drug.md")
    bioasq_reference = _read_repo("docs/reference/bioasq-benchmark.md")
    cli_mod = _read_repo("src/cli/mod.rs")
    cli_list = _read_repo("src/cli/list.rs")
    cli_list_reference = _read_repo("src/cli/list_reference.md")
    cli_reference_guide = _read_repo("docs/user-guide/cli-reference.md")
    drug_get = _read_repo("src/entities/drug/get.rs")
    ema_source = _read_repo("src/sources/ema.rs")
    health = _read_repo("src/cli/health.rs")
    drug_spec = _read_repo("spec/05-drug.md")
    bioasq_reference_ws = _normalize_ws(bioasq_reference)
    cli_reference_guide_ws = _normalize_ws(cli_reference_guide)
    local_runtime_section = _normalize_ws(
        _markdown_section(source_integration, "Local Runtime Sources and File-Backed Assets")
    )
    modifier_section = _normalize_ws(
        _markdown_section(source_integration, "Entity-Specific Command Modifiers")
    )

    assert "source-integration.md" in technical
    assert "# BioMCP Source Integration Architecture" in source_integration
    assert "## New Source vs Existing Source" in source_integration
    assert "`src/sources/<source>.rs`" in source_integration
    assert "`src/sources/mod.rs`" in source_integration
    assert "`shared_client()`" in source_integration
    assert "`streaming_http_client()`" in source_integration
    assert "`env_base(default, ENV_VAR)`" in source_integration
    assert "`read_limited_body()`" in source_integration
    assert "`body_excerpt()`" in source_integration
    assert "`retry_send()`" in source_integration
    assert "## Section-First Entity Integration" in source_integration
    assert "`src/cli/mod.rs`" in source_integration
    assert "`src/cli/list.rs`" in source_integration
    assert "`docs/user-guide/cli-reference.md`" in source_integration
    assert "default `get` output stays concise" in source_integration
    assert "## Local Runtime Sources and File-Backed Assets" in source_integration
    assert (
        "EMA and WHO Prequalification are the canonical local runtime drug sources"
        in local_runtime_section
    )
    assert "`BIOMCP_EMA_DIR` first, then the platform data directory" in local_runtime_section
    assert "`BIOMCP_WHO_DIR` first, then the platform data directory" in local_runtime_section
    assert (
        "`biomcp health` includes the EMA and WHO local-data readiness rows"
        in local_runtime_section
    )
    assert "`biomcp health --apis-only` excludes those rows" in local_runtime_section
    assert (
        "`configured`, `configured (stale)`, `available (default path)`, "
        "`available (default path, stale)`, `not configured`, and "
        "`error (missing: ...)`"
        in local_runtime_section
    )
    assert "`docs/user-guide/drug.md`" in local_runtime_section
    assert "BioASQ is the canonical file-backed non-runtime asset" in local_runtime_section
    assert (
        "do not join the runtime source inventory, `biomcp health`, or the source-readiness checklist"
        in local_runtime_section
    )
    assert "`docs/reference/bioasq-benchmark.md`" in local_runtime_section
    assert "`benchmarks/bioasq/`" in local_runtime_section
    assert "## EMA local data setup" in drug_guide
    assert "## WHO Prequalification local data setup" in drug_guide
    assert "`configured`:" in drug_guide
    assert drug_guide.count("`configured (stale)`:") >= 2
    assert "`available (default path)`:" in drug_guide
    assert drug_guide.count("`available (default path, stale)`:") >= 2
    assert "`not configured`:" in drug_guide
    assert "`error (missing: ...)`:" in drug_guide
    assert "pub(crate) fn resolve_ema_root() -> PathBuf {" in ema_source
    assert 'std::env::var("BIOMCP_EMA_DIR")' in ema_source
    assert "EMA local data" in health
    assert "available (default path)" in health
    assert "not configured" in health
    assert "error (missing:" in health
    assert "BioASQ" not in health
    assert "# BioASQ Benchmark" in bioasq_reference
    assert "offline benchmark input, not as a live runtime source" in bioasq_reference_ws
    assert (REPO_ROOT / "benchmarks" / "bioasq").is_dir()
    assert "## Entity-Specific Command Modifiers" in source_integration
    assert "The base grammar remains `get <entity> <id> [section...]`." in modifier_section
    assert "Entity-specific modifiers are named options" in modifier_section
    assert "The canonical example is `get drug <name> ... --region <us|eu|who|all>`." in modifier_section
    assert "`src/cli/mod.rs`" in modifier_section
    assert "`src/cli/list.rs`" in modifier_section
    assert "`src/cli/list_reference.md`" in modifier_section
    assert "`docs/user-guide/cli-reference.md`" in modifier_section
    assert "`docs/user-guide/drug.md`" in modifier_section
    assert "`spec/05-drug.md`" in modifier_section
    assert "Runtime validation belongs in the owning entity or CLI path" in modifier_section
    assert (
        "`--region` only changes the data plane for `regulatory`, `safety`, `shortage`, or `all`"
        in modifier_section
    )
    assert (
        "`--region who` is valid for `regulatory` and `all`, but not for `safety` or"
        in modifier_section
    )
    assert "`approvals` remains U.S.-only" in modifier_section
    assert "invalid flag/section combinations fail fast before data fetches" in modifier_section
    assert "biomcp get drug trastuzumab regulatory --region who" in cli_mod
    assert "biomcp get drug trastuzumab regulatory --region who" in cli_reference_guide
    assert "Data region for regional sections (regulatory, safety, shortage, or all)" in cli_mod
    assert "get drug <name> regulatory [--region <us|eu|who|all>]" in cli_list
    assert "get drug <name> safety [--region <us|eu|all>]" in cli_list
    assert "get drug <name> shortage [--region <us|eu|all>]" in cli_list
    assert "get drug <name> approvals" in cli_list
    assert "get drug <name> regulatory [--region <us|eu|who|all>]" in cli_list_reference
    assert "get drug <name> safety|shortage [--region <us|eu|all>]" in cli_list_reference
    assert (
        "For `get drug`, use `--region` only with `regulatory`, `safety`, `shortage`, or `all`"
        in cli_reference_guide_ws
    )
    assert "get drug <name> regulatory [--region <us|eu|who|all>]" in drug_spec
    assert "--region is not supported with approvals." in drug_get
    assert "--region can only be used with regulatory, safety, shortage, or all." in drug_get
    assert "## Provenance and Rendering" in source_integration
    assert "`source_label`" in source_integration
    assert "source-specific notes" in source_integration
    assert "## Auth, Cache, and Secrets" in source_integration
    assert "`BioMcpError::ApiKeyRequired`" in source_integration
    assert "`apply_cache_mode_with_auth(..., true)`" in source_integration
    assert "`docs/getting-started/api-keys.md`" in source_integration
    assert "`docs/reference/data-sources.md`" in source_integration
    assert "Do not log secrets" in source_integration
    assert "## Graceful Degradation and Timeouts" in source_integration
    assert "Optional enrichments must not take down the whole command" in source_integration
    assert "truthful about missing or unavailable data" in source_integration
    assert "## Rate Limits and Operational Constraints" in source_integration
    assert "`biomcp serve-http`" in source_integration
    assert "process-local" in source_integration
    assert "## Source Addition Checklist" in source_integration
    assert "`docs/reference/source-versioning.md`" in source_integration
    assert "`src/cli/health.rs`" in source_integration
    assert "`scripts/contract-smoke.sh`" in source_integration
    assert "`spec/`" in source_integration
    assert "`CHANGELOG.md`" in source_integration


def test_pull_request_contract_gate_matches_release_validation() -> None:
    ci = _read_repo(".github/workflows/ci.yml")
    release = _read_repo(".github/workflows/release.yml")
    contracts_smoke = _read_repo(".github/workflows/contracts.yml")
    spec_smoke = _read_repo(".github/workflows/spec-smoke.yml")
    expected_ci_contract_runs = [
        "cargo build --release --locked",
        "uv sync --extra dev",
        'uv run pytest tests/ -v --mcp-cmd "./target/release/biomcp serve"',
        "uv run mkdocs build --strict",
    ]
    expected_release_contract_runs = [
        "uv sync --extra dev",
        'uv run pytest tests/ -v --mcp-cmd "biomcp serve"',
        "uv run mkdocs build --strict",
    ]

    ci_contracts = _workflow_job_block(ci, "contracts")
    ci_spec = _workflow_job_block(ci, "spec-stable")
    ci_version_sync = _workflow_job_block(ci, "version-sync")
    ci_climb_hygiene = _workflow_job_block(ci, "climb-hygiene")
    release_validate = _workflow_job_block(release, "validate")
    smoke_spec = _workflow_job_block(spec_smoke, "spec-volatile-live")

    assert 'python-version: "3.12"' in ci_contracts
    assert 'python-version: "3.12"' in ci_spec
    assert 'python-version: "3.12"' in release_validate
    assert 'python-version: "3.12"' in smoke_spec
    assert _workflow_run_steps(ci_contracts) == expected_ci_contract_runs
    assert "- uses: actions/checkout@v4" in ci_spec
    assert "uses: arduino/setup-protoc@v3" in ci_spec
    assert "uses: dtolnay/rust-toolchain@stable" in ci_spec
    assert "uses: actions/setup-python@v5" in ci_spec
    assert "uses: astral-sh/setup-uv@v4" in ci_spec
    assert _workflow_run_steps(ci_spec) == [
        "cargo build --release --locked",
        "make spec-pr",
    ]
    assert _workflow_run_steps(release_validate)[-3:] == expected_release_contract_runs
    assert "- uses: actions/checkout@v4" in ci_version_sync
    assert _workflow_run_steps(ci_version_sync) == [
        "bash scripts/check-version-sync.sh"
    ]
    for forbidden in (
        "setup-python",
        "setup-uv",
        "setup-protoc",
        "rust-toolchain",
        "cargo ",
        "uv sync",
        "python-version:",
    ):
        assert forbidden not in ci_version_sync
    assert "- uses: actions/checkout@v4" in ci_climb_hygiene
    assert _workflow_run_steps(ci_climb_hygiene) == [
        "bash scripts/check-no-climb-tracked.sh"
    ]
    for forbidden in (
        "setup-python",
        "setup-uv",
        "setup-protoc",
        "rust-toolchain",
        "cargo ",
        "uv sync",
        "python-version:",
    ):
        assert forbidden not in ci_climb_hygiene

    assert "name: Contract Smoke Tests" in contracts_smoke
    assert 'cron: "0 6 * * *"' in contracts_smoke
    assert "workflow_dispatch:" in contracts_smoke
    assert "continue-on-error: true" in contracts_smoke
    assert "- run: bash scripts/contract-smoke.sh" in contracts_smoke

    assert "name: Spec smoke (volatile live-network)" in spec_smoke
    assert 'cron: "0 7 * * *"' in spec_smoke
    assert "workflow_dispatch:" in spec_smoke
    assert "- uses: actions/checkout@v4" in smoke_spec
    assert "uses: arduino/setup-protoc@v3" in smoke_spec
    assert "uses: dtolnay/rust-toolchain@stable" in smoke_spec
    assert "uses: actions/setup-python@v5" in smoke_spec
    assert "uses: astral-sh/setup-uv@v4" in smoke_spec
    assert _workflow_run_steps(smoke_spec) == [
        "cargo build --release --locked",
        "make spec",
    ]


def test_makefile_spec_split_contract_is_documented_and_executable() -> None:
    makefile = _read_repo("Makefile")

    assert ".PHONY: build test lint check check-quality-ratchet run clean spec spec-pr validate-skills test-contracts install" in makefile
    assert "Volatile live-network spec headings." in makefile
    assert "PR gate: repo-local checks plus live-backed headings that have been stable" in makefile
    assert "Smoke lane: `search article`, `gene articles`, `variant articles`," in makefile
    assert "To move a heading into the smoke lane, add its exact pytest markdown node ID" in makefile
    assert 'SPEC_PR_DESELECT_ARGS = \\' in makefile
    for node_id in (
        'spec/02-gene.md::Gene to Articles',
        'spec/03-variant.md::Variant to Articles',
        'spec/06-article.md::Searching by Gene',
        'spec/06-article.md::Searching by Keyword',
        'spec/06-article.md::Sort Behavior',
        'spec/07-disease.md::Disease to Articles',
    ):
        assert f'--deselect "{node_id}"' in makefile
    assert (
        "SPEC_SERIAL_FILES = spec/05-drug.md spec/13-study.md "
        "spec/21-cross-entity-see-also.md"
    ) in makefile
    assert "SPEC_XDIST_ARGS = -n auto --dist loadfile" in makefile
    assert re.search(r"^test:\n\tcargo nextest run$", makefile, flags=re.MULTILINE)
    assert re.search(
        r'^install:\n'
        r'\tmkdir -p "\$\(HOME\)/\.local/bin"\n'
        r"\tcargo build --release --locked\n"
        r'\tinstall -m 755 target/release/biomcp "\$\(HOME\)/\.local/bin/biomcp"$',
        makefile,
        flags=re.MULTILINE,
    )
    assert re.search(
        r"^spec:\n"
        r"\tXDG_CACHE_HOME=\"\$\(CURDIR\)/\.cache\" PATH=\"\$\(CURDIR\)/target/release:\$\(PATH\)\" \\\n"
        r"\t\tuv run --extra dev sh -c 'PATH=\"\$\(CURDIR\)/target/release:\$\$PATH\" pytest spec/ --mustmatch-lang bash --mustmatch-timeout 120 -v \$\(SPEC_XDIST_ARGS\) --ignore spec/05-drug\.md --ignore spec/13-study\.md --ignore spec/21-cross-entity-see-also\.md'\n"
        r"\tXDG_CACHE_HOME=\"\$\(CURDIR\)/\.cache\" PATH=\"\$\(CURDIR\)/target/release:\$\(PATH\)\" \\\n"
        r"\t\tuv run --extra dev sh -c 'PATH=\"\$\(CURDIR\)/target/release:\$\$PATH\" pytest \$\(SPEC_SERIAL_FILES\) --mustmatch-lang bash --mustmatch-timeout 120 -v'$",
        makefile,
        flags=re.MULTILINE,
    )
    assert re.search(
        r"^spec-pr:\n"
        r"\tXDG_CACHE_HOME=\"\$\(CURDIR\)/\.cache\" PATH=\"\$\(CURDIR\)/target/release:\$\(PATH\)\" \\\n"
        r"\t\tuv run --extra dev sh -c 'PATH=\"\$\(CURDIR\)/target/release:\$\$PATH\" pytest spec/ --mustmatch-lang bash --mustmatch-timeout 60 -v \$\(SPEC_XDIST_ARGS\) \$\(SPEC_PR_DESELECT_ARGS\) --ignore spec/05-drug\.md --ignore spec/13-study\.md --ignore spec/21-cross-entity-see-also\.md'\n"
        r"\tXDG_CACHE_HOME=\"\$\(CURDIR\)/\.cache\" PATH=\"\$\(CURDIR\)/target/release:\$\(PATH\)\" \\\n"
        r"\t\tuv run --extra dev sh -c 'PATH=\"\$\(CURDIR\)/target/release:\$\$PATH\" pytest \$\(SPEC_SERIAL_FILES\) --mustmatch-lang bash --mustmatch-timeout 60 -v'$",
        makefile,
        flags=re.MULTILINE,
    )
    assert re.search(
        r"^test-contracts:\n"
        r"\tcargo build --release --locked\n"
        r"\tuv sync --extra dev\n"
        r'\tuv run pytest tests/ -v --mcp-cmd "\./target/release/biomcp serve"\n'
        r"\tuv run mkdocs build --strict$",
        makefile,
        flags=re.MULTILINE,
    )


def test_repo_local_parallel_test_contract_is_documented() -> None:
    contributing = _read_repo("CONTRIBUTING.md")
    runbook = _read_repo("RUN.md")
    technical = _read_repo("architecture/technical/overview.md")
    contributing_ws = _normalize_ws(contributing)
    runbook_ws = _normalize_ws(runbook)
    technical_gate_section = _normalize_ws(
        _markdown_section(technical, "1. CI and Repo Gates", level=3)
    )
    technical_spec_section = _normalize_ws(
        _markdown_section(technical, "2. Spec Suite (`spec/`)", level=3)
    )

    assert "cargo install cargo-nextest --locked" in contributing
    assert "`make test` uses `cargo nextest run`." in contributing_ws
    assert "`make spec` and `make spec-pr` use `pytest-xdist`" in contributing_ws
    assert "`spec/05-drug.md`, `spec/13-study.md`, and `spec/21-cross-entity-see-also.md` stay serial" in contributing_ws
    assert "beelink" in contributing
    assert "2026-04-13" in contributing
    assert "/usr/bin/time -p" in contributing
    assert "warm-cache steady-state" in contributing_ws
    assert "| Command | Before | After |" in contributing
    for row in (
        "| `make test` |",
        "| `make spec-pr` |",
        "| `make check` |",
    ):
        assert row in contributing

    assert "cargo-nextest" in runbook
    assert "`make test` and `make check`" in runbook_ws
    assert "`cargo nextest run`" in runbook_ws
    assert "`make spec-pr`" in runbook_ws
    assert "`pytest-xdist`" in runbook_ws
    assert "`-n auto --dist loadfile`" in runbook_ws
    assert "`spec/05-drug.md`, `spec/13-study.md`, and `spec/21-cross-entity-see-also.md`" in runbook_ws

    assert "`cargo nextest run`" in technical_gate_section
    assert "`cargo test`" in technical_gate_section
    assert "`pytest-xdist` with `-n auto --dist loadfile`" in technical_spec_section
    assert (
        "`spec/05-drug.md`, `spec/13-study.md`, and `spec/21-cross-entity-see-also.md`"
        in technical_spec_section
    )


def test_runtime_contract_docs_and_scripts_align_on_release_target() -> None:
    staging_demo = _read_repo("architecture/technical/staging-demo.md")
    runbook = _read_repo("RUN.md")
    technical = _read_repo("architecture/technical/overview.md")
    scripts_readme = _read_repo("scripts/README.md")
    source_contracts = _read_repo("scripts/source-contracts.md")
    contract_smoke = _read_repo("scripts/contract-smoke.sh")
    genegpt_demo = _read_repo("scripts/genegpt-demo.sh")
    geneagent_demo = _read_repo("scripts/geneagent-demo.sh")

    assert "# BioMCP Staging and Demo Contract" in staging_demo
    assert "./target/release/biomcp" in staging_demo
    assert "shared merged-main target" in staging_demo
    assert "BIOMCP_BIN=./target/release/biomcp ./scripts/genegpt-demo.sh" in staging_demo
    assert "BIOMCP_BIN=./target/release/biomcp ./scripts/geneagent-demo.sh" in staging_demo
    assert "./scripts/contract-smoke.sh --fast" in staging_demo
    assert 'uv run pytest tests/test_mcp_contract.py -v --mcp-cmd "./target/release/biomcp serve"' in staging_demo
    assert "ONCOKB_TOKEN" in staging_demo
    assert "./target/release/biomcp serve-http --host 127.0.0.1 --port 8080" in staging_demo
    assert "POST/GET /mcp" in staging_demo
    assert "GET /health" in staging_demo
    assert "GET /readyz" in staging_demo
    assert "GET /" in staging_demo
    assert "tests/test_mcp_http_transport.py" in staging_demo
    assert "S2_API_KEY" in staging_demo
    assert "article citations 22663011 --limit 3" in staging_demo

    assert "# BioMCP Runbook" in runbook
    assert "cargo build --release --locked" in runbook
    assert "./target/release/biomcp serve" in runbook
    assert "./target/release/biomcp serve-http --host 127.0.0.1 --port 8080" in runbook
    assert 'uv run pytest tests/test_mcp_contract.py -v --mcp-cmd "./target/release/biomcp serve"' in runbook
    assert "curl http://127.0.0.1:8080/health" in runbook
    assert "curl http://127.0.0.1:8080/readyz" in runbook
    assert "curl http://127.0.0.1:8080/" in runbook
    assert "tests/test_mcp_http_surface.py" in runbook
    assert "tests/test_mcp_http_transport.py" in runbook
    assert "make spec" in runbook
    assert "make test-contracts" in runbook
    assert "S2_API_KEY" in runbook
    assert "./target/release/biomcp article citations 22663011 --limit 3" in runbook
    assert (
        '`make test-contracts` runs `cargo build --release --locked`, '
        '`uv sync --extra dev`, `pytest tests/ -v --mcp-cmd "./target/release/biomcp serve"`, '
        'and `mkdocs build --strict` - the same steps that PR CI `contracts` requires.'
        in runbook
    )
    assert "docs/user-guide/cli-reference.md" in runbook
    assert "docs/reference/mcp-server.md" in runbook

    assert "architecture/technical/staging-demo.md" in technical
    assert "RUN.md" in technical
    assert "S2_API_KEY" in technical
    assert "Semantic Scholar article enrichment/navigation" in technical
    assert "No `RUN.md` or staging-demo runbook exists" not in technical

    assert "lightweight commands for checking upstream source" in scripts_readme
    assert "source-facing contract probe" in scripts_readme
    assert "091 expansion scope" not in scripts_readme

    assert "# BioMCP Source Contract Probes" in source_contracts
    assert "source-facing API contract probes" in source_contracts
    assert "ONCOKB_TOKEN" in source_contracts
    assert "Semantic Scholar" in source_contracts
    assert "S2_API_KEY" in source_contracts
    assert "091 expansion scope" not in source_contracts

    assert "ONCOKB_TOKEN" in contract_smoke
    assert "ONCOKB_API_TOKEN" in contract_smoke
    assert "S2_API_KEY" in contract_smoke
    assert "Semantic Scholar" in contract_smoke
    assert "set ONCOKB_TOKEN to enable" in contract_smoke

    for demo_script in (genegpt_demo, geneagent_demo):
        assert 'BIN="${BIOMCP_BIN:-' in demo_script
        assert "$ROOT/target/release/biomcp" in demo_script
        assert "target/debug/biomcp" not in demo_script
        assert 'command -v biomcp >/dev/null 2>&1' in demo_script


def test_validation_profile_and_hook_contract_docs_are_pinned() -> None:
    runbook = _read_repo("RUN.md")
    technical = _read_repo("architecture/technical/overview.md")
    runbook_premerge = _normalize_ws(_markdown_section(runbook, "Pre-Merge Checks"))
    ci_gate_section = _normalize_ws(_markdown_section(technical, "1. CI and Repo Gates", level=3))

    assert "pre-commit hook" in runbook_premerge
    assert "`cargo fmt --check`" in runbook_premerge
    assert "`cargo clippy --lib --tests -- -D warnings`" in runbook_premerge
    assert "does not run" in runbook_premerge
    assert "`cargo test`" in runbook_premerge
    assert "`make check`" in runbook_premerge
    assert "`make spec-pr`" in runbook_premerge
    assert "`make test-contracts`" in runbook_premerge
    assert "git commit --no-verify" in runbook_premerge

    assert ".march/validation-profiles.toml" in ci_gate_section
    assert "`01-design` and `02-design-review` without a validation profile" in ci_gate_section
    for row in (
        "| `preflight` | `cargo check --all-targets` | `kickoff` |",
        "| `baseline` | `cargo check --all-targets` | declared, not assigned |",
        "| `focused` | `cargo test --lib && cargo clippy --lib --tests -- -D warnings` | `03-code`, `04-code-review` |",
        "| `full-blocking` | `make check && make spec-pr` | `05-verify` |",
        "| `full-contracts` | `make check && make spec-pr && make test-contracts` | declared, not assigned |",
    ):
        assert row in ci_gate_section
    assert (
        "`full-blocking` deliberately uses `make check && make spec-pr`, not full `make spec`"
        in ci_gate_section
    )
    assert (
        "`full-contracts` is declared for tickets that need the contracts lane, but the shared build flow does not assign it today"
        in ci_gate_section
    )
