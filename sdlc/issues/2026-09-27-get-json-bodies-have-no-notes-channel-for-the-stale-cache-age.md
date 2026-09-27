# Get-JSON bodies have no notes channel for the stale-cache age

Filed from ticket 1256. The stale-cache age now reaches the markdown
card (a trailing `Cache note:` line) and search JSON (`_meta.notes`),
but single-entity `get ... --json` bodies have no notes or `_meta`
field to carry it: they serialize the entity struct directly (for
example `src/cli/disease/dispatch.rs` `render_loaded_card` with
`json_output`).

## Why deferred

Adding a `_meta.notes` shape to every get-JSON body changes the
public JSON surface for all entities at once. That deserves its own
ticket with a schema decision (a shared wrapper versus per-entity
fields), not a rider on the wording fix. The markdown path — the
clinician-facing MCP card — is covered today, and the JSON gap fails
quietly: the note simply does not appear, the same as before ticket
1256.

## Revisit trigger

When a user or contract test asks for the stale age on a get-JSON
body, or when the flat-schema work (ADR 0002 follow-ups) next touches
the JSON envelope shape. Then: pick the envelope, add the notes
channel once, and reuse `crate::sources::take_stale_serve_sentences`
at the single build site.
