#!/usr/bin/env python3
from __future__ import annotations

from who_vaccines_apis import write_vaccine_identity_probe_result


def run() -> dict:
    return write_vaccine_identity_probe_result()


def main() -> None:
    run()


if __name__ == "__main__":
    main()
