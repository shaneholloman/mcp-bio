# Rare-Disease Trial Search Target Architecture

Ticket 412 surveyed the rare-disease trial workflow exposed by a Phelan-McDermid / SHANK3 search. This document records the target state. It does not describe shipped behavior until the implementation tickets land.

## Current problems

The current trial path has useful pieces, but they are not connected by a shared planning boundary:

- `discover` in `src/entities/discover.rs` detects trial intent with keywords, then routes from a single top concept. Mixed disease/gene/sponsor/trial queries can collapse into an unrelated concept and fail to suggest a useful trial command.
- `search trial` in `src/cli/trial/dispatch.rs` builds a flat `TrialSearchFilters` and sends one literal `condition` to ClinicalTrials.gov. Intervention aliases have a fan-out/dedupe/provenance path in `src/entities/trial/search/ctgov.rs`, but condition labels and gene-derived disease labels do not.
- Gene and disease pivots (`src/cli/gene/related.rs`, `src/cli/disease/dispatch.rs`) construct trial filters directly, so planner behavior added only to `discover` or only to `search trial` would be bypassed.
- `get trial` has progressive-disclosure sections, but the CTGov model/transform path drops module-level central contacts, contact email, and structured sex eligibility before rendering.
- Search rows intentionally stay compact. There is no opt-in orchestration layer that takes expanded terms, dedupes NCT IDs, fetches full records, ranks practical sites, and surfaces caveats such as open-label extension / antecedent study requirements.

## Target data flow

```text
CLI args / discover query / pivot helper
  -> RareDiseaseTrialRequest
  -> RareDiseaseTrialPlan
      - parsed roles: disease labels, gene symbols, sponsor terms, literal filters
      - bounded expansions with source/provenance
      - execution mode: strict, expanded, or action-summary
  -> TrialSearchExecutionPlan
      - one or more CTGov condition/biomarker requests
      - optional intervention alias requests
      - dedupe key = NCT ID
      - per-row matched labels/provenance
  -> optional TrialActionSummaryPlan
      - fetch selected NCT full records with locations, contacts, eligibility, design
      - classify practical trial type and caveats
      - rank sites using explicit user location/facility/state hints
  -> markdown / JSON renderers
```

The planner lives below the CLI and pivot helpers, for example under `src/entities/trial/planning.rs` or a small `src/entities/trial/rare_disease.rs` module. It must not depend on markdown rendering or source clients. It returns typed plan structs that request-contract tests can assert without network calls.

## Target types and boundaries

### `RareDiseaseTrialRequest`

A small entity-local request value built from `discover`, `search trial`, `gene trials`, and `disease trials` inputs.

Fields should include:

- `raw_query: Option<String>` for free-text discover/action-summary input.
- `condition: Option<String>` and `gene: Option<String>` for typed entry points.
- `sponsor: Option<String>` and existing trial filters that remain literal.
- `strict_condition: bool` or equivalent opt-out flag, mapped from a first-class CLI option such as `--no-condition-expand`.
- `mode: TrialPlanningMode` with at least `Search`, `DiscoverSuggestion`, and `ActionSummary`.
- Optional user geography/facility hints used only by ranking, not by hidden filtering.

Invariant: building a request never constructs a network client and never calls ClinicalTrials.gov, OLS, UMLS, or Monarch.

### `RareDiseaseTrialPlan`

The normalized output of request planning.

Fields should include:

- `primary_condition_labels: Vec<ConditionLabel>`.
- `gene_labels: Vec<GeneLabel>` for gene terms recognized in the query.
- `expanded_condition_labels: Vec<ConditionExpansion>` with label, source, reason, and confidence/bound.
- `query_terms: Vec<TrialQueryTerm>` describing which CTGov fields each term will search (`condition`, `biomarker`, or both).
- `warnings: Vec<PlanningWarning>` for bounded omissions, noisy terms rejected, or unknown gene/disease pivots.
- `suggested_commands: Vec<CommandSuggestion>` for `discover` and render metadata.

The first implementation may use a deliberately small curated seed map plus existing source metadata for request-contract coverage. The architecture should make later ontology-backed expansion additive, not a rewrite.

