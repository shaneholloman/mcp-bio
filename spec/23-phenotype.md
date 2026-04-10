# Phenotype

`search phenotype` accepts either canonical HPO IDs or symptom phrases that
resolve to HPO IDs before Monarch similarity search.

| Section | Command focus | Why it matters |
|---|---|---|
| HPO IDs | `search phenotype "HP:0001250 HP:0001263" --limit 3` | Confirms canonical HPO identifiers remain a stable contract |
| Symptom phrases | `search phenotype "seizure, developmental delay" --limit 3` | Confirms comma-separated symptom text resolves before ranking diseases |

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
