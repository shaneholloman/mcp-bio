# Article Queries

Article workflows mix typed biomedical anchors with broader keyword discovery.
These canaries keep the blocking lane honest about search structure, annotation
paths, and fulltext fallback behavior without depending on optional API keys.

## Article Request Planning Happens Before Federated Search

Article search normalizes CLI flags into a request-command seam before any
federated article backend executes. The request records filters, source, sort,
ranking, exact-keyword lookup intent, and the pre-execution `BackendPlan`, so
tests can prove routing decisions without depending on live PubMed, Europe PMC,
PubTator, LitSense2, or Semantic Scholar responses.

## Deterministic Source Contracts

Ticket 376 moves routine article-source proof from public upstream canaries to
source-local request-plan and fixture-backed contracts. Any irreducible public
availability check belongs in an explicit release/live-smoke lane; routine specs
must instead prove PubMed, Europe PMC, PubTator, LitSense2, and Semantic Scholar
request shape, status mapping, and redacted auth behavior locally.

```bash
cargo test --lib ticket_376_article_source_contracts -- --list \
  | mustmatch like 'ticket_376_article_source_contracts'
```

```bash
cargo test --lib ticket_376_article_source_status_contracts -- --list \
  | mustmatch like 'ticket_376_article_source_status_contracts'
```

## Deterministic Renderer Envelope Contracts

Ticket 377 moves routine article renderer/envelope proof into fixture-result
contracts. The deterministic tests should cover article JSON `_meta.next_commands`,
`_meta.source_status`, source degradation guidance, and markdown result-table
anchors without live PubMed, Europe PMC, PubTator, LitSense2, or Semantic Scholar
calls.

```bash
cargo test --lib ticket_377_article_renderer_envelope_contracts -- --list \
  | mustmatch like 'ticket_377_article_renderer_envelope_contracts'
```

## Gene Search

Gene-linked article search should still read like a literature intake surface:
clear heading, ranking note, and a PMID-first table.

## Keyword Search

Keyword search is a different planning path from typed gene search. The query
echo and source-aware table should make that distinction visible.

## Search Table & Source Ranking

The JSON contract should preserve the top article follow-up and keep per-result
source identity plus ranking metadata available to automation.

## PubTator Annotations

Annotations remain a first-class deepen path. The section should keep the
PubTator heading and explain that the extracted entities are normalized.

## Full-Text HTML Fallback

When the XML ladder misses, BioMCP should fall back to the PMC HTML article page
and still keep the saved-file contract on stdout.

```bash
bash ../fixtures/setup-article-fulltext-source-fixture.sh ../..
. ../../.cache/spec-article-fulltext-source-env
trap 'kill "${BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_PID:-}" 2>/dev/null || true' EXIT
out="$(../../tools/biomcp-ci get article 22663012 fulltext)"
echo "$out" | mustmatch like "## Full Text (PMC HTML)"
path="$(printf '%s\n' "$out" | sed -n 's/^Saved to: //p' | head -n1)"
test -n "$path"
saved="$(cat "$path")"
echo "$saved" | mustmatch like "PMC HTML fallback body text"
```

## PDF Fallback Is Opt-In

Semantic Scholar PDF is a last resort, not the default resolver order. The same
fixture-backed article should fail cleanly without `--pdf` and succeed with it.

```bash
bash ../fixtures/setup-article-fulltext-source-fixture.sh ../..
. ../../.cache/spec-article-fulltext-source-env
trap 'kill "${BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_PID:-}" 2>/dev/null || true' EXIT
default_out="$(../../tools/biomcp-ci get article 22663013 fulltext)"
echo "$default_out" | mustmatch like "XML and HTML sources did not return full text"
echo "$default_out" | mustmatch not like "Semantic Scholar PDF"
pdf_out="$(../../tools/biomcp-ci get article 22663013 fulltext --pdf)"
echo "$pdf_out" | mustmatch like "## Full Text (Semantic Scholar PDF)"
pdf_path="$(printf '%s\n' "$pdf_out" | sed -n 's/^Saved to: //p' | head -n1)"
test -n "$pdf_path"
test -f "$pdf_path"
```

