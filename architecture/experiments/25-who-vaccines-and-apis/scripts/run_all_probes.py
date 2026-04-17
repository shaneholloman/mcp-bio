#!/usr/bin/env python3
from __future__ import annotations

from api_linkage_probe import run as run_api_linkage_probe
from schema_probe import run as run_schema_probe
from vaccine_identity_probe import run as run_vaccine_identity_probe
from vaccine_metadata_and_device_probe import run as run_metadata_probe
from who_vaccines_apis_lib import RESULTS_DIR, iso_now, write_json

RESULT_PATH = RESULTS_DIR / "who_vaccines_apis_summary.json"


def main() -> None:
    schema = run_schema_probe()
    vaccines = run_vaccine_identity_probe()
    apis = run_api_linkage_probe()
    metadata = run_metadata_probe()
    payload = {
        "generated_at": iso_now(),
        "results": {
            "schema": schema,
            "vaccine_identity": vaccines,
            "api_linkage": apis,
            "metadata_and_devices": metadata,
        },
    }
    write_json(RESULT_PATH, payload)


if __name__ == "__main__":
    main()
