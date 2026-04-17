#!/usr/bin/env python3
from __future__ import annotations

from diagnostic_landscape import build_fda_device_probe_payload, write_json_result


def main() -> None:
    output_path = write_json_result("fda_device.json", build_fda_device_probe_payload())
    print(output_path)


if __name__ == "__main__":
    main()
