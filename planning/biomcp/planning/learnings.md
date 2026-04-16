# BioMCP Learnings

## 2026-04-16 — Centralized test support helpers

- `EnvVarGuard`, `TempDirGuard`, and `set_env_var` live in `crate::test_support`
  (`src/test_support.rs`). Test modules should import or re-export them from there
  instead of defining local copies.
- Production cleanup helpers that only share a name shape with the test helper
  should use a distinct name (for example `TempDirCleanup`) so definition guards
  stay unambiguous.
