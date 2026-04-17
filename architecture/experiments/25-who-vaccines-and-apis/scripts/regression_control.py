#!/usr/bin/env python3
from __future__ import annotations

from who_vaccines_apis import REGRESSION_CONTROL_PATH, write_regression_control_result


def main() -> None:
    write_regression_control_result()
    print(REGRESSION_CONTROL_PATH.resolve())


if __name__ == "__main__":
    main()
