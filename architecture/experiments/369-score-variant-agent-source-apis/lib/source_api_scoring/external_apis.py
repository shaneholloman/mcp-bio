"""Reusable public HTTP API probes for ticket 369."""

from __future__ import annotations

import json
import os
import time
import urllib.error
import urllib.parse
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from typing import Any

from .types import HttpProbe

TIMEOUT = 20
USER_AGENT = "biomcp-spike-369/0.1 (+https://github.com/genomoncology/biomcp)"


def json_at(value: Any, path: list[str | int]) -> Any:
    """Return a nested JSON value, or None if any path component is absent."""

    cur = value
    for part in path:
        if isinstance(part, int):
            if not isinstance(cur, list) or part >= len(cur):
                return None
            cur = cur[part]
        else:
            if not isinstance(cur, dict):
                return None
            cur = cur.get(part)
        if cur is None:
            return None
    return cur


def summarize_http_payload(service: str, payload: Any) -> dict[str, Any]:
    """Extract a stable, source-specific summary from a public API payload."""

    if not isinstance(payload, (dict, list)):
        return {"payload_type": type(payload).__name__}
    summary: dict[str, Any] = {"payload_type": type(payload).__name__}
    if isinstance(payload, dict):
        summary["top_keys"] = sorted(list(payload.keys()))[:20]

    if service == "mutalyzer":
        summary.update(
            normalized_description=json_at(payload, ["normalized_description"]),
            protein_descriptions=json_at(payload, ["protein", "description"]),
            messages=json_at(payload, ["messages"]),
        )
    elif service == "variantvalidator":
        key = next((k for k in payload.keys() if not k.startswith("flag")), None) if isinstance(payload, dict) else None
        row = payload.get(key, {}) if key else {}
        summary.update(
            record_key=key,
            validation_warnings=json_at(row, ["validation_warnings"]),
            protein=json_at(row, ["protein"]),
            primary_assembly_loci=bool(json_at(row, ["primary_assembly_loci"])),
        )
    elif service == "ncbi_spdi":
        data = json_at(payload, ["data"])
        summary.update(data=data, warnings=json_at(payload, ["data", "warnings"]))
    elif service == "clingen_allele_registry":
        summary.update(
            caid=json_at(payload, ["@id"]),
            community_standard_title=json_at(payload, ["communityStandardTitle", 0]),
            external_records_count=len(json_at(payload, ["externalRecords"]) or []),
        )
    elif service == "myvariant":
        hit = json_at(payload, ["hits", 0]) or {}
        summary.update(
            total=json_at(payload, ["total"]),
            top_id=hit.get("_id") if isinstance(hit, dict) else None,
            top_rsid=json_at(hit, ["dbsnp", "rsid"]),
            has_clinvar=json_at(hit, ["clinvar"]) is not None,
            has_gnomad=(json_at(hit, ["gnomad"]) is not None or json_at(hit, ["gnomad_exome"]) is not None),
        )
    elif service == "ensembl_vep":
        if isinstance(payload, list) and payload:
            summary.update(
                result_count=len(payload),
                first_keys=sorted(list(payload[0].keys()))[:20],
                most_severe_consequence=payload[0].get("most_severe_consequence"),
            )
        elif isinstance(payload, dict):
            summary.update(error=payload.get("error"))
    elif service == "gnomad":
        variant = json_at(payload, ["data", "variant"])
        summary.update(
            has_variant=variant is not None,
            errors=json_at(payload, ["errors"]),
            variant_id=json_at(variant, ["variant_id"]),
            exome_ac=json_at(variant, ["exome", "ac"]),
            genome_ac=json_at(variant, ["genome", "ac"]),
        )
    elif service in {"crossref", "openalex", "unpaywall"}:
        summary.update(
            title=json_at(payload, ["message", "title", 0]) or json_at(payload, ["title"]),
            doi=json_at(payload, ["message", "DOI"]) or json_at(payload, ["doi"]),
            is_oa=json_at(payload, ["open_access", "is_oa"]) or json_at(payload, ["is_oa"]),
            oa_status=json_at(payload, ["open_access", "oa_status"]) or json_at(payload, ["oa_status"]),
            best_oa_location=json_at(payload, ["best_oa_location"]),
            landing_page=json_at(payload, ["message", "URL"]) or json_at(payload, ["primary_location", "landing_page_url"]),
        )
    elif service in {"pubmed", "europepmc", "pubtator3", "litsense2", "semantic_scholar"}:
        summary.update(
            count=json_at(payload, ["esearchresult", "count"])
            or json_at(payload, ["hitCount"])
            or json_at(payload, ["count"])
            or json_at(payload, ["total"])
            or (len(payload) if isinstance(payload, list) else None),
            first_title=json_at(payload, ["resultList", "result", 0, "title"])
            or json_at(payload, ["results", 0, "title"])
            or json_at(payload, ["data", 0, "title"]),
            first_pmid=json_at(payload, ["esearchresult", "idlist", 0])
            or json_at(payload, ["resultList", "result", 0, "pmid"])
            or json_at(payload, ["results", 0, "pmid"])
            or json_at(payload, [0, "pmid"]),
        )
    return summary


