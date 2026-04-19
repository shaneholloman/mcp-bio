#!/usr/bin/env python3
from __future__ import annotations

import subprocess
import sys
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
PROBES = [
    "baseline_biomcp_hpo.py",
    "curated_source_landscape.py",
    "wikidata_p780_probe.py",
    "clinical_summary_medlineplus_probe.py",
]


def main() -> None:
    for probe in PROBES:
        path = SCRIPT_DIR / probe
        print(f"running {path}", flush=True)
        subprocess.run([sys.executable, str(path)], check=True)


if __name__ == "__main__":
    main()
