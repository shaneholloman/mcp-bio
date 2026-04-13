# BioMCP Runbook

## What This Runbook Covers

This is the exact operator guide for the merged-main release binary. For the
shared target, owned artifacts, and promotion contract, see
`architecture/technical/staging-demo.md`.

## Prerequisites

- Rust toolchain with `cargo`
- `cargo-nextest` for repo-local `make test` and `make check`
- `uv` for repo-local pytest and spec flows
- `curl` for `scripts/contract-smoke.sh`

Install `cargo-nextest` with:

```bash
cargo install cargo-nextest --locked
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
make check
make spec-pr
make test-contracts
```

The installed pre-commit hook is the fast local gate. It enforces
`cargo fmt --check` and `cargo clippy --lib --tests -- -D warnings`. It does
not run `cargo nextest run`, `make check`, `make spec-pr`, or
`make test-contracts`.

Use `make check` for the full Rust lint/test/quality-ratchet lane; its `test`
phase now shells out to `cargo nextest run`. Use `make spec-pr` for the stable
PR-blocking spec lane; it runs `pytest-xdist` with `-n auto --dist loadfile`
for the parallel-safe bulk, then runs `spec/05-drug.md`, `spec/13-study.md`,
and `spec/21-cross-entity-see-also.md` serially because those files share
repo-global local-data fixtures. Use `make test-contracts` for the Python/docs
contract lane. Use `git commit --no-verify` to skip the hook for a one-off
commit.

`make test-contracts` runs `cargo build --release --locked`, `uv sync --extra dev`, `pytest tests/ -v --mcp-cmd "./target/release/biomcp serve"`, and `mkdocs build --strict` - the same steps that PR CI `contracts` requires. Use this to catch docs-contract and Python regressions before pushing.

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
uv run pytest tests/test_mcp_contract.py -v --mcp-cmd "./target/release/biomcp serve"
uv run pytest tests/test_mcp_http_surface.py tests/test_mcp_http_transport.py -v
curl http://127.0.0.1:8080/health
curl http://127.0.0.1:8080/readyz
curl http://127.0.0.1:8080/
```

See `docs/reference/mcp-server.md` for the documented MCP surface.

## Spec Suite

```bash
make spec
```

`make spec` and `make spec-pr` both use `pytest-xdist` with
`-n auto --dist loadfile` for the parallel-safe bulk, then run
`spec/05-drug.md`, `spec/13-study.md`, and `spec/21-cross-entity-see-also.md`
serially because those files share repo-global local-data fixtures.
Use `spec/README-timings.md` as the current per-heading audit and the source of
truth for which headings stay smoke-only via `SPEC_PR_DESELECT_ARGS`.

When running repo-local checks through `uv run`, make sure `target/release` is
ahead of `.venv/bin` on `PATH` or refresh the editable install with
`uv pip install -e .` so `uv run` does not pick a stale `.venv/bin/biomcp`.