def request_http_probe(probe: HttpProbe, *, timeout: int = TIMEOUT) -> dict[str, Any]:
    """Run one public HTTP API probe."""

    headers = {"user-agent": USER_AGENT, "accept": "application/json"}
    if probe.headers:
        headers.update(probe.headers)
    if probe.service == "semantic_scholar" and (api_key := os.environ.get("S2_API_KEY")):
        headers["x-api-key"] = api_key
    data = None
    if probe.body is not None:
        data = json.dumps(probe.body).encode("utf-8")
        headers.setdefault("content-type", "application/json")
    req = urllib.request.Request(probe.url, data=data, method=probe.method, headers=headers)
    started = time.perf_counter()
    raw = b""
    status = None
    content_type = None
    error = None
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            status = resp.status
            content_type = resp.headers.get("content-type")
            raw = resp.read(2_000_000)
    except urllib.error.HTTPError as exc:
        status = exc.code
        content_type = exc.headers.get("content-type") if exc.headers else None
        raw = exc.read(200_000)
        error = f"HTTP {exc.code}: {exc.reason}"
    except Exception as exc:  # noqa: BLE001 - this is a probe helper
        error = type(exc).__name__ + ": " + str(exc)
    elapsed_ms = round((time.perf_counter() - started) * 1000, 2)

    parsed: Any = None
    parse_error = None
    if raw:
        try:
            parsed = json.loads(raw.decode("utf-8"))
        except Exception as exc:  # noqa: BLE001
            parse_error = type(exc).__name__ + ": " + str(exc)

    return {
        "group": probe.group,
        "service": probe.service,
        "label": probe.label,
        "method": probe.method,
        "url": probe.url,
        "status": status,
        "ok": bool(status and 200 <= status < 300 and error is None),
        "elapsed_ms": elapsed_ms,
        "bytes_read": len(raw),
        "content_type": content_type,
        "error": error,
        "parse_error": parse_error,
        "summary": summarize_http_payload(probe.service, parsed) if parsed is not None else {},
    }


