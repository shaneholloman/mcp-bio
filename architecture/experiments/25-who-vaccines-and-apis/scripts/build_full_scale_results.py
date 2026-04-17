#!/usr/bin/env python3
from __future__ import annotations

from who_vaccines_apis import build_full_scale_results


def main() -> None:
    artifacts = build_full_scale_results()
    for key in [
        "probe_summary",
        "validation",
        "sample_records",
        "loader_design",
        "full_scale_results",
    ]:
        print(artifacts.artifact_paths[key])


if __name__ == "__main__":
    main()
