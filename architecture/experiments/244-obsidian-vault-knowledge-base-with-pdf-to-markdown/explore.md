# Explore: Obsidian Vault Knowledge Base with PDF-to-Markdown

## Spike Question

Can BioMCP build a working local knowledge-base prototype that writes durable,
searchable Markdown notes into a temporary Obsidian vault from structured XML,
HTML, and PDF biomedical sources, and which ingestion paths are build-ready
versus blocked or best-effort?

Success for this spike meant:

- Create and populate a temporary vault with 5+ notes from different sources.
- Produce a PDF-to-Markdown quality matrix across `unpdf`, `pdf_oxide`, and a
  Python calibration baseline.
- Produce JATS XML to Markdown samples for at least 2 PMC articles.
- Produce HTML to Markdown samples for at least 3 biomedical page types.
- Produce an Obsidian CLI capability matrix and assess desktop-app dependency.
- Decide which paths are build-ready and document blockers for exploit.

## Prior Art Summary

The ticket did not list explicit prior art, but the repo already has relevant
implementation context.

Existing acquisition code in `src/sources/pmc_oa.rs` resolves PMC OA archive
links, rewrites FTP links to HTTPS, downloads bounded `.tgz` archives, and
extracts the first `.nxml` or `.xml` member. `src/sources/ncbi_efetch.rs`
fetches PMC full-text XML from E-Utilities, normalizes PMCID input, strips
DOCTYPE declarations, and extracts the inner `<article>` when responses are
wrapped in `<pmc-articleset>`.

Existing rendering code in `src/transform/article/jats.rs` and
`src/transform/article/jats/refs.rs` already implements a production-oriented
JATS Markdown path with titles, abstracts, recursive sections, paragraphs,
figures, simple tables, lists, inline formatting, links, and references.

The spike therefore used an independent minimal converter for measurement, but
exploit should reuse or extend the existing production JATS renderer rather
than build a parallel converter.

## Approaches Tried

### 1. Vault and Obsidian Integration

What: direct filesystem writes into an ignored temp vault plus probes for
Obsidian CLI, URI construction, raw text search, and structured frontmatter
search.

How: `run_all_probes.py --only vault` loaded prior JATS/HTML/PDF result records,
wrote Markdown notes with the proposed frontmatter schema, searched the vault,
and attempted `obsidian help`, `search`, `create`, `read`, and `tags`.

Measurements:

- Notes written: 8.
- Frontmatter fields present: `title`, `type`, `source_url`, `source_name`,
  `retrieved_at`, `license`, `doi`, `pmid`, `pmcid`, `nct_id`, `authors`,
  `journal`, `published_at`, `tags`, `biomcp_entities`, `status`.
- Structured frontmatter search: `type: article` found 3 notes,
  `type: preprint` found 1, `pmcid: PMC9984800` found 2, and
  `tags: source/pdf` found 3.
- Literal text search found body/tag strings such as `KEYTRUDA`,
  `long covid`, and `source/pdf`, but not quoted YAML field patterns like
  `type: article`.
- Obsidian CLI path existed at `/snap/bin/obsidian`, but all tested commands
  exited `-11` in this environment. No `x-scheme-handler/obsidian` handler was
  registered.

Assessment: direct vault writes and BioMCP-native structured search are
build-ready. Obsidian CLI and URI should remain optional handoff layers.

### 2. JATS XML Clean Ingest

What: fetch 2 PMC articles and convert JATS XML to Markdown using a small
Rust-native `roxmltree` converter in the experiment probe.

How: the script tried PMC OA archive fetch first and fell back to E-Utilities
XML when needed. Markdown samples were written under ignored `work/`, while
measurements were written to `results/jats_ingest_results.json`.

Measurements:

| PMCID | Success | Score | Headings | Words | Figures | Table-wraps | References |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `PMC9984800` | yes | 5 | 20 | 6,554 | 3 | 2 | 87 |
| `PMC9891841` | yes | 4 | 6 | 29,024 | 17 | 7 | 260 |

Assessment: build-ready as the primary clean ingest path. The existing repo
JATS renderer is stronger than the spike converter and should be the exploit
foundation.

### 3. HTML Open-Page Ingest

What: fetch and convert 3 biomedical page types with `readability-rust` plus
`html2md`.

How: the Rust probe extracted main article content with readability-rust and
converted the selected HTML to Markdown with `html2md::parse_html()`.

Measurements:

