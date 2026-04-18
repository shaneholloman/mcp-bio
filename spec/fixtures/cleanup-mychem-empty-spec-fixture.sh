#!/usr/bin/env bash
set -euo pipefail

workspace_root="${1:-$PWD}"
cache_dir="$workspace_root/.cache"
env_file="$cache_dir/spec-mychem-empty-env"

if [ -f "$env_file" ]; then
  # shellcheck disable=SC1090
  . "$env_file"
  if [ -n "${BIOMCP_MYCHEM_EMPTY_PID:-}" ] && kill -0 "$BIOMCP_MYCHEM_EMPTY_PID" 2>/dev/null; then
    kill "$BIOMCP_MYCHEM_EMPTY_PID" 2>/dev/null || true
    wait "$BIOMCP_MYCHEM_EMPTY_PID" 2>/dev/null || true
  fi
  if [ -n "${BIOMCP_MYCHEM_EMPTY_ROOT:-}" ] && [ -d "$BIOMCP_MYCHEM_EMPTY_ROOT" ]; then
    rm -rf "$BIOMCP_MYCHEM_EMPTY_ROOT"
  fi
  rm -f "$env_file"
fi
