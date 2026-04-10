# Discover

Use `biomcp discover` to resolve free-text biomedical phrases into the right
BioMCP follow-up commands. Run it when you know the phrase but do not yet know
whether the next step should be `get gene`, `search disease`, `search pathway`,
or another typed command.

Use `search all` after you already have typed slots such as `--gene`,
`--disease`, `--drug`, `--variant`, or `--keyword`. `discover` resolves free
text into concepts first; `search all` fans out from the typed slots you
already trust.

## Examples

```bash
biomcp discover ERBB1
biomcp discover Keytruda
biomcp discover "chest pain"
biomcp discover "developmental delay"
biomcp --json discover diabetes
```

## What it does

- Queries OLS4 for structured ontology-backed matches.
- Adds optional UMLS crosswalks when `UMLS_API_KEY` is set.
- Adds MedlinePlus plain-language context for disease and symptom queries.
- Suggests `biomcp search phenotype "HP:..."` first when symptom concepts
  resolve to HPO-backed IDs.
- Returns suggested BioMCP follow-up commands without auto-executing them.

## Output

Markdown groups concepts by type and shows suggested commands.

For symptom-first queries such as `biomcp discover "developmental delay"`,
discover can surface `HP:` identifiers directly in the concept list and suggest
`biomcp search phenotype "HP:..."` as the next command. Disease-specific
queries like `biomcp discover "symptoms of Marfan syndrome"` still route to
`biomcp get disease ... phenotypes`.

JSON preserves the same concepts and adds:

- `_meta.next_commands`
- `_meta.section_sources`
- `_meta.discovery_sources`
- `_meta.evidence_urls`

## Notes

- OLS4 is required; if it fails, `discover` fails.
- UMLS is optional. Without `UMLS_API_KEY`, discover still works and reports
  that clinical crosswalk enrichment is unavailable.
- MedlinePlus is supplemental and only shown for disease or symptom flows.
- Queries are sent to third-party biomedical APIs. Do not send PHI or other
  patient-identifying text.

## Related guides

- [Search All Workflow](../how-to/search-all-workflow.md)
- [Disease](disease.md)
- [Gene](gene.md)