Invariants:

- Expansions are bounded. Exact disease synonyms and curated gene/syndrome aliases outrank broad phenotype labels.
- Broad labels such as autism or unrelated SHANK-family terms are not introduced just because they co-occur with the input.
- Every expanded term that can affect a result is visible in markdown and JSON provenance.
- `--no-condition-expand` or strict mode uses only the literal condition supplied by the user.

### Trial search execution plan

`TrialSearchFilters` can either grow a typed condition expansion field or be paired with a new execution plan object. The CTGov path should generalize the existing intervention alias fan-out pattern:

- Build one CTGov request per accepted condition label when expansion is active.
- Preserve existing intervention alias expansion and dedupe all fan-out results by NCT ID.
- Set `matched_condition_label` on `TrialSearchResult`, analogous to `matched_intervention_label`.
- Surface expansion provenance in JSON `_meta` and in a compact markdown note/table column.
- Keep current literal `search trial -c ...` behavior available through the opt-out flag.

Invariant: default compact search output remains fast and readable; full location/contact/eligibility fields stay behind `get trial` sections or action-summary mode.

### Trial detail enrichment

The CTGov source/domain model should preserve action-critical fields:

- `protocolSection.contactsLocationsModule.centralContacts[]` as module-level contacts.
- Per-location contacts including email when CTGov supplies it.
- Structured `minimumAge`, `maximumAge`, and `sex` from `eligibilityModule`.
- Full `eligibilityCriteria` when the eligibility section is requested.

`get trial` should support a `contacts` section in addition to existing `locations` and `eligibility` sections. The documented progressive-disclosure contract is:

```bash
biomcp get trial NCT... locations contacts eligibility
```

Invariant: module-level central contacts and per-location contacts are both represented when present; rendering must not hide a contact email that survived source parsing.

### Action summary

The action summary is opt-in. It may be a helper command or a `search trial` mode, but it should not make ordinary `search trial` verbose or slow.

The action-summary orchestrator should:

1. Run the rare-disease trial plan.
2. Deduplicate candidate NCT IDs across expanded labels.
3. Fetch full CTGov records with locations, contacts, eligibility, arms/design, and status fields.
4. Classify studies into practical buckets such as treatment, gene therapy, supportive/behavioral, registry/observational, or open-label extension.
5. Surface access caveats such as antecedent-study requirement, invitation-only enrollment, no listed local site, or non-US-only sites.
6. Rank sites using explicit user hints (`--facility`, `--state`, `--lat/--lon/--distance`) and make clear whether ranking is based on listed CTGov locations only.

Invariants:

- The summary never implies unlisted or pending sites. It can say that no listed Michigan/University of Michigan site was present in the CTGov record at retrieval time.
- Caveats are structured data in JSON, not only prose.
- Classification rules are deterministic and pinned by fixture tests before live-smoke examples are used.

## Deterministic contract tests

Each implementation slice should add request/result contracts before relying on live upstream checks:

- Planner unit tests for the Phelan-McDermid / SHANK3 / 22q13 case.
- Negative expansion tests proving broad autism and unrelated SHANK-family labels are not added.
- CTGov request-plan tests proving condition fan-out and NCT dedupe preserve matched labels.
- `get trial` fixture tests for central contacts, contact email, sex eligibility, and the new `contacts` section.
- Action-summary fixture tests for site ranking and caveat classification, including an open-label extension requiring an antecedent study.
- CLI/spec tests for help/list/docs alignment when new flags or commands are introduced.

Routine proof remains `make lint`, `make test`, and `make spec`. Any live ClinicalTrials.gov smoke belongs in an explicit live/verify lane, not as the only proof of the workflow.

## Alignment with BioMCP strategy

This target preserves BioMCP's read-only federation model: it does not store patient data, write to ClinicalTrials.gov, or build a patient-matching engine. It strengthens the stable grammar and request-contract frontier by making the rare-disease trial workflow a typed, provenanced plan rather than ad hoc free-text retries. It also keeps progressive disclosure intact: compact search stays compact, while users and agents can explicitly ask for detail sections or an action summary.
