---
name: biomcp
description: Search and retrieve biomedical data - genes, variants, clinical trials, articles, drugs, diseases, pathways, proteins, adverse events, pharmacogenomics, and phenotype-disease matching. Use for gene function, variant pathogenicity, trials, drug safety, pathway context, disease workups, and literature evidence.
---

# BioMCP CLI

## Routing rules

- Start with the narrowest command that matches the question.
- Use `biomcp discover "<free text>"` when you only have free text and need the CLI to pick the first typed command.
- Use `biomcp search all --gene <gene> --disease "<disease>"` when you know the entities but not the next pivot.
- Treatment questions: `biomcp search drug --indication "<disease>" --limit 5`
- Symptom or phenotype questions: `biomcp get disease <name_or_id> phenotypes`
- Gene-function questions: `biomcp get gene <symbol>`
- Drug-safety questions: `biomcp drug adverse-events <name>` and `biomcp get drug <name> safety`
- Review-literature questions: `biomcp search article -k "<query>" --type review --limit 5`
- After `search article`, default to `biomcp article batch <id1> <id2> ...` instead of repeated `get article` calls. Batch up to 20 shortlisted papers in one call.
- Use `biomcp batch gene <GENE1,GENE2,...>` when you need the same basic card fields, chromosome, or sectioned output for multiple genes.
- For diseases with weak ontology-name coverage, run `biomcp discover "<disease>"` first, then pass a resolved `MESH:...`, `OMIM:...`, `ICD10CM:...`, `MONDO:...`, or `DOID:...` identifier to `biomcp get disease`.
- Avoid `--type` when recall matters across sources. `--type` is Europe PMC only today because PubTator3 and Semantic Scholar search results do not expose publication-type filtering.
- Multi-hop article follow-up: `biomcp article citations <id> --limit 5` and `biomcp article recommendations <id> --limit 5`

## Section reference

- `get gene ... protein`: UniProt function and localization detail
- `get gene ... hpa`: Human Protein Atlas tissue expression and localization
- `get gene ... expression`: GTEx tissue expression
- `get gene ... diseases`: disease associations
- `get article ... annotations`: PubTator normalized entity mentions for standardized extraction
- `get article ... tldr`: Semantic Scholar summary and influence
- `get disease ... genes`: associated genes
- `get disease ... phenotypes`: HPO phenotype annotations; source-backed and sometimes incomplete
- `get disease ... pathways`: pathways from associated genes
- `get drug ... label`: FDA label indications, warnings, and dosage
- `get drug ... regulatory`: regulatory summary
- `get drug ... safety`: safety context and warnings
- `get drug ... targets`: ChEMBL and OpenTargets targets
- `get drug ... indications`: OpenTargets indication evidence

## Cross-entity pivot rules

- `gene articles <symbol>` and `search article -g <symbol>` are equivalent starting points for gene-filtered literature.
- Use helpers when the pivot is obvious: `drug trials`, `disease trials`, `variant articles`, `article citations`.
- Use `search article -d "<disease>" --type review --limit 5` when disease phenotypes or drug indications look sparse.
- Use `article batch` as the default multi-article follow-up after `search article`; it replaces sequential `get article` calls and preserves Semantic Scholar enrichment when available.
- Use `batch <entity> <id1,id2,...> --sections <s1,s2,...>` when you need the same card shape for several entities.
- Use `enrich <GENE1,GENE2,...>` once you have a real gene set and want pathways or GO-style categories.

## Output and evidence rules

- Quote multi-word IDs or names in commands.
- Do not invent sections, filters, or helper flags that `biomcp list` does not show.
- Treat empty structured regulatory drug results as signal for approved-drug questions, not as a CLI failure.
- Prefer review articles for synthesis questions and structured sections for direct facts.
- Use `_meta.next_commands` from JSON mode as the executable follow-up contract.

## Answer commitment

- Only add more commands if a needed claim is still unsupported. If one command already answers the question, stop searching and answer.
- If a structured section already contains the answer, use it. Anti-pattern: after `biomcp get drug nivolumab regulatory` shows `Sponsor: BRISTOL MYERS SQUIBB`, do not search articles just to confirm who developed nivolumab.
- If 1-2 papers you already fetched state the answer in the abstract or TLDR, answer from those papers instead of hunting for a third paper.
- If 3+ searches keep returning relevant papers, the answer is in what you already have or you need a different approach. If you keep reformulating the same search with different keywords, the answer is in what you already have or you need a different approach. Example: once repeated tau PET or European influenza vaccine searches keep surfacing relevant review papers, stop keyword-churning and extract the answer from those results.

Run `biomcp skill list` for worked examples.
