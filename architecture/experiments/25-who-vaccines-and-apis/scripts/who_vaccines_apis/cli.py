#!/usr/bin/env python3
from __future__ import annotations

from .probes import (
    run_probe_suite,
    write_api_linkage_probe_result,
    write_metadata_probe_result,
    write_schema_probe_result,
    write_vaccine_identity_probe_result,
)
from .reports import (
    REGRESSION_CONTROL_PATH,
    VALIDATION_PATH,
    build_full_scale_results,
    write_regression_control_result,
    write_validation_result,
)


def schema_probe_main() -> None:
    write_schema_probe_result()


def vaccine_identity_probe_main() -> None:
    write_vaccine_identity_probe_result()


def api_linkage_probe_main() -> None:
    write_api_linkage_probe_result()


def metadata_probe_main() -> None:
    write_metadata_probe_result()


def run_all_probes_main() -> None:
    run_probe_suite()


def build_full_scale_results_main() -> None:
    artifacts = build_full_scale_results()
    for key in [
        "probe_summary",
        "validation",
        "sample_records",
        "loader_design",
        "full_scale_results",
    ]:
        print(artifacts.artifact_paths[key])


def regression_control_main() -> None:
    write_regression_control_result()
    print(REGRESSION_CONTROL_PATH.resolve())


def validate_main() -> None:
    write_validation_result()
    print(VALIDATION_PATH.resolve())
