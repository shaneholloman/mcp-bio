"""Synthesize ticket-369 feasibility matrix from probes + repo review.

Scores are 1-5 where 5 is better/lower-risk for BioMCP. This is a spike
artifact generator, not production code.
"""

from __future__ import annotations

import json
import statistics
from pathlib import Path
from typing import Any

CRITERIA = [
    "auth_terms_openness",
    "official_stable_machine_api",
    "accepted_input_forms",
    "failure_status_semantics",
    "provenance_quality",
    "rate_limit_operational_safety",
    "biomcp_fit",
    "clinical_legal_safety",
    "maintenance_safety",
]

CANDIDATES: list[dict[str, Any]] = [
    {
        "source": "Mutalyzer",
        "area": "normalization",
        "current_biomcp": "absent",
        "classification": "good BioMCP proxy candidate",
        "scores": [5, 5, 4, 4, 4, 4, 5, 5, 4],
        "accepted_inputs": ["transcript HGVS"],
        "notes": "Best first slice for transcript HGVS validation/protein consequence. MITF probe returned normalized cDNA and p.(Asn46ThrfsTer4).",
    },
    {
        "source": "VariantValidator",
        "area": "normalization",
        "current_biomcp": "absent",
        "classification": "good BioMCP proxy candidate",
        "scores": [5, 5, 5, 5, 5, 4, 5, 5, 4],
        "accepted_inputs": ["transcript HGVS", "genomic mapping", "build-aware validation"],
        "notes": "Excellent for transcript-version warnings and genomic loci; ERBB2 and MITF probes returned explicit TranscriptVersionWarning.",
    },
    {
        "source": "NCBI Variation/SPDI",
        "area": "normalization",
        "current_biomcp": "absent",
        "classification": "possible but gated",
        "scores": [5, 4, 3, 5, 4, 4, 4, 5, 4],
        "accepted_inputs": ["SPDI", "sequence-location alleles"],
        "notes": "Useful exact-allele primitive, but requires agent or upstream service to supply sequence accession/position/ref/alt. Probe surfaced explicit reference mismatch warning.",
    },
    {
        "source": "ClinGen Allele Registry",
        "area": "normalization",
        "current_biomcp": "absent for alleles; gene ClinGen exists",
        "classification": "possible but gated",
        "scores": [5, 4, 4, 3, 5, 4, 4, 5, 3],
        "accepted_inputs": ["HGVS", "CAid/equivalent allele expressions"],
        "notes": "Strong identity/provenance value, but probe returned blank-node @id rather than a stable CAid for MITF transcript HGVS; needs more endpoint/terms review.",
    },
    {
        "source": "MyVariant.info",
        "area": "variant annotation/normalization fallback",
        "current_biomcp": "core get/search variant source",
        "classification": "good BioMCP proxy candidate",
        "scores": [5, 5, 4, 4, 4, 5, 5, 4, 5],
        "accepted_inputs": ["rsID", "genomic HGVS", "Lucene fields", "gene/protein via dbNSFP"],
        "notes": "Already integrated; best to deepen explicit annotate/provenance/status rather than duplicate. Good fallback for MYD88 and KLHL6; provenance must preserve upstream source licensing.",
    },
    {
        "source": "Ensembl VEP REST",
        "area": "annotation/normalization",
        "current_biomcp": "absent",
        "classification": "possible but gated",
        "scores": [5, 5, 3, 4, 5, 4, 3, 5, 4],
        "accepted_inputs": ["genomic HGVS", "variant consequence when coordinate known"],
        "notes": "Valuable when genomic coordinate is known; MITF RefSeq transcript HGVS probe failed with clear 400, so not the first transcript-normalization slice.",
    },
    {
        "source": "gnomAD direct API",
        "area": "population",
        "current_biomcp": "gene constraint only; variant population via MyVariant",
        "classification": "possible but gated",
        "scores": [4, 4, 3, 4, 5, 3, 5, 4, 3],
        "accepted_inputs": ["gnomAD variant_id coordinate allele", "dataset version"],
        "notes": "High value as exact allele population proxy, but GraphQL contract/versioning and coordinate normalization must be pinned. Probe returned explicit Variant not found for guessed MYD88 allele, proving need for normalization first.",
    },
    {
        "source": "MyVariant.info gnomAD fallback",
        "area": "population",
        "current_biomcp": "variant population section",
        "classification": "good BioMCP proxy candidate",
        "scores": [5, 5, 4, 4, 3, 5, 5, 4, 5],
        "accepted_inputs": ["rsID", "genomic HGVS", "MyVariant hit fields"],
        "notes": "Already ships useful cached gnomAD fields; should remain labeled fallback/annotation, not absence proof.",
    },
    {
        "source": "PubMed/PMC/NCBI E-utilities",
        "area": "literature/access",
        "current_biomcp": "PubMed search/get; EFetch/ID Converter/PMC OA fulltext rungs",
        "classification": "good BioMCP proxy candidate",
        "scores": [5, 5, 4, 4, 5, 4, 5, 5, 5],
        "accepted_inputs": ["PMID", "PMCID", "DOI bridge", "keyword search"],
        "notes": "Already strong. Add access-status wrapper rather than new fulltext machinery.",
    },
    {
        "source": "Europe PMC",
        "area": "literature/access",
        "current_biomcp": "article search/get/fulltext metadata",
        "classification": "good BioMCP proxy candidate",
        "scores": [5, 5, 5, 4, 5, 5, 5, 5, 5],
        "accepted_inputs": ["PMID", "PMCID", "DOI", "keyword", "OA/fulltext filters"],
        "notes": "Already integrated and good for metadata/open flags/fulltext XML; extend status fields if needed.",
    },
    {
        "source": "PubTator3",
        "area": "literature/text-mining",
        "current_biomcp": "article search and annotations",
        "classification": "good BioMCP proxy candidate",
        "scores": [5, 5, 4, 4, 5, 4, 5, 5, 5],
        "accepted_inputs": ["PMID annotations", "entity search/autocomplete", "text search"],
        "notes": "Already integrated; expose as service-provenanced annotations/matches, not interpretation.",
    },
    {
        "source": "LitSense2",
        "area": "literature/text-mining",
        "current_biomcp": "keyword-gated article search",
        "classification": "good BioMCP proxy candidate",
        "scores": [5, 4, 4, 4, 4, 4, 5, 5, 4],
        "accepted_inputs": ["keyword/sentence search"],
        "notes": "Excellent service-provided snippet/match proxy; KLHL6 L65P probe returned PMID 29695787 among 100 sentence hits.",
    },
    {
        "source": "Semantic Scholar",
        "area": "literature/graph/access metadata",
        "current_biomcp": "optional article search enrichment, TLDR, citations, references, recommendations, PDF metadata",
        "classification": "possible but gated",
        "scores": [3, 5, 4, 4, 4, 3, 4, 4, 4],
        "accepted_inputs": ["PMID", "DOI", "paperId", "keyword"],
        "notes": "Already useful but terms/rate limits make it optional. Keep source-status and token guidance; don't make it required for access decisions.",
    },
    {
        "source": "Crossref",
        "area": "DOI/conference metadata",
        "current_biomcp": "absent",
        "classification": "good BioMCP proxy candidate",
        "scores": [5, 5, 5, 4, 5, 5, 5, 5, 5],
        "accepted_inputs": ["DOI", "title search"],
        "notes": "ASCO DOI probe succeeded and found title/publisher URL where current BioMCP article get failed. Strong first-slice DOI metadata proxy.",
    },
    {
        "source": "OpenAlex",
        "area": "DOI/OA/citation metadata",
        "current_biomcp": "absent",
        "classification": "good BioMCP proxy candidate",
        "scores": [5, 5, 5, 4, 5, 5, 5, 5, 4],
        "accepted_inputs": ["DOI", "title search", "OA status", "citation graph metadata"],
        "notes": "ASCO and CCR DOI probes succeeded with OA status closed and landing pages. Good complement to Crossref/Unpaywall.",
    },
    {
        "source": "Unpaywall",
        "area": "access status",
        "current_biomcp": "absent",
        "classification": "good BioMCP proxy candidate",
        "scores": [4, 5, 4, 4, 5, 5, 5, 5, 4],
        "accepted_inputs": ["DOI with registered email"],
        "notes": "CCR DOI probe returned closed status and null best OA location. Strong access-status component; requires product email/contact policy.",
    },
    {
        "source": "CIViC",
        "area": "curated variant evidence",
        "current_biomcp": "direct GraphQL sections for variant/gene/drug/disease plus MyVariant cached fields",
        "classification": "good BioMCP proxy candidate",
        "scores": [5, 5, 4, 4, 5, 4, 5, 4, 4],
        "accepted_inputs": ["molecular profile", "therapy", "disease"],
        "notes": "Already integrated and open. Keep opt-in and source-provenanced; agent owns interpretation/evidence weighting.",
    },
    {
        "source": "ClinVar",
        "area": "curated variant evidence",
        "current_biomcp": "indirect via MyVariant",
        "classification": "possible but gated",
        "scores": [5, 4, 4, 4, 5, 4, 4, 5, 3],
        "accepted_inputs": ["Variation ID", "VCV/RCV", "HGVS/rsID via NCBI services"],
        "notes": "Public and clinically important, but direct NCBI ClinVar integration is a separate source surface. Current indirect path is useful; direct ClinVar can wait behind normalization/access work.",
    },
    {
        "source": "cBioPortal",
        "area": "curated/cohort variant context",
        "current_biomcp": "variant gene-level mutation summary and study surfaces",
        "classification": "possible but gated",
        "scores": [4, 5, 3, 4, 4, 3, 4, 3, 3],
        "accepted_inputs": ["gene", "study/profile/sample list", "mutation data"],
        "notes": "Already present, but variant-specific frequency by alteration is more study-dependent and terms-sensitive. Keep supplemental, not classification-moving.",
    },
    {
        "source": "OncoKB",
        "area": "curated oncogenic/actionability",
        "current_biomcp": "explicit token-gated variant helper",
        "classification": "possible but gated",
        "scores": [2, 5, 4, 4, 5, 3, 3, 2, 3],
        "accepted_inputs": ["gene + protein alteration with token"],
        "notes": "Keep explicit and token-gated. Lead-only metadata mode may be acceptable, but proprietary interpretation/actionability summaries are terms-sensitive.",
    },
    {
        "source": "COSMIC",
        "area": "curated somatic variant evidence",
        "current_biomcp": "indirect MyVariant payload only",
        "classification": "reject/default-exclude",
        "scores": [1, 2, 3, 2, 3, 2, 1, 1, 1],
        "accepted_inputs": ["indirect COSMIC IDs only when aggregator returns them"],
        "notes": "Direct integration remains excluded by planning docs and license risk. Do not add a connector; preserve indirect-only caution.",
    },
]

