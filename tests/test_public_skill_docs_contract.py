from __future__ import annotations

from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]


def _read(path: str) -> str:
    return (REPO_ROOT / path).read_text(encoding="utf-8")


def test_public_skill_docs_match_current_cli_contract() -> None:
    readme = _read("README.md")
    docs_index = _read("docs/index.md")
    skill_file = _read("skills/SKILL.md")
    treatment_use_case = _read("skills/use-cases/01-treatment-lookup.md")
    symptom_use_case = _read("skills/use-cases/02-symptom-phenotype.md")
    orientation_use_case = _read("skills/use-cases/03-gene-disease-orientation.md")
    article_follow_up = _read("skills/use-cases/04-article-follow-up.md")
    skills = _read("docs/getting-started/skills.md")
    reproduce = _read("docs/how-to/reproduce-papers.md")
    cli_reference = _read("docs/user-guide/cli-reference.md")
    article_guide = _read("docs/user-guide/article.md")
    find_articles = _read("docs/how-to/find-articles.md")
    keyword_reference = _read("docs/reference/article-keyword-search.md")
    data_sources = _read("docs/reference/data-sources.md")
    quick_reference = _read("docs/reference/quick-reference.md")
    pivot_guide = _read("docs/how-to/cross-entity-pivots.md")
    blog = _read("docs/blog/biomcp-kuva-charts.md")
    mcp_server = _read("docs/reference/mcp-server.md")
    claude_desktop = _read("docs/getting-started/claude-desktop.md")

    assert "14 guided investigation workflows are built in" not in readme
    assert "biomcp skill install ~/.claude --force" in readme
    assert "`biomcp skill` to read the embedded BioMCP guide" in readme
    assert "biomcp skill list" not in readme
    assert "biomcp skill show 03" not in readme

    assert "14 guided investigation workflows are built in" not in docs_index
    assert "getting-started/skills.md" in docs_index
    assert "biomcp skill install ~/.claude --force" in docs_index

    assert "# Skills" in skills
    assert "biomcp skill" in skills
    assert "biomcp skill list" in skills
    assert "biomcp skill article-follow-up" in skills
    assert "SKILL.md" in skills
    assert "use-cases/" in skills
    assert "jq-examples.md" in skills
    assert "examples/" in skills
    assert "schemas/" in skills
    assert "Legacy compatibility note" not in skills
    assert "No skills found" not in skills

    assert "# Skills" in skills
    assert "biomcp skill install ~/.claude" in skills

    assert "biomcp skill list" not in reproduce
    assert "biomcp skill gene-function-lookup" not in reproduce
    assert "biomcp skill 03" not in reproduce
    assert "biomcp get gene BRAF" in reproduce
    assert 'biomcp get variant "BRAF V600E" population' in reproduce
    assert 'biomcp search trial -c melanoma --mutation "BRAF V600E" --status recruiting --limit 5' in reproduce
    assert "biomcp get article 22663011 fulltext" in reproduce

    assert "biomcp skill [list|install|<name>]" not in cli_reference
    assert "biomcp skill install [dir]" in cli_reference
    assert "biomcp cache path" in cli_reference
    assert "biomcp cache stats" in cli_reference
    assert "biomcp cache clean" in cli_reference
    assert "biomcp cache clear" in cli_reference
    assert "biomcp skill list                 # list embedded worked examples" in cli_reference
    assert (
        "`--json` normally returns structured output, but `biomcp cache path` "
        "is a plain-text exception. `biomcp cache stats`, `biomcp cache clean`, "
        "and `biomcp cache clear` respect `--json` on success. `biomcp cache clear` "
        "still refuses non-TTY destructive runs with plain stderr unless you pass `--yes`."
        in cli_reference
    )
    assert "biomcp serve-sse                  # removed compatibility command; use serve-http" not in cli_reference
    assert (
        "`biomcp serve-sse` remains available only as a hidden compatibility "
        "command that points users back to `biomcp serve-http`."
        in cli_reference
    )
    assert "Streamable HTTP" in cli_reference
    assert "/mcp" in cli_reference

    assert "one markdown resource per embedded skill use-case" in mcp_server
    assert "biomcp://help" in mcp_server
    assert "biomcp://skill/<slug>" in mcp_server
    assert "Streamable HTTP" in mcp_server
    assert "`biomcp serve-http`" in mcp_server
    assert "`/mcp`" in mcp_server
    assert "`/health`" in mcp_server
    assert "`/readyz`" in mcp_server
    assert "`/`" in mcp_server
    assert "`cache path`" in mcp_server
    assert "`cache stats`" in mcp_server
    assert "`cache clean`" in mcp_server
    assert "`cache clear`" in mcp_server
    assert "reveal workstation-local paths" in mcp_server

    assert "one markdown resource per embedded BioMCP worked example" in claude_desktop
    assert "biomcp://help" in claude_desktop
    assert "biomcp://skill/<slug>" in claude_desktop

    assert "## Routing rules" in skill_file
    assert "## Section reference" in skill_file
    assert "## Cross-entity pivot rules" in skill_file
    assert "## Output and evidence rules" in skill_file
    assert 'biomcp search drug --indication "<disease>"' in skill_file
    assert 'biomcp discover "<free text>"' in skill_file
    assert (
        "After `search article`, default to `biomcp article batch <id1> <id2> ...` instead of repeated `get article` calls."
        in skill_file
    )
    assert (
        "Use `biomcp batch gene <GENE1,GENE2,...>` when you need the same basic card fields, chromosome, or sectioned output for multiple genes."
        in skill_file
    )
    assert (
        "For diseases with weak ontology-name coverage, run `biomcp discover \"<disease>\"` first, then pass a resolved `MESH:...`, `OMIM:...`, `ICD10CM:...`, `MONDO:...`, or `DOID:...` identifier to `biomcp get disease`."
        in skill_file
    )
    assert (
        "Avoid `--type` when recall matters across sources. `--type` is Europe PMC only today because PubTator3 and Semantic Scholar search results do not expose publication-type filtering."
        in skill_file
    )
    assert "_meta.next_commands" in skill_file
    assert "Run `biomcp skill list` for worked examples" in skill_file

    assert "Use `article batch` as the default follow-up after `search article`" in article_guide
    assert "`--type` on `--source all` uses Europe PMC + PubMed" in article_guide
    assert "PMC-only note" in article_guide
    assert "LitSense2-derived semantic signal" in article_guide
    assert "Rows without LitSense2 provenance contribute `ranking.semantic_score = 0`" in article_guide
    assert "MeSH/title/abstract" not in article_guide
    assert (
        "Put a known gene, disease, or drug in `-g/--gene`, `-d/--disease`, or `--drug`."
        in article_guide
    )
    assert (
        'biomcp search article -k \'"cafe-au-lait spots" neurofibromas disease\' --type review --limit 5'
        in article_guide
    )
    assert (
        'biomcp search article --drug amiodarone -k "photosensitivity mechanism" --limit 5'
        in article_guide
    )
    assert (
        "Use `article batch` after search when you already know the candidate PMIDs or"
        in find_articles
    )
    assert "`--type` on the default `--source all` route uses Europe PMC + PubMed" in find_articles
    assert "Europe PMC-only with an explicit note" in find_articles
    assert "LitSense2-derived" in find_articles
    assert "semantic=0" in find_articles
    assert "MeSH/title/abstract" not in find_articles
    assert "Do not guess `-g`, `-d`, or `--drug`" in find_articles
    assert 'biomcp search article -k "TCGA mutation analysis dataset" --type review --limit 5' in find_articles
    assert "MeSH/title/abstract" not in keyword_reference
    assert "On the default `--source all` route, adding `-k/--keyword` also brings LitSense2" in keyword_reference
    assert "LitSense2-derived semantic signal" in keyword_reference
    assert "semantic=0" in keyword_reference
    assert "do not guess a disease or drug name" in keyword_reference
    assert (
        'biomcp search article --drug amiodarone -k "photosensitivity mechanism" --limit 5'
        in keyword_reference
    )

    assert "# Pattern: Treatment / approved-drug lookup" in treatment_use_case
    assert 'biomcp search drug --indication "myasthenia gravis" --limit 5' in treatment_use_case
    assert "# Pattern: Symptom / phenotype lookup" in symptom_use_case
    assert 'biomcp get disease "Marfan syndrome" phenotypes' in symptom_use_case
    assert "# Pattern: Gene-in-disease orientation" in orientation_use_case
    assert 'biomcp search all --gene BRAF --disease "melanoma"' in orientation_use_case
    assert "# Pattern: Article follow-up via citations and recommendations" in article_follow_up
    assert "biomcp article citations 22663011 --limit 5" in article_follow_up

    assert "publisher elision" in article_guide
    assert "next_commands" in article_guide

    assert "biomcp enrich` uses **g:Profiler**" in data_sources
    assert "Gene enrichment sections" in data_sources
    assert "Enrichr" in data_sources

    assert "biomcp article references 22663011 --limit 3" in quick_reference
    assert "biomcp article references 22663011 --limit 3" in pivot_guide

    assert "docs/blog/images/tp53-mutation-bar.svg" in blog
    assert "![TP53 mutation classes as a bar chart](images/tp53-mutation-bar.svg)" in blog
    assert "![Terminal screenshot placeholder: mutation-bar-terminal.png](images/mutation-bar-terminal.png)" in blog
    assert "![Terminal screenshot placeholder: ridgeline-terminal.png](images/ridgeline-terminal.png)" in blog
