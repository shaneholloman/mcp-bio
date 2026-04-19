from __future__ import annotations

import argparse
from pathlib import Path

from .reports import write_all_results


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Build spike 243 clinical feature enrichment exploit artifacts."
    )
    parser.add_argument(
        "--offline",
        action="store_true",
        help="Use committed explore MedlinePlus result fixtures instead of live API/cache fetches.",
    )
    parser.add_argument(
        "--refresh-cache",
        action="store_true",
        help="Refresh the local MedlinePlus cache under the experiment work directory.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    paths = write_all_results(
        allow_live=not args.offline,
        refresh_cache=args.refresh_cache,
    )
    for path in paths.values():
        print(Path(path).resolve())
    return 0
