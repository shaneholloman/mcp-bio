# Request-Plan Ratchets

BioMCP keeps source request construction deterministic before any live upstream call.
These ratchets now live in the language-native and Python/static contract lanes
instead of the routine Markdown spec gate. The remaining executable examples in
this document are user-facing help and documentation canaries.

## Update Help Keeps Unsafe Checksum Override on the Option Stanza

The update command's unsafe checksum escape hatch must be proven against the
rendered option stanza, not only against prose elsewhere in long help. The
Python docs contract runs the rendered CLI help and extracts the actual option
block.

## MyDisease Rejects Path and Query Separators Before Network

A disease ID is data, not a path fragment. The no-network Rust ratchet must
prove that slash, backslash, query, and fragment separators are rejected while a
valid ontology ID still plans the `/disease/{id}` request shape.

## Request Commands Consume Captured Fields at Execution Boundaries

Command dispatch should not construct request structs that executors ignore.
The Rust seam tests prove discover, disease search, disease fallback, and
article dispatch consume the request fields that carry user intent into source
or backend calls.

## PubMed and PubTator Consume Planned Auth and Cache Modes

Secret-aware article sources must use the plan's redacted auth/cache modes at
the executor boundary. These tests use synthetic keys and keyless clients so the
routine gate proves keyed behavior without requiring real credentials.

## Shared Retry-After Waits Stay Bounded

Shared HTTP retries should honor ordinary upstream `Retry-After` hints without
letting an extreme header park a CLI command or March worker indefinitely. The
Rust policy tests keep normal, malformed, extreme, and total-budget paths
deterministic without calling a live service.

## Ticket 401 Surface Ratchets

The post-migration spec runner keeps routine specs Markdown-only. The static
ratchets around spec quality and fixture realism live under `tests/surface/`,
where `make test` runs them without calling public services.

## Rare-Disease Trial Planning Keeps Expansion Bounded

Rare-disease trial planning should be a deterministic request contract before
any ClinicalTrials.gov or ontology execution. The Rust seam tests prove the
Phelan-McDermid / SHANK3 / 22q13 plan carries bounded condition and biomarker
terms, records provenance, rejects broad noisy labels, and exposes strict
condition mode as data.

## Rare-Disease Trial Search Executes Bounded Condition Expansion

Rare-disease trial search should consume the deterministic plan before any live
ClinicalTrials.gov call. The Rust request-contract tests prove the CTGov search
execution fans out accepted Phelan-McDermid condition labels, dedupes repeated
NCT IDs, records matched-condition provenance, keeps strict mode literal, and
labels combined condition/intervention fan-out workers.

## Rare-Disease Trial Pivots Reuse the Shared Plan

Discover, gene trial pivots, and disease trial pivots should enter the same
rare-disease trial plan as `search trial`. These no-network seam tests keep the
mixed Phelan-McDermid / SHANK3 first move, the SHANK3 gene pivot, the disease
pivot, and unsupported noisy text from drifting back to top-concept-only or
biomarker-only routing.

## Trial Search Documents Condition Expansion Controls

The strict/literal opt-out and matched-condition provenance are user-facing
search behavior, so the rendered help, list page, and user docs should teach the
same contract as the execution path.

```bash
../../tools/biomcp-ci search trial --help | mustmatch like "--no-condition-expand
matched_condition_label
Matched Condition"
```

```bash
../../tools/biomcp-ci list trial | mustmatch like "--no-condition-expand
matched_condition_label
Matched Condition"
```

```bash
grep -h "no-condition-expand\|matched_condition_label\|Matched Condition" ../../docs/user-guide/trial.md | mustmatch like "--no-condition-expand
matched_condition_label
Matched Condition"
```

```bash
grep -h "no-condition-expand\|matched_condition_label\|Matched Condition" ../../docs/user-guide/cli-reference.md | mustmatch like "--no-condition-expand
matched_condition_label
Matched Condition"
```

## Ticket 405 Architecture and Operator Contracts

Current repo docs must describe the shipped BioMCP architecture and operator
contracts, not migrated targets. The static contract suite keeps the routine spec
lane honest about the Rust crate surface, spec/surface participation,
cache/logging configuration, article fulltext dependencies, next-command
ownership, and docs navigation without calling public services.
