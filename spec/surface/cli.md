# CLI Surface

The top-level CLI is the stable envelope around every entity card and helper
surface. These canaries keep the entrypoint discoverability, operator commands,
and command-reference pages honest without re-testing entity-specific data here.

## Top-Level Help Keeps the Surface Visible

The first thing a user sees still needs to teach the major surfaces and the one
documented JSON exception for cache paths.

```bash
out="$(../../tools/biomcp-ci --help)"
printf '%s\n' "$out" | mustmatch like "leading public biomedical data sources"
printf '%s\n' "$out" | mustmatch like "serve-http"
printf '%s\n' "$out" | mustmatch '/suggest\s+Suggest .*biomedical question/'
printf '%s\n' "$out" | mustmatch like "cache path, which stays plain text"
```

## Static Command Guides Stay Task-Oriented

`biomcp list` is the durable command-reference surface. The discover and batch
subpages should keep teaching when to use the command, not just list flags.

```bash
discover="$(../../tools/biomcp-ci list discover)"
printf '%s\n' "$discover" | mustmatch like '`discover <query>`'
printf '%s\n' "$discover" | mustmatch like "If no biomedical entities resolve"
batch="$(../../tools/biomcp-ci list batch)"
printf '%s\n' "$batch" | mustmatch like '`batch <entity> <id1,id2,...>`'
printf '%s\n' "$batch" | mustmatch like "up to 10 IDs"
```

## List Command Documents Update Checksum Override

The root command reference is also the operator quick reference. Its update line
must list the unsafe missing-checksum override next to the default checksum
verification behavior. The source reference and rendered `biomcp list` output
must satisfy the same structural update-line contract, and the contract helper
must reject the stale `update [--check]` line from ticket 331.

```bash
set +e
list_contract_out="$(cd ../.. && uv run --no-sync pytest tests/test_update_command_docs_contract.py -k update_list_reference -v 2>&1)"
list_contract_status=$?
set -e
printf '%s\n' "$list_contract_out" | mustmatch like "test_update_list_reference_and_rendered_list_describe_checksum_override"
printf '%s\n' "$list_contract_out" | mustmatch like "test_update_list_reference_contract_rejects_stale_update_line"
test "$list_contract_status" -eq 0
```

## Validation Exit Codes Separate Bad Usage From Runtime Failures

Invalid command usage should exit `2` whether clap catches it during parsing or
BioMCP catches it during custom validation. Runtime and configuration failures
still use exit `1`, so scripts can distinguish bad usage from a command that was
well-formed but could not complete.

```bash
set +e
tmpdir="$(mktemp -d)"
../../tools/biomcp-ci get gene >"$tmpdir/clap.out" 2>"$tmpdir/clap.err"; clap_status=$?
../../tools/biomcp-ci search gene >"$tmpdir/gene.out" 2>"$tmpdir/gene.err"; gene_status=$?
set -e
test "$clap_status" -eq 2
test "$gene_status" -eq 2
cat "$tmpdir/gene.err" | mustmatch like "Query is required"
rm -rf "$tmpdir"
```

Diagnostic search has its own custom missing-filter validator and should follow
the same bad-usage exit policy.

```bash
set +e
tmpdir="$(mktemp -d)"
../../tools/biomcp-ci search diagnostic >"$tmpdir/diagnostic.out" 2>"$tmpdir/diagnostic.err"; diagnostic_status=$?
set -e
test "$diagnostic_status" -eq 2
cat "$tmpdir/diagnostic.err" | mustmatch like "requires at least one of"
rm -rf "$tmpdir"
```

The same mapping applies to custom validation outside entity search.

```bash
set +e
tmpdir="$(mktemp -d)"
ids="BRAF,TP53,EGFR,KRAS,NRAS,PIK3CA,ALK,ROS1,MET,RET,NTRK"
../../tools/biomcp-ci batch gene "$ids" >"$tmpdir/batch.out" 2>"$tmpdir/batch.err"; batch_status=$?
set -e
test "$batch_status" -eq 2
cat "$tmpdir/batch.err" | mustmatch like "Batch is limited to 10 IDs"
rm -rf "$tmpdir"
```

A non-validation failure stays on the runtime-failure exit code.

