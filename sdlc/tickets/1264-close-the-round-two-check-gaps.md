# Close the round-two check gaps

From sdlc/issues/2026-09-28-review-follow-ups-after-1255-through-1261.md
(Checks section). Ian's standing bar: every check catches the next
spelling it was never told about.

## Fix

1. Workflow contract: each pinned release job's step list (ids,
   order, count) matches exactly, so a new step slipped before a
   pinned one fails; BASH_ENV banned in every env block and with:
   value; duplicate needs: lines fail both the parsed-document
   comparison and a per-job line-count check.
2. Review grammar: whole-line allowlist — a status declaration in
   any decoration (heading, paragraph, bullet) with a state word
   fails, including compound shapes (a pending heading beside an
   honest ACCEPT record); verdicts must declare a state; negated
   resolutions do not resolve; scopes may hold only batch/item
   words and numbers.
3. Stdio guard: the .output() exemption is gone; inheriting setters
   and pre-bound inheriting variables fail; unknown spellings fail
   closed.
4. Wait ratchet: elapsed()/Instant::now() comparisons in either
   order and any operator (>= floor assertions exempt, including
   multi-line assert!); Rust use-aliases and assigned Python time
   modules resolve per file.
5. Ticket 1258 carries a Deferred gaps section naming every hole
   and this ticket as closer.

## Deferred gaps

- Only the four pinned jobs get exact step lists; a new first step
  in pypi-publish, homebrew-tap, build, or publish-release is
  guarded by text assertions only.
- A Rust function-pointer binding (`let nap = std::thread::sleep`)
  escapes alias resolution.
- The multi-line assert! floor exemption has no dedicated test.
- The gencc.rs pin has slack of one (pinned 1, actual 0) — re-pin
  down with the tool's --update.

## Review

- Design review: n/a (the review file named the structures)
- Code review: REJECT once (the allowlist never scanned non-bullet
  lines; two normalizations invented verdicts; one truncated a
  recorded verification), folded and verified 2026-09-28
- Verification: yellow gate at 4de8e686 — lint, test, spec, stress
  OK, zero failed lines; the ratchet and the five suites green
  locally and on the host
