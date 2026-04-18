# VAERS Vaccine Adverse Events

This file locks down the public `search adverse-event --source <faers|vaers|all>`
contract for vaccine workflows. VAERS coverage is aggregate-only and should stay
explicit about what it can and cannot do.

| Section | Command focus | Why it matters |
|---|---|---|
| Help documents source modes | `search adverse-event --help` | Confirms the public `--source` grammar is discoverable |
| List documents VAERS scope | `list adverse-event` | Confirms unsupported filters and aggregate-only caveats are visible |
| VAERS-only markdown summary | `search adverse-event "MMR vaccine" --source vaers` | Confirms the aggregate CDC VAERS output shape |
| VAERS-only JSON contract | `--json search adverse-event "MMR vaccine" --source vaers` | Confirms the VAERS-first envelope and matched vaccine identity |
| Default combined vaccine search | `search adverse-event "COVID-19 vaccine"` | Confirms FAERS stays present while VAERS is appended additively |
| Unsupported filters skip VAERS in `all` mode | `--json search adverse-event "COVID-19 vaccine" --source all --reaction fever` | Confirms FAERS succeeds while VAERS is truthfully skipped |

## Help Documents Source Modes

The command help should advertise the new source grammar and explain the
vaccine-specific default behavior.

```bash
bin="${BIOMCP_BIN:-$(git rev-parse --show-toplevel)/target/release/biomcp}"
out="$("$bin" search adverse-event --help)"
echo "$out" | mustmatch like "--source <faers|vaers|all>"
echo "$out" | mustmatch like "combined OpenFDA FAERS + CDC VAERS"
echo "$out" | mustmatch like "aggregate-only"
```

## List Documents VAERS Scope

The list surface should explain when VAERS participates and which FAERS filters
do not carry over.

```bash
bin="${BIOMCP_BIN:-$(git rev-parse --show-toplevel)/target/release/biomcp}"
out="$("$bin" list adverse-event)"
echo "$out" | mustmatch like "--source <faers|vaers|all>"
echo "$out" | mustmatch like "search adverse-event <vaccine query> --source vaers"
echo "$out" | mustmatch like "supports plain vaccine query text"
echo "$out" | mustmatch like "query resolves to a vaccine"
echo "$out" | mustmatch like "VAERS intentionally does not support --reaction"
```

## VAERS-only Markdown Summary

The VAERS-only path should render a source-specific summary with vaccine
identity, counts, age buckets, and reaction rows.

```bash
bin="${BIOMCP_BIN:-$(git rev-parse --show-toplevel)/target/release/biomcp}"
bash fixtures/setup-vaers-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-vaers-env"
out="$("$bin" search adverse-event "MMR vaccine" --source vaers --limit 5)"
echo "$out" | mustmatch like "# Adverse Events: MMR vaccine"
echo "$out" | mustmatch like "## CDC VAERS Summary"
echo "$out" | mustmatch like "Matched vaccine: MMR"
echo "$out" | mustmatch like "CDC WONDER code: MMR"
echo "$out" | mustmatch like "CVX codes: 03, 94"
echo "$out" | mustmatch like "### Age distribution"
echo "$out" | mustmatch like "| Age bucket | Reports |"
echo "$out" | mustmatch like "### Top reactions"
echo "$out" | mustmatch like "Source: CDC VAERS"
```

## Influenza Family Queries Resolve To VAERS

The approved design called for a narrow family-alias fallback for common
queries such as influenza/flu. Those generic vaccine-family queries should
still resolve to a CDC WONDER code.

```bash
bin="${BIOMCP_BIN:-$(git rev-parse --show-toplevel)/target/release/biomcp}"
bash fixtures/setup-vaers-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-vaers-env"
out="$("$bin" search adverse-event "influenza vaccine" --source vaers --limit 5)"
echo "$out" | mustmatch like "Matched vaccine: Influenza vaccine"
echo "$out" | mustmatch like "CDC WONDER code: FLU"
```

## VAERS-only JSON Contract

The VAERS-only JSON path should return the VAERS-first envelope rather than the
FAERS search shape.

```bash
bin="${BIOMCP_BIN:-$(git rev-parse --show-toplevel)/target/release/biomcp}"
bash fixtures/setup-vaers-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-vaers-env"
json_out="$("$bin" --json search adverse-event "MMR vaccine" --source vaers --limit 5)"
echo "$json_out" | mustmatch like '"source": "vaers"'
echo "$json_out" | jq -e '.source == "vaers"' > /dev/null
echo "$json_out" | jq -e '.query == "MMR vaccine"' > /dev/null
echo "$json_out" | jq -e '.vaers.status == "ok"' > /dev/null
echo "$json_out" | jq -e '.vaers.matched_vaccine.display_name == "MMR"' > /dev/null
echo "$json_out" | jq -e '.vaers.matched_vaccine.wonder_code == "MMR"' > /dev/null
echo "$json_out" | jq -e '.vaers.matched_vaccine.cvx_codes == ["03", "94"]' > /dev/null
echo "$json_out" | jq -e '.vaers.summary.total_reports > 0' > /dev/null
echo "$json_out" | jq -e '.vaers.summary.age_distribution | length > 0' > /dev/null
echo "$json_out" | jq -e '.vaers.summary.top_reactions | length > 0' > /dev/null
```

## Default Combined Vaccine Search

The default vaccine path should keep the FAERS table and append the VAERS
aggregate summary additively.

```bash
bin="${BIOMCP_BIN:-$(git rev-parse --show-toplevel)/target/release/biomcp}"
bash fixtures/setup-vaers-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-vaers-env"
out="$("$bin" search adverse-event "COVID-19 vaccine" --limit 5)"
echo "$out" | mustmatch like "# Adverse Events: drug=COVID-19 vaccine"
echo "$out" | mustmatch like "Total reports (OpenFDA FAERS)"
echo "$out" | mustmatch like "|Report ID|Drug|Reactions|Serious|"
echo "$out" | mustmatch like "## CDC VAERS Summary"
echo "$out" | mustmatch like "Source: CDC VAERS"

json_out="$("$bin" --json search adverse-event "COVID-19 vaccine" --limit 5)"
echo "$json_out" | jq -e '.source == "all"' > /dev/null
echo "$json_out" | jq -e '.vaers.status == "ok"' > /dev/null
echo "$json_out" | jq -e '.summary.total_reports >= 0' > /dev/null
echo "$json_out" | jq -e '.results | type == "array"' > /dev/null
```

## Unsupported Filters Skip VAERS In `all` Mode

When the user asks for `--source all` with a FAERS-only filter, FAERS should
still run and JSON should record that VAERS was skipped instead of failing the
whole search.

```bash
bin="${BIOMCP_BIN:-$(git rev-parse --show-toplevel)/target/release/biomcp}"
bash fixtures/setup-vaers-spec-fixture.sh "$PWD"
. "$PWD/.cache/spec-vaers-env"
json_out="$("$bin" --json search adverse-event "COVID-19 vaccine" --source all --reaction fever --limit 5)"
echo "$json_out" | jq -e '.source == "all"' > /dev/null
echo "$json_out" | jq -e '.vaers.status == "unsupported_filters"' > /dev/null
echo "$json_out" | jq -e '.vaers.message | test("unsupported")' > /dev/null

md_out="$("$bin" search adverse-event "COVID-19 vaccine" --source all --reaction fever --limit 5)"
echo "$md_out" | mustmatch like "Total reports (OpenFDA FAERS)"
echo "$md_out" | mustmatch not like "## CDC VAERS Summary"
```
