# BioMCP dead-code evaluation

Date: 2026-06-17

## Question answered

What code can be removed after the test rebuild, and did the coverage/dead-code
checks expose anything important?

Short answer:

- Coverage is now measured at the whole-crate level: **71.32% line coverage**
  from `cargo llvm-cov nextest --summary-only`.
- All normal tests passed under coverage: **2332 passed, 28 skipped**. The skipped
  tests are live-network smoke tests.
- `cargo machete` found **no unused Cargo dependencies**.
- The Rust compiler dead-code pass found useful leads, but many are not real
  removal candidates because Rust checks each target separately. Test-only,
  binary-only, and generated-code paths can look unused in one target while still
  being used in another.

## Commands run

```bash
RUSTFLAGS="--force-warn dead_code" \
  cargo check --all-targets --locked --message-format=json \
  > /tmp/biomcp-dead-code.jsonl \
  2> /tmp/biomcp-dead-code-stderr.jsonl

jq ... /tmp/biomcp-dead-code.jsonl \
  | sort -u \
  > /tmp/biomcp-dead-code-local.tsv

cargo machete
```

The local source report had **164 warning rows**:

| Area | Rows |
|---|---:|
| `src/sources` | 71 |
| `src/cli` | 50 |
| `src/entities` | 24 |
| `src/render` | 14 |
| `src/main.rs` | 2 |
| `src/cache` | 2 |
| `tests/pty_helpers.rs` | 1 |

Generated AlphaGenome files under `target/debug/build/...` also warned. Those
are generated build output and are not source cleanup work.

## High-confidence code that can probably be removed

These had definition-only references in local source/spec/docs, or an obsolete
test-helper shape. Remove one small group at a time and run focused tests.

| Candidate | Why it looks removable | Suggested check |
|---|---|---|
| `src/sources/litsense2.rs::paragraph_search` | Definition-only. The article flow uses sentence search, not paragraph search. | `cargo nextest run -E 'test(/sources::litsense2/) | test(/entities::article/)'` |
| `src/sources/gtex.rs::resolve_versioned_gencode_id` | Public async wrapper is definition-only. The private unlocked resolver is still used by median expression. | `cargo nextest run -E 'test(/sources::gtex/) | test(/entities::gene/)'` |
| `src/entities/adverse_event.rs::search_device` | No-offset wrapper is definition-only. CLI uses `search_device_page`. | `cargo nextest run -E 'test(/entities::adverse_event/) | test(/cli::adverse_event/)'` |
| `src/entities/adverse_event.rs::search_recalls` | No-offset wrapper is definition-only. CLI uses `search_recalls_page`. | `cargo nextest run -E 'test(/entities::adverse_event/) | test(/cli::adverse_event/)'` |
| `src/sources/ddinter.rs::DdinterClient::root` and possibly the `root` field | The accessor is definition-only. The field is only used to return that accessor after the index is loaded and cached by root. | `cargo nextest run -E 'test(/sources::ddinter/) | test(/entities::drug/)'` |

The `ddinter` cleanup needs one extra look before editing: `ready()` currently
stores `root` in the client, but interactions only need the loaded index. If no
caller needs to inspect the cache path, both the accessor and field can go.

## Medium-confidence cleanup candidates

These are likely removable if BioMCP does not treat its Rust crate as a stable
library API for outside callers. They are less safe than the list above because
they are public facade functions, not just private leftovers.

| Candidate family | Why it exists | Cleanup shape |
|---|---|---|
| No-offset entity wrappers like `entities::<kind>::search(...)` | Convenience wrappers around `search_page(..., offset = 0)`. The CLI generally calls the page-aware form. | Remove wrappers only if external Rust callers are not supported, or after a deprecation window. |
| No-footer markdown wrappers like `gene_search_markdown`, `variant_search_markdown`, `pgx_search_markdown`, etc. | Facade functions around richer `_with_footer` or context-aware renderers. Some are now mostly test-facing. | Remove only if the public render facade is allowed to shrink. Otherwise keep them as compatibility helpers. |
| Some serde DTO fields marked `#[allow(dead_code)]` | Upstream response structs often keep fields for payload shape, diagnostics, or future transforms. | Review source by source. Removing fields is usually safe with normal serde defaults, but it can make fixtures and debugging poorer. |

## Code I would not remove from this audit

| Area | Why not remove |
|---|---|
| `src/cli/benchmark/**` | It is intentionally an internal/test-only benchmark harness. `architecture/technical/benchmark-cli-ownership-decision.md` explicitly says to keep it, and `tests/benchmark_cli_structure.rs` ratchets that shape. |
| `RequestPlan` structs and request-plan helper types used by construction tests/specs | These are the new no-network unit-test surface. The compiler can warn because they are test-facing, but they are doing the job we just built them for. |
| `src/main.rs::main` and `init_tracing` warnings | False positives from target-specific compilation. They are the binary entry point. |
| `tests/pty_helpers.rs::run_biomcp_with_tty` | Used by `tests/cache_clear.rs`. |
| `src/cache/config.rs::resolve_cache_config_from_parts` | Used by cache tests after the env-lock cleanup. |
| Generated AlphaGenome warnings under `target/debug/build` | Not checked-in source. |
| Cargo dependencies | `cargo machete` found none unused. |

## Coverage and dead-code conclusion

The test rebuild did not leave an obvious large cleanup pile. The practical
removal work is small:

1. Remove the high-confidence definition-only helpers above.
2. Decide separately whether BioMCP's Rust crate promises public API stability.
   If not, the no-offset entity wrappers and no-footer render wrappers are the
   next cleanup pass.
3. Leave the benchmark harness and request-plan test surface alone unless there
   is a product decision to retire them.

