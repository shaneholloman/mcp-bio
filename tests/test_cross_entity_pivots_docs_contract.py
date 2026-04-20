from __future__ import annotations

from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]


def _read(path: str) -> str:
    return (REPO_ROOT / path).read_text(encoding="utf-8")


def test_cross_entity_pivot_guide_stays_in_docs_navigation() -> None:
    mkdocs = _read("mkdocs.yml")

    assert "Cross-Entity Pivots: how-to/cross-entity-pivots.md" in mkdocs


def test_cross_entity_pivot_guide_stays_linked_from_public_entry_points() -> None:
    assert "See the [cross-entity pivot guide](docs/how-to/cross-entity-pivots.md)" in _read(
        "README.md"
    )
    assert "[cross-entity pivot guide](how-to/cross-entity-pivots.md)" in _read(
        "docs/index.md"
    )
    assert "[cross-entity pivot guide](../how-to/cross-entity-pivots.md)" in _read(
        "docs/getting-started/first-query.md"
    )
    assert "[Cross-Entity Pivot Guide](../how-to/cross-entity-pivots.md)" in _read(
        "docs/reference/quick-reference.md"
    )
