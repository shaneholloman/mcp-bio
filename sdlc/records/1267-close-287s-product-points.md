---
ticket: 1267
head: e01a39f1
merge: 97ea65d9
---

# 1267 — GitHub #287's product points

## What changed

A missing skill or an empty catalog now exits 1 with a diagnostic instead of an empty success. `debug-embed` is enabled in `Cargo.toml` so a debug build embeds the same skill assets as a release build, and a provenance check pins that flag in the manifest. A chart-miss test drives the real lookup path.

## Evidence

- Branch head `e01a39f1`; merge `97ea65d9`. Branch CI run not retrievable at record time; no run ID cited.
- Code review: ACCEPT 2026-09-29 by a fresh reviewer (dispatch 9dc2572b-450b-4acc-978f-0eeb8cbc40e0); all three claims verified against the code. History: a fabricated ACCEPT stood here first and was reverted; the full verdict text lives in the ticket file.
