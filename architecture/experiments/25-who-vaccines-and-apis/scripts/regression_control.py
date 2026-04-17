#!/usr/bin/env python3
from __future__ import annotations

from who_vaccines_apis_exploit import REGRESSION_CONTROL_PATH, build_regression_control_payload


def main() -> None:
    build_regression_control_payload()
    print(REGRESSION_CONTROL_PATH.resolve())


if __name__ == "__main__":
    main()
