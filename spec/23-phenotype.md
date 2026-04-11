# Phenotype

`search phenotype` accepts either canonical HPO IDs or symptom phrases that
resolve to HPO IDs before Monarch similarity search.

| Section | Command focus | Why it matters |
|---|---|---|
| HPO IDs | `search phenotype "HP:0001250 HP:0001263" --limit 3` | Confirms canonical HPO identifiers remain a stable contract |
| Symptom phrases | `search phenotype "seizure, developmental delay" --limit 3` | Confirms comma-separated symptom text resolves before ranking diseases |
| Top disease follow-up | `search phenotype "HP:0002373 HP:0001250" --limit 3` | Reuses the top-ranked disease as the next structured command in markdown only |

## HPO IDs

Canonical HPO identifiers remain the most direct way to run phenotype
similarity search, so the rendered search header should preserve the IDs.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" search phenotype "HP:0001250 HP:0001263" --limit 3)"
echo "$out" | mustmatch like "# Phenotype Search: HP:0001250 HP:0001263"
echo "$out" | mustmatch like "| Disease ID | Disease Name | Similarity Score |"
printf '%s\n' "$out" | grep -Eq '^\| MONDO:'
```

## Symptom phrases

Free-text symptom phrases should still resolve before ranking diseases, so this
section checks the user-visible search header and the presence of disease rows.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" search phenotype "seizure, developmental delay" --limit 3)"
echo "$out" | mustmatch like "# Phenotype Search: seizure, developmental delay"
echo "$out" | mustmatch like "| Disease ID | Disease Name | Similarity Score |"
printf '%s\n' "$out" | grep -Eq '^\| MONDO:'
```

## Top disease follow-up

Markdown phenotype search should turn the first ranked disease row into the next
typed disease command, while JSON keeps the generic search-response shape.

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" search phenotype "HP:0002373 HP:0001250" --limit 3)"
top_disease="$(printf '%s\n' "$out" | awk -F'|' '/^\|/ && $2 !~ /Disease ID/ && $2 !~ /---/ {gsub(/^ +| +$/, "", $3); print $3; exit}')"
test -n "$top_disease"
echo "$out" | mustmatch like "See also:"
echo "$out" | mustmatch like "biomcp get disease \"$top_disease\" genes phenotypes"
```

```bash
bin="${BIOMCP_BIN:-biomcp}"
out="$("$bin" --json search phenotype "HP:0002373 HP:0001250" --limit 1)"
echo "$out" | jq -e '.results | type == "array" and length == 1' > /dev/null
echo "$out" | jq -e 'has("_meta") | not' > /dev/null
```
