"""Constants for ticket 381 source-coverage analysis."""

from __future__ import annotations

SOURCE_FAMILIES: dict[str, str] = {
    "europe_pmc_core_search": "europe_pmc_core_metadata",
    "current_europe_pmc_fullTextXML_by_pmcid": "current_europe_pmc_fulltextxml_pmcid",
    "current_europe_pmc_fullTextXML_by_pmid": "current_europe_pmc_fulltextxml_pmid",
    "ncbi_bioc_pmc_by_pmcid": "ncbi_bioc_pmcid",
    "ncbi_bioc_pmc_by_pmid": "ncbi_bioc_pmid",
    "pubtator3_biocjson_by_pmcid": "pubtator3_pmcid",
    "pubtator3_biocjson_by_pmid": "pubtator3_pmid",
    "pmc_oa_manifest": "pmc_oa_manifest",
}

QUALITY_KEYS = [
    "has_title",
    "has_abstract",
    "section_count",
    "paragraph_count",
    "table_count",
    "reference_count",
    "has_fulltext_signal",
    "has_tables",
    "has_references",
    "has_entity_annotations",
    "has_tgz",
]

COUNT_KEYS = [
    "abstract_chars",
    "section_count",
    "section_title_count",
    "paragraph_count",
    "table_count",
    "reference_count",
    "document_count",
    "passage_count",
    "section_type_count",
    "text_chars",
    "annotation_count",
]
