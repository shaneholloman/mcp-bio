#!/usr/bin/env python3
from __future__ import annotations

import csv
from collections import Counter
from pathlib import Path
from typing import Any

from common import DISEASES, GENES, contains_gene_symbol, download_file, matched_diseases, pct, top_counts, write_json

WHO_IVD_URL = "https://extranet.who.int/prequal/vitro-diagnostics/prequalified/in-vitro-diagnostics/export?page&_format=csv"


def load_rows(path: Path) -> list[dict[str, str]]:
    with path.open("rt", encoding="utf-8", newline="") as handle:
        return list(csv.DictReader(handle))


def main() -> None:
    csv_path = download_file(WHO_IVD_URL, "who_ivd.csv")
    rows = load_rows(csv_path)

    assay_format_counts: Counter[str] = Counter()
    manufacturer_present = 0
    marker_present = 0
    regulatory_version_present = 0
    year_present = 0
    regulatory_present = 0

    sample_gene_matches: dict[str, list[dict[str, Any]]] = {gene: [] for gene in GENES}
    sample_disease_matches: dict[str, list[dict[str, Any]]] = {disease: [] for disease in DISEASES}
    gene_match_seen: dict[str, set[str]] = {gene: set() for gene in GENES}
    disease_match_seen: dict[str, set[str]] = {disease: set() for disease in DISEASES}

    for row in rows:
        product_name = (row.get("Product name") or "").strip()
        product_code = (row.get("Product Code") or "").strip()
        marker = (row.get("Pathogen/Disease/Marker") or "").strip()
        manufacturer = (row.get("Manufacturer name") or "").strip()
        assay_format = (row.get("Assay Format") or "").strip()
        regulatory_version = (row.get("Regulatory Version") or "").strip()
        prequalification_year = (row.get("Year prequalification") or "").strip()
        search_text = " ".join(part for part in [product_name, marker] if part)

        manufacturer_present += int(bool(manufacturer))
        marker_present += int(bool(marker))
        regulatory_version_present += int(bool(regulatory_version))
        year_present += int(bool(prequalification_year))
        regulatory_present += int(bool(regulatory_version and prequalification_year))
        if assay_format:
            assay_format_counts[assay_format] += 1

        for gene in GENES:
            if contains_gene_symbol(search_text, gene) and product_code not in gene_match_seen[gene]:
                gene_match_seen[gene].add(product_code)
                sample_gene_matches[gene].append(
                    {
                        "product_code": product_code,
                        "name": product_name,
                        "manufacturer": manufacturer,
                        "marker": marker,
                        "regulatory_version": regulatory_version,
                        "prequalification_year": prequalification_year,
                    }
                )

        for disease in matched_diseases(search_text):
            if product_code not in disease_match_seen[disease]:
                disease_match_seen[disease].add(product_code)
                sample_disease_matches[disease].append(
                    {
                        "product_code": product_code,
                        "name": product_name,
                        "manufacturer": manufacturer,
                        "marker": marker,
                        "regulatory_version": regulatory_version,
                        "prequalification_year": prequalification_year,
                    }
                )

    total = len(rows)
    sample_name_index: set[str] = set()
    for payload in list(sample_gene_matches.values()) + list(sample_disease_matches.values()):
        for example in payload:
            name = example["name"].strip().lower()
            if name:
                sample_name_index.add(name)

    payload = {
        "approach": "WHO IVD CSV parse",
        "source": "who_ivd",
        "file": str(csv_path),
        "record_counts": {
            "rows": total,
        },
        "schema_completeness": {
            "manufacturer_pct": pct(manufacturer_present, total),
            "pathogen_disease_marker_pct": pct(marker_present, total),
            "regulatory_version_pct": pct(regulatory_version_present, total),
            "prequalification_year_pct": pct(year_present, total),
            "regulatory_metadata_pct": pct(regulatory_present, total),
        },
        "assay_formats": top_counts(dict(assay_format_counts)),
        "sample_gene_matches": {
            gene: {
                "count": len(sample_gene_matches[gene]),
                "examples": sample_gene_matches[gene][:10],
            }
            for gene in GENES
        },
        "sample_disease_matches": {
            disease: {
                "count": len(sample_disease_matches[disease]),
                "examples": sample_disease_matches[disease][:10],
            }
            for disease in DISEASES
        },
        "sample_name_index": sorted(sample_name_index),
        "success_signals": {
            "all_sample_genes_have_hits": all(sample_gene_matches[gene] for gene in GENES),
            "all_sample_diseases_have_hits": all(sample_disease_matches[disease] for disease in DISEASES),
        },
    }
    output_path = write_json("who_ivd.json", payload)
    print(output_path)


if __name__ == "__main__":
    main()
