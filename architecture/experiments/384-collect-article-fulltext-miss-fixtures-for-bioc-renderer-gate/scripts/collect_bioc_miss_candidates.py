#!/usr/bin/env -S uv run --script
#
# /// script
# requires-python = ">=3.12"
# ///
"""CLI wrapper for the reusable ticket 384 BioC miss/degradation collector."""

from __future__ import annotations

import sys

from bioc_miss_candidates.cli import main


if __name__ == "__main__":
    sys.exit(main())
