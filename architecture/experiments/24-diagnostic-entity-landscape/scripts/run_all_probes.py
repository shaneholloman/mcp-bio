#!/usr/bin/env python3
from __future__ import annotations

import subprocess
import sys
from pathlib import Path


def main() -> None:
    script_dir = Path(__file__).resolve().parent
    scripts = [
        "gtr_bulk_probe.py",
        "gtr_api_probe.py",
        "who_ivd_probe.py",
        "fda_device_probe.py",
        "cross_source_matrix.py",
    ]

    for script in scripts:
        print(f"running {script}")
        subprocess.run([sys.executable, str(script_dir / script)], check=True)


if __name__ == "__main__":
    main()