BOUNDARY = [
    {"request": "messy report/entity cleanup", "owner": "agent-owned", "rationale": "BioMCP should not parse clinical prose or guess aliases."},
    {"request": "regex/pattern validation", "owner": "BioMCP input contract", "rationale": "Use only to validate supported command/service input shapes and return guardrail messages."},
    {"request": "biological normalization/equivalence", "owner": "BioMCP only through upstream services", "rationale": "Proxy Mutalyzer/VariantValidator/SPDI/ClinGen; preserve input and service output separately."},
    {"request": "source lookup/provenance/status", "owner": "BioMCP", "rationale": "Core read-only service-proxy job."},
    {"request": "local PDF/Word/Markdown search", "owner": "agent-owned", "rationale": "Use local tools after BioMCP retrieves allowed metadata/fulltext."},
    {"request": "classification/oncogenicity/actionability", "owner": "agent-owned", "rationale": "BioMCP can return source records, not interpret them clinically."},
]

FOLLOW_UPS = [
    {"order": 1, "title": "Add service capability discovery for source/input/status metadata", "first_slice": "`biomcp list services --json` plus selected `service <name> capabilities --json` for existing article/variant sources."},
    {"order": 2, "title": "Add public variant normalization proxies", "first_slice": "Mutalyzer + VariantValidator for transcript HGVS with typed success/warning/error statuses; evaluate SPDI/ClinGen as second slice."},
    {"order": 3, "title": "Add DOI/conference metadata and access-status proxies", "first_slice": "Crossref + OpenAlex DOI lookup and Unpaywall/OpenAlex access status for known DOI inputs."},
    {"order": 4, "title": "Add direct exact-allele population probe behind normalized coordinate input", "first_slice": "gnomAD exact variant ID after normalization; MyVariant remains labeled fallback."},
]


