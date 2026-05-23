#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# ///
"""CLI wrapper for the ticket 371 request-contract inventory library."""

from __future__ import annotations

import sys
from pathlib import Path

EXPERIMENT_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(EXPERIMENT_DIR / "lib"))

from biomcp_test_strategy import default_config, main  # noqa: E402


if __name__ == "__main__":
    print(main(default_config(Path(__file__))))
