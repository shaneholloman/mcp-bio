Decision: approved

## Checkpoint Summary

- Rebased onto `origin/main` at start and again before sign-off; branch was already up to date both times.
- Preflight diff/staging check found no untracked ticket files before exercise. The required verify artifact was written at the end of this step.
- Manual exercise, docs/help audit, spec audit, full-blocking validation, and direct changed-spec audit completed.
- No bounded runtime/doc repairs were needed in verify.

## Planning/FAQ Watch Results — relevant watching/answered entries probed

- Relevant `watching`: FAQ #12 (`rand 0.8.5` advisory waiver) is not touched by this ticket. It remains absorbed by the advisory/check gate; `make release-gate` passed.
- Relevant `answered`: FAQ #17 says build verify runs `full-blocking` exactly once and keeps Rust unit tests and mustmatch specs complementary. Verify ran `make release-gate` once; it passed and included both Rust/Python/docs checks and the deterministic spec-contracts lane.
- Relevant `answered`: FAQ #15 says bash spec collection must remain executable. The changed `spec/surface/cli.md` article manifest blocks ran in spec-contracts; the changed `spec/entity/article.md` lane also passed directly.
- Security/safety boundary probes: malformed, empty, and traversal-shaped article IDs failed with clean validation errors; `--pdf` without `fulltext` failed cleanly; checked outputs for obvious secret/temp-root leak markers.

## Exercise Results — ran, inputs, observations

- `target/release/biomcp get article --help` — help documents `fulltext`, `--pdf`, and `--json`; `--pdf` wording says it requires `fulltext`.
- Deterministic fixture, `tools/biomcp-ci --json get article 22663011 fulltext` — JATS/XML winner emitted legacy `full_text_source` and `full_text_manifest` with `source_kind: jats_xml`, provider `Europe PMC XML`/`Europe PMC`, `source_identifier: PMC123456`, sections/tables/references/fulltext quality flags true, entity annotations false, `open_access: true`, and `CC BY` license.
- Deterministic fixture, `tools/biomcp-ci --json get article 22663012 fulltext` — PMC HTML fallback emitted legacy `full_text_source.kind: html` plus manifest `source_kind: pmc_html`, provider `PMC HTML`/`PMC`, `source_identifier: PMC123457`, fulltext signal true, structural flags conservative false, `open_access: true`, `license_present: false`, and a license/reuse warning.
- Deterministic fixture, `tools/biomcp-ci --json get article 22663013 fulltext --pdf` — explicit PDF fallback emitted manifest `source_kind: pdf`, Semantic Scholar PDF provider, PDF URL source identifier, fulltext signal true, `pdf_fallback_used: true`, and `CC BY` license.
- Markdown compatibility: `tools/biomcp-ci get article 22663012 fulltext` still printed `## Full Text (PMC HTML)` and `Saved to:`.
- Default no-PDF behavior: `tools/biomcp-ci --json get article 22663014 fulltext` did not request the Semantic Scholar PDF rung; request log showed ID bridge, XML rungs, and HTML only. With `--pdf`, request log included the PDF rung.

## Edge Cases Tested — specific cases, results

- Missing ID: `tools/biomcp-ci get article` exited 2 with clap usage.
- Empty ID: `tools/biomcp-ci --json get article '' fulltext` exited 2 with unsupported identifier guidance.
- Traversal-shaped ID: `tools/biomcp-ci --json get article 'PMC../../etc/passwd' fulltext` exited 2 with unsupported identifier guidance.
- Unknown section/path-like token: `tools/biomcp-ci --json get article 22663011 '../fulltext'` exited 2 with known section list.
- `--pdf` without `fulltext`: `tools/biomcp-ci --json get article 22663011 --pdf` exited 2 with the documented precondition message.
- Fulltext miss: `tools/biomcp-ci --json get article 22663014 fulltext` returned a note and omitted `full_text_manifest`/`full_text_source`.

## Spec Audit — specs reviewed, gaps found, counts before/after, spec-only result

