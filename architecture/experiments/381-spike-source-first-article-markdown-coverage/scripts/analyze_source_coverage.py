#!/usr/bin/env -S uv run --script
#
# /// script
# requires-python = ">=3.12"
# ///
"""Compatibility CLI for ticket 381 source-coverage analysis."""

from __future__ import annotations

import sys

from source_coverage_analysis.cli import main


if __name__ == "__main__":
    sys.exit(main())
