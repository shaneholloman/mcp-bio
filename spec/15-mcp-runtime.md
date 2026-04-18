# MCP Runtime Help

BioMCP exposes a stdio MCP server for local agent use and a Streamable HTTP
server for remote/shared deployments. This file documents the help contract
users see when choosing between those runtime modes.

## Streamable HTTP Help

`serve-http --help` should describe the current transport honestly and point
operators at the canonical HTTP path.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
out="$("$bin" serve-http --help)"
echo "$out" | mustmatch like "Streamable HTTP"
echo "$out" | mustmatch like "Run the MCP Streamable HTTP server at /mcp"
echo "$out" | mustmatch like "Host address to bind"
echo "$out" | mustmatch like "Port to listen on"
echo "$out" | mustmatch not like "--json"
echo "$out" | mustmatch not like "--no-cache"
```

## Top-Level Discovery

`biomcp --help` should list the current Streamable HTTP transport, include the
local-only `cache` family for operators, and omit the legacy SSE migration shim
from first-run discovery.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
out="$("$bin" --help)"
echo "$out" | mustmatch like "serve-http"
echo "$out" | mustmatch like "leading public biomedical data sources"
echo "$out" | mustmatch like "cache       Inspect the managed HTTP cache"
echo "$out" | mustmatch not like "15 biomedical sources"
echo "$out" | mustmatch not like "serve-sse"
```

## Legacy SSE Help

`serve-sse --help` exists only as a migration aid and should point back to
`serve-http`.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
out="$("$bin" serve-sse --help)"
echo "$out" | mustmatch like "removed legacy SSE compatibility command"
echo "$out" | mustmatch like "serve-http"
echo "$out" | mustmatch like 'connect remote clients to `/mcp` instead.'
echo "$out" | mustmatch not like "--json"
echo "$out" | mustmatch not like "--no-cache"
```

## Stdio Tool Identity

The stdio MCP handshake should advertise the BioMCP execution tool as
`biomcp`, not the old `shell` name.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
out="$( (printf '%s\n%s\n%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"spec","version":"0.1"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'; \
  sleep 1) | "$bin" serve 2>/dev/null)"
echo "$out" | mustmatch like '"name":"biomcp"'
echo "$out" | mustmatch not like '"name":"shell"'
```

## Read-only Study Boundary

The stdio MCP server must reject mutating study installs before any
download/install path runs. The exact `study download --list` catalog form
remains part of the safe MCP contract, but this spec does not execute it
because it depends on live remote network access.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
out="$( (printf '%s\n%s\n%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"spec","version":"0.1"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"biomcp","arguments":{"command":"biomcp study download msk_impact_2017"}}}'; \
  sleep 1) | "$bin" serve 2>/dev/null)"
echo "$out" | mustmatch like '"isError":true'
echo "$out" | mustmatch like "BioMCP allows read-only commands only"
```

## GTR Sync Stays CLI-only

`biomcp gtr sync` mutates the local runtime root, so MCP must reject it instead
of advertising or executing it remotely.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
out="$( (printf '%s\n%s\n%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"spec","version":"0.1"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"biomcp","arguments":{"command":"biomcp gtr sync"}}}'; \
  sleep 1) | "$bin" serve 2>/dev/null)"
echo "$out" | mustmatch like '"isError":true'
echo "$out" | mustmatch like "BioMCP allows read-only commands only"
```

## Cache Family Stays CLI-only

The MCP runtime must reject `biomcp cache path` even though it is read-only,
because the command reveals workstation-local filesystem paths.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
out="$( (printf '%s\n%s\n%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"spec","version":"0.1"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"biomcp","arguments":{"command":"biomcp cache path"}}}'; \
  sleep 1) | "$bin" serve 2>/dev/null)"
echo "$out" | mustmatch like '"isError":true'
echo "$out" | mustmatch like "CLI-only over MCP"
echo "$out" | mustmatch like "workstation-local filesystem paths"
```

`biomcp cache stats` stays on the same CLI-only side of that boundary. The
report includes the same workstation-local cache path context, so MCP must
reject it rather than advertise or execute it remotely.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
out="$( (printf '%s\n%s\n%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"spec","version":"0.1"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"biomcp","arguments":{"command":"biomcp cache stats"}}}'; \
  sleep 1) | "$bin" serve 2>/dev/null)"
echo "$out" | mustmatch like '"isError":true'
echo "$out" | mustmatch like "CLI-only over MCP"
echo "$out" | mustmatch like "workstation-local filesystem paths"
```

`biomcp cache clean` is also CLI-only. Even though it targets local maintenance,
MCP must reject it with the same cache-family boundary message instead of exposing
operator cache cleanup remotely.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
out="$( (printf '%s\n%s\n%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"spec","version":"0.1"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"biomcp","arguments":{"command":"biomcp cache clean"}}}'; \
  sleep 1) | "$bin" serve 2>/dev/null)"
echo "$out" | mustmatch like '"isError":true'
echo "$out" | mustmatch like "CLI-only over MCP"
echo "$out" | mustmatch like "workstation-local filesystem paths"
```

`biomcp cache clear` stays on that same CLI-only side of the boundary. Even
with `--yes`, it is a destructive local filesystem wipe and must be rejected
instead of being exposed through MCP.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
out="$( (printf '%s\n%s\n%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"spec","version":"0.1"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"biomcp","arguments":{"command":"biomcp cache clear"}}}'; \
  sleep 1) | "$bin" serve 2>/dev/null)"
echo "$out" | mustmatch like '"isError":true'
echo "$out" | mustmatch like "CLI-only over MCP"
echo "$out" | mustmatch like "workstation-local filesystem paths"
```
