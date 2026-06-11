from __future__ import annotations

from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[2]
URL_RE = re.compile(r"`(https?://[^`]+)`")


def _read(relative: str) -> str:
    return (ROOT / relative).read_text(encoding="utf-8")


def test_source_versioning_covers_data_source_urls() -> None:
    data_sources = _read("docs/reference/data-sources.md")
    source_versioning = _read("docs/reference/source-versioning.md")
    urls = sorted(set(URL_RE.findall(data_sources)))
    missing = [url for url in urls if url not in source_versioning]
    assert not missing, "source-versioning.md missing documented URLs: " + ", ".join(missing)


def test_configuration_reference_classifies_env_var_families() -> None:
    config = _read("docs/reference/configuration.md")
    for heading in [
        "## Operator API Keys",
        "## Operator Data and Cache Knobs",
        "## Test and Fixture Override Seams",
        "## Release and Install Variables",
        "## Observability and Degradation",
    ]:
        assert heading in config

    for env_var in [
        "ALPHAGENOME_API_KEY",
        "DISGENET_API_KEY",
        "NCBI_API_KEY",
        "NCI_API_KEY",
        "ONCOKB_TOKEN",
        "OPENFDA_API_KEY",
        "S2_API_KEY",
        "UMLS_API_KEY",
        "BIOMCP_CACHE_DIR",
        "BIOMCP_STUDY_DIR",
        "BIOMCP_BIN",
    ]:
        assert f"`{env_var}`" in config


def test_observability_policy_names_public_status_surfaces() -> None:
    config = _read("docs/reference/configuration.md")
    assert "stderr" in config
    assert "`_meta.source_status`" in config
    assert "biomcp health --apis-only" in config
    assert "SourceUnavailable" in config
