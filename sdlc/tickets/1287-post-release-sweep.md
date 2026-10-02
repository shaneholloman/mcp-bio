# 1287 — the 0.9.1 post-release sweep

Filed 2026-10-02 from the sixth review file's sweep list, after v0.9.1 published green (release run 36952191952; the six issues #250, #282, #283, #284, #286, #287 closed with evidence from the published artifacts).

## What landed

- The checklist's rehearsal item now describes the rehearsal as it actually ran: the public scratch repository kept for the purpose, the stated differences (scratch image name, credential-free distribution check, stubbed docs gate), the rerun-not-retag continuation, the cleanup, and the billing rationale — the GenomOncology account carries no payment method on GitHub, so private-repository minutes past the included allocation are refused, while public runners are free. Section 4 gains the item naming what the rehearsal's stubs skipped.
- The licensing evidence page drops the internal "go-request follow-up" phrasing, states the 48-versus-50 counts once, plainly, and explains why the Sanger DepMap Data Usage Policy covers Cell Model Passports (the portal documents itself under Sanger DepMap's documentation and links there for licensing).
- Record 1286 carries the Intel-runner retirement window (August 2027) and the Rosetta migration path.
- The DDInter judgment for the wheel-smoke sync, recorded here: DDInter's terms are CC BY-NC-SA 4.0 (academic, non-commercial, share-alike; the licensing entry carries the completeness disclaimer). The release workflow's smoke step downloads the bundle into an ephemeral runner environment to verify the wheel, uses it for nothing else, ships none of it, and the command remains `biomcp ddinter sync` for users — internal verification use, no redistribution, consistent with the 2026-09-27 review's handling of the source.
- The "P2"-style shorthand is removed from this cycle's round records; findings are named in words. Records from earlier cycles (0083, 0326, 0580, 0592, 1221) keep their historical wording — rewriting them would rewrite what those reviews actually said.

## The three post-verdict commits

The sixth review verified the content of the three commits that landed after ticket 1284's verdict (the rehearsal-status merge 8af33dc9, the rehearsal-evidence merge d7176bce, and the corrections merge 992df4c8) in its GO review, and Ian checked 992df4c8 line by line himself before recording the GO at 3f696d63. This ticket records that coverage; the sweep branch itself carries this round's review dispatch below.

## Review

- Pending: the verdict line lands with the dispatch before the merge.