def load_probe_index(path: Path) -> dict[str, list[dict[str, Any]]]:
    if not path.exists():
        return {}
    data = json.loads(path.read_text())
    out: dict[str, list[dict[str, Any]]] = {}
    for row in data.get("results", []):
        out.setdefault(row.get("service") or row.get("group") or "unknown", []).append(row)
    return out


def enrich(candidate: dict[str, Any], external: dict[str, list[dict[str, Any]]]) -> dict[str, Any]:
    scores = dict(zip(CRITERIA, candidate.pop("scores"), strict=True))
    candidate["scores"] = scores
    candidate["overall_score"] = round(statistics.mean(scores.values()), 2)
    key = candidate["source"].lower().replace(" ", "_").replace(".", "").replace("/", "_")
    aliases = {
        "mutalyzer": ["mutalyzer"],
        "variantvalidator": ["variantvalidator"],
        "ncbi_variation_spdi": ["ncbi_spdi"],
        "clingen_allele_registry": ["clingen_allele_registry"],
        "myvariantinfo": ["myvariant"],
        "ensembl_vep_rest": ["ensembl_vep"],
        "gnomad_direct_api": ["gnomad"],
        "myvariantinfo_gnomad_fallback": ["myvariant"],
        "pubmed_pmc_ncbi_e-utilities": ["pubmed"],
        "europe_pmc": ["europepmc"],
        "pubtator3": ["pubtator3"],
        "litsense2": ["litsense2"],
        "semantic_scholar": ["semantic_scholar"],
        "crossref": ["crossref"],
        "openalex": ["openalex"],
        "unpaywall": ["unpaywall"],
    }.get(key, [])
    rows = [row for alias in aliases for row in external.get(alias, [])]
    if rows:
        candidate["probe_summary"] = {
            "count": len(rows),
            "ok": sum(1 for row in rows if row.get("ok")),
            "statuses": sorted({row.get("status") for row in rows}),
            "median_elapsed_ms": round(statistics.median(row.get("elapsed_ms", 0) for row in rows), 2),
            "labels": [row.get("label") for row in rows],
        }
    return candidate