```bash
set +e
tmpdir="$(mktemp -d)"
../../tools/biomcp-ci search trial -c melanoma --source nci >"$tmpdir/nci.out" 2>"$tmpdir/nci.err"; nci_status=$?
set -e
test "$nci_status" -eq 1
cat "$tmpdir/nci.err" | mustmatch like "API key required"
cat "$tmpdir/nci.err" | mustmatch like "NCI_API_KEY"
rm -rf "$tmpdir"
```

## List Command Honors Global JSON

`biomcp list` remains the human command-reference page by default, but scripts
and agents that pass the global `--json` flag need structured reference data
instead of Markdown. The root list exposes the entity/command inventory, while an
entity page exposes the named command entries for that surface.

```bash
set -e
root_json="$(../../tools/biomcp-ci --json list)"
gene_json="$(../../tools/biomcp-ci --json list gene)"
ROOT_JSON="$root_json" GENE_JSON="$gene_json" uv run --no-sync python3 - <<'PY'
import json, os
root = json.loads(os.environ["ROOT_JSON"])
gene = json.loads(os.environ["GENE_JSON"])
def entries(value): return value if isinstance(value, list) else []
entities = entries(root.get("entities"))
assert "gene" in entities
refs = [*entries(root.get("patterns")), *entries(root.get("commands"))]
assert any("search all" in str(entry) for entry in refs)
assert gene.get("entity") == "gene"
assert any("get gene <symbol>" in str(command) for command in entries(gene.get("commands")))
PY
printf '%s\n' "$root_json" | mustmatch like '"entities"'
```

## Operator Commands Keep Distinct Output Modes

The operator-facing cache and version commands intentionally differ from the
query surface: cache path stays plain text, while verbose version output exposes
the executable/build identity for debugging.

```bash
path="$(../../tools/biomcp-ci --json cache path)"
printf '%s\n' "$path" | mustmatch '/^\/.*\/\.cache\/biomcp-specs\/http$/'
version="$(../../tools/biomcp-ci version --verbose)"
printf '%s\n' "$version" | mustmatch '/^biomcp 0\.[0-9]+\.[0-9]+/'
printf '%s\n' "$version" | mustmatch like "Executable:"
printf '%s\n' "$version" | mustmatch like 'Build: version='
```

## Emitted Commands Stay Shell-Safe

Suggested commands are part of the CLI surface because operators and agents copy
and paste them directly into shells. Multiword phrases, apostrophes, and
parenthesized tokens must stay runnable when they appear inside emitted follow-up
commands, while plain single-token anchors should stay readable without extra
quoting.

```bash
set -e
plain="$(../../tools/biomcp-ci suggest "What drugs treat melanoma?")"
spaced="$(../../tools/biomcp-ci suggest "What drugs treat paclitaxel protein-bound?")"
printf '%s\n' "$plain" | mustmatch like "biomcp search drug --indication melanoma"
printf '%s\n' "$spaced" | mustmatch like 'biomcp search drug --indication "paclitaxel protein-bound"'
```

Discover is the brittle case because it emits commands directly from free text.
The quoted form below is the copy-paste contract, not presentation polish.

