# MCP Surface

BioMCP exposes the same biomedical command surface through stdio MCP and
Streamable HTTP. These canaries keep the transport entrypoints, probe routes,
and remote tool execution honest without re-encoding the whole MCP test suite.

## Stdio Entry Points Stay Guided

`mcp` and `serve` are both documented stdio entrypoints. The user-visible
contract here is that one remains the canonical stdio command and the other
stays the Claude Desktop-friendly alias.

```bash
mcp_help="$(../../tools/biomcp-ci mcp --help)"
echo "$mcp_help" | mustmatch like "Run MCP server over stdio"
echo "$mcp_help" | mustmatch like "Usage: biomcp mcp"
serve_help="$(../../tools/biomcp-ci serve --help)"
echo "$serve_help" | mustmatch like 'Alias for `mcp`'
echo "$serve_help" | mustmatch like "Usage: biomcp serve"
```

## Manual Stdio Startup Points Operators to HTTP

When an operator launches a stdio entrypoint without an MCP client, BioMCP
should fail closed but still explain the recovery path. Both spellings should
print the same stderr guidance and keep stdout free for MCP protocol traffic.

```bash
for cmd in mcp serve; do
  stdout_file="$(mktemp)"
  stderr_file="$(mktemp)"
  set +e
  ../../target/release/biomcp "$cmd" </dev/null >"$stdout_file" 2>"$stderr_file"
  status=$?
  set -e
  test "$status" -ne 0
  test ! -s "$stdout_file"
  stderr="$(cat "$stderr_file")"
  echo "$stderr" | mustmatch like "expects an MCP client on stdin"
  echo "$stderr" | mustmatch like "biomcp serve-http"
  echo "$stderr" | mustmatch not like "connection closed"
  echo "$stderr" | mustmatch not like "initialized request"
done
```

## Streamable HTTP Help Names the Canonical Route

The remote/server deployment mode should keep pointing operators at `/mcp` and
the lightweight probe routes rather than drifting back toward legacy SSE copy.

```bash
out="$(../../tools/biomcp-ci serve-http --help)"
echo "$out" | mustmatch like "Streamable HTTP server at /mcp"
echo "$out" | mustmatch like "GET /health, GET /readyz, GET /."
echo "$out" | mustmatch like "--host <HOST>"
```

## Probe Routes Stay Lightweight

The HTTP surface is intentionally tiny: two readiness probes and one root
descriptor that advertises the streamable transport and canonical MCP path.

```bash
port=39087
../../tools/biomcp-ci serve-http --host 127.0.0.1 --port "$port" >/tmp/biomcp-mcp-routes.log 2>&1 &
pid=$!
trap 'kill "$pid" 2>/dev/null || true' EXIT
for _ in $(seq 1 40); do
  if curl -fsS "http://127.0.0.1:$port/readyz" >/dev/null || curl -fsS "http://127.0.0.1:$port/health" >/dev/null; then
    break
  fi
  sleep 0.25
done
curl -fsS "http://127.0.0.1:$port/readyz" >/dev/null || curl -fsS "http://127.0.0.1:$port/health" >/dev/null
curl -fsS "http://127.0.0.1:$port/health" | mustmatch like '"status":"ok"'
curl -fsS "http://127.0.0.1:$port/readyz" | mustmatch like '"status":"ok"'
root="$(curl -fsS "http://127.0.0.1:$port/")"
echo "$root" | mustmatch like '"transport":"streamable-http"'
echo "$root" | mustmatch like '"mcp":"/mcp"'
```

## Remote Workflow Calls Keep BioMCP Text

The remote tool should execute normal BioMCP workflows, not collapse them into
an MCP-specific summary. This routine proof owns a fixture-backed local command
so the public streamable-HTTP demo can remain a live operator walkthrough.

