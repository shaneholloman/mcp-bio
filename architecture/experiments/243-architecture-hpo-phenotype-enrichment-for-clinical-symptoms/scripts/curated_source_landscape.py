#!/usr/bin/env python3
from __future__ import annotations

import os
import urllib.parse
from typing import Any

from phenotype_spike_common import (
    DISEASES,
    RESULTS_DIR,
    ensure_results_dir,
    http_json,
    main_guard,
    utc_now_iso,
    write_json,
)


SOURCE_METADATA: dict[str, dict[str, Any]] = {
    "current_hpo_monarch": {
        "quality_tier": "curated HPO/Monarch",
        "api_availability": "available without BioMCP-specific key",
        "refresh_cadence": "follows upstream HPO annotation/Monarch KG releases; BioMCP resolves live API responses",
        "source_url": "https://monarchinitiative.org/",
        "integration_note": "Already integrated in `get disease <id> phenotypes`.",
    },
    "opentargets_hpo": {
        "quality_tier": "curated/aggregated HPO evidence",
        "api_availability": "public GraphQL",
        "refresh_cadence": "versioned Open Targets Platform releases",
        "source_url": "https://platform.opentargets.org/api",
        "integration_note": "Client already queries disease phenotypes for prevalence context, not phenotype rows.",
    },
    "medgen": {
        "quality_tier": "curated NCBI concept aggregation",
        "api_availability": "public NCBI E-utilities",
        "refresh_cadence": "NCBI-managed Entrez database updates",
        "source_url": "https://www.ncbi.nlm.nih.gov/medgen/",
        "integration_note": "Useful for disease resolution/xrefs; symptom lists are not exposed as a simple phenotype table.",
    },
    "orphanet": {
        "quality_tier": "curated rare-disease clinical/HPO annotations",
        "api_availability": "public ORDO via OLS and Orphadata downloads; direct downloads may require terms acceptance",
        "refresh_cadence": "Orphadata product releases with dated XML/diff files",
        "source_url": "https://www.orphadata.com/",
        "integration_note": "Strong for rare diseases, weak fit for common diseases in this sample.",
    },
    "omim": {
        "quality_tier": "curated clinical synopsis",
        "api_availability": "API key and OMIM terms required",
        "refresh_cadence": "OMIM is updated daily",
        "source_url": "https://www.omim.org/api",
        "integration_note": "Potentially high value for Mendelian diseases; clinical synopsis licensing is the main blocker.",
    },
    "disease_ontology": {
        "quality_tier": "curated disease classification",
        "api_availability": "public OLS API",
        "refresh_cadence": "monthly Disease Ontology data releases; OBO Foundry also updates daily from GitHub",
        "source_url": "https://disease-ontology.org/",
        "integration_note": "Good disease identity/xrefs; does not carry symptom annotations as a core relation.",
    },
    "snomed_ct": {
        "quality_tier": "clinical terminology, licensed",
        "api_availability": "browser APIs exist, but production use requires SNOMED CT licensing/edition management",
        "refresh_cadence": "SNOMED CT International Edition monthly releases",
        "source_url": "https://www.snomed.org/snomed-ct",
        "integration_note": "Finding-site/morphology relations are useful for anatomy/pathology, not direct symptom lists.",
    },
}


def first_query(disease: dict[str, Any]) -> str:
    return disease["source_queries"][0]


def ols_search(ontology: str, query: str) -> dict[str, Any]:
    params = urllib.parse.urlencode(
        {
            "q": query,
            "ontology": ontology,
            "rows": "5",
            "queryFields": "label,synonym",
            "fieldList": "iri,label,short_form,ontology_name,description",
        }
    )
    url = f"https://www.ebi.ac.uk/ols4/api/search?{params}"
    response = http_json(url)
    docs = (
        response.get("json", {})
        .get("response", {})
        .get("docs", [])
        if response.get("ok")
        else []
    )
    return {
        "ok": response["ok"],
        "status": response["status"],
        "elapsed_ms": response["elapsed_ms"],
        "hit_count": len(docs),
        "hits": [
            {
                "label": doc.get("label"),
                "short_form": doc.get("short_form"),
                "ontology_name": doc.get("ontology_name"),
                "iri": doc.get("iri"),
            }
            for doc in docs[:3]
        ],
        "error": response.get("error"),
    }


