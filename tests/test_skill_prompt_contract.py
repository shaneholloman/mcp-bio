from __future__ import annotations

import re
import subprocess
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
RELEASE_BIN = REPO_ROOT / "target" / "release" / "biomcp"
EXPECTED_SLUGS = [
    "treatment-lookup",
    "symptom-phenotype",
    "gene-disease-orientation",
    "article-follow-up",
]
REMOVED_ACTIVE_SLUGS = [
    "variant-to-treatment",
    "drug-investigation",
    "gene-function-lookup",
    "trial-searching",
    "literature-synthesis",
]


def _require_release_binary() -> Path:
    assert RELEASE_BIN.exists(), f"missing release binary: {RELEASE_BIN}"
    return RELEASE_BIN


def _run_bytes(*args: str) -> bytes:
    binary = _require_release_binary()
    result = subprocess.run(
        [str(binary), *args],
        cwd=REPO_ROOT,
        check=True,
        capture_output=True,
    )
    return result.stdout


def _run_text(*args: str) -> str:
    return _run_bytes(*args).decode("utf-8")


def _listed_slugs(*args: str) -> list[str]:
    listing = _run_text(*args)
    return re.findall(r"^\d{2} ([a-z0-9-]+) -", listing, flags=re.MULTILINE)


def test_skill_prompt_render_install_and_slug_surfaces_match(tmp_path: Path) -> None:
    overview_stdout = _run_bytes("skill")
    render_stdout = _run_bytes("skill", "render")

    assert overview_stdout == render_stdout
    assert render_stdout.endswith(b"\n")
    assert not render_stdout.endswith(b"\n\n")

    prompt = render_stdout.decode("utf-8")
    for marker in (
        "## Routing rules",
        "## Section reference",
        "## Cross-entity pivot rules",
        "## How-to reference",
        "## Anti-patterns",
        "## Output and evidence rules",
        "## Answer commitment",
    ):
        assert marker in prompt
    assert "../docs/" not in prompt
    assert ".md)" not in prompt

    agent_root = tmp_path / "agent"
    _run_text("skill", "install", str(agent_root), "--force")
    installed_root = agent_root / "skills" / "biomcp"
    assert (installed_root / "SKILL.md").read_bytes() == render_stdout

    slugs = _listed_slugs("skill", "list")
    assert slugs == EXPECTED_SLUGS
    assert _listed_slugs("list", "skill") == slugs
    for slug in slugs:
        body = _run_text("skill", slug)
        assert body.strip()
        assert body.startswith("# ")

    installed_use_case_slugs = [
        path.stem[3:] for path in sorted((installed_root / "use-cases").glob("[0-9][0-9]-*.md"))
    ]
    assert installed_use_case_slugs == slugs

    examples_readme = (REPO_ROOT / "examples" / "README.md").read_text(encoding="utf-8")
    listing = _run_text("skill", "list")
    list_skill_listing = _run_text("list", "skill")
    for removed in REMOVED_ACTIVE_SLUGS:
        assert removed not in listing
        assert removed not in list_skill_listing
        assert removed not in prompt
        assert removed not in examples_readme
