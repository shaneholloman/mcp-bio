#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SKILLS_DIR="$REPO_ROOT/skills"
EXAMPLES_DIR="$SKILLS_DIR/examples"
SCHEMAS_DIR="$SKILLS_DIR/schemas"
JQ_EXAMPLES_FILE="$SKILLS_DIR/jq-examples.md"

PASS=0
FAIL=0
SKIP=0

log_pass() {
  PASS=$((PASS + 1))
  printf 'PASS: %s\n' "$1"
}

log_fail() {
  FAIL=$((FAIL + 1))
  printf 'FAIL: %s\n' "$1"
}

log_skip() {
  SKIP=$((SKIP + 1))
  printf 'SKIP: %s\n' "$1"
}

run_check() {
  local desc="$1"
  shift
  if "$@" >/dev/null; then
    log_pass "$desc"
  else
    log_fail "$desc"
  fi
}

require_command() {
  local cmd="$1"
  if command -v "$cmd" >/dev/null 2>&1; then
    return 0
  fi
  printf 'ERROR: required command not found: %s\n' "$cmd" >&2
  exit 1
}

jsonschema_validate_file() {
  local schema_file="$1"
  local data_file="$2"
  python3 - "$schema_file" "$data_file" <<'PY'
import json
import pathlib
import sys

try:
    from jsonschema import Draft202012Validator
except ImportError as exc:
    raise SystemExit(
        "python package 'jsonschema' is required (pip install jsonschema)"
    ) from exc

schema_path = pathlib.Path(sys.argv[1])
data_path = pathlib.Path(sys.argv[2])
with schema_path.open("r", encoding="utf-8") as fh:
    schema = json.load(fh)
with data_path.open("r", encoding="utf-8") as fh:
    data = json.load(fh)

Draft202012Validator.check_schema(schema)
Draft202012Validator(schema).validate(data)
PY
}

validate_search_rows() {
  local schema_file="$1"
  local search_file="$2"
  local tmp_row
  tmp_row="$(mktemp)"
  # Validate each result row against the entity schema.
  jq -c '.results[]' "$search_file" | while IFS= read -r row; do
    printf '%s\n' "$row" >"$tmp_row"
    jsonschema_validate_file "$schema_file" "$tmp_row"
  done
  rm -f "$tmp_row"
}

validate_drug_search_rows() {
  local schema_file="$1"
  local search_file="$2"
  local tmp_row
  tmp_row="$(mktemp)"
  jq -c '.regions | to_entries[] | .value.results[]' "$search_file" | while IFS= read -r row; do
    printf '%s\n' "$row" >"$tmp_row"
    jsonschema_validate_file "$schema_file" "$tmp_row"
  done
  rm -f "$tmp_row"
}

validate_search_wrapper() {
  local search_file="$1"
  jq -e '
    (.pagination | type == "object") and
    (.count | type == "number" and . > 0) and
    (.results | type == "array" and length > 0) and
    (.pagination.returned | type == "number" and . > 0)
  ' "$search_file"
}

validate_drug_search_wrapper() {
  local search_file="$1"
  jq -e '
    (.region | type == "string") and
    (.regions | type == "object" and length > 0) and
    ([.regions[] |
      (.pagination | type == "object") and
      (.count | type == "number" and . > 0) and
      (.results | type == "array" and length > 0) and
      (.pagination.returned | type == "number" and . > 0)
    ] | all) and
    (has("pagination") | not) and
    (has("count") | not) and
    (has("results") | not) and
    (has("query") | not)
  ' "$search_file"
}

run_jq_example_line() {
  local line="$1"
  local biomcp_dir="${2:-}"
  local output
  local compact

  if [ -n "$biomcp_dir" ]; then
    if ! output="$(PATH="$biomcp_dir:$PATH" bash -lc "$line" 2>&1)"; then
      printf 'Command failed: %s\n%s\n' "$line" "$output" >&2
      return 1
    fi
  elif ! output="$(bash -lc "$line" 2>&1)"; then
    printf 'Command failed: %s\n%s\n' "$line" "$output" >&2
    return 1
  fi

  compact="$(printf '%s' "$output" | tr -d '[:space:]')"
  if [ -z "$compact" ]; then
    printf 'Command returned empty output: %s\n' "$line" >&2
    return 1
  fi
  if [ "$compact" = "null" ] || [ "$compact" = "[]" ] || [ "$compact" = "{}" ]; then
    printf 'Command returned null/empty JSON output: %s\n' "$line" >&2
    return 1
  fi
  return 0
}

discover_biomcp() {
  if [ -n "${BIOMCP_BIN:-}" ] && [ -x "${BIOMCP_BIN}" ]; then
    printf '%s\n' "$BIOMCP_BIN"
    return 0
  fi

  if [ -x "$REPO_ROOT/target/release/biomcp" ]; then
    printf '%s\n' "$REPO_ROOT/target/release/biomcp"
    return 0
  fi

  if command -v biomcp >/dev/null 2>&1; then
    command -v biomcp
    return 0
  fi

  return 1
}

