# Disease Clinical Features Port Target State

This document captures the target architecture for porting the validated HPO
phenotype-enrichment spike into the BioMCP disease module. It records the
current problems from ticket 249's survey, the intended Rust module boundaries,
and the incremental build path. The current implementation does not yet expose
clinical features; this file describes the target state.

## Current Problems

The survey identified five root causes in the current disease and MedlinePlus
architecture:

1. `src/sources/medlineplus.rs` hardcodes `retmax=3` in
   `MedlinePlusClient::search()`. Discover should keep that behavior, but the
   clinical-features path needs `retmax=5`.
2. `src/entities/disease/mod.rs` has `DiseasePhenotype` and
   `Disease.phenotypes`, but no source-native clinical-feature row type or
   `Disease.clinical_features` field.
3. The disease section surface is missing `clinical_features` in
   `DiseaseSections`, `DISEASE_SECTION_NAMES`, `parse_sections()`, and
   `apply_requested_sections()`.
4. The Rust codebase has no disease-level configuration for MedlinePlus
   `source_queries` or expected symptom patterns from the spike's `DISEASES`
   fixture.
5. The current MedlinePlus source has no multi-query disease loader, URL-based
   deduplication, or offline fixture fallback for checksum regression tests.

## Target Module Boundaries

### Existing Source Extension

`src/sources/medlineplus.rs` remains the only MedlinePlus client. The target
extends it instead of adding a second source module.

- Keep `MedlinePlusClient::search(&self, query: &str)` with current semantics:
  trim empty queries, call the MedlinePlus `ws/query` endpoint, and request
  `retmax=3`.
- Add `MedlinePlusClient::search_n(&self, query: &str, retmax: u8)` and have
  `search()` delegate to `search_n(query, 3)`.
- Clamp or validate `retmax` inside `search_n` so callers cannot accidentally
  send an invalid or very large value. The clinical-features path should call
  `search_n(query, 5)`.
- Add a source-native ID helper for MedlinePlus topic URLs. Either extend
  `MedlinePlusTopic` with `source_native_id: String` or provide a small
  `source_native_id(url: &str) -> String` helper owned by the clinical-features
  module. The helper strips the last URL path component and drops the extension
  (`https://medlineplus.gov/uterinefibroids.html` -> `uterinefibroids`).

This is a pure extension. Existing callers in `src/entities/discover.rs` and
`src/cli/health.rs` continue to call `search()` and keep `retmax=3`.

### Disease Data Model

The target keeps the existing curated phenotype tier intact and adds a second
source-native tier:

- `Disease.phenotypes: Vec<DiseasePhenotype>` remains the curated
  HPO/Monarch/HPO section.
- `Disease.clinical_features: Vec<DiseaseClinicalFeature>` is added beside it.
  It is not a replacement for `phenotypes`.

`DiseaseClinicalFeature` should live in `src/entities/disease/mod.rs` with
Serde derives and `#[serde(default, skip_serializing_if = "Vec::is_empty")]` on
the parent field. Its row contract mirrors the spike's `ClinicalFeature`
`TypedDict`:

```rust
pub struct DiseaseClinicalFeature {
    pub rank: u16,
    pub label: String,
    pub feature_type: String,
    pub source: String,
    pub source_url: Option<String>,
    pub source_native_id: String,
    pub evidence_tier: String,
    pub evidence_text: String,
    pub evidence_match: String,
    pub body_system: Option<String>,
    pub topic_title: Option<String>,
    pub topic_relation: Option<String>,
    pub topic_selection_score: Option<f64>,
    pub normalized_hpo_id: Option<String>,
    pub normalized_hpo_label: Option<String>,
    pub mapping_confidence: f64,
    pub mapping_method: String,
}
```

All `Disease` constructors must initialize `clinical_features` to an empty
vector until the section is requested.

### Disease Section Surface

The target adds a named `clinical_features` disease section and keeps it opt-in.
It must not be included in `all`, matching the existing `diagnostics`,
`funding`, and `disgenet` pattern for expensive or separately governed data.

Files that must change together:

- `src/entities/disease/mod.rs`
  - Add `const DISEASE_SECTION_CLINICAL_FEATURES: &str = "clinical_features"`.
  - Add it to `DISEASE_SECTION_NAMES` so validation/help can advertise it.
  - Declare `mod clinical_features;` once the logic module exists.
- `src/entities/disease/get.rs`
  - Add `include_clinical_features: bool` to `DiseaseSections`.
  - Add the `parse_sections()` arm for `clinical_features`.
  - Do not set the flag when `all` is requested.
- `src/entities/disease/enrichment.rs`
  - Add the guarded `add_clinical_features_section()` call in
    `apply_requested_sections()`.
  - Clear `disease.clinical_features` if the section is not requested.
- `src/cli/disease/mod.rs`
  - Include `clinical_features` in the section help text.
- `src/render/markdown/sections.rs`
  - Include `clinical_features` in disease follow-on section suggestions.

### Clinical Features Logic Module

Create `src/entities/disease/clinical_features.rs` for the ported spike logic.
The module depends on `src/sources/medlineplus.rs` and the disease model. No
other entity module should depend on it.

Target functions and data structures:

- `ClinicalFeatureDiseaseConfig`
  - Loaded from `src/entities/disease/fixtures/clinical_features_config.json`.
  - Contains `key`, `label`, `biomcp_query`, `identifiers`, `source_queries`,
    `expected_symptoms`, and optional `body_system`.
- `ClinicalFeatureHpoMapping`
  - A Rust `const` slice or embedded JSON fixture containing the 19 reviewed
    mappings from `hpo_mapping.py`.
  - Do not add `phf` or any new dependency. Use a normalized-label linear
    lookup; the fixture is intentionally small.
- `normalize_text(s: &str) -> String`
  - Must match the spike exactly: lowercase, replace non-ASCII-alphanumeric
    runs with spaces, collapse whitespace, and trim.
- `slugify(s: &str) -> String`
  - Lowercase and remove non-ASCII-alphanumeric characters.
- `source_native_id(url: &str) -> String`
  - Strip trailing slash, take the last path component, and drop a file
    extension if present.
- `clinical_feature_config_for(disease: &Disease, requested_lookup: Option<&str>)`
  - Resolve one of the embedded configs by normalized requested lookup,
    disease name, disease ID, and known xrefs.
  - If no config exists, leave `clinical_features` empty rather than
    fabricating rows.
- `load_topics_for_disease(config, client) -> Vec<MedlinePlusTopic>`
  - Query each `source_queries` entry with `search_n(query, 5)`.
  - Deduplicate by canonical URL while preserving first-seen order.
  - If live/cache queries return no topics for a configured disease, load the
    committed offline MedlinePlus topics for that disease and tag the internal
    source mode as `explore_result_fixture`.
- `select_topics(config, topics) -> TopicSelection`
  - Score exact title matches, exact URL slugs, title-token overlap, and query
    mentions in summaries.
  - Select all direct pages when any direct page exists.
  - Otherwise select the top three related pages.
- `extract_features(config, selected_topics) -> Vec<DiseaseClinicalFeature>`
  - Iterate `expected_symptoms` in fixture order.
  - Match configured patterns plus the spike's reviewed extra patterns:
    `abdominal pain -> lower abdomen` and
    `urinary frequency -> urinating peeing often`.
  - Emit evidence excerpts around the matched anchor.
  - Attach HPO mapping metadata with `map_feature(label)`.
- `add_clinical_features_section(disease, requested_lookup, client)`
  - Find config, load and select topics, extract features, and assign
    `disease.clinical_features`.

The module may expose small `pub(super)` functions for unit tests, but the
public disease facade remains `src/entities/disease/get.rs`.

## Spike API Mapping

| Spike function | Rust target |
|---|---|
| `load_topics_for_disease(disease, ...)` | `clinical_features::load_topics_for_disease(config, &MedlinePlusClient)` |
| `select_topics(disease, topics)` | `clinical_features::select_topics(config, topics)` |
| `extract_features(disease, selected_topics)` | `clinical_features::extract_features(config, selected_topics)` |
| `map_feature(label)` | `clinical_features::map_feature(label)` using the reviewed 19-entry fixture |
| `extract_disease_clinical_features(disease, hpo_rows, ...)` | `clinical_features::add_clinical_features_section(&mut Disease, requested_lookup, &MedlinePlusClient)` |
| `summarize_clinical_feature_dataset(rows)` | Test-only helper code for checksum validation; not a runtime disease API |
| `phenotype_coverage(...)` and `stable_checksum(...)` | Test-only helpers; not exposed in `Disease` |

## Runtime Data Flow

1. `src/entities/disease/get.rs::get()` resolves the base disease through the
   existing MyDisease path and enriches the base card.
2. `get()` calls `apply_requested_sections(disease, sections, requested_lookup)`.
3. If `sections.include_clinical_features` is true,
   `enrichment.rs` calls `clinical_features::add_clinical_features_section()`.
4. The clinical-features module resolves a local disease config. Unsupported
   diseases return an empty `clinical_features` vector.
5. Configured diseases issue up to three MedlinePlus searches through
   `MedlinePlusClient::search_n(..., 5)`, deduplicate topics by URL, and fall
   back to committed topic fixtures only when no live/cache topics are returned.
6. Topic selection chooses direct pages first, then top related pages only when
   no direct page exists.
7. Feature extraction emits source-native rows with MedlinePlus provenance and
   reviewed HPO mapping metadata.
8. Render and JSON output expose `clinical_features` only when requested.

## Rendering and Metadata

The user-visible ticket should add these surfaces after the data path exists:

- `src/render/markdown/disease.rs`
  - Add a `DiseaseClinicalFeatureRenderRow` and
    `disease_clinical_feature_rows()`.
  - Pass rows and `show_clinical_features_section` into
    `templates/disease.md.j2`.
- `templates/disease.md.j2`
  - Add a `## Clinical Features (MedlinePlus)` section.
  - Render columns: rank, label, HPO ID, confidence, source, evidence.
- `src/render/markdown/evidence.rs`
  - Add MedlinePlus evidence URLs from `clinical_features.source_url`.
- `src/render/provenance.rs`
  - Add `_meta.section_sources` entry with key `clinical_features`, label
    `Clinical Features`, and source `MedlinePlus` when rows exist.
- `src/render/markdown/sections.rs` and `src/render/markdown/related.rs`
  - Keep follow-on section suggestions aligned with the new section.
- `spec/07-disease.md`
  - Add structural assertions for the section heading, MedlinePlus source
    label, and JSON `clinical_features` rows.

## Fixtures and Regression Contract

The initial production port is scoped to the same three diseases as spike 243:

- uterine fibroid
- endometriosis
- chronic venous insufficiency

Commit these fixtures:

- `src/entities/disease/fixtures/clinical_features_config.json`
  - The three disease configs with `source_queries`, identifiers, body system,
    and `expected_symptoms`.
- `tests/fixtures/medlineplus/uterine_fibroid_topics.json`
- `tests/fixtures/medlineplus/endometriosis_topics.json`
- `tests/fixtures/medlineplus/chronic_venous_insufficiency_topics.json`

The topic fixtures should be minimal copies of the corresponding disease rows
from
`architecture/experiments/243-architecture-hpo-phenotype-enrichment-for-clinical-symptoms/results/clinical_summary_medlineplus_probe.json`.

Regression tests must preserve the spike checksum:

```text
f08c35ff31306ff4696bd953eaba4b00aeed9e6746a1228469e1479238e3d34f
```

Checksum invariants:

- Text normalization matches the spike's `normalize_text()`.
- Checksum JSON is compact, sorted by key, and ASCII-compatible.
- Pattern matching is case-insensitive after whitespace normalization.
- Feature checksum input rows keep the spike shape:
  `{ "label": ..., "hpo": ..., "source": ... }`.

## Dependency and License Position

No new Rust crates are required. The target uses only dependencies already in
`Cargo.toml`:

| Crate | Version in lockfile | SPDX license from `cargo metadata` | Verdict |
|---|---:|---|---|
| `reqwest` | 0.12.28 | MIT OR Apache-2.0 | Accept |
| `reqwest-middleware` | 0.4.2 | MIT OR Apache-2.0 | Accept |
| `roxmltree` | 0.20.0 | MIT OR Apache-2.0 | Accept |
| `regex` | 1.12.3 | MIT OR Apache-2.0 | Accept |
| `cacache` | 13.1.0 | Apache-2.0 | Accept |
| `sha2` | 0.10.9 | MIT OR Apache-2.0 | Accept |
| `serde` | 1.0.228 | MIT OR Apache-2.0 | Accept |
| `serde_json` | 1.0.149 | MIT OR Apache-2.0 | Accept |
| `rust-embed` | 8.11.0 | MIT | Accept |

`cargo deny` is not currently configured in this repo. The audit evidence above
comes from the installed crates.io manifests through `cargo metadata`; a future
repo-wide license-gate ticket can add `deny.toml` if the team wants CI
enforcement.

## Invariants

- `clinical_features` is a source-native MedlinePlus tier; it does not mutate,
  replace, or backfill `phenotypes`.
- `clinical_features` is opt-in and excluded from `all`.
- `MedlinePlusClient::search()` remains backward-compatible for discover and
  health checks.
- Existing disease constructors and transforms initialize
  `clinical_features` to an empty vector.
- Unsupported diseases return no clinical feature rows instead of fabricated
  rows.
- Every emitted clinical feature preserves `source`, `source_url`,
  `source_native_id`, evidence text, evidence match, and HPO mapping confidence.
- Offline regression tests must not require live network access.
- Each intermediate ticket must pass `make check`.

## Build Ticket Decomposition

### Ticket A: Add Disease Clinical Feature Model and MedlinePlus Retmax Support

Foundation only. Add `search_n`, the disease row type, the new section flag and
constant, and the embedded disease config fixture. Initialize empty fields in
all constructors and tests. Do not add extraction logic, rendering, or
user-visible spec assertions.

Proof: `make check` and focused MedlinePlus/disease unit tests pass with no
behavior change for existing disease or discover paths.

### Ticket B: Port Clinical Feature Extraction and Wire Disease Enrichment

Depends on Ticket A. Add `src/entities/disease/clinical_features.rs`, port topic
selection, symptom extraction, HPO mapping, multi-query deduplication, and
offline fixture fallback. Wire `apply_requested_sections()` so the requested
section populates `Disease.clinical_features`.

Proof: `make check` passes, including the checksum regression test for the
three-disease fixture.

### Ticket C: Render Clinical Features and Publish the Section Contract

Depends on Ticket B. Add markdown rendering, evidence/provenance metadata,
section suggestions, CLI help text, user-facing docs, and `spec/07-disease.md`
coverage.

Proof: `make check` and `make spec-pr` pass.

## Open Decisions Captured

- The target chooses opt-in `clinical_features`, excluded from `all`.
- The target uses committed embedded JSON/test fixtures, not Rust-only disease
  constants, so future fixture expansion changes data files rather than
  algorithm code.
- The target ships the spike's 19-entry HPO mapping fixture exactly. A broader
  live HPO mapper remains out of scope.