def medgen_search(query: str) -> dict[str, Any]:
    params = urllib.parse.urlencode(
        {
            "db": "medgen",
            "term": query,
            "retmode": "json",
            "retmax": "5",
        }
    )
    search = http_json(f"https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esearch.fcgi?{params}")
    ids = search.get("json", {}).get("esearchresult", {}).get("idlist", []) if search.get("ok") else []
    summaries: list[dict[str, Any]] = []
    if ids:
        summary_params = urllib.parse.urlencode(
            {
                "db": "medgen",
                "id": ",".join(ids[:5]),
                "retmode": "json",
            }
        )
        summary = http_json(
            f"https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esummary.fcgi?{summary_params}"
        )
        result = summary.get("json", {}).get("result", {}) if summary.get("ok") else {}
        for medgen_id in ids[:5]:
            row = result.get(medgen_id)
            if isinstance(row, dict):
                summaries.append(
                    {
                        "uid": medgen_id,
                        "title": row.get("title"),
                        "concept_id": row.get("conceptid"),
                        "definition": row.get("definition"),
                        "semantic_type": row.get("semantictype"),
                    }
                )
    return {
        "ok": search["ok"],
        "status": search["status"],
        "elapsed_ms": search["elapsed_ms"],
        "hit_count": len(ids),
        "hits": summaries,
        "error": search.get("error"),
    }


def http_graphql(body: dict[str, Any]) -> dict[str, Any]:
    import json
    import time
    import urllib.request
    from phenotype_spike_common import HTTP_TIMEOUT_SECONDS, USER_AGENT

    req = urllib.request.Request(
        "https://api.platform.opentargets.org/api/v4/graphql",
        data=json.dumps(body).encode("utf-8"),
        headers={
            "Accept": "application/json",
            "Content-Type": "application/json",
            "User-Agent": USER_AGENT,
        },
        method="POST",
    )
    started = time.perf_counter()
    try:
        with urllib.request.urlopen(req, timeout=HTTP_TIMEOUT_SECONDS) as resp:
            return {
                "ok": True,
                "status": resp.status,
                "elapsed_ms": round((time.perf_counter() - started) * 1000, 1),
                "json": json.loads(resp.read().decode("utf-8")),
            }
    except Exception as exc:  # noqa: BLE001 - probe records failures.
        return {
            "ok": False,
            "status": None,
            "elapsed_ms": round((time.perf_counter() - started) * 1000, 1),
            "error": f"{type(exc).__name__}: {exc}",
        }


def opentargets_phenotypes(query: str) -> dict[str, Any]:
    search_body = {
        "query": """
query SearchDisease($query: String!) {
  search(queryString: $query, entityNames: ["disease"], page: {index: 0, size: 3}) {
    hits { id name entity }
  }
}
""",
        "variables": {"query": query},
    }
    search = http_graphql(search_body)
    hits = (
        search.get("json", {})
        .get("data", {})
        .get("search", {})
        .get("hits", [])
        if search.get("ok")
        else []
    )
    if not hits:
        return {
            "ok": search["ok"],
            "status": search["status"],
            "elapsed_ms": search["elapsed_ms"],
            "resolved": None,
            "phenotype_count": 0,
            "phenotypes": [],
            "error": search.get("error"),
        }
    resolved = hits[0]
    phenotype_body = {
        "query": """
query DiseasePhenotypes($efoId: String!) {
  disease(efoId: $efoId) {
    id
    name
    phenotypes(page: {index: 0, size: 20}) {
      rows {
        phenotypeHPO { id name }
        evidence {
          frequency
          frequencyHPO { id name }
          resource
          evidenceType
          sex
          onset { id name }
        }
      }
    }
  }
}
""",
        "variables": {"efoId": resolved.get("id")},
    }
    response = http_graphql(phenotype_body)
    rows = (
        response.get("json", {})
        .get("data", {})
        .get("disease", {})
        .get("phenotypes", {})
        .get("rows", [])
        if response.get("ok")
        else []
    )
    phenotypes: list[dict[str, Any]] = []
    for row in rows:
        hpo = row.get("phenotypeHPO") or {}
        evidence = row.get("evidence") or []
        phenotypes.append(
            {
                "id": hpo.get("id"),
                "name": hpo.get("name"),
                "evidence_count": len(evidence),
                "resources": sorted(
                    {
                        ev.get("resource")
                        for ev in evidence
                        if isinstance(ev, dict) and ev.get("resource")
                    }
                ),
            }
        )
    return {
        "ok": response["ok"],
        "status": response["status"],
        "elapsed_ms": round(search["elapsed_ms"] + response["elapsed_ms"], 1),
        "resolved": {
            "id": resolved.get("id"),
            "name": resolved.get("name"),
        },
        "phenotype_count": len(phenotypes),
        "phenotypes": phenotypes,
        "error": response.get("error"),
    }