collect_live_payloads() {
  local biomcp_bin="$1"
  local out_dir="$2"
  local stderr_file="$3"

  "$biomcp_bin" --json get gene BRAF >"$out_dir/gene.json" 2>"$stderr_file" || return 1
  "$biomcp_bin" --json get variant rs113488022 all >"$out_dir/variant.json" 2>"$stderr_file" || return 1
  "$biomcp_bin" --json get trial NCT02576665 >"$out_dir/trial.json" 2>"$stderr_file" || return 1
  "$biomcp_bin" --json get article 21639808 >"$out_dir/article.json" 2>"$stderr_file" || return 1
  "$biomcp_bin" --json get drug vemurafenib >"$out_dir/drug.json" 2>"$stderr_file" || return 1
  "$biomcp_bin" --json get disease melanoma >"$out_dir/disease.json" 2>"$stderr_file" || return 1
  "$biomcp_bin" --json get pathway R-HSA-6802949 >"$out_dir/pathway.json" 2>"$stderr_file" || return 1
}

require_command jq
require_command python3

printf '== Tier 1: static JSON/schema checks ==\n'
for file in "$EXAMPLES_DIR"/*.json "$SCHEMAS_DIR"/*.json; do
  run_check "valid JSON: $(basename "$file")" jq empty "$file"
done

for schema_file in "$SCHEMAS_DIR"/*.json; do
  run_check "valid JSON Schema: $(basename "$schema_file")" \
    python3 - "$schema_file" <<'PY'
import json
import pathlib
import sys

try:
    from jsonschema import Draft202012Validator
except ImportError as exc:
    raise SystemExit(
        "python package 'jsonschema' is required (pip install jsonschema)"
    ) from exc

path = pathlib.Path(sys.argv[1])
with path.open("r", encoding="utf-8") as fh:
    schema = json.load(fh)
Draft202012Validator.check_schema(schema)
PY
done

run_check "example get-gene matches gene schema" \
  jsonschema_validate_file "$SCHEMAS_DIR/gene.json" "$EXAMPLES_DIR/get-gene-BRAF.json"
run_check "example get-variant matches variant schema" \
  jsonschema_validate_file "$SCHEMAS_DIR/variant.json" "$EXAMPLES_DIR/get-variant-rs113488022-all.json"
run_check "example get-trial matches trial schema" \
  jsonschema_validate_file "$SCHEMAS_DIR/trial.json" "$EXAMPLES_DIR/get-trial.json"

run_check "search-article wrapper shape" \
  validate_search_wrapper "$EXAMPLES_DIR/search-article.json"
run_check "search-drug wrapper shape" \
  validate_drug_search_wrapper "$EXAMPLES_DIR/search-drug.json"

run_check "search-article rows match article schema" \
  validate_search_rows "$SCHEMAS_DIR/article.json" "$EXAMPLES_DIR/search-article.json"
run_check "search-drug rows match drug schema" \
  validate_drug_search_rows "$SCHEMAS_DIR/drug.json" "$EXAMPLES_DIR/search-drug.json"

printf '\n== Tier 2-3: live payload + jq example checks ==\n'
if BIOMCP_BIN_DISCOVERED="$(discover_biomcp)"; then
  TMP_DIR="$(mktemp -d)"
  trap 'rm -rf "$TMP_DIR"' EXIT

  if collect_live_payloads "$BIOMCP_BIN_DISCOVERED" "$TMP_DIR" "$TMP_DIR/live.stderr"; then
    for entity in gene variant trial article drug disease pathway; do
      run_check "live $entity payload matches schema" \
        jsonschema_validate_file "$SCHEMAS_DIR/$entity.json" "$TMP_DIR/$entity.json"
    done

    BIOMCP_DIR="$(dirname "$BIOMCP_BIN_DISCOVERED")"
    while IFS= read -r line; do
      [ -z "$line" ] && continue
      run_check "jq example output: ${line}" run_jq_example_line "$line" "$BIOMCP_DIR"
    done < <(awk '/^```bash/{in_block=1; next} /^```/{in_block=0} in_block && /^biomcp / {print}' "$JQ_EXAMPLES_FILE")
  else
    log_skip "live payload checks (network/source access unavailable)"
    if [ "${VALIDATE_SKILLS_REQUIRE_LIVE:-0}" = "1" ]; then
      cat "$TMP_DIR/live.stderr" >&2
      log_fail "live payload checks required but failed"
    fi
  fi
else
  log_skip "live payload + jq example checks (biomcp binary not found)"
fi

printf '\nSummary: %d passed, %d failed, %d skipped\n' "$PASS" "$FAIL" "$SKIP"
if [ "$FAIL" -ne 0 ]; then
  exit 1
fi
