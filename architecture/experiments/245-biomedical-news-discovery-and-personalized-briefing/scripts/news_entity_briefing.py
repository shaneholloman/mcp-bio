#!/usr/bin/env -S uv run
# /// script
# dependencies = [
#   "beautifulsoup4>=4.12",
#   "python-dateutil>=2.9",
#   "requests>=2.32",
#   "trafilatura>=2.0",
# ]
# ///

from __future__ import annotations

import argparse
import json
import re
import subprocess
from collections import Counter
from pathlib import Path
from typing import Any

import requests
import trafilatura

from news_common import fetch_url, json_dump, paywall_signals, simple_html_text, utc_now_iso

DOI_RE = re.compile(r"\b10\.\d{4,9}/[-._;()/:A-Z0-9]+\b", re.I)
PMID_RE = re.compile(r"\bPMID[:\s]*(\d{6,9})\b", re.I)
NCT_RE = re.compile(r"\bNCT\d{8}\b", re.I)
PHASE_RE = re.compile(r"\bphase\s+(?:1/2|2/3|I/II|II/III|I|II|III|IV|[1234])\b", re.I)

GENES = [
    "KRAS",
    "BRAF",
    "EGFR",
    "TP53",
    "BRCA1",
    "BRCA2",
    "ERBB2",
    "ALK",
    "ROS1",
    "NTRK",
    "RET",
    "MET",
    "PIK3CA",
    "IDH1",
    "IDH2",
    "JAK2",
    "CDK4",
    "CDK6",
    "PDCD1",
    "PDL1",
    "CD274",
]

DRUGS = [
    "pembrolizumab",
    "Keytruda",
    "nivolumab",
    "Opdivo",
    "ipilimumab",
    "Yervoy",
    "atezolizumab",
    "Tecentriq",
    "durvalumab",
    "Imfinzi",
    "avelumab",
    "Bavencio",
    "cemiplimab",
    "Libtayo",
    "dostarlimab",
    "sotorasib",
    "Lumakras",
    "adagrasib",
    "Krazati",
    "osimertinib",
    "Tagrisso",
    "trastuzumab",
    "Herceptin",
    "olaparib",
    "Lynparza",
]

DISEASES = [
    "melanoma",
    "lung cancer",
    "non-small cell lung cancer",
    "NSCLC",
    "breast cancer",
    "colorectal cancer",
    "pancreatic cancer",
    "leukemia",
    "lymphoma",
    "myeloma",
    "glioblastoma",
    "solid tumor",
    "cancer",
]

COMPANIES = [
    "Merck",
    "Bristol Myers Squibb",
    "BMS",
    "Roche",
    "Genentech",
    "Novartis",
    "Pfizer",
    "AstraZeneca",
    "GSK",
    "Eli Lilly",
    "Lilly",
    "Amgen",
    "Regeneron",
    "Moderna",
    "BioNTech",
    "AbbVie",
    "Sanofi",
    "Johnson & Johnson",
    "J&J",
    "Gilead",
    "Vertex",
    "Bayer",
    "Takeda",
    "BeiGene",
]

PROFILE_TERMS = {
    "oncologist": 3,
    "oncology": 3,
    "cancer": 3,
    "tumor": 2,
    "melanoma": 5,
    "immunotherapy": 5,
    "immune checkpoint": 4,
    "checkpoint inhibitor": 4,
    "PD-1": 4,
    "PD-L1": 4,
    "KRAS": 5,
    "sotorasib": 4,
    "adagrasib": 4,
    "clinical trial": 2,
    "phase": 1,
}


def find_terms(text: str, terms: list[str]) -> list[str]:
    found = []
    for term in terms:
        if re.search(rf"(?<![A-Za-z0-9]){re.escape(term)}(?![A-Za-z0-9])", text, re.I):
            found.append(term)
    return sorted(set(found), key=str.lower)


