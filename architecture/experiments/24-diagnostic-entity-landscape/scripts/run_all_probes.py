#!/usr/bin/env python3
from __future__ import annotations

from diagnostic_landscape import (
    build_cross_source_matrix_payload,
    build_fda_device_probe_payload,
    build_gtr_api_probe_payload,
    build_gtr_bulk_probe_payload,
    build_who_ivd_probe_payload,
    write_json_result,
)


def main() -> None:
    gtr_bulk = build_gtr_bulk_probe_payload()
    print(write_json_result("gtr_bulk.json", gtr_bulk))

    gtr_api = build_gtr_api_probe_payload()
    print(write_json_result("gtr_api.json", gtr_api))

    who_ivd = build_who_ivd_probe_payload()
    print(write_json_result("who_ivd.json", who_ivd))

    fda_device = build_fda_device_probe_payload()
    print(write_json_result("fda_device.json", fda_device))

    cross_source = build_cross_source_matrix_payload(
        gtr_bulk,
        who_ivd,
        fda_device,
        gtr_api,
    )
    print(write_json_result("cross_source_matrix.json", cross_source))


if __name__ == "__main__":
    main()
