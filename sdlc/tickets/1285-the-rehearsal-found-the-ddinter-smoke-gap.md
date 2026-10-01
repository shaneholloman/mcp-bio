# 1285 — the rehearsal found the DDInter smoke gap

Filed 2026-10-01 from the first release rehearsal (scratch repository `genomoncology/biomcp-release-rehearsal`, run 36872650687).

## The finding

The rehearsal's tag push (the first time the release workflow ever ran its tag-push path) failed all five wheel-smoke legs with "Source unavailable: DDInter is not available". Reproduced locally with the rehearsal's own wheel: `drug interactions apixaban` on a cold cache errors by design (the DDInter bundle is a local dataset; the query path never downloads it), `biomcp ddinter sync` succeeds, and the query works after. The smoke step called `drug interactions` in a clean venv with no sync, so the real v0.9.1 tag would have failed wheel-smoke on every platform and stopped before PyPI. The DDInter provider itself was healthy (all eight CSVs answer 200 with the biomcp user agent).

## The fix

wheel-smoke runs `require_exit 0 ddinter sync` before the interactions query, with a workflow comment naming the rehearsal and the failure mode. The step's pinned hash is updated in the same commit.

## Evidence

- Rehearsal run 36872650687: version-check, docs-live (stubbed), all five pypi-build legs, all five build legs, create-draft green; all five wheel-smoke legs failed twice (initial and the failed-job rerun) at the same line; container-publish and everything after skipped.
- Local reproduction: the run's wheel artifact, installed into a clean venv, prints the same error cold and answers the query after `ddinter sync`.

## Review

- Pending: the verdict line lands with the dispatch before the merge.
