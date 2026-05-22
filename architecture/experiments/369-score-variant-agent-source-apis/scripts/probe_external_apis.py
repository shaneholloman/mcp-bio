#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# ///
"""Small public API probes for ticket 369.

This is intentionally shallow: it measures reachability, response status,
latency, payload shape, and a few source-specific fields for the motivating
variant-agent examples. It does not implement BioMCP connectors.
"""

from __future__ import annotations

import argparse
import json
import time
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any

TIMEOUT = 20
USER_AGENT = "biomcp-spike-369/0.1 (+https://github.com/genomoncology/biomcp)"


@dataclass(frozen=True)
class Probe:
    group: str
    service: str
    label: str
    method: str
    url: str
    body: dict[str, Any] | None = None
    headers: dict[str, str] | None = None


def get_json_at(value: Any, path: list[str | int]) -> Any:
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


def summarize_payload(service: str, payload: Any) -> dict[str, Any]:
    if not isinstance(payload, (dict, list)):
        return {"payload_type": type(payload).__name__}
    summary: dict[str, Any] = {"payload_type": type(payload).__name__}
    if isinstance(payload, dict):
        summary["top_keys"] = sorted(list(payload.keys()))[:20]

    if service == "mutalyzer":
        summary.update(
            normalized_description=get_json_at(payload, ["normalized_description"]),
            protein_descriptions=get_json_at(payload, ["protein", "description"]),
            messages=get_json_at(payload, ["messages"]),
        )
    elif service == "variantvalidator":
        key = next((k for k in payload.keys() if not k.startswith("flag")), None) if isinstance(payload, dict) else None
        row = payload.get(key, {}) if key else {}
        summary.update(
            record_key=key,
            validation_warnings=get_json_at(row, ["validation_warnings"]),
            protein=get_json_at(row, ["protein"]),
            primary_assembly_loci=bool(get_json_at(row, ["primary_assembly_loci"])),
        )
    elif service == "ncbi_spdi":
        data = get_json_at(payload, ["data"])
        summary.update(data=data, warnings=get_json_at(payload, ["data", "warnings"]))
    elif service == "clingen_allele_registry":
        summary.update(
            caid=get_json_at(payload, ["@id"]),
            community_standard_title=get_json_at(payload, ["communityStandardTitle", 0]),
            external_records_count=len(get_json_at(payload, ["externalRecords"]) or []),
        )
    elif service == "myvariant":
        hit = get_json_at(payload, ["hits", 0]) or {}
        summary.update(
            total=get_json_at(payload, ["total"]),
            top_id=hit.get("_id") if isinstance(hit, dict) else None,
            top_rsid=get_json_at(hit, ["dbsnp", "rsid"]),
            has_clinvar=get_json_at(hit, ["clinvar"]) is not None,
            has_gnomad=(get_json_at(hit, ["gnomad"]) is not None or get_json_at(hit, ["gnomad_exome"]) is not None),
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
        variant = get_json_at(payload, ["data", "variant"])
        summary.update(
            has_variant=variant is not None,
            errors=get_json_at(payload, ["errors"]),
            variant_id=get_json_at(variant, ["variant_id"]),
            exome_ac=get_json_at(variant, ["exome", "ac"]),
            genome_ac=get_json_at(variant, ["genome", "ac"]),
        )
    elif service in {"crossref", "openalex", "unpaywall"}:
        summary.update(
            title=get_json_at(payload, ["message", "title", 0]) or get_json_at(payload, ["title"]),
            doi=get_json_at(payload, ["message", "DOI"]) or get_json_at(payload, ["doi"]),
            is_oa=get_json_at(payload, ["open_access", "is_oa"]) or get_json_at(payload, ["is_oa"]),
            oa_status=get_json_at(payload, ["open_access", "oa_status"]) or get_json_at(payload, ["oa_status"]),
            best_oa_location=get_json_at(payload, ["best_oa_location"]),
            landing_page=get_json_at(payload, ["message", "URL"]) or get_json_at(payload, ["primary_location", "landing_page_url"]),
        )
    elif service in {"pubmed", "europepmc", "pubtator3", "litsense2", "semantic_scholar"}:
        summary.update(
            count=get_json_at(payload, ["esearchresult", "count"])
            or get_json_at(payload, ["hitCount"])
            or get_json_at(payload, ["count"])
            or get_json_at(payload, ["total"])
            or (len(payload) if isinstance(payload, list) else None),
            first_title=get_json_at(payload, ["resultList", "result", 0, "title"])
            or get_json_at(payload, ["results", 0, "title"])
            or get_json_at(payload, ["data", 0, "title"]),
            first_pmid=get_json_at(payload, ["esearchresult", "idlist", 0])
            or get_json_at(payload, ["resultList", "result", 0, "pmid"])
            or get_json_at(payload, ["results", 0, "pmid"])
            or get_json_at(payload, [0, "pmid"]),
        )
    return summary


def request_probe(probe: Probe) -> dict[str, Any]:
    headers = {"user-agent": USER_AGENT, "accept": "application/json"}
    if probe.headers:
        headers.update(probe.headers)
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
        with urllib.request.urlopen(req, timeout=TIMEOUT) as resp:
            status = resp.status
            content_type = resp.headers.get("content-type")
            raw = resp.read(2_000_000)
    except urllib.error.HTTPError as exc:
        status = exc.code
        content_type = exc.headers.get("content-type") if exc.headers else None
        raw = exc.read(200_000)
        error = f"HTTP {exc.code}: {exc.reason}"
    except Exception as exc:  # noqa: BLE001 - this is a probe script
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
        "summary": summarize_payload(probe.service, parsed) if parsed is not None else {},
    }


