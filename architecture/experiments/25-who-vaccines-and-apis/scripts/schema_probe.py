#!/usr/bin/env python3
from __future__ import annotations

from who_vaccines_apis_lib import (
    REQUIRED_FINISHED_HEADERS,
    RESULTS_DIR,
    device_schema_summary,
    iso_now,
    load_apis,
    load_devices,
    load_finished_pharma,
    load_vaccines,
    sample_rows,
    schema_overlap,
    schema_summary,
    write_json,
)

RESULT_PATH = RESULTS_DIR / "who_schema_comparison.json"


def run() -> dict:
    finished = load_finished_pharma()
    vaccines = load_vaccines()
    apis = load_apis()
    devices = load_devices()
    devices_schema = device_schema_summary(devices)

    payload = {
        "generated_at": iso_now(),
        "record_counts": {
            "finished_pharma": len(finished["entries"]),
            "vaccines": len(vaccines["rows"]),
            "apis": len(apis["rows"]),
            "immunization_devices": devices_schema["item_count"],
            "immunization_device_categories": devices_schema["category_count"],
        },
        "source_schemas": {
            "finished_pharma": schema_summary(finished["headers"], finished["rows"]),
            "vaccines": schema_summary(vaccines["headers"], vaccines["rows"]),
            "apis": schema_summary(apis["headers"], apis["rows"]),
            "immunization_devices": devices_schema,
        },
        "header_overlap_against_finished_pharma": {
            "vaccines": schema_overlap(finished["headers"], vaccines["headers"]),
            "apis": schema_overlap(finished["headers"], apis["headers"]),
            "immunization_devices": schema_overlap(
                REQUIRED_FINISHED_HEADERS, devices_schema["headers"]
            ),
        },
        "current_contract_direct_header_coverage": {
            "vaccines_shared_required_headers": schema_overlap(
                REQUIRED_FINISHED_HEADERS, vaccines["headers"]
            ),
            "apis_shared_required_headers": schema_overlap(REQUIRED_FINISHED_HEADERS, apis["headers"]),
            "devices_shared_required_headers": schema_overlap(
                REQUIRED_FINISHED_HEADERS, devices_schema["headers"]
            ),
        },
        "sample_rows": {
            "finished_pharma": sample_rows(finished["entries"], 2),
            "vaccines": sample_rows(vaccines["rows"], 2),
            "apis": sample_rows(apis["rows"], 2),
            "immunization_devices": sample_rows(devices["items"], 2),
        },
    }
    write_json(RESULT_PATH, payload)
    return payload


def main() -> None:
    run()


if __name__ == "__main__":
    main()
