#!/usr/bin/env python3
from __future__ import annotations

from who_vaccines_apis import VALIDATION_PATH, write_validation_result


def main() -> None:
    write_validation_result()
    print(VALIDATION_PATH.resolve())


if __name__ == "__main__":
    main()
