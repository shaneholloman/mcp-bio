# 1293 — bound article search time and report partial sources

Proposed 2026-10-03 by the BioMCP owner for the next 0.9 release. Status: accepted, ready to build.

## Evidence

- Experiment 432: tool calls took about 10,400 s against 1,500 s of model time. The slowest calls were `search article "SCN5A Brugada syndrome mutations" --limit 10` (206 s), `search article -k "tumor immune evasion mechanisms" --type review --full` (186 s) and a glioblastoma `--type review` search (180 s). At a 300 s agent budget, 21 of 60 agent cells timed out.
- Experiment 433: 10 of 120 TREC topics first returned zero rows, and 7 still failed with `source_unavailable` after three tries.

## Change

1. Measure which source and which step consume the time in those commands.
2. Give article search one overall deadline with a stated default. When it passes, return the rows already found and mark the slow sources as unavailable in the output.
3. Print the deadline and per-source timing in `--full` diagnostics.

## Retained behavior

Ranking, sources and output shape stay the same when every source answers in time.

## Proof

A spec case with a recorded slow source that returns partial rows and names the slow source. The three slow commands rerun with timing in the record.

## Deferred

Changing rate-limit handling for NCBI keys.

## Review

- Design review: ACCEPT 2026-10-03 on the first pass, dispatch 26de9b8f (fresh SWE-2 researcher, read-only).

## What landed

- Measurement: the three slow commands were rerun against this branch's binary and every one now settles at the deadline with rows instead of multi-minute stalls. `search article "SCN5A Brugada syndrome mutations" --limit 10` finished in 1m0.037s (ticket: 206s; a pre-change rerun on this worktree still blew a 240s external timeout after the PubTator3 leg timed out at 12s, so the missing time sat in the unbounded enrichment loops). `search article -k "tumor immune evasion mechanisms" --type review --full` finished in 1m0.072s (was 186s). `search article -k "glioblastoma treatment resistance mechanisms" --type review --full` finished in 1m0.034s (was ~180s). All three returned 10 rows, exit 0.
- Default: 60 seconds, matching the existing 60s `VariantArticleDeadline` budget that already bounds the same provider mix for variant article search. The federated legs keep their own 12s timeout, leaving ~48s for the sequential Semantic Scholar batches and per-row PubTator/Europe PMC metadata fallback that consumed the measured time.
- Change: `search_page` installs one `VariantArticleDeadline` per invocation through the existing task-local provider seam, covering federated legs, single-backend plans (which previously had no leg timeout at all), the Semantic Scholar enrichment batches, and the per-row metadata fallback loop. On expiry the page returns the rows that already answered; sources that outlasted the deadline are reported `degraded` with a message naming the deadline ("did not answer before the article search deadline" / "article search deadline elapsed during <stage>"). A search where nothing answered fails with `SourceUnavailable`. When both primaries fail, PubMed/Semantic Scholar/LitSense2 rows now return as a partial page instead of `primary_error` discarding them. `--full` JSON carries `diagnostics.deadline_ms` plus per-source/per-stage `source_timings`; compact output is unchanged. The recorded fixtures override the budget through `BIOMCP_TEST_ARTICLE_SEARCH_DEADLINE_MS`, the same env seam `BIOMCP_TEST_UNPACED_ORIGIN` uses. Held-source waits are channel/event releases, not sleeps.
- Gates: `make lint` exit 0. `make spec` exit 0 (the new held-source article spec case ran inside the article.md batch, 178 passed). `make test` exit 2 on pre-existing load flakes outside this change: `cache::manager::tests::stale_marker_tests::*` where `get` returns `None` immediately after `put` under parallel load (files identical to main; the three tests pass standalone, pass/fail/pass on direct rerun), and in one run `cli::stale_json_note_tests::*` plus two `entities::disease::get` stale-cache tests failing `Connection refused` to their own fixture servers. All 426 article-scope nextest tests passed, including the three new deadline tests; the same archive rerun minus `test(stale)` showed no other failures.
- Code review:
