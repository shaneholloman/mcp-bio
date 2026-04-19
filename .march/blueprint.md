## Executive Summary

The disease module is missing the production shape needed to port spike 243: MedlinePlus search is fixed at `retmax=3`, `Disease` has no source-native clinical-feature tier, the disease section surface has no `clinical_features` section, and the spike's disease query/symptom fixtures plus offline fallback do not exist in Rust. The target keeps the working HPO `phenotypes` architecture intact, adds an opt-in MedlinePlus-backed `clinical_features` tier beside it, extends the existing MedlinePlus client without breaking discover/health callers, and ports the spike logic behind a focused disease submodule. The path is three independently shippable build tickets.

## Survey Issues Addressed

- **Issue 1: `MedlinePlusClient` retmax is hardcoded at 3**
  - Fixed by ticket 252 with `MedlinePlusClient::search_n(query, retmax)` while preserving `search(query)` as `retmax=3`.
- **Issue 2: `Disease` struct has no `clinical_features` field**
  - Fixed by ticket 252 with `DiseaseClinicalFeature` and `Disease.clinical_features`.
- **Issue 3: `DiseaseSections`, section constants, and section names are incomplete**
  - Fixed in layers: ticket 252 adds parser/constants/section flag, ticket 253 wires enrichment, ticket 254 publishes rendering/help/spec surfaces.
- **Issue 4: No disease configuration mechanism for MedlinePlus queries and symptom patterns**
  - Fixed by ticket 252 for the embedded config fixture and ticket 253 for config loading/matching.
- **Issue 5: No MedlinePlus multi-query dedup and explore-fixture fallback**
  - Fixed by ticket 253 with multi-query `retmax=5`, URL deduplication, topic selection, and offline fixture fallback.

## Target Architecture

### MedlinePlus Source Extension

- **Current:** `src/sources/medlineplus.rs::MedlinePlusClient::search()` always sends `retmax=3`. Existing callers are `src/entities/discover.rs` and `src/cli/health.rs`.
- **Target:** Keep `search()` unchanged for existing callers and add `search_n(&self, query: &str, retmax: u8)` for the disease clinical-features path.
- **Key changes:** `search()` delegates to `search_n(query, 3)`; disease clinical features call `search_n(query, 5)`; focused wiremock tests prove both query contracts.
- **Invariants:** Discover and health remain behavior-compatible; no duplicate MedlinePlus source client; no new dependencies.

### Disease Model and Section Surface

- **Current:** `src/entities/disease/mod.rs` has `DiseasePhenotype` and `Disease.phenotypes`; `get.rs` parses 11 disease section flags; `clinical_features` is unknown.
- **Target:** Add `DiseaseClinicalFeature` and `Disease.clinical_features` beside existing phenotypes. Add `clinical_features` as an explicit disease section, excluded from `all`.
- **Key changes:** Touch `src/entities/disease/mod.rs`, `src/entities/disease/get.rs`, `src/entities/disease/enrichment.rs`, `src/transform/disease.rs`, `src/entities/disease/test_support.rs`, and direct test constructors.
- **Invariants:** `phenotypes` remains curated HPO/Monarch/HPO data; `clinical_features` remains source-native MedlinePlus data; unsupported or unrequested clinical features serialize as absent/empty rather than fabricated rows.

### Clinical Features Logic

- **Current:** The only implementation is the Python spike under `architecture/experiments/243-architecture-hpo-phenotype-enrichment-for-clinical-symptoms/scripts/clinical_features_spike/`.
- **Target:** Port the spike contract into `src/entities/disease/clinical_features.rs`, depending only on the disease model and `src/sources/medlineplus.rs`.
- **Key changes:** Implement `normalize_text`, `slugify`, `source_native_id`, `clinical_feature_config_for`, `load_topics_for_disease`, `select_topics`, `extract_features`, `map_feature`, and `add_clinical_features_section`. Add embedded disease config and offline MedlinePlus topic fixtures for the three spike diseases.
- **Invariants:** Topic selection prefers direct pages; related pages are used only when no direct page exists; URL dedup preserves first-seen order; HPO mapping uses the reviewed 19-entry fixture; regression checksum stays `f08c35ff31306ff4696bd953eaba4b00aeed9e6746a1228469e1479238e3d34f`.

### Rendering, Provenance, and Specs

- **Current:** `src/render/markdown/disease.rs`, `templates/disease.md.j2`, `src/render/provenance.rs`, and `spec/07-disease.md` know about phenotypes but not clinical features.
- **Target:** Render `## Clinical Features (MedlinePlus)` only when requested, expose MedlinePlus evidence URLs and `_meta.section_sources`, and document the opt-in section in CLI help/list/docs/specs.
- **Key changes:** Update `src/render/markdown/disease.rs`, `templates/disease.md.j2`, `src/render/markdown/evidence.rs`, `src/render/provenance.rs`, `src/render/markdown/sections.rs`, `src/render/markdown/related.rs`, `src/cli/disease/mod.rs`, `src/cli/list.rs`, `src/cli/list_reference.md`, `docs/user-guide/disease.md`, `docs/reference/data-sources.md`, and `spec/07-disease.md`.
- **Invariants:** The new section does not displace existing disease pivots; `all` still excludes clinical features; spec assertions stay structural and offline-friendly.

## Ticket Sequence

