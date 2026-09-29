# Review follow-ups after tickets 1263 and 1264

Filed 2026-09-28 from an independent read-only review of main at `7d2d85a7`. Three fresh reviewers covered the release fixes in `4d05cfd8` and `064cac91`, GitHub #287, ticket 1263, ticket 1264, the licensing evidence table, the records round and the README source count. Reviewers ran local dry runs and mutation probes in scratch copies, read-only GitHub queries and public web fetches only. The release fixes hold. Nothing here blocks the 0.9.1 tag. The items below are claims that went further than the code, and gaps left open.

## Ian's direction, 2026-09-28

- Resolve every issue before tagging 0.9.1. That covers every item in this file, every item in `2026-09-28-review-follow-ups-after-1255-through-1261.md`, and each point the reporter raised in GitHub #287.
- Resolved means fixed and proven, or given a recorded decision with a reason. A deferral needs Ian's OK.
- Ticket 1260 and the other items already assigned to 1.0 stay with 1.0.
- Before asking for the go, add a disposition section to each review file. Each line names the commit and the CI run that proves it. Main must stay green across the whole round.

## Release

- The changelog gate now reports 32 missing tickets at `7d2d85a7`, not 30. Tickets 1263 and 1264 landed after the count was taken. The missing list is 1220, 1222 through 1224, 1226 through 1239, 1241 through 1244, 1247, 1248, 1250, 1254, 1255, 1257, 1259, 1261, 1263 and 1264.
- The record pattern at `scripts/check-changelog-coverage.py:16` still reads `2026-q3-review.md` and `2026-sept-review.md` as ticket 2026. It now drops real ticket records whose slug starts with a digit, such as `1265-3-sources-...`, records with an uppercase slug, and five-digit ticket numbers. The regression test checks the pattern alone and not `record_tickets()`. Its fake `git` setup at lines 302-304 does nothing.
- The Homebrew tap updates before `publish-release` makes the GitHub release public (`.github/workflows/release.yml:491`, `:669`). The formula points at tarballs on a draft release until that last job succeeds. If it fails, `brew install` fails until someone reruns it. Make the tap wait for `publish-release`, or make the release public before the tap runs.
- The wheel smoke comment says the asset checks "name real content". Only `skill list` and `--json get article` check output. `skill article-follow-up` and `chart bar` check the exit code only (`release.yml:394-395`). Assert a known skill name and a known chart heading.
- No test names `--release` in the wheel builds. The only guard is the step hash pin (`tests/test_release_workflow_provenance.py:284-285`), and a repin clears it. Add a direct assertion that every wheel build passes `--release --locked`.

## GitHub #287

- The reporter raised four points beyond the build profile. None is answered or filed:
  - `skill`, `skill list` and `chart bar` printed an error and exited 0 on 0.9.0. A missing skill exits 1 on current main. `skill list` printing "No skills found" with exit 0 has not been checked.
  - Enabling the `debug-embed` feature of `rust_embed` would make debug and release builds embed the same assets. Record a decision.
  - Nobody checked whether the 0.8.x wheels carry the same defect. The published wheel sizes on PyPI answer it.
  - The issue is still open with no reply.
- Record each point with a decision in this repo before the #287 reply goes out.

## Stale-cache note (1263)

- The GWAS exclusion is false. `src/sources/gwas.rs:76` marks requests no-store, and `apply_cache_mode` at `gwas.rs:101` replaces that mark with force-cache under `BIOMCP_CACHE_MODE=infinite` (`src/sources/mod.rs:667-668`). A stale GWAS reply is then served, and GWAS JSON has no notes field since `64df4d72`. Skip `apply_cache_mode` for GWAS, or restore the notes field.
- The ClinGen prefetch carries the note list into its spawned task (`src/entities/gene.rs:2523-2527`). It does not carry the per-task `NO_CACHE` flag (`mod.rs:531`). `get gene X --no-cache` still reads and writes the cache for ClinGen.
- The tests do not parse `_meta.notes`:
  - The article test looks for the sentence anywhere in stdout (`src/cli/stale_json_note_tests.rs:131`).
  - The search-all test checks two separate substrings and overrides only EuropePMC, so other sections likely reach live services.
  - ClinGen has no end-to-end test. The file header at line 3 claims GWAS and ClinGen coverage.
- The TLS handshake retry and the global-counter probe test (`src/cli/outcome/probe_tests.rs:79`) from the prior review have no fix and no disposition.

## Checks (1264)

Every bypass the prior review listed is now caught. These neighboring spellings pass:

- Workflow contract:
  - An expression job env such as `env: ${{ fromJSON('{"BASH_ENV":"/tmp/p"}') }}`.
  - `BASH_FUNC_python%%`, `PATH` or `ENV` in a workflow env block.
  - A new first step in `pypi-publish` that writes `BASH_ENV` to `$GITHUB_ENV`. Ticket 1258 says BASH_ENV is banned everywhere. `pypi-publish` is the job that uploads to PyPI, so pin its step list next.
- Review grammar:
  - `Code review: not done`, `not yet passed`, `skipped` and `n/a`.
  - `REJECT, never fixed` and `REJECT, to be folded`.
  - `Security review: awaiting`.
  - A table row `| Code review | pending |`.