```bash
set -e
discover_get_disease_cmd="$(../../tools/biomcp-ci discover "Graves'" | grep 'biomcp get disease' | head -1 | tr -d '`' | sed 's/^- //')"
discover_trials_cmd="$(../../tools/biomcp-ci discover "Graves'" | grep 'biomcp disease trials' | head -1 | tr -d '`' | sed 's/^- //')"
discover_article_cmd="$(../../tools/biomcp-ci discover "Graves'" | grep 'biomcp search article -k' | head -1 | tr -d '`' | sed 's/^- //')"
discover_get_disease_argv="$(uv run --no-sync python3 -c 'import shlex,sys; print(" ".join(shlex.split(sys.argv[1])))' "$discover_get_disease_cmd")"
discover_trials_argv="$(uv run --no-sync python3 -c 'import shlex,sys; print(" ".join(shlex.split(sys.argv[1])))' "$discover_trials_cmd")"
discover_article_argv="$(uv run --no-sync python3 -c 'import shlex,sys; print(" ".join(shlex.split(sys.argv[1])))' "$discover_article_cmd")"
printf '%s\n' "$discover_get_disease_argv" | mustmatch like "get disease Graves' disease"
printf '%s\n' "$discover_trials_argv" | mustmatch like "disease trials Graves' disease"
printf '%s\n' "$discover_article_argv" | mustmatch like "search article -k Graves' disease"
```

Cross-entity orientation needs the same contract in its counts-only follow-up
links, including parenthesized protein-complex-like terms.

```bash
set -e
paren_cmd="$(../../tools/biomcp-ci search all --keyword "AP-1(c-Jun/c-Fos)" --counts-only | grep 'type review' | head -1 | tr -d '`' | sed 's/^- //')"
paren_argv="$(uv run --no-sync python3 -c 'import shlex,sys; print(shlex.split(sys.argv[1]))' "$paren_cmd")"
printf '%s\n' "$paren_argv" | mustmatch like "AP-1(c-Jun/c-Fos)"
```

Apostrophe-bearing counts-only follow-ups are the current broken surface in
`search all` and must become copy-pasteable.

```bash
set -e
apostrophe_cmd="$(../../tools/biomcp-ci search all --keyword "Graves'" --counts-only | grep 'type review' | head -1 | tr -d '`' | sed 's/^- //')"
apostrophe_argv="$(uv run --no-sync python3 -c 'import shlex,sys; print(" ".join(shlex.split(sys.argv[1])))' "$apostrophe_cmd")"
printf '%s\n' "$apostrophe_argv" | mustmatch like "--keyword Graves'"
```

## Health and Admin Help Stay Explicit

The CLI-only operator surface should still render a health table for quick
inspection and keep local-runtime admin help truthful about what each sync owns.

```bash
health="$(../../tools/biomcp-ci health --apis-only)"
printf '%s\n' "$health" | mustmatch like "# BioMCP Health Check"
printf '%s\n' "$health" | mustmatch like "| API | Status | Latency | Affects |"
whoivd="$(../../tools/biomcp-ci who-ivd sync --help)"
printf '%s\n' "$whoivd" | mustmatch like "WHO Prequalified IVD diagnostic CSV export"
printf '%s\n' "$whoivd" | mustmatch like "Usage: biomcp who-ivd sync"
```

The health implementation should also keep its documented decomposition ratchet
executable in the spec lane so the operator surface cannot regress into one
large catch-all module.

```bash
set +e
structure_out="$(cd ../.. && cargo test --test health_cli_structure -- --nocapture 2>&1)"
structure_status=$?
set -e
printf '%s\n' "$structure_out" | mustmatch like "health_split_files_exist_with_doc_headers"
test "$structure_status" -eq 0
```

## List Command Reference Decomposition Stays Executable

The list command reference should keep its documented decomposition ratchet
executable in the spec lane so page builders cannot regress into one large
catch-all module.

```bash
set +e
list_structure_out="$(cd ../.. && cargo test --test list_cli_structure -- --nocapture 2>&1)"
list_structure_status=$?
set -e
printf '%s\n' "$list_structure_out" | mustmatch like "list_split_files_exist_with_doc_headers"
test "$list_structure_status" -eq 0
```

## Article CLI Test Ownership Stays Decomposed

The article CLI tests should keep the same executable ownership ratchet: a split
sidecar tree with named domains, module headers, and the CLI 700-line cap.

```bash
set +e
structure_out="$(cd ../.. && cargo test --test article_cli_tests_structure -- --nocapture 2>&1)"
structure_status=$?
set -e
printf '%s\n' "$structure_out" | mustmatch like "article_cli_test_split_files_exist_with_doc_headers"
test "$structure_status" -eq 0
```

## Global CLI Line-Cap Allowlist Is Fully Absorbed

The global `src/cli` 700-line ratchet should no longer need the ticket-334
bootstrap exceptions once residual oversized files are decomposed. Keep the
allowlist empty of ticket-347 follow-ups and keep every tracked CLI Rust file
under the cap.

```bash
set +e
structure_out="$(cd ../.. && cargo test --test cli_line_cap_absorption -- --nocapture 2>&1)"
structure_status=$?
set -e
printf '%s\n' "$structure_out" | mustmatch like "ticket_347_residual_allowlist_entries_are_absorbed"
test "$structure_status" -eq 0
```

## Update Verifies Release Checksum

The self-update command must fail closed when the release `.sha256` sidecar
is missing or mismatched. The unsafe override has to be opt-in per
invocation, marked UNSAFE in `--help`, and the underlying policy must stay
covered by named unit tests so the operator surface can never silently
downgrade to TLS-only trust.

