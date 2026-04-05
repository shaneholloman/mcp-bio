# Code Review Log — Ticket 151: Guard PubMed rescue against zero-overlap false positives

## Phase 1 — Critique

### Design Completeness Audit

Every design item has a corresponding code change:

| Design Item | Status | Location |
|---|---|---|
| `pubmed_rescue_metadata()` accepts `combined_hits`, rejects when `== 0` | Done | `article.rs:1244,1254` |
| `ARTICLE_RELEVANCE_RANKING_POLICY` updated with "at least one anchor hit" | Done | `article.rs:402` |
| `combined_hits` threaded from `rank_articles_by_directness` caller | Done | `article.rs:1317-1321` |
| Test: zero-overlap mesh-synonym renamed + inverted | Done | `mesh_synonym_zero_overlap_pubmed_row_does_not_rescue_above_literal_competitor` |
| Test: rescue metadata updated for zero-overlap unique case | Done | `rescue_metadata_records_kind_and_position` |
| New test: zero-overlap position-0 not rescued | Done | `zero_overlap_pubmed_unique_position_zero_is_not_rescued` |
| New test: one-hit boundary rescued | Done | `exactly_one_anchor_hit_pubmed_unique_position_zero_is_rescued` |
| Spec: markdown ranking-policy wording | Done | `spec/06-article.md:27` |
| Spec: JSON ranking-policy wording | Done | `spec/06-article.md:324` |

### Test-Design Traceability

All 11 proof matrix entries verified:

| Proof Matrix Entry | Test Location | Status |
|---|---|---|
| Zero-overlap PubMed-only position-0 row is not rescued | `zero_overlap_pubmed_unique_position_zero_is_not_rescued` | Present, correct assertions |
| Exactly one anchor hit still allows PubMed unique rescue | `exactly_one_anchor_hit_pubmed_unique_position_zero_is_rescued` | Present, correct assertions |
| Zero-overlap mesh-synonym fixture no longer outranks competitor | `mesh_synonym_zero_overlap_pubmed_row_does_not_rescue_above_literal_competitor` | Present, correct assertions |
| Rescue metadata absent for zero-overlap unique case | `rescue_metadata_records_kind_and_position` | Present, asserts `None` |
| Multi-hit unique rescue still works | `anchor_count_pubmed_rescue_surfaces_above_higher_title_hit_competitor` | Unchanged, passes |
| PubMed-led rescue still works | `pubmed_led_row_rescues_when_pubmed_position_is_strictly_best` | Unchanged, passes |
| Position > 0 rows still do not rescue | `pubmed_nonfirst_position_does_not_rescue` | Unchanged, passes |
| Ranking-policy wording in markdown output | `spec/06-article.md` — Searching by Gene | Present |
| Ranking-policy wording in JSON output | `spec/06-article.md` — Article Search JSON Without Semantic Scholar Key | Present |
| Real regression query no longer ranks PMID 41721224 first | Live smoke check documented in code-log | Verified by code step |
| Repo baseline and final state are green | `make check` | 1301 tests pass, 0 failures |

### Quality Assessment

- **Implementation**: Minimal, correct one-line guard addition. `combined_hits == 0` complements the existing `directness_tier > 1` guard — they cover different cases (zero overlap vs. strong overlap that doesn't need rescue). No redundancy.
- **Tests**: Verify behavioral contracts, not implementation details. The one-hit boundary test uses mixed anchor types (gene + disease + keyword) to isolate exactly one matching anchor, avoiding accidental multi-hit.
- **Performance**: Zero overhead — `combined_hits` is already computed in the caller, passed as a `usize` parameter.
- **Security**: No untrusted input flows. The parameter is computed internally from anchor matching.
- **Conventions**: Follows existing patterns in `article.rs` — parameter threading matches `directness_tier`, guard condition uses the same `||` chain.
- **Specs**: Both markdown and JSON output surfaces assert the updated "at least one anchor hit" wording.

## Phase 2 — Fix Plan

No defects found. No fixes needed.

## Phase 3 — Repair

No fixes applied. Implementation is clean.

### Spec Coverage

- `spec/06-article.md` proves the updated ranking-policy string on both rendered surfaces (markdown and JSON).
- Unit tests cover all rescue eligibility edge cases specified by the design: zero-overlap rejection, one-hit boundary rescue, multi-hit rescue, led rescue, position > 0 rejection.

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| — | — | — | None found |

## Residual Concerns

None. The implementation faithfully follows the design, all proof matrix tests exist and assert the correct conditions, and the public surfaces (markdown, JSON, spec) are aligned.