- Stdio guard:
  - A second `.stderr(Stdio::inherit())` after a null one.
  - Setters inside a comment or string literal.
  - `<Command>::new(x)` and `let mk = Command::new; mk(x)`.
  - An inherit set through a reborrow.
  - A `cfg` whose feature name contains "test", which the guard strips as test code.
- Wait ratchet:
  - `use std::thread::{sleep as nap}`.
  - `import os, time as t`.
  - `let e = start.elapsed(); if e < d`.
  - `Instant::now().duration_since(s) < d`.
  - `watchdog(` text on an `Instant::now() +` line, even in a comment.
  - Python `while time.monotonic() < deadline` polls, which are never counted.
- The marker ceiling went from 25 to 40, then 41, then 42 during the 1264 gate cycles. The gencc line pin went from 1008 to 1016. Every raise after the code review was folded went unreviewed. The waits the new patterns found were marked, not converted to signals. `src/sources/gencc.rs` is pinned at 7 unmarked waits and has 4. `--update` repins everything in one command. Require a reviewed reason for each ceiling raise, and set the gencc pin to 4.
- 1264's Deferred gaps section omits the expression-env hole and the bypasses above.

## Stdio test

- The 2-second stderr read at `tests/tls_ca_bundle_contract.rs:687-714` takes the pipe. If the server takes longer than 2 seconds to exit, the timeout discards the text read so far, and the later read finds no pipe. The warning count is then 0 and line 719 fails under load. Read stderr once, after `child.wait()`.

## Licensing and records

- The pass record still lists PharmGKB as unverifiable with the old dates 2026-12-15 and 2027-03-20 (`sdlc/records/2026-09-27-source-licensing-review-pass.md:51-60`). Lines 83-89 still say 44 entries and four unverifiable. Only lines 76-79 carry the correction.
- `sdlc/issues/2026-09-27-review-follow-ups-after-the-1238-close.md:119-125` still says "44 verified unchanged", "four unverifiable" and "the 2027-03 crossing handled by the pass itself". Line 89 still cites gate `5754b910`, which exists only on `origin/tickets/1256-output`.
- Two of the three unverifiable calls are wrong:
  - WikiPathways' about page links to https://www.wikipathways.org/terms.html, which is live and adopts CC0.
  - CIViC states CC0 in plain HTML at https://docs.civicdb.org/en/latest/about/faq.html.
  - Only Enrichr looks blocked.
- Stale addresses remain:
  - `docs/reference/sources.json:1543` and `:1546`, and `source-licensing.md:105`, `:744` and `:746`, point WikiPathways at the retired classic site.
  - `source-licensing.md:92` and `:958` give PharmGKB's terms address as api.pharmgkb.org, which the evidence calls unresolvable.
  - `sources.json:357` says current CPIC URLs continue to resolve, which contradicts the redirect finding.
- The owner for the unverifiable sources is "the biomcp queue". Name the role that runs the retry.
- Sixteen spot-checked evidence rows matched their live pages. The table reads as real work.
- The 2026-09-26 disposition (`sdlc/issues/2026-09-26-review-follow-ups-for-1242-through-1254.md:67-138`):
  - It is still hard-wrapped.
  - It still cites "the 1251 fix merge", "the 1250 fix merge" and "the pending-review merge" instead of commits.
  - It keeps shorthand such as "1242 b1" and "1252/1248".
  - It says 1264 is in flight, and 1264 has landed.
- Ticket 1260's code paths name only `src/entities/variant/` and "the variant CLI module". Deferred gaps says "None yet". It names no prior experiment evidence.
- The evidence file, the pass record and the aspirin issue are hard-wrapped.
- The README pin at `tests/test_public_landing_copy_docs_contract.py:64-66` hard-codes "reaches 70 trusted". Its comment says the count cannot drift from the source. Compute the count from `sources.json`, and pin the "eight further" count the same way. The count of 70 is correct today.
- `sdlc/issues/2026-09-28-review-follow-ups-after-1255-through-1261.md` has no disposition section.

## Main's run history

Main had seven failed CI runs and three green runs on 2026-09-28. The fix commit `4d05cfd8` failed lint, and `a236173d` and `84920f31` failed too. Three runs failed on a package file count of 1370 against a pin of 1369 and a missing `llms.txt` entry. One failed the README landing contract. None was the stdio flake, which passed every run in 2.6 to 4.2 seconds. Every failing commit went straight to main, and main stayed red for about five of six hours. Run the full canonical gate on the ticket branch before merging, including docs-only and record commits.

## Addendum 2026-09-28 (ticket 1267, pre-disposition note)

The #287 product points, decided: exit codes fixed with tests (a
missing skill exits 1; an empty skill catalog now fails loudly
naming the embedded skills/ tree instead of printing "No skills
found" with exit 0); `debug-embed` enabled (decision recorded in
ticket 1267 with the reason); the 0.8.x wheel-size check is the
supervisor's (needs PyPI); the reply waits for Ian's go and for
0.9.1 on PyPI.
