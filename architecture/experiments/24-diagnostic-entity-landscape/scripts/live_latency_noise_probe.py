#!/usr/bin/env python3
from __future__ import annotations

from diagnostic_landscape import build_live_latency_noise_probe_payload, write_result


def main() -> None:
    output_path = write_result(
        "diagnostic_live_latency_noise_probe.json",
        build_live_latency_noise_probe_payload(),
    )
    print(output_path)


if __name__ == "__main__":
    main()