def extract_entities(text: str) -> dict[str, Any]:
    suffix_drugs = sorted(
        {
            match.group(0)
            for match in re.finditer(
                r"\b[A-Za-z][A-Za-z0-9-]{3,}(?:mab|nib|parib|ciclib|limab|zumab|tinib)\b",
                text,
            )
        },
        key=str.lower,
    )
    drugs = sorted(set(find_terms(text, DRUGS) + suffix_drugs), key=str.lower)
    return {
        "dois": sorted(set(m.group(0).rstrip(".,);") for m in DOI_RE.finditer(text))),
        "pmids": sorted(set(m.group(1) for m in PMID_RE.finditer(text))),
        "nct_ids": sorted(set(m.group(0).upper() for m in NCT_RE.finditer(text))),
        "genes": find_terms(text, GENES),
        "drugs": drugs[:20],
        "diseases": find_terms(text, DISEASES),
        "companies": find_terms(text, COMPANIES),
        "phase_mentions": sorted(set(m.group(0) for m in PHASE_RE.finditer(text))),
        "approval_cues": find_terms(text, ["FDA", "approval", "approved", "label", "CRL"]),
        "trial_cues": find_terms(text, ["trial", "study", "readout", "endpoint", "overall survival"]),
    }


def fetch_extract(session: requests.Session, url: str) -> tuple[str, dict[str, Any]]:
    fetch, html = fetch_url(session, url)
    text = ""
    if html:
        text = trafilatura.extract(
            html,
            url=fetch.get("final_url") or url,
            include_comments=False,
            include_tables=False,
            favor_precision=True,
        ) or ""
    if len(text) < 500 and html:
        text = simple_html_text(html)
    signals = paywall_signals(html[:20000], text[:4000])
    return text, {"fetch": fetch, "text_chars": len(text), "paywall_signals": signals}


def profile_score(title: str, text: str, entities: dict[str, Any]) -> tuple[int, list[str]]:
    combined = f"{title}\n{text[:4000]}"
    score = 0
    reasons = []
    for term, weight in PROFILE_TERMS.items():
        if re.search(rf"(?<![A-Za-z0-9]){re.escape(term)}(?![A-Za-z0-9])", combined, re.I):
            score += weight
            reasons.append(term)
    if entities["genes"]:
        score += 2
    if entities["drugs"]:
        score += 1
    if entities["nct_ids"] or entities["trial_cues"]:
        score += 1
    return score, reasons


def run_biomcp(args: list[str], timeout: int = 25) -> dict[str, Any]:
    command = ["biomcp", "--json", *args]
    try:
        proc = subprocess.run(command, capture_output=True, text=True, timeout=timeout, check=False)
    except Exception as exc:
        return {"command": command, "ok": False, "error": f"{exc.__class__.__name__}: {exc}"}
    parsed: Any = None
    if proc.stdout.strip():
        try:
            parsed = json.loads(proc.stdout)
        except json.JSONDecodeError:
            parsed = None
    summary = {
        "command": command,
        "ok": proc.returncode == 0,
        "returncode": proc.returncode,
        "stderr": proc.stderr.strip()[-500:],
    }
    if isinstance(parsed, dict):
        if "count" in parsed:
            summary["count"] = parsed.get("count")
        if "name" in parsed:
            summary["name"] = parsed.get("name")
        if "symbol" in parsed:
            summary["symbol"] = parsed.get("symbol")
        if "results" in parsed and isinstance(parsed["results"], list) and parsed["results"]:
            first = parsed["results"][0]
            summary["first_result"] = {
                key: first.get(key)
                for key in ("id", "nct_id", "title", "name", "source")
                if isinstance(first, dict) and first.get(key) is not None
            }
    return summary


def choose_pivots(article_results: list[dict[str, Any]]) -> list[dict[str, Any]]:
    pivots = []
    seen: set[tuple[str, str]] = set()
    for article in article_results:
        entities = article["entities"]
        for drug in entities.get("drugs", []):
            key = ("drug", drug.lower())
            if key not in seen:
                seen.add(key)
                pivots.append(
                    {
                        "article_url": article["url"],
                        "article_title": article["title"],
                        "type": "drug",
                        "value": drug,
                        "result": run_biomcp(["get", "drug", drug]),
                    }
                )
                break
        for gene in entities.get("genes", []):
            key = ("gene", gene.upper())
            if key not in seen:
                seen.add(key)
                pivots.append(
                    {
                        "article_url": article["url"],
                        "article_title": article["title"],
                        "type": "gene",
                        "value": gene,
                        "result": run_biomcp(["get", "gene", gene]),
                    }
                )
                break
        for nct_id in entities.get("nct_ids", []):
            key = ("trial", nct_id)
            if key not in seen:
                seen.add(key)
                pivots.append(
                    {
                        "article_url": article["url"],
                        "article_title": article["title"],
                        "type": "trial",
                        "value": nct_id,
                        "result": run_biomcp(["get", "trial", nct_id]),
                    }
                )
                break
        if len(pivots) >= 6:
            break

    if not any(pivot["type"] == "trial_search" for pivot in pivots):
        drug_counts = Counter(
            drug.lower()
            for article in article_results
            for drug in article["entities"].get("drugs", [])
            if len(drug) > 3
        )
        if drug_counts:
            drug = drug_counts.most_common(1)[0][0]
            pivots.append(
                {
                    "type": "trial_search",
                    "value": drug,
                    "result": run_biomcp(["search", "trial", "-i", drug, "--limit", "1"]),
                }
            )
    return pivots[:6]


