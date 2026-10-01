# 1286 — pin the x86_64 macOS wheel smoke to an Intel host

Filed 2026-10-01 from the second release rehearsal (scratch run 36885506628).

## The finding

The rehearsal's wheel-smoke (macos-latest, wheel-x86_64-apple-darwin) leg failed at install: "biomcp_cli-0.9.1-py3-none-macosx_10_12_x86_64.whl is not a supported wheel on this platform." The job's runner image log shows `Image: macos-26-arm64` — GitHub moved the macos-latest label to ARM64 between the first rehearsal run (14:30 UTC, same leg installed fine on an x86_64 host) and the second (16:08 UTC, arm64 host, pip refuses the foreign-arch tag). The first run's five legs and the second run's other four legs pass with the DDInter fix (ticket 1285). The build and pypi-build legs are unaffected in output correctness: they cross-compile with an explicit cargo `--target`, and the refused install proves the wheel carries genuine x86_64 tags. Only the smoke assumes the host matches the wheel.

## The fix

The x86_64-apple-darwin wheel-smoke leg runs on `macos-15-intel` (the documented Intel image; macos-latest and macos-15 are arm64 now, and pinning macos-15 alone does not select Intel). The matrix pin in the provenance test is updated in the same commit. When Intel runners retire, the leg moves to Rosetta on an arm64 image; that migration is future work, not part of this fix.

## Evidence

- Run 36885506628: the failing leg's log carries "Image: macos-26-arm64" and the pip refusal above; the four other wheel-smoke legs passed with ticket 1285's sync line in.
- Runner tables: GitHub's runner reference lists macos-15-intel (and macos-26-intel) as Intel, macos-latest and macos-15 as arm64.

## Review

- Pending: the verdict line lands with the dispatch before the merge.
