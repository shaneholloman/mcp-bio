from __future__ import annotations

from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
PUBLIC_DOCS = [
    ROOT / "README.md",
    ROOT / "docs" / "index.md",
    ROOT / "src" / "cli" / "list_reference.md",
]


def _read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def test_public_docs_label_search_only_entities() -> None:
    for path in PUBLIC_DOCS:
        text = _read(path)
        assert "Search-Only Entities" in text or "Search-only entities" in text, path
        assert "gwas" in text and "search gwas" in text, path
        assert "phenotype" in text and "search phenotype" in text, path


def test_public_docs_do_not_imply_get_gwas_or_get_phenotype() -> None:
    forbidden = ["get gwas", "get phenotype"]
    for path in PUBLIC_DOCS:
        text = _read(path).lower()
        for phrase in forbidden:
            assert phrase not in text, f"{path} must not document `{phrase}`"


def test_spec_architecture_routes_public_search_only_surfaces() -> None:
    text = _read(ROOT / "architecture" / "technical" / "spec-v2.md")
    assert "`gwas` is covered by `spec/entity/variant.md`" in text
    assert "CDC WONDER VAERS aggregate lane in `spec/entity/vaers.md`" in text
