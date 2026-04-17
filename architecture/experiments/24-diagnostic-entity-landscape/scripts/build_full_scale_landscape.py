#!/usr/bin/env python3
from __future__ import annotations

from diagnostic_landscape import build_full_scale_landscape, write_full_scale_results


def main() -> None:
    landscape = build_full_scale_landscape()
    landscape = write_full_scale_results(landscape)

    print(landscape.artifact_paths["full_scale_path"])
    print(landscape.artifact_paths["matrix_rows_path"])
    print(landscape.artifact_paths["gtr_sample_path"])
    print(landscape.artifact_paths["who_sample_path"])
    print(landscape.artifact_paths["fda_sample_path"])
    print(landscape.artifact_paths["validation_path"])


if __name__ == "__main__":
    main()
