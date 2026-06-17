# Coverage baseline — per-source parity bars

Method: filtered `cargo llvm-cov nextest -E 'test(/sources::<src>::/)'` against the OLD
suite on `main@43d6fb4e`. Captured **per-source** because a whole-suite run does not
finish — live-network-leak tests hang >15 min (see `TEST-REBUILD.md` §1). Per
decision #4, a source's old tests may be deleted only once its new Tier 1–3 tests meet
or beat the numbers below.

| Source | Old tests | Region % | Function % | Line % | lines (total / missed) |
|---|---|---|---|---|---|
| `sources/mygene.rs`  | 12 | 87.67% | 85.94% | **88.93%** | 533 / 59 |
| `sources/nci_cts.rs` |  4 | 84.49% | 61.54% | **88.46%** | 338 / 39 |

Pilot run: 16 tests, all pass, **0.158 s** total (slowest 0.152 s) — vs. the old
whole-suite tests hanging >15 min each.

Captured 2026-06-16. Raw report: `coverage/pilot-baseline.log`.

---

## ⚠ Measurement correction (2026-06-16, from the mygene pilot)

The file-level line% in the table above is **inflated**: the old suite's tests live
**inline** in the source file, and test code always executes, so those lines count as
"covered" and pad the denominator. When tests move to separate `tests/` files, the
production-only file shows its *true* (lower) %, so a naive %-vs-% check falsely reads as
a regression.

**Fix — judge parity on production code only**, two ways:
- Exclude test files (`--ignore-filename-regex '/tests/'`); the new tier files are
  separate, so the source-file row is already production-only.
- Gate on **uncovered production lines not increasing** (denominator-robust), not %.

mygene production-only (`mygene.rs` = 263 exec lines after tests moved out):
- old uncovered ≈ **59** lines (file showed 88.93% only because ~270 inline-test lines padded it)
- new **pure** Tier 2+3: 95 uncovered (63.88%) — async glue not exercised by pure tests
- new **pure + live** Tier 4: **32 uncovered (87.83%)** → fewer than old 59 ⇒ **no coverage lost (improved)**
- (~11 of the 32 are dead `new_for_test`/`endpoint`, being removed → ~21)

## Fan-out log (one row per source as it is converted)
<!-- src | uncovered: old -> new(pure+live) | parity? | old deleted? | issues -->
- **mygene** — uncovered 59 → **23** (90.94%; pure-only 95) — ✅ improved — old tests deleted — issues: none
- **nci_cts** — uncovered 39 → **10** (91.94%) — ✅ improved — old tests deleted; 12 entity-level nci tests (behavior oracle) stay green — issues: none
- **myvariant** — uncovered 178 → **4** (99.14%, pure+live; pure-only 49) — ✅ improved — sub-agent converted; 174 variant consumer tests green; clippy clean — issues: none

## Final aggregate coverage check (2026-06-17)

After the full source conversion and mock-server/env-lock removal, ran:

```bash
cargo llvm-cov nextest --summary-only
```

Result: nextest 2332/2332 passed with 28 skipped. Aggregate line coverage is
**71.32%** (93,150 executable lines / 26,715 missed lines). The command exited
0. It printed `warning: 5 functions have mismatched data`; no test failed, and
this was treated as an llvm-cov reporting warning rather than a gate failure.

This aggregate check is not a per-source parity table, but it closes the final
bookkeeping gap: the converted suite still executes cleanly under coverage
instrumentation after all old routine mock/network tests were removed.