```bash
help="$(../../tools/biomcp-ci update --help)"
printf '%s\n' "$help" | grep -Eiq "SHA-?256"
printf '%s\n' "$help" | grep -Eiq "checksum"
printf '%s\n' "$help" | mustmatch like "--allow-missing-checksum"
```

The unsafe marker belongs on the override option itself. This block extracts the
`--allow-missing-checksum` help stanza and checks the warning and checksum
concept inside that stanza rather than matching a floating short token.

```bash
help="$(../../tools/biomcp-ci update --help)"
allow_block="$(HELP_TEXT="$help" uv run --no-sync python3 - <<'PY'
import os
import re

lines = os.environ["HELP_TEXT"].splitlines()
options_start = next(
    index for index, line in enumerate(lines)
    if line.strip() == "Options:"
)
start = next(
    index for index, line in enumerate(lines[options_start + 1 :], options_start + 1)
    if line.strip().startswith("--allow-missing-checksum")
)
block = []
for line in lines[start:]:
    if block and re.match(r"\s*(?:-[A-Za-z],\s*)?--[A-Za-z0-9-]+\b", line):
        break
    block.append(line)
text = "\n".join(block)
assert "UNSAFE" in text, text
assert "checksum" in text.lower(), text
assert re.search(r"SHA-?256", text, flags=re.IGNORECASE), text
print(text)
PY
)"
printf '%s\n' "$allow_block" | mustmatch like "--allow-missing-checksum"
printf '%s\n' "$allow_block" | grep -q "UNSAFE"
printf '%s\n' "$allow_block" | grep -Eiq "checksum"
printf '%s\n' "$allow_block" | grep -Eiq "SHA-?256"
```

```bash
set +e
update_out="$(cd ../.. && cargo test --lib enforce_checksum_policy_missing_sidecar_without_override_fails_closed -- --nocapture 2>&1)"
update_status=$?
set -e
printf '%s\n' "$update_out" | mustmatch like "enforce_checksum_policy_missing_sidecar_without_override_fails_closed"
test "$update_status" -eq 0
```

## MCP Description Audit Filters Update Structurally

The MCP tool description is read-only, so mutating `update` Ops lines must be
filtered by command shape rather than one historical flag list. The quality
ratchet synthesizes a future grammar drift and rejects an exact-marker-only
filter before such a line can leak to MCP clients.

```bash
set +e
mcp_policy_out="$(cd ../.. && uv run --no-sync pytest tests/test_quality_ratchet_contract.py::test_mcp_description_policy_rejects_legacy_update_marker_only -v 2>&1)"
mcp_policy_status=$?
set -e
printf '%s\n' "$mcp_policy_out" | mustmatch like "test_mcp_description_policy_rejects_legacy_update_marker_only"
test "$mcp_policy_status" -eq 0
```

## Benchmark Internal Harness Ratchet Stays Executable

The benchmark tree is an internal regression harness, not a public CLI command.
Its documented decomposition and runtime-wiring ratchet should stay executable
in the spec lane so suite execution, regression analysis, command normalization,
score rendering, and the non-public CLI contract cannot drift.

```bash
set +e
benchmark_structure_out="$(cd ../.. && cargo test --test benchmark_cli_structure -- --nocapture 2>&1)"
benchmark_structure_status=$?
set -e
printf '%s\n' "$benchmark_structure_out" | mustmatch like "benchmark_internal_harness_split_files_exist_with_doc_headers"
printf '%s\n' "$benchmark_structure_out" | mustmatch like "benchmark_internal_harness_contract_pins_runtime_and_docs"
test "$benchmark_structure_status" -eq 0
```

## Article Fulltext JSON Manifests Carry Provenance

Article fulltext still uses the existing `get article <id> fulltext` command, but
agents that request JSON need enough manifest detail to decide whether the saved
Markdown is structured, reusable, and source-native. The deterministic fixture
keeps this contract out of live article services.