def omim_probe(query: str) -> dict[str, Any]:
    api_key = os.environ.get("OMIM_API_KEY")
    if not api_key:
        return {
            "ok": False,
            "status": None,
            "credential_required": True,
            "hit_count": None,
            "clinical_synopsis_available": "not measured without OMIM_API_KEY",
        }
    params = urllib.parse.urlencode(
        {
            "search": query,
            "include": "clinicalSynopsis",
            "format": "json",
            "apiKey": api_key,
        }
    )
    response = http_json(f"https://api.omim.org/api/entry/search?{params}")
    entries = (
        response.get("json", {})
        .get("omim", {})
        .get("searchResponse", {})
        .get("entryList", [])
        if response.get("ok")
        else []
    )
    return {
        "ok": response["ok"],
        "status": response["status"],
        "credential_required": False,
        "hit_count": len(entries),
        "clinical_synopsis_available": any(
            bool(entry.get("entry", {}).get("clinicalSynopsis")) for entry in entries
        ),
        "error": response.get("error"),
    }


def snomed_probe(query: str) -> dict[str, Any]:
    params = urllib.parse.urlencode(
        {
            "term": query,
            "active": "true",
            "limit": "5",
        }
    )
    url = (
        "https://browser.ihtsdotools.org/snowstorm/snomed-ct/browser/MAIN/descriptions?"
        + params
    )
    response = http_json(url)
    items = response.get("json", {}).get("items", []) if response.get("ok") else []
    return {
        "ok": response["ok"],
        "status": response["status"],
        "elapsed_ms": response["elapsed_ms"],
        "hit_count": len(items),
        "hits": [
            {
                "term": item.get("term"),
                "concept_id": item.get("concept", {}).get("conceptId"),
                "fsn": item.get("concept", {}).get("fsn", {}).get("term"),
            }
            for item in items[:3]
        ],
        "error": response.get("error"),
    }


def summarize_source_for_disease(source: str, disease: dict[str, Any]) -> dict[str, Any]:
    query = first_query(disease)
    if source == "medgen":
        return medgen_search(query)
    if source == "orphanet":
        return ols_search("ordo", query)
    if source == "disease_ontology":
        return ols_search("doid", query)
    if source == "opentargets_hpo":
        return opentargets_phenotypes(query)
    if source == "omim":
        return omim_probe(query)
    if source == "snomed_ct":
        return snomed_probe(query)
    return {"ok": None, "note": "measured by baseline_biomcp_hpo.py"}


def source_coverage(rows: list[dict[str, Any]], source: str) -> dict[str, Any]:
    measured = [row["sources"][source] for row in rows]
    if source == "opentargets_hpo":
        diseases_with_data = sum(1 for row in measured if row.get("phenotype_count", 0) > 0)
    else:
        diseases_with_data = sum(1 for row in measured if row.get("hit_count", 0) and row.get("ok") is not False)
    return {
        "diseases_with_any_resolution_or_data": diseases_with_data,
        "disease_count": len(measured),
    }


def main() -> None:
    main_guard()
    ensure_results_dir()
    sources = [
        "opentargets_hpo",
        "medgen",
        "orphanet",
        "omim",
        "disease_ontology",
        "snomed_ct",
    ]
    diseases: list[dict[str, Any]] = []
    for disease in DISEASES:
        diseases.append(
            {
                "disease_key": disease["key"],
                "label": disease["label"],
                "queries": disease["source_queries"],
                "identifiers": disease.get("identifiers", {}),
                "sources": {
                    source: summarize_source_for_disease(source, disease)
                    for source in sources
                },
            }
        )

    payload = {
        "generated_at": utc_now_iso(),
        "approach": "curated_source_landscape_probe",
        "metric_definitions": {
            "hit_count": "Number of small-scale query hits returned by the source-specific resolution probe.",
            "phenotype_count": "For OpenTargets HPO only, number of disease phenotype rows returned by GraphQL.",
            "credential_required": "Whether the source could not be measured without credentials or license/API-key setup.",
        },
        "source_metadata": SOURCE_METADATA,
        "summary_by_source": {
            source: source_coverage(diseases, source) for source in sources
        },
        "diseases": diseases,
    }
    write_json(RESULTS_DIR / "curated_source_landscape.json", payload)


if __name__ == "__main__":
    main()
