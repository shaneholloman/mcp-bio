# Cache Commands

`biomcp cache path`, `biomcp cache stats`, `biomcp cache clean`, and
`biomcp cache clear` are the local operator commands for the managed HTTP cache.
One locates the resolved cache directory, one reports local cache inventory and
configured limits, one safely removes orphan blobs plus optional age/size
evictions, and one destructively wipes the managed `http/` tree. They stay
CLI-only because they expose workstation-local filesystem paths.

The only `cache clear` behavior intentionally proven outside this markdown spec
is the interactive TTY accept/decline flow; that contract is covered by the
Unix-only Rust PTY tests.

## Cache Path

The command should print `<resolved cache_root>/http` exactly, using the same
cache-root resolution rules as runtime HTTP caching while avoiding directory
creation or migration side effects.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
tmp_root="$(mktemp -d)"
trap 'rm -rf "$tmp_root"' EXIT
mkdir -p "$tmp_root/cache-home" "$tmp_root/config-home/biomcp"
cat >"$tmp_root/config-home/biomcp/cache.toml" <<'EOF'
[cache]
dir = "relative-cache"
EOF

out="$(env XDG_CACHE_HOME="$tmp_root/cache-home" XDG_CONFIG_HOME="$tmp_root/config-home" "$bin" cache path)"
echo "$out" | mustmatch like "relative-cache/http"
test ! -d "$tmp_root/relative-cache"
```

## JSON Flag Exception

`biomcp cache path` is a documented exception to the usual query-command JSON
contract. Even under the global `--json` flag, it must print the same plain-text
path instead of a JSON object.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
tmp_root="$(mktemp -d)"
trap 'rm -rf "$tmp_root"' EXIT
mkdir -p "$tmp_root/cache-home" "$tmp_root/config-home"

plain="$(env XDG_CACHE_HOME="$tmp_root/cache-home" XDG_CONFIG_HOME="$tmp_root/config-home" "$bin" cache path)"
json_flag="$(env XDG_CACHE_HOME="$tmp_root/cache-home" XDG_CONFIG_HOME="$tmp_root/config-home" "$bin" --json cache path)"

test "$plain" = "$json_flag"
echo "$json_flag" | mustmatch like "$tmp_root/cache-home/biomcp/http"
echo "$json_flag" | mustmatch not like "{"
```

## Cache Stats JSON

`biomcp cache stats --json` is the machine-readable companion to `cache path`.
On a fresh XDG cache root it should report the resolved cache path, zeroed local
blob counts, and the default cache-policy limits in one JSON object.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
tmp_root="$(mktemp -d)"
trap 'rm -rf "$tmp_root"' EXIT
mkdir -p "$tmp_root/cache-home" "$tmp_root/config-home"
out="$(env XDG_CACHE_HOME="$tmp_root/cache-home" XDG_CONFIG_HOME="$tmp_root/config-home" "$bin" --json cache stats)"
echo "$out" | mustmatch like '"max_age_origin": "default"'
echo "$out" | jq -e --arg path "$tmp_root/cache-home/biomcp/http" '
  . == {
    path: $path,
    blob_bytes: 0,
    referenced_blob_bytes: 0,
    blob_count: 0,
    orphan_count: 0,
    age_range: null,
    max_size_bytes: 10000000000,
    max_size_origin: "default",
    min_disk_free: "10%",
    min_disk_free_origin: "default",
    max_age_secs: 86400,
    max_age_origin: "default"
  }
' > /dev/null
```

## Cache Stats Markdown

Default `cache stats` output is the operator-facing markdown summary. The empty
cache fixture should still render every visible row, including the configured
limit rows that explain the current size and age policy.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
tmp_root="$(mktemp -d)"
trap 'rm -rf "$tmp_root"' EXIT
mkdir -p "$tmp_root/cache-home" "$tmp_root/config-home"
out="$(env XDG_CACHE_HOME="$tmp_root/cache-home" XDG_CONFIG_HOME="$tmp_root/config-home" "$bin" cache stats)"
echo "$out" | mustmatch like "| Path | $tmp_root/cache-home/biomcp/http |"
echo "$out" | mustmatch like "| Blob bytes | 0 |"
echo "$out" | mustmatch like "| Referenced blob bytes | 0 |"
echo "$out" | mustmatch like "| Blob files | 0 |"
echo "$out" | mustmatch like "| Orphan blobs | 0 |"
echo "$out" | mustmatch like "| Age range | none |"
echo "$out" | mustmatch like "| Max size | 10000000000 bytes (default) |"
echo "$out" | mustmatch like "| Min disk free | 10% (default) |"
echo "$out" | mustmatch like "| Max age | 86400 s (default) |"
```

## Cache Clean JSON

`biomcp cache clean --json` should expose the stable machine contract for cleanup
reports on an empty cache, including the dry-run flag and an explicit error list.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
tmp_root="$(mktemp -d)"
trap 'rm -rf "$tmp_root"' EXIT
mkdir -p "$tmp_root/cache-home" "$tmp_root/config-home"
out="$(env XDG_CACHE_HOME="$tmp_root/cache-home" XDG_CONFIG_HOME="$tmp_root/config-home" "$bin" --json cache clean)"
echo "$out" | mustmatch like '"dry_run": false'
echo "$out" | jq -e --argjson dry_run false '
  . == {
    dry_run: $dry_run,
    orphans_removed: 0,
    entries_removed: 0,
    bytes_freed: 0,
    errors: []
  }