## JATS Converter Keeps Evidence-Carrying Floats, Supplements, and Complex Table Markers

Saved Markdown should surface evidence-bearing JATS content that is already
present in the XML. Figures in the body and floats group, declared supplement
files, and unflattened merged-cell tables must be visible to an agent reading
the saved article.

```bash
bash ../fixtures/render-article-fulltext-jats-markdown.sh ../.. | mustmatch like "Europe PMC body text with callout (Figure 2) and B-RAF^V600E^. PLX4032 boundary text."
```

```bash
bash ../fixtures/render-article-fulltext-jats-markdown.sh ../.. | mustmatch like "> **Figure 1.** Inline figure caption preserves n=10 cell counts."
```

```bash
bash ../fixtures/render-article-fulltext-jats-markdown.sh ../.. | mustmatch like "> **Figure 2.** Floats-group figure reports measurement bar is 70 μm."
```

```bash
bash ../fixtures/render-article-fulltext-jats-markdown.sh ../.. | mustmatch like "Supplementary Data S1"
```

```bash
bash ../fixtures/render-article-fulltext-jats-markdown.sh ../.. | mustmatch like "Measurement traces for the treatment cohort."
```

```bash
bash ../fixtures/render-article-fulltext-jats-markdown.sh ../.. | mustmatch like "traces-s1.csv"
```

```bash
bash ../fixtures/render-article-fulltext-jats-markdown.sh ../.. | mustmatch like "**Table 2.** Merged treatment table."
```

```bash
bash ../fixtures/render-article-fulltext-jats-markdown.sh ../.. | mustmatch like "*[complex table omitted: 2×3, merged cells]*"
```

```bash
bash ../fixtures/render-article-fulltext-jats-markdown.sh ../.. | mustmatch not like "((Figure 2))"
```

## Fulltext Provenance, Reuse, and Quality Metadata

Saved fulltext Markdown is evidence material, so the JSON response must carry a
machine-readable manifest for the artifact. The manifest identifies the source,
records whether the representation has useful structure, and separates known
license context from unknown reuse state.

```bash
bash ../fixtures/setup-article-fulltext-source-fixture.sh ../..
. ../../.cache/spec-article-fulltext-source-env
trap 'kill "${BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_PID:-}" 2>/dev/null || true' EXIT
jats_json="$(../../tools/biomcp-ci --json get article 22663011 fulltext)"
echo "$jats_json" | mustmatch like '"full_text_source"'
ARTICLE_JSON="$jats_json" uv run --no-sync python3 - <<'PY'
import json, os
doc = json.loads(os.environ["ARTICLE_JSON"])
manifest = doc.get("full_text_manifest") or {}
assert manifest.get("source_kind") == "jats_xml", "missing JATS full_text_manifest source_kind"
assert manifest.get("source_identifier") == "PMC123456"
provider = manifest.get("provider") or {}
assert provider.get("label") == "Europe PMC XML"
assert provider.get("source") == "Europe PMC"
quality = manifest.get("quality") or {}
assert quality.get("has_sections") is True
assert quality.get("has_tables") is True
assert quality.get("has_references") is True
assert quality.get("has_fulltext_signal") is True
assert quality.get("has_entity_annotations") is False
provenance = manifest.get("provenance") or {}
assert provenance.get("open_access") is True
reuse = manifest.get("reuse") or {}
assert reuse.get("license_present") is True
assert "CC BY" in str(reuse.get("license", ""))
PY
```

PMC HTML fallback can still provide useful readable Markdown, but it is weaker
than source XML and can lack article-level license context. Unknown reuse state
must stay explicit instead of serializing as a safe or blank license.

