#!/usr/bin/env python3
from __future__ import annotations

from who_vaccines_apis_lib import (
    RESULTS_DIR,
    count_presence,
    device_schema_summary,
    iso_now,
    load_devices,
    load_vaccines,
    sample_rows,
    write_json,
)

RESULT_PATH = RESULTS_DIR / "vaccine_metadata_and_device_probe.json"


def ticket_validation_samples(rows: list[dict]) -> dict:
    probes = {
        "BCG": ["bcg"],
        "measles": ["measles"],
        "HPV": ["human papillomavirus", "hpv"],
        "COVID-19": ["covid-19"],
        "yellow fever": ["yellow fever"],
    }
    results = {}
    for label, terms in probes.items():
        matches = []
        for row in rows:
            haystack = " | ".join(
                [row.get("Vaccine Type", ""), row.get("Commercial Name", "")]
            ).lower()
            if any(term in haystack for term in terms):
                matches.append(row)
        results[label] = {"count": len(matches), "samples": sample_rows(matches, 2)}
    return results


def run() -> dict:
    vaccines = load_vaccines()["rows"]
    devices = load_devices()
    device_summary = device_schema_summary(devices)

    payload = {
        "generated_at": iso_now(),
        "vaccine_row_count": len(vaccines),
        "vaccine_field_completeness": {
            "date_of_prequalification": count_presence(vaccines, "Date of Prequalification"),
            "vaccine_type": count_presence(vaccines, "Vaccine Type"),
            "commercial_name": count_presence(vaccines, "Commercial Name"),
            "presentation": count_presence(vaccines, "Presentation"),
            "no_of_doses": count_presence(vaccines, "No. of doses"),
            "manufacturer": count_presence(vaccines, "Manufacturer"),
            "responsible_nra": count_presence(vaccines, "Responsible NRA"),
        },
        "vaccine_specific_field_assessment": {
            "target_or_pathogen_proxy": {
                "available": True,
                "source_field": "Vaccine Type",
                "completeness": count_presence(vaccines, "Vaccine Type"),
            },
            "immunization_schedule": {
                "available": False,
                "source_field": None,
                "completeness": {"present": 0, "total": len(vaccines), "percent": 0.0},
            },
            "cold_chain_or_storage": {
                "available": False,
                "source_field": None,
                "completeness": {"present": 0, "total": len(vaccines), "percent": 0.0},
            },
            "dose_count": {
                "available": True,
                "source_field": "No. of doses",
                "completeness": count_presence(vaccines, "No. of doses"),
            },
        },
        "ticket_validation_samples": ticket_validation_samples(vaccines),
        "immunization_device_catalog": device_summary,
    }
    write_json(RESULT_PATH, payload)
    return payload


def main() -> None:
    run()


if __name__ == "__main__":
    main()