def probes() -> list[Probe]:
    q = urllib.parse.quote
    return [
        Probe("normalization", "mutalyzer", "MITF transcript HGVS", "GET", f"https://mutalyzer.nl/api/normalize/{q('NM_000248.3:c.135del')}", None),
        Probe("normalization", "variantvalidator", "MITF transcript HGVS", "GET", f"https://rest.variantvalidator.org/VariantValidator/variantvalidator/GRCh38/{q('NM_000248.3:c.135del')}/all?content-type=application%2Fjson"),
        Probe("normalization", "variantvalidator", "ERBB2 transcript HGVS", "GET", f"https://rest.variantvalidator.org/VariantValidator/variantvalidator/GRCh38/{q('NM_004448.2:c.829G>T')}/all?content-type=application%2Fjson"),
        Probe("normalization", "ncbi_spdi", "MYD88 SPDI canonical", "GET", "https://api.ncbi.nlm.nih.gov/variation/v0/spdi/NC_000003.12:38182031:C:G/canonical_representative"),
        Probe("normalization", "clingen_allele_registry", "MITF transcript HGVS", "GET", f"https://reg.clinicalgenome.org/allele?hgvs={q('NM_000248.3:c.135del')}", None, {"accept": "application/json"}),
        Probe("normalization", "myvariant", "MYD88 S219C", "GET", "https://myvariant.info/v1/query?q=dbnsfp.genename:MYD88%20AND%20dbnsfp.hgvsp:%22p.S219C%22&size=1&fields=_id,dbsnp,clinvar,gnomad,gnomad_exome,dbnsfp"),
        Probe("normalization", "myvariant", "KLHL6 rs148924291", "GET", "https://myvariant.info/v1/query?q=dbsnp.rsid:rs148924291&size=1&fields=_id,dbsnp,clinvar,gnomad,gnomad_exome,dbnsfp"),
        Probe("normalization", "ensembl_vep", "MITF transcript HGVS", "GET", f"https://rest.ensembl.org/vep/human/hgvs/{q('NM_000248.3:c.135del')}?content-type=application/json"),
        Probe("population", "gnomad", "MYD88 exact allele", "POST", "https://gnomad.broadinstitute.org/api", {"query": "query Variant($variantId: String!, $dataset: DatasetId!) { variant(variantId: $variantId, dataset: $dataset) { variant_id chrom pos ref alt exome { ac an filters populations { id ac an } } genome { ac an filters populations { id ac an } } } }", "variables": {"variantId": "3-38182032-C-G", "dataset": "gnomad_r4"}}),
        Probe("literature", "pubmed", "MYD88 S219C", "GET", "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esearch.fcgi?db=pubmed&retmode=json&retmax=3&term=MYD88%20S219C"),
        Probe("literature", "europepmc", "MYD88 S219C", "GET", "https://www.ebi.ac.uk/europepmc/webservices/rest/search?query=MYD88%20S219C&format=json&pageSize=3"),
        Probe("literature", "pubtator3", "PMID 36053490 annotations", "GET", "https://www.ncbi.nlm.nih.gov/research/pubtator3-api/publications/export/biocjson?pmids=36053490"),
        Probe("literature", "litsense2", "KLHL6 L65P sentence", "GET", "https://www.ncbi.nlm.nih.gov/research/litsense2-api/api/sentences/?query=KLHL6%20L65P&rerank=true"),
        Probe("literature", "semantic_scholar", "PMID 29967253", "GET", "https://api.semanticscholar.org/graph/v1/paper/PMID:29967253?fields=paperId,externalIds,title,venue,year,isOpenAccess,openAccessPdf,citationCount"),
        Probe("article_metadata", "crossref", "ASCO DOI e24316", "GET", "https://api.crossref.org/works/10.1200/JCO.2018.36.15_suppl.e24316"),
        Probe("article_metadata", "openalex", "ASCO DOI e24316", "GET", "https://api.openalex.org/works/https://doi.org/10.1200/JCO.2018.36.15_suppl.e24316"),
        Probe("access", "unpaywall", "CCR DOI 29967253", "GET", "https://api.unpaywall.org/v2/10.1158/1078-0432.CCR-18-0991?email=team@biomcp.local"),
        Probe("access", "openalex", "CCR DOI 29967253", "GET", "https://api.openalex.org/works/https://doi.org/10.1158/1078-0432.CCR-18-0991"),
    ]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    results = [request_probe(probe) for probe in probes()]
    output = {
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "probe_count": len(results),
        "results": results,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(output, indent=2, sort_keys=True) + "\n")
    print(f"wrote {args.out} ({len(results)} probes)")


if __name__ == "__main__":
    main()
