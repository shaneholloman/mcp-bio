#!/usr/bin/env bash
set -euo pipefail

ROOT="${1:?repo root required}"
FIXTURE_PID=""
cleanup() {
  if [[ -n "${FIXTURE_PID:-}" ]]; then
    kill "$FIXTURE_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

bash "$ROOT/spec/fixtures/setup-article-federated-timeout-fixture.sh" "$ROOT"
# shellcheck source=/dev/null
. "$ROOT/.cache/spec-article-federated-timeout-env"
FIXTURE_PID="${BIOMCP_ARTICLE_FEDERATED_TIMEOUT_FIXTURE_PID:-}"

BIOMCP_CACHE_DIR="$ROOT/.cache/biomcp-article-federated-timeout" \
  timeout 25s "$ROOT/tools/biomcp-ci" search article -k "BRAF melanoma" --source all --debug-plan --limit 3
