#!/usr/bin/env bash
set -euo pipefail

workspace_root="${1:-$PWD}"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cache_dir="$workspace_root/.cache"
fixture_src="$script_dir/who-pq"
fixture_root="$cache_dir/spec-who-pq"

rm -rf "$fixture_root"
mkdir -p "$cache_dir"
cp -R "$fixture_src" "$fixture_root"
find "$fixture_root" -type f -exec touch {} +

printf 'export BIOMCP_WHO_DIR=%q\n' "$fixture_root" > "$cache_dir/spec-who-pq-env"
printf '%s\n' "$fixture_root"