```bash
port=39088
../../tools/biomcp-ci serve-http --host 127.0.0.1 --port "$port" >/tmp/biomcp-mcp-demo.log 2>&1 &
pid=$!
trap 'kill "$pid" 2>/dev/null || true' EXIT
for _ in $(seq 1 40); do
  if curl -fsS "http://127.0.0.1:$port/readyz" >/dev/null || curl -fsS "http://127.0.0.1:$port/health" >/dev/null; then
    break
  fi
  sleep 0.25
done
curl -fsS "http://127.0.0.1:$port/readyz" >/dev/null || curl -fsS "http://127.0.0.1:$port/health" >/dev/null
out="$(uv run --no-sync python3 - "$port" <<'PY'
import asyncio
import sys
from datetime import timedelta
from mcp import ClientSession, types
from mcp.client.streamable_http import streamable_http_client

async def main(port: str) -> None:
    async with streamable_http_client(f"http://127.0.0.1:{port}/mcp", terminate_on_close=False) as (read_stream, write_stream, _):
        async with ClientSession(read_stream, write_stream, read_timeout_seconds=timedelta(seconds=30)) as session:
            await session.initialize()
            command = "biomcp study query --study msk_impact_2017 --gene TP53 --type mutations"
            result = await session.call_tool("biomcp", arguments={"command": command})
            print(f"Command: {command}")
            print(next(c.text for c in result.content if isinstance(c, types.TextContent)))

asyncio.run(main(sys.argv[1]))
PY
)"
echo "$out" | mustmatch like 'Command: biomcp study query --study msk_impact_2017 --gene TP53 --type mutations'
echo "$out" | mustmatch like "# Study Mutation Frequency: TP53 (msk_impact_2017)"
```

## Read-Only Boundaries and Charted Calls Stay Visible

The transport should still reject CLI-only filesystem commands while returning
ordinary study text plus inline SVG for chart-safe read-only calls.

```bash
port=39089
../../tools/biomcp-ci serve-http --host 127.0.0.1 --port "$port" >/tmp/biomcp-mcp-boundary.log 2>&1 &
pid=$!; trap 'kill "$pid" 2>/dev/null || true' EXIT
for _ in $(seq 1 40); do
  if curl -fsS "http://127.0.0.1:$port/readyz" >/dev/null || curl -fsS "http://127.0.0.1:$port/health" >/dev/null; then
    break
  fi
  sleep 0.25
done
curl -fsS "http://127.0.0.1:$port/readyz" >/dev/null || curl -fsS "http://127.0.0.1:$port/health" >/dev/null
out="$(uv run --no-sync python3 - "$port" <<'PY'
import asyncio
import sys
from datetime import timedelta
from mcp import ClientSession, types
from mcp.client.streamable_http import streamable_http_client

def mime_type(content):
    return getattr(content, "mimeType", getattr(content, "mime_type", None))

async def main(port: str) -> None:
    async with streamable_http_client(f"http://127.0.0.1:{port}/mcp", terminate_on_close=False) as (read_stream, write_stream, _):
        async with ClientSession(read_stream, write_stream, read_timeout_seconds=timedelta(seconds=30)) as session:
            await session.initialize()
            reject = await session.call_tool("biomcp", arguments={"command": "biomcp cache path"})
            chart = await session.call_tool("biomcp", arguments={"command": "biomcp study query --study msk_impact_2017 --gene TP53 --type mutations --chart bar"})
            print(next(c.text for c in reject.content if isinstance(c, types.TextContent)))
            print(next(c.text.splitlines()[0] for c in chart.content if isinstance(c, types.TextContent)))
            print(f"IMAGE: {next(mime_type(c) for c in chart.content if isinstance(c, types.ImageContent))}")

asyncio.run(main(sys.argv[1]))
PY
)"
echo "$out" | mustmatch like "CLI-only over MCP"
echo "$out" | mustmatch like "workstation-local filesystem paths"
echo "$out" | mustmatch like "# Study Mutation Frequency: TP53 (msk_impact_2017)"
echo "$out" | mustmatch like "IMAGE: image/svg+xml"
```

## Repository Test Gate Runs Both Runtime Layers

`make test` is the gate March uses for focused and baseline validation. It must
run the Rust unit suite and the Python CLI/MCP/docs contract lane so neither
runtime layer can report a silent green.

```bash
out="$(make -C ../.. -n test 2>&1)"
echo "$out" | mustmatch like "cargo nextest run"
echo "$out" | mustmatch like 'uv run --no-sync pytest tests/ -v --mcp-cmd "./target/release/biomcp serve"'
echo "$out" | mustmatch like "uv run --no-sync mkdocs build --strict"
```

## Repository Lint Keeps The Quality Ratchet

Dropping `make check` must not orphan the quality-ratchet policy that used to run
through that target. The standard `make lint` gate should continue to run the
repo lint script and the ratchet script.

```bash
make -C ../.. -n lint 2>&1 | mustmatch like "./bin/lint
tools/check-quality-ratchet.sh"
```

## Repository Release Gate Uses The Three Standard Gates

The routine release gate should compose the workspace-standard commands
directly. Keeping the dependency line visible prevents an obsolete shim or narrow
spec subset from replacing the standard `lint`, `test`, and `spec` gates.