| # | ID | Name | Addresses | Dependencies | Priority | Status |
|---|-----|------|-----------|-------------|----------|--------|
| 1 | 252 | Add disease clinical feature model and MedlinePlus retmax support | Issues 1, 2, 3, 4 foundation | none | 8 | ready |
| 2 | 253 | Port disease clinical feature extraction and enrichment | Issues 3, 4, 5 | 252-add-disease-clinical-feature-model-and-medlineplus-retmax-support | 8 | draft |
| 3 | 254 | Render disease clinical features and publish section contract | Issue 3 blast radius and public contract | 253-port-disease-clinical-feature-extraction-and-enrichment | 5 | draft |

## Doc Updates

- Added `architecture/functional/clinical-features-port.md`.
  - Documents the current survey problems.
  - Defines the target disease model, section surface, MedlinePlus extension, clinical-features module, rendering/provenance contract, dependency/license position, invariants, and build-ticket decomposition.
- No unrelated architecture, design, or user docs were edited in this blueprint step.

## Assumptions

- **Assumption:** `clinical_features` should remain opt-in and excluded from `all`.
  - **Basis:** The survey recommends opt-in, and existing expensive/separately governed sections (`diagnostics`, `funding`, `disgenet`) are excluded from `all`.
  - **Validation:** Ticket 252 tests parser/`all` behavior; ticket 254 adds public spec assertions.
  - **Fallback:** If Ian or a lead wants it in `all`, change the `get.rs` all-expansion, update performance expectations, and adjust specs/docs in ticket 254.

- **Assumption:** The three minimal MedlinePlus topic fixtures are sufficient for checksum parity.
  - **Basis:** Spike 243's offline mode uses the committed `clinical_summary_medlineplus_probe.json`, and the target needs only the per-disease topic rows for the regression.
  - **Validation:** Ticket 253 computes the exact spike checksum from the Rust output.
  - **Fallback:** If subset extraction loses needed fields, copy the full relevant disease payloads from the probe JSON into the test fixtures.

- **Assumption:** Runtime MedlinePlus calls of up to three queries with `retmax=5` are acceptable behind an explicit section.
  - **Basis:** The section is opt-in, existing HTTP cache middleware is already present, and the spike processed the fixture in milliseconds.
  - **Validation:** Ticket 253 exercises live/cache request shape and keeps offline regression tests network-free.
  - **Fallback:** If live latency/noise is unacceptable, tighten query caps, rely more heavily on cached/offline fixtures for the initial three diseases, or require an explicit cache mode for specs.

- **Assumption:** A linear lookup over the 19-entry HPO mapping fixture is enough.
  - **Basis:** The reviewed mapping fixture is tiny and no new crates are needed.
  - **Validation:** Ticket 253 unit tests known mappings and checksum output.
  - **Fallback:** If the mapping grows large, switch to a `std::collections::BTreeMap`/`HashMap` built from embedded data, or audit a new perfect-hash dependency in a separate ticket.

- **Assumption:** Rust can match the Python checksum serialization exactly with standard crates.
  - **Basis:** `serde_json`, `sha2`, and ordered map construction are already available; the checksum input shape is small and explicit.
  - **Validation:** Ticket 253's checksum regression is the proof.
  - **Fallback:** If direct serialization differs, implement a small test-only checksum encoder that mirrors Python's sorted compact JSON contract.

- **Assumption:** No standalone research ticket is required.
  - **Basis:** The survey and spike already validate the approach; remaining uncertainty is implementation fidelity, not source feasibility.
  - **Validation:** Ticket 253 owns algorithm/checksum proof; ticket 254 owns rendering/spec proof.
  - **Fallback:** If ticket 253 exposes unexpected MedlinePlus or fixture incompatibility, pause ticket 254 and split a focused research/fix ticket from 253.

## Risk Assessment

- Adding a field to `Disease` can break direct struct initializers across tests.
  Watch for compile failures in `src/render/markdown/*`, `src/render/provenance.rs`, `src/sources/seer.rs`, and CLI next-command tests after ticket 252. Rollback is local to ticket 252 because it has no runtime behavior change.
- Section wiring has several synchronized surfaces. Missing one of `DISEASE_SECTION_NAMES`, `DiseaseSections`, `parse_sections()`, enrichment cleanup, or renderer suggestions can create a silently unavailable section. Ticket 252 and 254 tests should cover both parser and user-visible paths.
- Checksum drift is likely if normalization, fixture order, or JSON serialization differs from Python. Ticket 253 must fail fast on checksum mismatch before rendering work starts.
- Live MedlinePlus results can change or fail. The target keeps specs and checksum tests on committed fixtures, and live failures should degrade to empty/fallback rows for configured diseases rather than failing unrelated disease sections.
- Users may confuse curated HPO phenotypes with source-native clinical features. Ticket 254 should label the section as MedlinePlus-backed and preserve HPO mapping confidence instead of presenting mappings as authoritative phenotype annotations.
- If ticket 253 produces noisy rows for chronic venous insufficiency, do not broaden extraction in ticket 254. Roll back or patch ticket 253's selection/extraction fixture logic first.

Rollback strategy:

- Ticket 252 can be reverted independently because it only adds empty model/config/client surfaces.
- Ticket 253 can be reverted while leaving ticket 252's inert schema/client extension in place.
- Ticket 254 can be reverted independently if rendering/specs expose user-facing issues; JSON extraction from ticket 253 remains available for debugging.

## Open Questions

- Should unsupported diseases eventually expose a `clinical_features_note` in JSON/markdown, or is an empty opt-in section enough for the initial three-disease port?
- Does Ian want a repo-wide `deny.toml`/`cargo deny check licenses` gate as a separate quality ticket? The current port does not need new crates, and this blueprint used `cargo metadata` license evidence.
- After the three-disease port lands, what is the next expansion source of truth for disease configs: manually reviewed JSON additions, a generated fixture pipeline, or a future live HPO/MedlinePlus mapping workflow?
