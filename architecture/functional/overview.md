# BioMCP Functional Overview

## What BioMCP Is For

BioMCP is a biomedical data access layer for AI agents and human researchers.
It provides a unified CLI and MCP server that queries 15+ biomedical databases
through a single consistent command grammar. Users ask biomedical questions
("what trials are enrolling for BRAF V600E?", "what is the clinical significance
of this variant?") and get structured, markdown-formatted answers drawn from
authoritative upstream sources.

The design contract: one binary, one grammar, no API key juggling for common
queries, no knowledge of upstream API idiosyncrasies required.

## Audience

**Primary users:**
- Biomedical researchers — literature review, variant interpretation, pathway
  analysis
- Clinicians and clinical informaticists — trial matching, drug safety review,
  variant clinical significance
- Bioinformaticians — gene-set enrichment, cross-entity pivots, protein
  structure queries
- AI agents (via MCP) — structured biomedical data retrieval within agent
  investigation workflows

**Secondary users:**
- Tool developers embedding BioMCP in their own pipelines
- Educators and students learning genomics and oncology workflows

## Entity Surface

BioMCP exposes 12 entity types. All support `search` and `get` commands.

| Entity | Key Sources | Representative Command |
|--------|-------------|----------------------|
| gene | MyGene.info, UniProt, Reactome, QuickGO, STRING, GTEx, Human Protein Atlas, DGIdb, ClinGen, gnomAD, CIViC | `biomcp get gene BRAF pathways` |
| variant | MyVariant.info, ClinVar, gnomAD, CIViC, OncoKB, cBioPortal, GWAS Catalog, AlphaGenome | `biomcp get variant "BRAF V600E" clinvar` |
| article | PubMed, PubTator3, Europe PMC, Semantic Scholar (optional) | `biomcp search article -g BRAF --limit 5` |
| trial | ClinicalTrials.gov, NCI CTS API | `biomcp search trial -c melanoma -s recruiting` |
| drug | MyChem.info, ChEMBL, OpenTargets, Drugs@FDA, CIViC | `biomcp get drug pembrolizumab targets` |
| disease | MyDisease.info, Monarch Initiative, MONDO, OpenTargets, Reactome, CIViC, SEER Explorer | `biomcp get disease "Hodgkin lymphoma" survival` |
| pathway | Reactome, KEGG, g:Profiler | `biomcp get pathway R-HSA-5673001 genes` |
| protein | UniProt, InterPro, STRING, ComplexPortal, PDB, AlphaFold | `biomcp get protein P15056 domains` |
| adverse-event | OpenFDA (FAERS, MAUDE, Recalls) | `biomcp search adverse-event -d pembrolizumab` |
| pgx | CPIC, PharmGKB | `biomcp get pgx CYP2D6 recommendations` |
| gwas | GWAS Catalog | `biomcp search gwas --trait "type 2 diabetes"` |
| phenotype | Monarch Initiative (HPO) | `biomcp search phenotype "HP:0001250"` |

This 12-row table is the high-level public entity surface. It intentionally
does not fold the local `study` analytics family into the entity list.

This table is a high-level shipped source map; section-specific constraints and
transport details live in the technical architecture docs.

## Study Command Family

`study` is a separate local analytics surface for downloaded cBioPortal-style
datasets. It complements the remote read-only entities above rather than
expanding the public README entity table.

Primary command family:

`biomcp study list|download|filter|query|co-occurrence|cohort|survival|compare`

What it adds:
- Local dataset discovery and download (`study list`, `study download`)
- Cohort slicing by mutation/CNA/expression filters (`study filter`, `study cohort`)
- Per-gene and comparative analysis (`study query`, `study compare`)
- Cohort-level association workflows (`study co-occurrence`, `study survival`)

## Skills Surface

BioMCP ships an embedded agent guide plus worked examples. `biomcp skill`
prints the embedded `skills/SKILL.md` overview, `biomcp skill list` shows
embedded worked examples, and `biomcp skill <name>` opens an embedded worked
example by number or slug. `biomcp skill install <dir>` exports that guide,
the `use-cases/` catalog, and supporting references into an agent directory.

The current runtime contract is:

- `biomcp skill` shows the BioMCP agent guide
- `biomcp skill list` shows embedded worked examples
- `biomcp skill <name>` opens an embedded worked example
- `biomcp skill install <dir>` installs that guide into `skills/biomcp/`
- MCP resource listing includes `biomcp://help` plus `biomcp://skill/<slug>`
  for each embedded worked example

The durable user value is guided investigation support with a layered help
system: overview first, then executable examples on demand.

## Command Grammar

```
search <entity> [filters]    → discovery across a source type
get <entity> <id> [sections] → focused detail with progressive disclosure
<entity> <helper> <id>       → cross-entity pivot
discover <query>            → free-text concept resolution into typed follow-up commands
enrich <GENE1,GENE2,...>     → gene-set enrichment
batch <entity> <id1,id2,...> → parallel gets
search all [slot filters]    → unified fan-out across all entities
```

`search all` is slot-first. The primary contract is typed slots such as
`--gene`, `--variant`, `--disease`, `--drug`, and `--keyword`:

- `biomcp search all --gene BRAF --disease melanoma`
- `biomcp search all --keyword "checkpoint inhibitor" --counts-only`

Positional keyword search remains available as a secondary alias:

- `biomcp search all BRAF`

Key cross-entity pivot examples:
- `biomcp variant trials "BRAF V600E"` — trials for a variant
- `biomcp gene drugs BRAF` — drugs targeting a gene
- `biomcp disease articles "Lynch syndrome"` — articles for a disease
- `biomcp pathway drugs R-HSA-5673001` — drugs in a pathway

Progressive disclosure: every `get` command returns a summary card by default.
Named sections extend the output: `biomcp get gene BRAF pathways civic all`.

## Done-Enough Criteria

### For G002 (Community Value)

- A researcher with no prior BioMCP knowledge can install it, run
  `biomcp health`, install the BioMCP agent guide, and complete a guided
  investigation in one session
- The embedded BioMCP skill guide plus worked examples for treatment,
  phenotypes, article follow-up, and broad gene-disease investigation produce
  correct, well-formatted output against live upstream APIs
- PyPI package (`biomcp-cli`) is available and installs cleanly
- Documentation at biomcp.org covers install, quick start, and the
  `biomcp skill` / `biomcp skill install` workflow

### For G003 (v1.0)

- The embedded BioMCP skill guide exports cleanly and references only real
  commands
- `search all` works reliably as the unified entry point
- CLI help, error messages, and next-step suggestions are accurate
  (no stale command references)
- Evidence URLs (`_meta.evidence_urls`) are present in output
- Spec suite passes (`spec/` BDD docs all green)
- Bug-free on: BRAF V600E variant lookup, melanoma trial search, pembrolizumab
  drug safety, BRCA1 article search
- Paper or citation published
