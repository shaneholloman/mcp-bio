# Harden: Obsidian Vault Knowledge Base with PDF-to-Markdown

## Decomposition

The optimized implementation is now split into reusable library code and thin
wrappers.

Rust extraction code lives in:

- `architecture/experiments/244-obsidian-vault-knowledge-base-with-pdf-to-markdown/scripts/rust_probe/src/lib.rs`

It contains the reusable JATS, HTML, PDF, metrics, scoring, and report types.
The Rust CLI wrapper is:

- `architecture/experiments/244-obsidian-vault-knowledge-base-with-pdf-to-markdown/scripts/rust_probe/src/main.rs`

That wrapper is 78 lines and only handles Clap argument parsing, timing, and
JSON printing.

Python benchmark/vault orchestration lives in:

- `architecture/experiments/244-obsidian-vault-knowledge-base-with-pdf-to-markdown/scripts/obsidian_kb_spike/exploit.py`
- `architecture/experiments/244-obsidian-vault-knowledge-base-with-pdf-to-markdown/scripts/obsidian_kb_spike/__init__.py`

The historical entry point remains:

- `architecture/experiments/244-obsidian-vault-knowledge-base-with-pdf-to-markdown/scripts/run_exploit.py`

That wrapper is 20 lines and only imports `obsidian_kb_spike.run_full_scale`.
The hardened harness builds the Rust probe once, then invokes the built probe
binary directly for benchmark extraction calls. Downstream Rust consumers
should import the Rust library instead of shelling out.

The ticket and external planning graph do not name explicit downstream spike
IDs. The practical downstream consumer is the future BioMCP knowledge-base
build work, which needs these proven surfaces: clean JATS ingest, open HTML
ingest, bounded PDF fallback extraction, Obsidian-compatible note writing,
BioMCP-owned frontmatter search, optional Obsidian handoff probing, and
regression/validation helpers.

## Public API

Rust crate: `biomcp_kb_rust_probe`

- `PdfEngine`: shared selector for `Unpdf` and `PdfOxide`.
- `ProbeReport`: serializable report shape used by harnesses and CLIs.
- `extract_jats_markdown(xml: &str)`: converts JATS XML to Markdown and
  metrics.
- `extract_html_markdown(html: &str, base_url: &str)`: runs readability-rust
  plus html2md and returns Markdown plus metrics.
- `extract_pdf_markdown(input: &Path, engine: PdfEngine, page_limit: u32)`:
  runs bounded PDF extraction through `unpdf` or `pdf_oxide`.
- `run_jats_file`, `run_html_file`, `run_pdf_file`: file-oriented wrappers
  that read input, write Markdown, time the extraction, and return
  `ProbeReport`.
- `markdown_metrics`, `score_jats`, `score_html`, `score_pdf`: reusable
  quality and regression helpers.

Rust import example:

```rust
use std::path::Path;

use biomcp_kb_rust_probe::{
    extract_html_markdown, extract_jats_markdown, extract_pdf_markdown, PdfEngine,
};

let (jats_markdown, jats_metrics) = extract_jats_markdown(&xml)?;
let (html_markdown, html_metrics) = extract_html_markdown(&html, source_url)?;
let (pdf_markdown, pdf_metrics, engine_name) =
    extract_pdf_markdown(Path::new("article.pdf"), PdfEngine::Unpdf, 12)?;
```

Python package: `obsidian_kb_spike`

This package is the benchmark/vault orchestration layer. Product extraction
consumers should import the Rust crate directly; the Python extraction-family
helpers exist to rerun this spike's measured workload.

- `run_full_scale()`: runs the optimized full ticket workload and writes the
  result JSON artifacts.
- `run_jats_exploit()`, `run_html_exploit()`, `run_pdf_exploit(page_limit)`:
  reusable benchmark family entry points.
- `build_vault(jats, html, pdf)`: writes Obsidian-compatible Markdown notes
  with the agreed frontmatter schema and unique filenames.
- `run_frontmatter_searches(vault_dir, queries)`: structured BioMCP-owned
  frontmatter search.
- `run_local_searches(vault_dir, queries)`: literal local filesystem search.
- `run_obsidian_probe(vault_path)`: optional local CLI/URI handoff probe.
- `build_regression_control`, `build_validation`, `build_contract_numbers`:
  report and guard helpers.