def run(extraction_path: str, output: str, max_articles: int) -> None:
    extraction = json.loads(Path(extraction_path).read_text(encoding="utf-8"))
    candidates = [
        article
        for article in extraction.get("articles", [])
        if article.get("fetch", {}).get("status") and article.get("url")
    ]
    candidates.sort(
        key=lambda article: (
            article.get("has_useful_text", False),
            article.get("quality_score_0_5", 0),
            article.get("trafilatura_text_chars", 0),
        ),
        reverse=True,
    )

    session = requests.Session()
    article_results = []
    for article in candidates[:max_articles]:
        text, fetch_meta = fetch_extract(session, article["url"])
        combined = f"{article.get('title') or ''}\n{text}"
        entities = extract_entities(combined)
        score, reasons = profile_score(article.get("title") or "", text, entities)
        article_results.append(
            {
                "source_id": article.get("source_id"),
                "source_name": article.get("source_name"),
                "title": article.get("title"),
                "url": article.get("url"),
                "fetch_extract": fetch_meta,
                "entities": entities,
                "profile_score": score,
                "profile_reasons": reasons,
                "precision_assessment": {
                    "ids": "high precision regex" if entities["dois"] or entities["pmids"] or entities["nct_ids"] else "no ids found",
                    "genes": "dictionary exact-match; possible false positives for short symbols",
                    "drugs": "dictionary plus suffix heuristic; suffix heuristic needs curation",
                    "companies": "small dictionary; high precision but low recall",
                },
            }
        )

    briefing = sorted(article_results, key=lambda item: item["profile_score"], reverse=True)[:5]
    pivots = choose_pivots(article_results)
    successful_pivots = [pivot for pivot in pivots if pivot.get("result", {}).get("ok")]

    json_dump(
        output,
        {
            "generated_at": utc_now_iso(),
            "approach": "heuristic_entities_biomcp_pivots_keyword_profile_briefing",
            "input_extraction_results": extraction_path,
            "profile": {
                "role": "oncologist",
                "interests": ["immunotherapy", "KRAS", "melanoma"],
            },
            "summary": {
                "articles_analyzed": len(article_results),
                "articles_with_any_entity": sum(1 for a in article_results if any(a["entities"].values())),
                "successful_pivots": len(successful_pivots),
                "pivot_attempts": len(pivots),
            },
            "articles": article_results,
            "cross_reference_pivots": pivots,
            "personalized_briefing": [
                {
                    "rank": idx,
                    "source_id": article["source_id"],
                    "title": article["title"],
                    "url": article["url"],
                    "profile_score": article["profile_score"],
                    "reasons": article["profile_reasons"],
                    "entities": {
                        "genes": article["entities"]["genes"],
                        "drugs": article["entities"]["drugs"][:8],
                        "diseases": article["entities"]["diseases"],
                        "companies": article["entities"]["companies"],
                        "nct_ids": article["entities"]["nct_ids"],
                    },
                }
                for idx, article in enumerate(briefing, start=1)
            ],
        },
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--extraction",
        default=(
            "architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/"
            "results/article_extraction_results.json"
        ),
    )
    parser.add_argument(
        "--output",
        default=(
            "architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/"
            "results/entity_briefing_results.json"
        ),
    )
    parser.add_argument("--max-articles", type=int, default=10)
    args = parser.parse_args()
    run(args.extraction, args.output, args.max_articles)


if __name__ == "__main__":
    main()
