#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

SPEC_ROUTINE_PATHS=(
  spec/entity/article.md
  spec/entity/study.md
  spec/entity/variant.md
  spec/surface/mcp.md
  spec/surface/request-plan-ratchets.md
  spec/surface/test_architecture_docs_parity_contract.py
  spec/surface/test_biomcp_ci_path_contract.py
  spec/surface/test_complexportal_fixture_contract.py
  spec/surface/test_parallel_isolation_contract.py
  spec/surface/test_search_all_cli_structure.py
  spec/surface/test_semantic_scholar_retry_after_contract.py
  spec/surface/test_ticket_401_surface_ratchets.py
  spec/surface/test_trial_help_contract.py
  spec/surface/test_variant_normalization_docs_contract.py
)

SPEC_LIVE_PATHS=(
  spec/entity/diagnostic.md
  spec/entity/disease.md
  spec/entity/drug.md
  spec/entity/gene.md
  spec/entity/pathway.md
  spec/entity/pgx.md
  spec/entity/phenotype.md
  spec/entity/protein.md
  spec/entity/trial.md
  spec/entity/vaers.md
  spec/surface/cli.md
  spec/surface/discover.md
)

usage() {
  echo "usage: scripts/run-specs.sh <spec|spec-pr|spec-contracts|verify>" >&2
}

mustmatch_dir() {
  local candidate version
  for candidate in "${MUSTMATCH_BIN:-}" "$HOME/.local/bin/mustmatch" "$(command -v mustmatch 2>/dev/null || true)"; do
    if [[ -n "$candidate" && -x "$candidate" ]]; then
      version="$("$candidate" --version 2>/dev/null || true)"
      case "$version" in
        "mustmatch 0.0.4"*) ;;
        "mustmatch "*) dirname "$candidate"; return 0 ;;
      esac
    fi
  done
  echo "standalone mustmatch binary not found on PATH or at ~/.local/bin/mustmatch" >&2
  return 1
}

partition_paths() {
  MD_PATHS=()
  PY_PATHS=()
  local path
  for path in "$@"; do
    case "$path" in
      *.md) MD_PATHS+=("$path") ;;
      *.py) PY_PATHS+=("$path") ;;
      *) echo "unsupported spec path extension: $path" >&2; return 1 ;;
    esac
  done
}

source_if_present() {
  local path="$1"
  if [[ -f "$path" ]]; then
    # shellcheck source=/dev/null
    . "$path"
  fi
}

sync_python_dev() {
  uv sync --extra dev --no-install-project
}

run_study_fixture() {
  bash spec/fixtures/setup-study-spec-fixture.sh "$ROOT"
  source_if_present "$ROOT/.cache/spec-study-env"
}

run_ddinter_fixture() {
  bash spec/fixtures/setup-ddinter-spec-fixture.sh "$ROOT"
  source_if_present "$ROOT/.cache/spec-ddinter-env"
}

run_markdown_specs() {
  if ((${#MD_PATHS[@]})); then
    mustmatch test "${MD_PATHS[@]}" --lang bash "${timeout_args[@]}"
  fi
}

run_python_contracts() {
  if ((${#PY_PATHS[@]})); then
    uv run --no-sync pytest "${PY_PATHS[@]}" -v
  fi
}

mode="${1:-}"
run_python=0
case "$mode" in
  spec)
    timeout_args=(--timeout 120)
    paths=("${SPEC_ROUTINE_PATHS[@]}")
    run_python=1
    mustmatch_path_dir="$(mustmatch_dir)"
    sync_python_dev
    run_study_fixture
    run_ddinter_fixture
    ;;
  spec-pr)
    timeout_args=(--timeout 180)
    paths=("${SPEC_ROUTINE_PATHS[@]}")
    run_python=1
    mustmatch_path_dir="$(mustmatch_dir)"
    sync_python_dev
    run_study_fixture
    run_ddinter_fixture
    ;;
  spec-contracts)
    timeout_args=(--timeout 180)
    paths=(
      spec/entity/article.md
      spec/surface/mcp.md
      spec/surface/request-plan-ratchets.md
      spec/surface/test_parallel_isolation_contract.py
    )
    run_python=1
    mustmatch_path_dir="$(mustmatch_dir)"
    sync_python_dev
    run_study_fixture
    ;;
  verify)
    timeout_args=(--timeout 180)
    paths=("${SPEC_LIVE_PATHS[@]}")
    mustmatch_path_dir="$(mustmatch_dir)"
    ;;
  *)
    usage
    exit 2
    ;;
esac

export BIOMCP_BIN="$ROOT/target/release/biomcp"
export PATH="$mustmatch_path_dir:$ROOT/target/release:$PATH"

partition_paths "${paths[@]}"
run_markdown_specs
if ((run_python)); then
  run_python_contracts
fi
