# BioMCP Runbook

## What This Runbook Covers

This is the exact operator guide for the merged-main release binary. For the
shared target, owned artifacts, and promotion contract, see
`architecture/technical/staging-demo.md`.

## Prerequisites

- Rust toolchain with `cargo`
- `cargo-nextest` for repo-local `make test` and `make check`
- `cargo-deny` for the repo-local license and advisory policy checks in `make check`
- `uv` for repo-local pytest and spec flows
- `curl` for `scripts/contract-smoke.sh`

Install the Rust helper tools with:

```bash
cargo install cargo-nextest --locked
cargo install cargo-deny --locked
```

## Build The Shared Target

```bash
cargo build --release --locked
```

The shared target path is `./target/release/biomcp`.

## Run: CLI Mode

```bash
./target/release/biomcp health --apis-only
./target/release/biomcp get gene BRAF
./target/release/biomcp get article 22663011 tldr   # requires S2_API_KEY
```

Use `docs/user-guide/cli-reference.md` for the full command grammar and entity
surface.

## Run: MCP Stdio Mode

```bash
./target/release/biomcp serve
```

Minimal client configuration:

```json
{
  "mcpServers": {
    "biomcp": {
      "command": "./target/release/biomcp",
      "args": ["serve"]
    }
  }
}
```

`serve` is the canonical operator spelling and is equivalent to `biomcp mcp`.

## Run: Streamable HTTP Mode

```bash
./target/release/biomcp serve-http --host 127.0.0.1 --port 8080
```

This serves MCP over Streamable HTTP at `/mcp`. Use `--host 0.0.0.0` only when
the endpoint must be reachable from other machines or containers on the network.

Owned routes:

- `POST/GET /mcp`
- `GET /health`
- `GET /readyz`
- `GET /`

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `BIOMCP_CACHE_MODE` | Set `infinite` to replay cached responses locally |
| `NCBI_API_KEY` | Higher rate limits for PubTator3, PubMed/efetch, PMC OA, and NCBI helpers |
| `S2_API_KEY` | Optional Semantic Scholar TLDR, citation graph, and recommendations |
| `OPENFDA_API_KEY` | Higher OpenFDA rate limits |
| `NCI_API_KEY` | Required for NCI CTS trial queries |
| `ONCOKB_TOKEN` | Canonical OncoKB production token |
| `ALPHAGENOME_API_KEY` | Required for AlphaGenome variant prediction |

## Pre-Merge Checks

Run the heavier local ticket proofs explicitly:

```bash
make release-gate       # full routine release-readiness: check + spec-contracts
make spec               # offline deterministic routine spec gate
make spec-contracts     # deterministic executable contracts for routine proof
make verify             # opt-in live public-upstream confidence
make test-contracts     # rerun just Python/docs contract lane
```

The installed pre-commit hook is the fast local gate. It should run
`scripts/pre-commit-reject-march-artifacts.sh` before `cargo fmt --check` and
`cargo clippy --lib --tests -- -D warnings`. The March helper rejects staged
non-deletion `.march/*` paths outside the exhaustive allowlist:
`.march/code-review-log.md` and `.march/validation-profiles.toml`. The hook
does not run `cargo nextest run`, `make check`, `make spec-contracts`,
`make spec-pr`, `make release-gate`, or `make test-contracts`.

Use `make check` for the canonical local gate; it runs the full Rust
lint/test/security/quality-ratchet lane and now includes `make test-contracts`,
so landing-copy, Python, and strict-docs regressions fail the same local gate.
Its `lint` phase runs both `cargo deny check licenses` and
`cargo deny check advisories`, and its `test` phase shells out to
`cargo nextest run`. Use `make release-gate` for the single routine
release-readiness signal; it runs `make check` followed by deterministic
`make spec-contracts`. Use `make verify` only as an explicit opt-in live
public-upstream confidence lane; `make release-live-smoke` is a compatibility
alias for that operator lane. `make spec-pr` remains available for the
offline executable-spec corpus by itself; it runs explicit local/fixture-backed
`SPEC_ROUTINE_PATHS` with `pytest-xdist` (`-n auto --dist loadfile`) and the
longer mustmatch timeout. `make spec` runs the same offline path set with the
shorter local timeout and should pass with external network blocked.