```bash
bash ../fixtures/setup-article-fulltext-source-fixture.sh ../..
. ../../.cache/spec-article-fulltext-source-env
trap 'kill "${BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_PID:-}" 2>/dev/null || true' EXIT
jats_json="$(../../tools/biomcp-ci --json get article 22663011 fulltext)"
printf '%s\n' "$jats_json" | mustmatch like '"full_text_source"'
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

PMC HTML fallback is still a useful saved artifact, but the manifest must not
pretend reuse is safe when the fixture has no article-level license. The warning
contract is semantic: a consumer must see that license/reuse is unknown without
depending on exact advisory wording.

```bash
bash ../fixtures/setup-article-fulltext-source-fixture.sh ../..
. ../../.cache/spec-article-fulltext-source-env
trap 'kill "${BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_PID:-}" 2>/dev/null || true' EXIT
html_json="$(../../tools/biomcp-ci --json get article 22663012 fulltext)"
printf '%s\n' "$html_json" | mustmatch like '"full_text_source"'
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

PDF remains an explicit last-resort fallback. When the user opts into that rung,
the JSON manifest must make the PDF fallback and its Semantic Scholar license
visible so downstream ingestion can distinguish it from source-native full text.

```bash
bash ../fixtures/setup-article-fulltext-source-fixture.sh ../..
. ../../.cache/spec-article-fulltext-source-env
trap 'kill "${BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_PID:-}" 2>/dev/null || true' EXIT
pdf_json="$(../../tools/biomcp-ci --json get article 22663013 fulltext --pdf)"
printf '%s\n' "$pdf_json" | mustmatch like '"full_text_source"'
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

## Article Asset Surface Stays Discoverable

Article asset access is a public article surface, not an internal fulltext side
effect. Help, list output, and user-facing docs should teach both the JSON-only
manifest and raw byte retrieval handle so downstream agents can find the assets
without guessing PMC OA URLs.

```bash
../../tools/biomcp-ci get article --help | mustmatch like "assets
asset <name>
raw bytes"
```

```bash
../../tools/biomcp-ci list article | mustmatch like "get article <id> assets
get article <id> asset <name>
raw bytes"
```

```bash
uv run --no-sync python3 -c '
from pathlib import Path
root = Path("../..")
paths = [
    "architecture/functional/article-fulltext.md",
    "architecture/ux/cli-reference.md",
    "docs/user-guide/cli-reference.md",
    "docs/user-guide/article.md",
]
for rel in paths:
    text = (root / rel).read_text(encoding="utf-8")
    assert "get article <id> assets" in text or "get article 22663011 assets" in text, rel
    assert "get article <id> asset <name>" in text or "get article 22663011 asset traces-s1.csv" in text, rel
    assert "no conversion" in text.lower() or "without conversion" in text.lower() or "raw bytes" in text.lower(), rel
print("article asset docs aligned")
' | mustmatch like "article asset docs aligned"
```

## Validation Lanes Stay Split

Routine validation should be deterministic by default: `make spec` and March
`spec-only` run offline/local executable contracts. Public upstream confidence
remains available, but only through the explicit operator-run `make verify` lane
that keeps using the BioMCP spec wrapper.

```bash
out="$(make -C ../.. -n spec 2>&1 || true)"
printf '%s\n' "$out" | mustmatch like "spec/surface/mcp.md"
printf '%s\n' "$out" | mustmatch like "test_parallel_isolation_contract.py"
printf '%s\n' "$out" | mustmatch not like "spec/entity/phenotype.md"
printf '%s\n' "$out" | mustmatch not like "spec/surface/cli.md"
```

The live lane is intentionally named and opt-in so operators can run upstream
checks without making unrelated ordinary tickets depend on public service
availability.

```bash
out="$(make -C ../.. -n verify 2>&1 || true)"
printf '%s\n' "$out" | mustmatch like "tools/biomcp-ci discover"
printf '%s\n' "$out" | mustmatch like "tools/biomcp-ci search disease"
printf '%s\n' "$out" | mustmatch like "spec/entity/phenotype.md"
printf '%s\n' "$out" | mustmatch like "spec/surface/discover.md"
```

The smoke matrix should include article source-status and variant-normalization
confidence through the same wrapper instead of a second cache or replay system.

```bash
out="$(make -C ../.. -n verify 2>&1 || true)"
printf '%s\n' "$out" | mustmatch like "tools/biomcp-ci search article"
printf '%s\n' "$out" | mustmatch like "tools/biomcp-ci variant normalize"
printf '%s\n' "$out" | mustmatch like "spec/entity/pathway.md"
```