Python import example:

```python
from pathlib import Path

from obsidian_kb_spike import build_vault, run_frontmatter_searches

vault = build_vault(jats_results, html_results, pdf_results)
matches = run_frontmatter_searches(
    Path(vault["vault_path"]),
    ["type: article", "pmcid: PMC9984800", "tags: source/pdf"],
)
```

## Build System

This spike is Rust/Python, not Zig; there is no `build.zig` in the experiment.
The equivalent build-system update is in:

- `architecture/experiments/244-obsidian-vault-knowledge-base-with-pdf-to-markdown/scripts/rust_probe/Cargo.toml`

`Cargo.toml` now declares both:

- `[lib] name = "biomcp_kb_rust_probe", path = "src/lib.rs"`
- `[[bin]] name = "biomcp-kb-rust-probe", path = "src/main.rs"`

A downstream Rust spike can depend on the library with a path dependency:

```toml
[dependencies]
biomcp-kb-rust-probe = { path = "../244-obsidian-vault-knowledge-base-with-pdf-to-markdown/scripts/rust_probe" }
```

Then import it with Rust's underscore crate name:

```rust
use biomcp_kb_rust_probe::{extract_jats_markdown, PdfEngine};
```

For Python harness reuse, add the experiment `scripts/` directory to
`PYTHONPATH` and import `obsidian_kb_spike`.

## Regression Check

Commands run:

```bash
cargo check --manifest-path architecture/experiments/244-obsidian-vault-knowledge-base-with-pdf-to-markdown/scripts/rust_probe/Cargo.toml
python3 -m py_compile \
  architecture/experiments/244-obsidian-vault-knowledge-base-with-pdf-to-markdown/scripts/run_exploit.py \
  architecture/experiments/244-obsidian-vault-knowledge-base-with-pdf-to-markdown/scripts/obsidian_kb_spike/__init__.py \
  architecture/experiments/244-obsidian-vault-knowledge-base-with-pdf-to-markdown/scripts/obsidian_kb_spike/exploit.py
python3 architecture/experiments/244-obsidian-vault-knowledge-base-with-pdf-to-markdown/scripts/run_exploit.py
```

Final hardened benchmark:

| Metric | Hardened result |
| --- | ---: |
| Total elapsed | 45,674 ms |
| Optimized final elapsed | 45,726 ms |
| JATS success | 2/2 |
| HTML success | 3/3 |
| Rust PDF success | 5/6 |
| Overall PDF success | 8/9 |
| Vault note records | 8 |
| Vault unique note files | 8 |
| Duplicate path mismatches | 0 |
| Structured `type: article` matches | 4 |
| Structured `type: preprint` matches | 1 |
| Structured `pmcid: PMC9984800` matches | 3 |
| Structured `tags: source/pdf` matches | 3 |
| Structured `doi:` matches | 8 |
| Obsidian CLI working commands | 0 |
| Regression control | pass |
| Validation | pass |

PDF winners stayed unchanged:

| Document | Winning engine | Score |
| --- | --- | ---: |
| `pmc_oa_article_pdf` | `unpdf` | 4 |
| `dailymed_keytruda_label` | `pdf_oxide` | 3 |
| `cdc_sti_guideline` | `unpdf` | 4 |

The refactor preserved correctness and matched or beat the optimized elapsed
contract by 52 ms.

## Reusable Assets

Downstream work inherits:

- Rust JATS XML-to-Markdown converter with section, paragraph, figure, table,
  list, link, inline formatting, reference, and quality metric support.
- Rust HTML-to-Markdown converter using readability-rust plus html2md.
- Rust bounded PDF fallback extraction through `unpdf` and `pdf_oxide`.
- Shared `PdfEngine` and `ProbeReport` types.
- Markdown quality metrics and scoring helpers for JATS, HTML, and PDF paths.
- Python Obsidian-compatible vault writer with unique filename handling.
- YAML frontmatter schema and frontmatter parser/search helpers.
- Optional Obsidian CLI/URI probe helpers.
- Regression, validation, and contract-number builders.
- Cargo library/binary split pattern for future experiment hardening.