```bash
bash ../fixtures/setup-article-fulltext-source-fixture.sh ../..
. ../../.cache/spec-article-fulltext-source-env
trap 'kill "${BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_PID:-}" 2>/dev/null || true' EXIT
html_json="$(../../tools/biomcp-ci --json get article 22663012 fulltext)"
echo "$html_json" | mustmatch like '"full_text_source"'
ARTICLE_JSON="$html_json" uv run --no-sync python3 - <<'PY'
import json, os
doc = json.loads(os.environ["ARTICLE_JSON"])
manifest = doc.get("full_text_manifest") or {}
assert manifest.get("source_kind") == "pmc_html", "missing PMC HTML full_text_manifest source_kind"
assert manifest.get("source_identifier") == "PMC123457"
provider = manifest.get("provider") or {}
assert provider.get("label") == "PMC HTML"
assert provider.get("source") == "PMC"
quality = manifest.get("quality") or {}
assert quality.get("has_fulltext_signal") is True
assert quality.get("has_entity_annotations") is False
provenance = manifest.get("provenance") or {}
assert provenance.get("open_access") is True
reuse = manifest.get("reuse") or {}
assert reuse.get("license_present") is False
assert not reuse.get("license")
warning = str(reuse.get("reuse_warning", "")).lower()
assert "license" in warning or "reuse" in warning
PY
```

PDF remains an opt-in fallback. The manifest must mark PDF-derived fulltext so an
agent can decide whether PDF conversion is adequate for evidence ingestion and
can carry any license fact returned by Semantic Scholar.

```bash
bash ../fixtures/setup-article-fulltext-source-fixture.sh ../..
. ../../.cache/spec-article-fulltext-source-env
trap 'kill "${BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_PID:-}" 2>/dev/null || true' EXIT
pdf_json="$(../../tools/biomcp-ci --json get article 22663013 fulltext --pdf)"
echo "$pdf_json" | mustmatch like '"full_text_source"'
ARTICLE_JSON="$pdf_json" uv run --no-sync python3 - <<'PY'
import json, os
doc = json.loads(os.environ["ARTICLE_JSON"])
manifest = doc.get("full_text_manifest") or {}
assert manifest.get("source_kind") == "pdf", "missing PDF full_text_manifest source_kind"
assert "/pdf/22663013.pdf" in str(manifest.get("source_identifier", ""))
provider = manifest.get("provider") or {}
assert provider.get("label") == "Semantic Scholar PDF"
assert provider.get("source") == "Semantic Scholar"
quality = manifest.get("quality") or {}
assert quality.get("has_fulltext_signal") is True
provenance = manifest.get("provenance") or {}
assert provenance.get("pdf_fallback_used") is True
reuse = manifest.get("reuse") or {}
assert reuse.get("license_present") is True
assert "CC BY" in str(reuse.get("license", ""))
PY
```

## Semantic Scholar Degrades Truthfully Without a Key

The blocking lane is intentionally keyless. Article search should stay usable
and explicit about the no-key path rather than hard-failing or pretending the
keyed data plane ran.

## Semantic Scholar Source Status Appears in Debug Plans

Debug plans are for operators and benchmark agents who need to explain the
route BioMCP used. The Semantic Scholar leg should carry the same redacted
auth and availability state there, without requiring stderr parsing.

## Authenticated Source Status Is Redacted

When an operator provides `S2_API_KEY`, article search should identify the
authenticated mode but never echo the key, a prefix, or any secret-derived
string in JSON metadata.

## Markdown Notes Semantic Scholar Unavailability

Markdown should stay quiet on healthy paths, but a failed Semantic Scholar leg is
operator-relevant. When the source is unavailable, the page should still render
primary article rows and include one concise source-status note.

## Entity Follow-Up

`article entities` is the compact follow-up in this bootstrap slice. It should
still expose the gene subsection and typed follow-up commands.