BOUNDARY_CLASSIFICATIONS = BOUNDARY
FOLLOW_UP_RECOMMENDATIONS = FOLLOW_UPS


def build_feasibility_matrix(external_probe_report_or_index: dict[str, Any]) -> dict[str, Any]:
    """Build the ticket-369 source feasibility matrix.

    Accepts either a full probe report with a `results` list, or a pre-indexed
    mapping of service/group key to probe rows as returned by `load_probe_index`.
    """

    if "results" in external_probe_report_or_index:
        external: dict[str, list[dict[str, Any]]] = {}
        for row in external_probe_report_or_index.get("results", []):
            external.setdefault(row.get("service") or row.get("group") or "unknown", []).append(row)
    else:
        external = external_probe_report_or_index  # type: ignore[assignment]
    candidates = [enrich(dict(row), external) for row in CANDIDATES]
    return {
        "criteria_scale": "1-5; 5 is best/lower risk for BioMCP",
        "criteria": CRITERIA,
        "candidates": candidates,
        "boundary_classification": BOUNDARY_CLASSIFICATIONS,
        "recommended_follow_up_tickets": FOLLOW_UP_RECOMMENDATIONS,
    }


def write_feasibility_matrix(path: Path, external_path: Path) -> dict[str, Any]:
    """Read external probe output, build the matrix, and write it as JSON."""

    output = build_feasibility_matrix(load_probe_index(external_path))
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(output, indent=2, sort_keys=True) + "\n")
    return output
