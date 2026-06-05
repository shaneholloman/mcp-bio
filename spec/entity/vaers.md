# VAERS Queries

The VAERS slice of BioMCP is an aggregate vaccine-safety view, not a case-level
report browser. These canaries keep vaccine-first routing, aggregate-only
reporting, source-specific limitations, and combined/default behavior visible.

## Source Selection Contract

The adverse-event surface should keep the VAERS source switch visible in help so
users can tell when they are asking for FAERS, VAERS, or the combined path.

```bash
../../target/release/biomcp search adverse-event --help | mustmatch like '--source <faers|vaers|all>
biomcp search adverse-event "COVID-19 vaccine" --source all --limit 5
biomcp search adverse-event "MMR vaccine" --source vaers --limit 5'
```

## Vaccine-Only Truthfulness

If the user forces the VAERS source for a non-vaccine query, BioMCP should say
that plainly instead of pretending the source searched nothing.

```bash
../../tools/biomcp-ci search adverse-event --drug aspirin --source vaers | mustmatch like 'Status: query_not_vaccine
VAERS is vaccine-only; this query did not resolve to a vaccine identity.'
```

## Source-Specific Limitations

FAERS-style filters should fail truthfully when the user forces the VAERS
source, instead of being silently ignored.

```bash
../../tools/biomcp-ci search adverse-event --drug 'COVID-19 vaccine' --source vaers --outcome death 2>&1 | mustmatch like '--source vaers only supports
unsupported flags: --outcome'
```