The executable docs do not hand-roll env setup inside bash blocks anymore.
`tools/biomcp-ci` is the only spec runner seam: it resolves the repo root from
its own path, points `BIOMCP_CACHE_DIR` and `XDG_*` under
`.cache/biomcp-specs/`, defaults `RUST_LOG=error`, unsets optional auth keys,
and only forces `BIOMCP_CACHE_MODE=infinite` when CI restored a warm cache and
exported `BIOMCP_SPEC_CACHE_HIT=1`. Cold runs leave `BIOMCP_CACHE_MODE`
untouched so the shared cache can refill naturally. Use `make test-contracts`
to rerun just the Python/docs contract lane. Repo-root Ruff still runs through
`bin/lint`, but `pyproject.toml` excludes `architecture/experiments/**` so
scratch experiment scripts do not block the production Python lint gate. Use
`git commit --no-verify` to skip the hook for a one-off commit.

`make test-contracts` runs `cargo build --release --locked`, `uv sync --extra dev --no-install-project`, `uv run --no-sync pytest tests/ -v --mcp-cmd "./target/release/biomcp serve"`, and `uv run --no-sync mkdocs build --strict` - the same steps that PR CI `contracts` requires. The `--no-install-project`/`--no-sync` split is intentional: Python/docs/spec lanes install only Python dev tooling and exercise the already-built `target/release/biomcp` binary instead of rebuilding the maturin package into `.venv`. `make check` now pulls that lane in transitively, while `make test-contracts` remains the direct rerun command when only the Python/docs contract lane needs another pass.

## Smoke Checks

```bash
BIOMCP_BIN=./target/release/biomcp ./scripts/genegpt-demo.sh
BIOMCP_BIN=./target/release/biomcp ./scripts/geneagent-demo.sh
./scripts/contract-smoke.sh --fast
# Optional keyed article proof:
./target/release/biomcp article citations 22663011 --limit 3
```

Use `architecture/technical/staging-demo.md` for the promotion contract and
`scripts/source-contracts.md` for the deeper source probe inventory.

## MCP Contract Verification

```bash
uv sync --extra dev --no-install-project
uv run --no-sync pytest tests/test_mcp_contract.py -v --mcp-cmd "./target/release/biomcp serve"
uv run --no-sync pytest tests/test_mcp_http_surface.py tests/test_mcp_http_transport.py -v
curl http://127.0.0.1:8080/health
curl http://127.0.0.1:8080/readyz
curl http://127.0.0.1:8080/
```

See `docs/reference/mcp-server.md` for the documented MCP surface.

## Spec Suite

```bash
make spec               # offline deterministic routine spec gate
make spec-contracts
make verify             # opt-in live public-upstream confidence
make release-live-smoke # compatibility alias for make verify
make spec-pr
```

`make spec` is the offline deterministic routine spec gate. `make
spec-contracts` is the deterministic subset used by March `spec-only` and
`release-gate`; it keeps local/fixture-backed contracts executable without
running live smoke. `make verify` is the explicit opt-in live lane for
discover/OLS4, disease, article source-status, variant-normalization,
phenotype, protein, pathway, and the other public-upstream specs through
`tools/biomcp-ci`; `make release-live-smoke` delegates to `make verify` for old
operator muscle memory.

`make spec` and `make spec-pr` both run explicit `SPEC_ROUTINE_PATHS` only:
`spec/entity/article.md`, `spec/entity/study.md`, `spec/entity/variant.md`,
`spec/surface/mcp.md`, and deterministic `spec/surface/test_*.py` contracts.
Live-upstream specs such as `spec/entity/phenotype.md`, `spec/entity/protein.md`,
`spec/entity/disease.md`, `spec/surface/discover.md`, `spec/entity/pathway.md`,
and `spec/surface/cli.md` run only in `make verify`. Every bash block in those
lanes should call `tools/biomcp-ci`, which owns release-binary resolution,
repo-owned cache roots, optional-key stripping, and warm-cache replay on CI
cache hits.

Use `spec/README-timings.md` as the current validation-lane audit/reference for
the offline deterministic routine lane, the opt-in live verify lane, the active
canary corpus, the wrapper/cache contract, and warm-cache expectations.

When running repo-local Python/docs/spec checks through `uv`, use
`uv sync --extra dev --no-install-project` followed by `uv run --no-sync ...`.
Keep `target/release` ahead of `.venv/bin` on `PATH` and pass
`BIOMCP_BIN=./target/release/biomcp` when invoking executable specs manually.
Do not use `uv run --extra dev ...` for Python-only gate lanes: that asks uv to
install the maturin-backed current project and can redundantly rebuild the Rust
CLI before pytest or mkdocs starts.
