#!/usr/bin/env python3
from __future__ import annotations

from who_vaccines_apis import run_probe_suite


def run() -> tuple[dict[str, dict], dict[str, float]]:
    return run_probe_suite()


def main() -> None:
    run()


if __name__ == "__main__":
    main()
