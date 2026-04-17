#!/usr/bin/env python3
from __future__ import annotations

from diagnostic_landscape import build_who_ivd_probe_payload, write_json_result


def main() -> None:
    output_path = write_json_result("who_ivd.json", build_who_ivd_probe_payload())
    print(output_path)


if __name__ == "__main__":
    main()