```bash
awk '/^release-gate:/{print}' ../../Makefile | mustmatch like "release-gate: lint test spec"
```

## Repository Make Check Is Not A Public Target

BioMCP should not keep a compatibility `check` target now that March validates by
make-target convention. Operators should use the standard gates directly.

```bash
awk '/^check:/{print}' ../../Makefile | mustmatch not like "check:"
```

## Root Agent Guide Declares The Contract

A dispatched agent starts at the repository root. The root guide must declare
the executable contract path, the three gates, and the hybrid Rust/Python skill
rail without requiring the agent to infer them from stale docs.

```bash
cat ../../AGENTS.md 2>/dev/null | mustmatch like "spec/*.md
make lint
make test
make spec
rust-standards
python-standards
cli-design
mustmatch
testing-mindset"
```

## Runtime Artifacts Stay Ignored

March runtime state belongs outside git. The ignore rules should keep the local
`.march-runtime/` tree from appearing as a trackable repository path.

```bash
cat ../../.gitignore | mustmatch like ".march-runtime/"
```

## Public Streamable HTTP Demo Keeps The BRAF Workflow

The shipped Streamable HTTP demo is the public live walkthrough. It should keep
the documented discovery, variant evidence, and melanoma trial commands rather
than shrinking to the offline study fixture used by routine specs.

```bash
uv run --no-sync python3 - <<'PY' | mustmatch like 'biomcp search all --gene BRAF --disease melanoma --counts-only
biomcp get variant "BRAF V600E" clinvar
biomcp search trial -c melanoma --mutation "BRAF V600E" --limit 5'
import ast
from pathlib import Path

module = ast.parse(Path("../../examples/streamable-http/streamable_http_client.py").read_text())
for node in module.body:
    if isinstance(node, ast.Assign) and any(getattr(target, "id", None) == "WORKFLOW" for target in node.targets):
        print("\n".join(ast.literal_eval(node.value)))
        break
PY
```

## MCP Surface Spec Owns Its Offline Workflow

Routine MCP proof should not execute the public demo script. The spec owns its
fixture-backed local command so the demo can remain a live operator walkthrough.

```bash
sed '/Read-Only Boundaries and Charted Calls Stay Visible/q' ../../spec/surface/mcp.md | mustmatch not like 'examples/streamable-http/streamable_http_client.py'
```

## Spec Gates Use The Mustmatch Binary Runner

The executable spec gates should enter through the shared runner script and that
script should use the standalone `mustmatch test` binary. This keeps the routine
and live lane split visible while preventing the deleted pytest plugin from
remaining the real runner.

```bash
make -C ../.. -n spec 2>&1 | mustmatch like "scripts/run-specs.sh"
make -C ../.. -n spec-pr 2>&1 | mustmatch like "scripts/run-specs.sh"
make -C ../.. -n spec-contracts 2>&1 | mustmatch like "scripts/run-specs.sh"
make -C ../.. -n verify 2>&1 | mustmatch like "scripts/run-specs.sh"
find ../../scripts -maxdepth 1 -name run-specs.sh -type f -exec sed -n '1,240p' {} \; | mustmatch like "mustmatch test
--lang bash
--timeout 120
--timeout 180
SPEC_ROUTINE_PATHS
SPEC_LIVE_PATHS"
```

## Mustmatch Is No Longer A Python Dev Dependency

The binary cutover makes mustmatch a tool on `PATH`, not a Python package in the
repo development environment. The gate and dependency files should not retain
pytest-plugin flags or the temporary `0.0.4` pin.

```bash
sed -n '/mustmatch/p;/--mustmatch/p' ../../Makefile ../../pyproject.toml ../../tests/test_version_sync_script.py ../../uv.lock | mustmatch not like "mustmatch==0.0.4"
sed -n '/mustmatch/p;/--mustmatch/p' ../../Makefile ../../pyproject.toml ../../tests/test_version_sync_script.py ../../uv.lock | mustmatch not like 'specifier = "==0.0.4"'
sed -n '/mustmatch/p;/--mustmatch/p' ../../Makefile ../../pyproject.toml ../../tests/test_version_sync_script.py ../../uv.lock | mustmatch not like "mustmatch-lang"
sed -n '/mustmatch/p;/--mustmatch/p' ../../Makefile ../../pyproject.toml ../../tests/test_version_sync_script.py ../../uv.lock | mustmatch not like "mustmatch-timeout"
```
