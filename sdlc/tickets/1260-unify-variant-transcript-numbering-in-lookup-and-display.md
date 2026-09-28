# Unify variant transcript numbering in lookup and display

From issue `2026-09-11-variant-display-mixes-transcript-numbering.md`
(a real clinical-report failure: a report's `c.1187G>A` finds no
match because the display mixes protein numbering from one transcript
with cDNA numbering from another; the not-found reads as "nothing to
see" instead of "wrong transcript"). The 2026-09-27 review ordered a
real ticket with an owner and trigger, not a parked line.

## Problem

Variant output names a protein consequence and a cDNA change whose
numbering can come from different transcripts (MUTYH c.1187G>A:
p.Gly396Asp on the common transcript, p.Gly382Asp as legacy
numbering on another). A consumer matching a report identifier
against that output fails, and the failure mode is silent.

## Scope

1. Every variant rendering that pairs cDNA and protein numbering
   names the transcript each comes from, once, plainly.
2. The variant lookup path accepts either transcript's cDNA
   numbering as a key and answers on the transcript it matched,
   saying so — the not-found case distinguishes "not in the source"
   from "that identifier belongs to a different transcript".
3. Curated transcript rules (which transcript is primary per gene)
   are data, recorded with provenance, not per-callsite choices.

## Acceptance

- The MUTYH c.1187G>A case from the issue: lookup by either
  numbering finds ClinVar 5294 and the output names its transcript.
- A test with two transcript numberings of one variant proves both
  keys resolve and the display labels its transcripts.
- The transcript-priority data records its source and date.

## Retained behavior

Single-transcript output is unchanged. The ClinVar Variation ID and
rsID lookups keep their current resolution. No existing identifier
stops resolving; the change only adds alternate keys and names the
transcript in use.

## Code paths

The variant entity's parse and render sites (`src/entities/variant/`,
the markdown and JSON renderers) and the lookup argument parsing in
the variant CLI module carry the numbering today; the exact sites
are named at implementation after a tracing pass, because the
2026-09-11 report names the display shape, not the files.

## Proof

The acceptance tests are the proof: both transcript numberings of
the MUTYH case resolve to ClinVar 5294 with the transcript named,
and a two-numbering fixture proves both keys work and the display
labels its transcripts.

## Deferred gaps

None yet; anything deferred during implementation gets its own
issue file with an owner and a revisit trigger, per house rule.

## Owner and trigger

Owner: the biomcp queue (developer agents; no clinical judgement
needed to render provenance already present in the data). Revisit
trigger: any new molecular-tumor-board experiment that matches
report identifiers against BioMCP output, or the start of the 1.0
feature track, whichever comes first. Ian can re-prioritize at any
time.