| Page | Type | Success | Score | Words | Headings | Links | Table rows |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: |
| PMC article page | article | yes | 5 | 9,599 | 17 | 529 | 82 |
| bioRxiv preprint page | preprint | yes | 4 | 700 | 1 | 0 | 0 |
| NIH news release | news | yes | 5 | 1,215 | 3 | 11 | 0 |

Assessment: build-ready for open pages and news/press pages. For preprints,
HTML is usable but should remain behind structured metadata/JATS discovery
when a JATS path exists.

### 4. PDF Fallback Extraction

What: run `unpdf`, `pdf_oxide`, and `pymupdf4llm` against 3 real biomedical
PDF types: PMC OA article, DailyMed drug label, and CDC clinical guideline.

How: PDF sources were downloaded into ignored `work/pdf/source/`; converter
outputs were written under ignored `work/pdf/markdown/`; measurements were
written to `results/pdf_quality_matrix.json`.

Quality matrix:

| Document | Engine | Success | Score | Heading | Table | Figure | Refs | Runtime |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| PMC OA article | `unpdf` | yes | 4 | 5 | 5 | 3 | 1 | 111 ms |
| PMC OA article | `pdf_oxide` | yes | 4 | 4 | 5 | 3 | 1 | 744 ms |
| PMC OA article | `pymupdf4llm` | yes | 3 | 5 | 5 | 3 | 1 | 5,435 ms |
| DailyMed label | `unpdf` | yes | 2 | 4 | 1 | 1 | 1 | 11,507 ms |
| DailyMed label | `pdf_oxide` | yes | 3 | 5 | 3 | 1 | 1 | 10,004 ms |
| DailyMed label | `pymupdf4llm` | yes | 3 | 5 | 3 | 1 | 1 | 3,820 ms |
| CDC guideline | `unpdf` | yes | 4 | 5 | 5 | 1 | 3 | 3,387 ms |
| CDC guideline | `pdf_oxide` | no | n/a | n/a | n/a | n/a | n/a | 90 s timeout |
| CDC guideline | `pymupdf4llm` | yes | 3 | 5 | 3 | 1 | 3 | 3,177 ms |

Assessment: PDF extraction is viable only as rough text-first fallback.
`unpdf` is the best Rust default from this run because it won or tied on the
PMC article and guideline and finished quickly on the guideline. `pdf_oxide`
is useful on some PDFs, especially the DailyMed label, but the guideline
timeout is a blocker for making it the default without strict time/page limits.
The Python baseline was helpful calibration but did not beat the Rust options.

## Decision

Promote the knowledge-base feature into exploit with this shape:

- Primary storage: direct filesystem writes into an Obsidian-compatible vault.
- Primary retrieval: BioMCP-native local search with structured frontmatter
  parsing, not raw substring search alone.
- Primary article ingest: JATS/XML using existing repo acquisition and JATS
  rendering code.
- Secondary ingest: open HTML through readability-rust plus html2md.
- Fallback ingest: PDF-to-Markdown as rough extraction for agent readability,
  with `unpdf` as the default Rust engine and strict timeout/page limits.
- Optional handoff: Obsidian CLI and URI integration only when the local
  desktop environment proves it works.

Grounding:

- JATS succeeded on 2/2 articles with scores 5 and 4.
- HTML succeeded on 3/3 page types with scores 5, 4, and 5.
- The temp vault held 8 notes and structured search found expected article,
  preprint, PMCID, and PDF-tagged notes.
- PDF had useful outputs but inconsistent scores and one `pdf_oxide` timeout.
- Obsidian CLI was installed but no tested command worked noninteractively.

## Outcome

promote

## Risks for Exploit

- Obsidian CLI behavior varies by installer/app state. Treat it as optional,
  detect capabilities at runtime, and never make it the durable write path.
- Frontmatter search should parse YAML structurally. Raw text search misses
  quoted YAML values and is not enough for identifier queries.
- JATS table support needs hardening for spanning cells, nested tables, math,
  and richer figure/asset handling.
- HTML extraction quality depends on page templates. bioRxiv HTML was readable
  but shallow, so structured preprint APIs/JATS paths should be preferred.
- PDF extraction needs page limits, timeouts, source-specific handling, and
  clear UX copy that results are rough and may lose figure/table fidelity.
- Downloaded PDFs/XML/HTML and generated vault contents must remain ignored
  under `work/`; exploit code should persist only user-requested vault notes
  and controlled metadata.
