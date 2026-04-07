# Code Review Log — Ticket 152

## Critique

I reviewed `.march/ticket.md`, `.march/design-draft.md`, `.march/design-final.md`,
`.march/code-log.md`, and the full `git diff main..HEAD` against the final NCI
contract. The implementation already covered the runtime request model, the NCI
trial branch, the health probe, and the named docs/help/spec surfaces from the
design.

I also checked the changed path for security regressions (untrusted input into
URLs/auth handling/output), searched for reinvention before accepting the new
helpers, verified that the executable specs remain outside-in, and confirmed
from `.march/code-log.md` that docs/help/spec work landed before the runtime
translation. No additional runtime defects surfaced in those passes.

The defects I found were in proof strength rather than runtime translation:

1. The keyword-fallback regression only exercised the MyDisease upstream-error
   path. The final design also requires keyword fallback when the best hit lacks
   an NCI xref and when MyDisease returns no hit.
2. The NCI status regression only proved `recruiting` and `completed`, leaving
   the rest of the documented single-value mapping table unprotected.
3. The NCI phase/help/list proof did not lock the direct CTS token mapping for
   `2 -> II` / `NA -> NA` or the operator-facing `early_phase1` rejection note.

## Fix Plan

1. Add trial-layer fallback regressions for no-xref and no-hit resolution paths
   and tighten the keyword-fallback request assertion to the current CTS concept
   key.
2. Expand the NCI status and phase regression tests so they cover the full
   documented mapping set instead of only representative happy paths.
3. Extend help/list/spec assertions so the user-visible `early_phase1` note is
   locked alongside the existing NCI status/phase/geo guidance.

## Repair

- Added a shared keyword-fallback mock helper in `src/entities/trial.rs` tests
  and new regressions for the no-xref and no-hit fallback paths.
- Expanded `nci_status_mapping_uses_documented_single_value_filters` to cover
  every documented single-value NCI status mapping.
- Expanded `nci_phase_mapping_uses_i_ii_for_combined_phase` to prove direct
  `II`, `NA`, and combined `I_II` emission.
- Tightened `spec/04-trial.md`, `src/cli/mod.rs`, and `src/cli/list.rs` tests
  so the `early_phase1` rejection note remains part of the documented NCI
  operator contract.
- Re-ran `make spec` and `make check` after the review repairs.

## Residual Concerns

None. The live NCI/MyDisease behavior still depends on upstream auth and
availability, but the request translation and operator-facing contract are now
covered by hermetic regression proof.

## Defect Register

| # | Category | Lintable | Description |
|---|----------|----------|-------------|
| 1 | missing-test | yes | Design-required NCI keyword fallback paths for no-xref and no-hit resolution were not covered by tests. |
| 2 | weak-assertion | no | NCI status proof only exercised a subset of the documented single-value mappings. |
| 3 | weak-assertion | no | NCI phase/help/list proof did not lock direct CTS phase tokens and the `early_phase1` operator note. |
