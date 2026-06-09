# Variant Queries

Variant workflows need to balance exact identity with search-time normalization.
These canaries keep the stable column contracts, normalization rules, and
opt-in clinical sections without depending on brittle row counts.

## Deterministic Source Contracts

Ticket 376 moves routine variant-source proof from live/cache-backed MyVariant
and normalization-service canaries to source-local request-plan and
fixture-backed contracts. Any irreducible public availability check belongs in
an explicit release/live-smoke lane; routine specs must instead prove MyVariant
search/get request shape, identifier normalization, and Mutalyzer/
VariantValidator status mapping locally.

```bash
cargo test --lib ticket_376_variant_source_contracts -- --nocapture \
  | mustmatch like 'ticket_376_variant_source_contracts'
```

```bash
cargo test --lib ticket_376_variant_normalization_contracts -- --nocapture \
  | mustmatch like 'ticket_376_variant_normalization_contracts'
```

## Deterministic Renderer Envelope Contracts

Ticket 377 moves routine variant renderer/envelope proof into fixture-result
contracts. The deterministic tests should cover variant search JSON
`_meta.next_commands`, markdown related anchors, and normalization JSON/markdown
per-service status, warnings, and genomic-description rendering without live
MyVariant, Mutalyzer, or VariantValidator calls.

```bash
cargo test --lib ticket_377_variant_renderer_envelope_contracts -- --nocapture \
  | mustmatch like 'ticket_377_variant_renderer_envelope_contracts'
```

## Gene-Scoped Variant Search

Gene-first search should still return the canonical variant identity columns and
preserve the BRAF V600E row as a recognizable anchor.

## Search Table Contract

The JSON path should keep the same follow-up shape so agents can pivot into the
default card without scraping markdown helper text.

## Protein-Filter Narrowing

Long-form protein filters should normalize to the same compact spelling that the
short-form query uses, rather than leaking a second variant identifier shape.

## Residue-Alias Search

Residue aliases should stay on the typed variant path instead of falling
through to free-text or disease-style fallback behavior.

## Clinical Significance

ClinVar remains an opt-in deepen path. The section should keep the human heading
and a compact JSON disease anchor without bloating the default card.

## Population Frequency

Population frequency also stays opt-in. The markdown and JSON views should keep
the same compact gnomAD frequency story.

## Variant Follow-Ups

The default card should still advertise typed follow-ups for downstream trial
and article pivots even when those surfaces are covered elsewhere.

## ID Normalization

Exact variant lookup should normalize equivalent identifiers back to the same
canonical record instead of splitting the user into parallel identities.

## Transcript HGVS Normalization Proxies

Transcript HGVS strings are not exact MyVariant IDs, but agents often already
have a source-shaped transcript candidate from a report or another database. The
normalization proxy keeps that input separate from each upstream service's
returned notation and warnings.

## ERBB2 Transcript HGVS Canary

The proxy must handle transcript strings with substitution notation and shell
metacharacters such as `>` without losing source warnings or conflating service
outputs.

## Unsupported Normalization Inputs

BioMCP should not guess transcripts or convert gene-protein shorthand into a
transcript HGVS query. Unsupported input gets a typed guardrail so an agent can
choose a better source-shaped string.

```bash
set +e
out="$(../../tools/biomcp-ci --json variant normalize all 'BRAF V600E' 2>&1)"
rc=$?
set -e
test "$rc" -ne 0
mustmatch like 'unsupported_notation
BRAF V600E
transcript HGVS' <<<"$out"
```

## Normalization Command Discoverability

The explicit proxy command should be visible from help and structured list
output so agents can find it without trying hidden `get variant` rewrites.

```bash
../../tools/biomcp-ci variant normalize --help | mustmatch like 'all, mutalyzer, or variantvalidator
NM_000248.3:c.135del'
../../tools/biomcp-ci --json list variant | jq -e '.commands | any(. == "variant normalize <service> <transcript_hgvs>")' >/dev/null
```