- Reviewed changed executable specs in `spec/surface/cli.md` and `spec/entity/article.md` plus article fulltext docs/spec context.
- The new manifest assertions are behavioral: source family/provider/identifier, quality booleans, reuse/license truth, open-access provenance, and explicit PDF fallback status. They avoid field counts, occurrence counts, line-qualified nodes, and exact warning prose.
- Spec-only before ticket: 52 passing blocks from preflight/design baseline.
- Spec-only after ticket in verify: `make release-gate` ran `make spec-contracts`; result was 55 passed, including the three new `spec/surface/cli.md::Article Fulltext JSON Manifests Carry Provenance` blocks.
- Extra changed-spec audit: `PATH="$PWD/target/release:$PATH" BIOMCP_BIN="$PWD/target/release/biomcp" uv run --no-sync pytest spec/entity/article.md --mustmatch-lang bash --mustmatch-timeout 180 -v` — 8 passed.
- Gap found: PMC OA Archive XML winner manifest package/retraction/license provenance has source-client unit support but no user-visible fixture/spec row. Filed design issue `planning/biomcp/issues/383-pmc-oa-archive-manifest-spec-gap.md`.

## Regression Results — existing features verified

- `tools/biomcp-ci --json get article 22663011` default summary returned the article title/PMID and did not populate fulltext fields or hit fulltext request-log entries.
- `tools/biomcp-ci --json get article 22663011 annotations` remained available and did not populate fulltext manifest fields.
- `tools/biomcp-ci get article 22663013 fulltext --pdf` still printed `## Full Text (Semantic Scholar PDF)` and `Saved to:`.
- Existing HTML fallback and PDF opt-in article spec blocks passed in the direct `spec/entity/article.md` lane.

## Test Suite — full-blocking result

- Full-blocking profile command run exactly once in verify: `make release-gate`.
- Result: passed.
- Observed sub-results from the gate: nextest `2086 passed, 1 skipped`; `pytest tests/` `251 passed`; `mkdocs build --strict` passed; spec-contracts `55 passed`.

## Documentation — parity audit of docs/help/examples

- Runtime help: `biomcp get article --help` documents `fulltext`, `--pdf`, `--json`, and the `--pdf` precondition.
- `docs/user-guide/article.md` documents `full_text_manifest` fields in fulltext and JSON mode, including quality, reuse/license warning, provenance, package, and PDF-fallback facts.
- `architecture/functional/article-fulltext.md` documents resolver order, compatibility fields, manifest shape, license/reuse boundaries, quality semantics, and failure visibility.
- `biomcp list article` still documents the unchanged command surface and `--pdf` behavior. No new command/help surface was required.

## Issues Found and Fixed — fixes + proof

- No bounded runtime, docs, or assertion relaxations were needed in verify.
- Proof: manual exercise passed; `make release-gate` passed; direct changed article spec lane passed; `git diff --check` passed.

## Issues Filed — list with paths

- `/home/ian/workspace/planning/biomcp/issues/383-pmc-oa-archive-manifest-spec-gap.md` — design/spec ratchet for user-visible PMC OA Archive XML manifest provenance coverage.

## Planning Updates — concrete issues filed or FAQ watching proposal (or "none")

- Filed one concrete specs issue. No FAQ watching update proposed; the discovered concern can become an executable fixture/spec ratchet.

## UX Quality — CLI/UI assessment (if applicable)

- CLI behavior remains additive and compatible: no new command, old Markdown labels and `Saved to:` remain, legacy `full_text_source` remains, default fulltext still does not use PDF, and `--pdf` misuse has a clear validation error.
- JSON manifest is machine-readable and avoids implying safe reuse when license facts are unknown.

## Assertion-quality delta

- Weak assertions relaxed: none.
- Weak assertions escalated: PMC OA Archive manifest package/retraction/license user-visible coverage gap escalated to design issue `383-pmc-oa-archive-manifest-spec-gap.md`.
- Syntactic-red/process gaps found: none in changed specs; both spec-contracts and direct article spec lane passed.
- Verify authored no new shipped-contract assertions and no trivia-strengthening assertions.

Issues filed: 1