' > /dev/null
```

## Cache Clean Summary

Default `cache clean` output is a single operator summary line rather than a
markdown block.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
tmp_root="$(mktemp -d)"
trap 'rm -rf "$tmp_root"' EXIT
mkdir -p "$tmp_root/cache-home" "$tmp_root/config-home"
out="$(env XDG_CACHE_HOME="$tmp_root/cache-home" XDG_CONFIG_HOME="$tmp_root/config-home" "$bin" cache clean)"
echo "$out" | mustmatch like "Cache clean: dry_run=false orphans_removed=0 entries_removed=0 bytes_freed=0 errors=0"
test "$(printf '%s\n' "$out" | wc -l | tr -d ' ')" = "1"
```

## Cache Clean Dry Run

`--dry-run` should keep the same structured report shape while marking the run as
planned-only.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
tmp_root="$(mktemp -d)"
trap 'rm -rf "$tmp_root"' EXIT
mkdir -p "$tmp_root/cache-home" "$tmp_root/config-home"
out="$(env XDG_CACHE_HOME="$tmp_root/cache-home" XDG_CONFIG_HOME="$tmp_root/config-home" "$bin" --json cache clean --dry-run)"
echo "$out" | mustmatch like '"dry_run": true'
echo "$out" | jq -e '.dry_run == true and .orphans_removed == 0 and .entries_removed == 0 and .bytes_freed == 0 and (.errors | length) == 0' > /dev/null
```

## Cache Clean Flags

<!-- mustmatch-lint: skip -->

The operator cleanup flags should parse together on an empty cache so scripts can
preview targeted cleanup without a seeded fixture.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
tmp_root="$(mktemp -d)"
trap 'rm -rf "$tmp_root"' EXIT
mkdir -p "$tmp_root/cache-home" "$tmp_root/config-home"
env XDG_CACHE_HOME="$tmp_root/cache-home" XDG_CONFIG_HOME="$tmp_root/config-home" \
  "$bin" cache clean --max-age 30d --max-size 500M --dry-run > /dev/null
```

## Cache Clear Refuses Non-Interactive Destructive Runs

`biomcp cache clear` is destructive, so non-interactive use must refuse unless
the operator opts in with `--yes`. That refusal stays on plain stderr even when
the global `--json` flag is present.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
tmp_root="$(mktemp -d)"
trap 'rm -rf "$tmp_root"' EXIT
mkdir -p "$tmp_root/cache-home" "$tmp_root/config-home"
set +e
env XDG_CACHE_HOME="$tmp_root/cache-home" XDG_CONFIG_HOME="$tmp_root/config-home" \
  "$bin" --json cache clear >"$tmp_root/stdout" 2>"$tmp_root/stderr"
status=$?
set -e
test "$status" = "1"
test ! -s "$tmp_root/stdout"
stderr="$(cat "$tmp_root/stderr")"
echo "$stderr" | mustmatch like "cache clear requires a TTY or --yes for non-interactive use."
echo "$stderr" | mustmatch not like "{"
```

## Cache Clear Supports Full-Wipe Automation with --yes

`--yes` is the automation escape hatch for the destructive full wipe. When the
managed `http/` tree exists, the command should remove it completely.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
tmp_root="$(mktemp -d)"
trap 'rm -rf "$tmp_root"' EXIT
mkdir -p "$tmp_root/cache-home/biomcp/http/nested" "$tmp_root/config-home"
printf 'cache-data' >"$tmp_root/cache-home/biomcp/http/nested/entry.bin"
out="$(env XDG_CACHE_HOME="$tmp_root/cache-home" XDG_CONFIG_HOME="$tmp_root/config-home" \
  "$bin" cache clear --yes)"
echo "$out" | mustmatch like "Cache clear: bytes_freed="
test ! -e "$tmp_root/cache-home/biomcp/http"
```

## Cache Clear Reports Machine-Readable Results

Successful `cache clear --json` output keeps the report shape stable for scripts:
exactly `{ bytes_freed, entries_removed }`.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
tmp_root="$(mktemp -d)"
trap 'rm -rf "$tmp_root"' EXIT
mkdir -p "$tmp_root/cache-home/biomcp/http" "$tmp_root/config-home"
printf '12345' >"$tmp_root/cache-home/biomcp/http/entry.bin"
out="$(env XDG_CACHE_HOME="$tmp_root/cache-home" XDG_CONFIG_HOME="$tmp_root/config-home" "$bin" --json cache clear --yes)"
echo "$out" | mustmatch like '"bytes_freed": 5'
echo "$out" | mustmatch like '"entries_removed": 2'
```

## Cache Clear Is Idempotent When the HTTP Cache Is Already Gone

Running `cache clear --yes` against a missing managed `http/` directory should
be a no-op with the zero-removal report.

```bash
bin="$(git rev-parse --show-toplevel)/target/release/biomcp"
tmp_root="$(mktemp -d)"
trap 'rm -rf "$tmp_root"' EXIT
mkdir -p "$tmp_root/cache-home" "$tmp_root/config-home"
out="$(env XDG_CACHE_HOME="$tmp_root/cache-home" XDG_CONFIG_HOME="$tmp_root/config-home" "$bin" --json cache clear --yes)"
echo "$out" | mustmatch like '"bytes_freed": 0'
echo "$out" | mustmatch like '"entries_removed": 0'
```
