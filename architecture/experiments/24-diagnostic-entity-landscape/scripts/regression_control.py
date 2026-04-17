#!/usr/bin/env python3
from __future__ import annotations

from diagnostic_landscape import build_regression_control_payload, write_result


def main() -> None:
    output_path = write_result(
        "diagnostic_regression_control.json",
        build_regression_control_payload(),
    )
    print(output_path)


if __name__ == "__main__":
    main()
