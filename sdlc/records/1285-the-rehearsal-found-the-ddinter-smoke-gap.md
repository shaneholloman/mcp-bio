---
ticket: 1285
head: (this branch)
merge: (this record's merge)
---

# 1285 — the rehearsal found the DDInter smoke gap

The first release rehearsal (run 36872650687 on genomoncology/biomcp-release-rehearsal, 2026-10-01) failed all five wheel-smoke legs because the smoke queried DDInter interactions in a clean venv without syncing the bundle first; the query path is local-dataset by design. Reproduced locally with the run's own wheel. The fix adds `ddinter sync` to wheel-smoke before the query, with the step pin updated in the same commit. This is the rehearsal's first catch: the release workflow's tag-push path had never run before, and CI cannot run it (CI and the release workflow are separate). Branch run and merge run recorded in the go request.