def default_http_probes() -> list[HttpProbe]:
    """Return the ticket-369 regression-control public HTTP probes."""

    q = urllib.parse.quote
    return [
        HttpProbe("normalization", "mutalyzer", "MITF transcript HGVS", "GET", f"https://mutalyzer.nl/api/normalize/{q('NM_000248.3:c.135del')}", None),
        HttpProbe("normalization", "variantvalidator", "MITF transcript HGVS", "GET", f"https://rest.variantvalidator.org/VariantValidator/variantvalidator/GRCh38/{q('NM_000248.3:c.135del')}/all?content-type=application%2Fjson"),
        HttpProbe("normalization", "variantvalidator", "ERBB2 transcript HGVS", "GET", f"https://rest.variantvalidator.org/VariantValidator/variantvalidator/GRCh38/{q('NM_004448.2:c.829G>T')}/all?content-type=application%2Fjson"),
        HttpProbe("normalization", "ncbi_spdi", "MYD88 SPDI canonical", "GET", "https://api.ncbi.nlm.nih.gov/variation/v0/spdi/NC_000003.12:38182031:C:G/canonical_representative"),
        HttpProbe("normalization", "clingen_allele_registry", "MITF transcript HGVS", "GET", f"https://reg.clinicalgenome.org/allele?hgvs={q('NM_000248.3:c.135del')}", None, {"accept": "application/json"}),
        HttpProbe("normalization", "myvariant", "MYD88 S219C", "GET", "https://myvariant.info/v1/query?q=dbnsfp.genename:MYD88%20AND%20dbnsfp.hgvsp:%22p.S219C%22&size=1&fields=_id,dbsnp,clinvar,gnomad,gnomad_exome,dbnsfp"),
        HttpProbe("normalization", "myvariant", "KLHL6 rs148924291", "GET", "https://myvariant.info/v1/query?q=dbsnp.rsid:rs148924291&size=1&fields=_id,dbsnp,clinvar,gnomad,gnomad_exome,dbnsfp"),
        HttpProbe("normalization", "ensembl_vep", "MITF transcript HGVS", "GET", f"https://rest.ensembl.org/vep/human/hgvs/{q('NM_000248.3:c.135del')}?content-type=application/json"),
        HttpProbe("population", "gnomad", "MYD88 exact allele", "POST", "https://gnomad.broadinstitute.org/api", {"query": "query Variant($variantId: String!, $dataset: DatasetId!) { variant(variantId: $variantId, dataset: $dataset) { variant_id chrom pos ref alt exome { ac an filters populations { id ac an } } genome { ac an filters populations { id ac an } } } }", "variables": {"variantId": "3-38182032-C-G", "dataset": "gnomad_r4"}}),
        HttpProbe("literature", "pubmed", "MYD88 S219C", "GET", "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esearch.fcgi?db=pubmed&retmode=json&retmax=3&term=MYD88%20S219C"),
        HttpProbe("literature", "europepmc", "MYD88 S219C", "GET", "https://www.ebi.ac.uk/europepmc/webservices/rest/search?query=MYD88%20S219C&format=json&pageSize=3"),
        HttpProbe("literature", "pubtator3", "PMID 36053490 annotations", "GET", "https://www.ncbi.nlm.nih.gov/research/pubtator3-api/publications/export/biocjson?pmids=36053490"),
        HttpProbe("literature", "litsense2", "KLHL6 L65P sentence", "GET", "https://www.ncbi.nlm.nih.gov/research/litsense2-api/api/sentences/?query=KLHL6%20L65P&rerank=true"),
        HttpProbe("literature", "semantic_scholar", "PMID 29967253", "GET", "https://api.semanticscholar.org/graph/v1/paper/PMID:29967253?fields=paperId,externalIds,title,venue,year,isOpenAccess,openAccessPdf,citationCount"),
        HttpProbe("article_metadata", "crossref", "ASCO DOI e24316", "GET", "https://api.crossref.org/works/10.1200/JCO.2018.36.15_suppl.e24316"),
        HttpProbe("article_metadata", "openalex", "ASCO DOI e24316", "GET", "https://api.openalex.org/works/https://doi.org/10.1200/JCO.2018.36.15_suppl.e24316"),
        HttpProbe("access", "unpaywall", "CCR DOI 29967253", "GET", "https://api.unpaywall.org/v2/10.1158/1078-0432.CCR-18-0991?email=team@biomcp.local"),
        HttpProbe("access", "openalex", "CCR DOI 29967253", "GET", "https://api.openalex.org/works/https://doi.org/10.1158/1078-0432.CCR-18-0991"),
    ]


def run_external_api_suite(
    probes: list[HttpProbe] | None = None,
    *,
    max_workers: int = 6,
    timeout: int = TIMEOUT,
) -> dict[str, Any]:
    """Run the public HTTP probe suite and return the JSON report."""

    probe_list = probes or default_http_probes()
    with ThreadPoolExecutor(max_workers=max_workers) as executor:
        results = list(executor.map(lambda probe: request_http_probe(probe, timeout=timeout), probe_list))
    return {
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "probe_count": len(results),
        "results": results,
    }


def write_external_api_report(
    path: Path,
    probes: list[HttpProbe] | None = None,
    *,
    max_workers: int = 6,
    timeout: int = TIMEOUT,
) -> dict[str, Any]:
    """Run and write the public HTTP probe report."""

    output = run_external_api_suite(probes, max_workers=max_workers, timeout=timeout)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(output, indent=2, sort_keys=True) + "\n")
    return output
