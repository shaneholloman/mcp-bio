# 1281 — carry the kids26 branch's still-true work, then retire it

Filed 2026-09-30 under Ian's branch direction (second go-request review): `tickets/kids26-cell-line-dataset` holds four records for cell-line work whose CODE landed on main long ago (the records name their green gates — 37336c22 and later) and a ChEMBL licensing row with citation URLs main lacks. This ticket carries the still-true parts and deletes the branch.

## What is carried

- The four branch-only records: 1202 (cell-line entity), 1205 (PharmacoDB drug-response sections), 1213 (HPA expression), 1214 (ChEMBL cell-line section). Each describes landed, gated work; main had no record for it.
- ChEMBL's licensing citations into `docs/reference/sources.json` and the licensing page: the gitbook about page and the release LICENSE file that both carry the CC BY-SA 3.0 grant, and the history that the shipped attribution line has said CC BY-SA 3.0 since ticket 1214. Main's licence summary already named CC BY-SA 3.0 (2026-09-27 pass); what was missing is the citations and the ShareAlike-aware reuse summary.

## What is NOT carried (and why)

- The branch's sources.json otherwise predates the 2026-09-27 licensing pass (its rows say 2026-03-20); main's data is newer and more accurate, including Enrichr's restricted relabel and the CPIC and PharmGKB corrections.
- The 1206 DepMap draft hold: superseded by the recorded decision that DepMap stays out of the repository (rehosting restricted bytes in a public MIT repo imposes narrower terms; the 21-month-stale mirror; recorded in the licensing pass).
- The entity-track planning notes and the 0.9.1-1.0 backlog edits: superseded by the planning files on main.

After this lands, the branch is deleted; its remaining diff against main is exactly the superseded material above.
