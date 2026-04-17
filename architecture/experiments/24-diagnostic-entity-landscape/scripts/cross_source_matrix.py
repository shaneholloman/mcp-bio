#!/usr/bin/env python3
from __future__ import annotations

from diagnostic_landscape import (
    build_cross_source_matrix_payload,
    load_json_result,
    write_json_result,
)


def main() -> None:
    payload = build_cross_source_matrix_payload(
        load_json_result("gtr_bulk.json"),
        load_json_result("who_ivd.json"),
        load_json_result("fda_device.json"),
        load_json_result("gtr_api.json"),
    )
    output_path = write_json_result("cross_source_matrix.json", payload)
    print(output_path)


if __name__ == "__main__":
    main()
