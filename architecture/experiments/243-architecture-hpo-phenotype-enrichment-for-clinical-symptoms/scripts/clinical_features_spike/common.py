from __future__ import annotations

import hashlib
import json
import re
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


EXPERIMENT_DIR = Path(__file__).resolve().parents[2]
SCRIPTS_DIR = EXPERIMENT_DIR / "scripts"
RESULTS_DIR = EXPERIMENT_DIR / "results"
WORK_DIR = EXPERIMENT_DIR / "work"


def utc_now_iso() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat()


def normalize_text(value: str) -> str:
    return re.sub(r"\s+", " ", re.sub(r"[^a-z0-9]+", " ", value.lower())).strip()


def slugify(value: str) -> str:
    return re.sub(r"[^a-z0-9]+", "", value.lower())


def source_native_id(url: str) -> str:
    value = url.rstrip("/").rsplit("/", maxsplit=1)[-1]
    if "." in value:
        value = value.split(".", maxsplit=1)[0]
    return value


def stable_checksum(payload: Any) -> str:
    encoded = json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    return hashlib.sha256(encoded.encode("utf-8")).hexdigest()


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=True) + "\n",
        encoding="utf-8",
    )


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def compact_evidence(text: str, pattern: str, radius: int = 150) -> str:
    tokens = re.findall(r"[A-Za-z0-9]+", pattern)
    normalized_pattern = r"\W+".join(re.escape(token) for token in tokens)
    match = re.search(normalized_pattern, text, flags=re.IGNORECASE) if tokens else None
    if not match:
        return re.sub(r"\s+", " ", text[: radius * 2]).strip()
    start = max(0, match.start() - radius)
    end = min(len(text), match.end() + radius)
    evidence = text[start:end]
    evidence = re.sub(r"\s+", " ", evidence).strip()
    if start > 0:
        evidence = "..." + evidence
    if end < len(text):
        evidence = evidence + "..."
    return evidence
