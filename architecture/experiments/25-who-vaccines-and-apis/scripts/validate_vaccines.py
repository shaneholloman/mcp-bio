#!/usr/bin/env python3
from __future__ import annotations

from who_vaccines_apis_exploit import VALIDATION_PATH, build_validation_payload
from who_vaccines_apis_lib import write_json


def main() -> None:
    payload = build_validation_payload()
    write_json(VALIDATION_PATH, payload)
    print(VALIDATION_PATH.resolve())


if __name__ == "__main__":
    main()
