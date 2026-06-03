# Contributing to BioMCP

BioMCP does not accept outside pull requests.

We do welcome:

- GitHub Issues for bugs, regressions, and reproducible problems
- GitHub Discussions for feature ideas, usage questions, and documentation requests

This policy keeps release provenance, supply-chain control, and copyright review
for AI-assisted code with the core maintainers. We still want problem reports
and product feedback, and the team will fix confirmed issues in the main repo.

When you open an issue or discussion, include:

- the BioMCP version
- the command you ran
- the relevant output or error text
- any source or API context needed to reproduce the problem

## Repo-Local Test Setup

Install `cargo-nextest` before running repo-local Rust verification:

```bash
cargo install cargo-nextest --locked
```

`make test` uses `cargo nextest run`. `make spec-contracts` is the
deterministic routine executable-contract lane used by March `spec-only` and
`make release-gate`; it runs validation-lane docs/static surface contracts
without live smoke. `make release-live-smoke` is the explicit opt-in live
public-upstream confidence lane and owns the pathway live assertions. `make
spec` and `make spec-pr` remain available for the active canary tree under
`spec/entity/` and `spec/surface/`, excluding the pathway live-smoke spec, with
`pytest-xdist` (`-n auto --dist loadfile`). The executable docs themselves call
`tools/biomcp-ci`, which owns release-binary resolution, the repo-owned
`.cache/biomcp-specs/` cache/XDG roots, optional-key stripping, and warm-hit
`BIOMCP_CACHE_MODE=infinite` replay when CI sets `BIOMCP_SPEC_CACHE_HIT=1`.
`make check` now runs `lint`, `test`, `test-contracts`, and
`check-quality-ratchet`, so the canonical local gate already includes the
Python/docs contract lane. `make release-gate` is the single routine
release-readiness command; it runs `make check` and then `make spec-contracts`.
Use `make test-contracts` to rerun just the release-critical Python/docs lane.

### Local Pre-Commit Hook

Developers who opt in to the repo-local pre-commit hook should install it at
`$(git rev-parse --git-path hooks/pre-commit)`. The hook is local Git state;
the repo does not install it automatically.

Use this shape so `scripts/pre-commit-reject-march-artifacts.sh` runs before
`cargo fmt --check` and `cargo clippy --lib --tests -- -D warnings`:

```bash
hook_path="$(git rev-parse --git-path hooks/pre-commit)"
mkdir -p "$(dirname "$hook_path")"
cat >"$hook_path" <<'HOOK'
#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

scripts/pre-commit-reject-march-artifacts.sh
cargo fmt --check
cargo clippy --lib --tests -- -D warnings
HOOK
chmod +x "$hook_path"
```

The helper allows only `.march/code-review-log.md` and
`.march/validation-profiles.toml` under `.march/`, and it permits staged
deletions so cleanup commits can remove old March artifacts from tracking.

### Timing Method

Measured on beelink on 2026-04-23 with `/usr/bin/time -p` using warm-cache
steady-state runs. Each command was run once untimed to warm build artifacts and
the repo-owned spec cache under `.cache/biomcp-specs/`, then once with timing
enabled. The `make spec-pr` row was refreshed on 2026-04-24 after the spec-v2
canary cutover. `make release-gate` is a thin wrapper over `make check` and
`make spec-contracts`, so its warm timing tracks the current sum of those warmed
routine component lanes.

| Command | Observed warm-cache | Notes |
|---|---|---|
| `make check` | `344.11s` | now includes `make test-contracts` |
| `make spec-contracts` | `386.98s` | deterministic routine lane, including release rebuild and 48 spec assertions (2026-05-23) |
| `make spec-pr` | `56.16s` | full canary lane (refreshed 2026-04-24) |
| `make release-live-smoke` | `operator-run` | opt-in live public-upstream smoke; not part of routine gates |
| `make release-gate` | `763.21s` | observed `make check` plus `make spec-contracts` routine gate (2026-05-23) |
