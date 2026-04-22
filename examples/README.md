# BioMCP Examples

This folder contains runnable example surfaces for local experimentation. Most
subfolders are paper-style benchmark harnesses with `prompt.md`, `run.sh`, and
`score.sh`; `streamable-http/` is a standalone transport demo.

## Canonical Workflows

For day-to-day agent use, the canonical workflow interface is the embedded
skills, not this examples folder.

Use:

```bash
biomcp skill list
biomcp skill <number-or-slug>
```

## Mapping

| Example folder | Canonical skill |
|----------------|-----------------|
| [genegpt/](genegpt/README.md) | `gene-disease-orientation` |
| [geneagent/](geneagent/README.md) | `gene-disease-orientation` |
| [trialgpt/](trialgpt/README.md) | `treatment-lookup` |
| [pubmed-beyond/](pubmed-beyond/README.md) | `article-follow-up` |

## Example Index

| Example folder | What it does |
|----------------|--------------|
| [geneagent/](geneagent/README.md) | Replays a gene-set-analysis workflow with prompt, run, and scoring assets. |
| [genegpt/](genegpt/README.md) | Reproduces a gene-function lookup workflow with captured benchmark harness files. |
| [pubmed-beyond/](pubmed-beyond/README.md) | Replays a literature-synthesis workflow over BioMCP with benchmark assets. |
| [trialgpt/](trialgpt/README.md) | Reproduces a patient-matching and trial-search workflow with benchmark assets. |

## Standalone Examples

| Example folder | What it does |
|----------------|--------------|
| [streamable-http/](streamable-http/README.md) | Runs a Streamable HTTP client against `biomcp serve-http` and proves the remote `biomcp` MCP tool can complete a three-step BRAF workflow. |

## When to Use This Folder

Use the paper-style examples when you want a quick local benchmark harness with
captured outputs or metrics. Use `streamable-http/` when you want a runnable
remote-transport proof. Use embedded skills when you want the production
workflow instructions agents should follow.
