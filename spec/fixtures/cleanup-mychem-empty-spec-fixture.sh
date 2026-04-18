#!/usr/bin/env bash
set -euo pipefail

workspace_root="${1:-$PWD}"
cache_dir="$workspace_root/.cache"
env_file="$cache_dir/spec-mychem-empty-env"

if [ ! -f "$env_file" ]; then
  exit 0
fi

set +u
# shellcheck disable=SC1090
. "$env_file"
set -u

pid_matches_fixture() {
  local pid="$1"
  local ready_file="$2"

  [ -r "/proc/$pid/cmdline" ] || return 1
  tr '\0' '\n' <"/proc/$pid/cmdline" | grep -Fqx -- "$ready_file"
}

if [ -n "${BIOMCP_MYCHEM_EMPTY_PID:-}" ] \
  && [ -n "${BIOMCP_MYCHEM_EMPTY_READY_FILE:-}" ] \
  && kill -0 "$BIOMCP_MYCHEM_EMPTY_PID" 2>/dev/null \
  && pid_matches_fixture "$BIOMCP_MYCHEM_EMPTY_PID" "$BIOMCP_MYCHEM_EMPTY_READY_FILE"; then
  kill "$BIOMCP_MYCHEM_EMPTY_PID" 2>/dev/null || true
fi

case "${BIOMCP_MYCHEM_EMPTY_ROOT:-}" in
  "$cache_dir"/spec-mychem-empty.*)
    rm -rf "$BIOMCP_MYCHEM_EMPTY_ROOT"
    ;;
esac

rm -f "$env_file"
