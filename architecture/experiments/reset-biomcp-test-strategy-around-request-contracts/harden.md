# Harden — Reset BioMCP test strategy around request contracts

## Decomposition

The optimized implementation for ticket 371 was the lightweight static inventory script:

- Before: `architecture/experiments/reset-biomcp-test-strategy-around-request-contracts/scripts/analyze_test_strategy.py` contained all data types, regexes, inventory functions, JSON assembly, writing, and CLI behavior.
- After: reusable logic lives in `architecture/experiments/reset-biomcp-test-strategy-around-request-contracts/lib/biomcp_test_strategy/`.
  - `inventory.py` owns the data types, filesystem configuration, static inventory functions, summary/proposed-profile generation, and stable JSON writer.
  - `__init__.py` re-exports the intended import surface for downstream spikes.
  - `scripts/analyze_test_strategy.py` is now a 19-line CLI wrapper that only adds the experiment `lib/` directory to `sys.path`, imports `default_config`/`main`, passes its own path for checksum compatibility, and prints the generated result path.

Why this shape: ticket 371 is a Rust BioMCP test-strategy spike, but its optimized implementation is a Python evidence/inventory tool, not product runtime code. Downstream follow-up tickets need to import the inventory library to compare contract counts and inspect source/plan seams; they should not shell out to the CLI wrapper or copy the analysis code.

## Public API

Import root:

```text
architecture/experiments/reset-biomcp-test-strategy-around-request-contracts/lib
```

Public package:

```python
biomcp_test_strategy
```

Public types:

- `InventoryConfig(repo_root: Path, results_dir: Path, march_runtime: Path, script_path: Path | None)` — explicit filesystem inputs/output for one inventory run.
- `SpecSection` — structured metadata for one representative spec section: path, heading, line, dependency tags, proof-type tags, fixture flag, and live-CLI flag.

Public functions:

- `default_config(script_path: Path | None = None) -> InventoryConfig` — default BioMCP worktree config. CLI wrappers pass their path to preserve the historic JSON `script` field.
- `build_inventory(config: InventoryConfig) -> dict[str, Any]` — assemble the complete inventory payload without writing it.
- `write_inventory(payload: dict[str, Any], results_dir: Path) -> Path` — write `test_strategy_inventory.json` with the stable checked-in JSON format.
- `main(config: InventoryConfig | None = None) -> Path` — convenience runner equivalent to the CLI.
- Focused sub-inventories: `extract_sections(path, root)`, `validation_profiles(config)`, `makefile_targets(config)`, `source_contract_inventory(config)`, `plan_seam_inventory(config)`, `march_preflight_evidence(config)`, `summarize(...)`, and `proposed_profiles(summary)`.

Downstream import example:

```python
from pathlib import Path
import sys

repo = Path.cwd()
sys.path.insert(
    0,
    str(repo / "architecture/experiments/reset-biomcp-test-strategy-around-request-contracts/lib"),
)

from biomcp_test_strategy import build_inventory, default_config, source_contract_inventory

config = default_config()
payload = build_inventory(config)
source_rows = source_contract_inventory(config)

assert payload["summary"]["source_contract_totals"]["mock_given"] >= 40
assert len(source_rows) == 6
```

CLI compatibility example:

```bash
./architecture/experiments/reset-biomcp-test-strategy-around-request-contracts/scripts/analyze_test_strategy.py
sha256sum architecture/experiments/reset-biomcp-test-strategy-around-request-contracts/results/test_strategy_inventory.json
```

The wrapper still emits the same result path and preserves the checksum contract when invoked this way.

## Build System

There is no `build.zig` in this BioMCP Rust/Python spike, and no product Cargo/build.rs change was needed. The importable library is shipped as plain Python package files under the experiment `lib/` directory:

```text
architecture/experiments/reset-biomcp-test-strategy-around-request-contracts/lib/biomcp_test_strategy/__init__.py
architecture/experiments/reset-biomcp-test-strategy-around-request-contracts/lib/biomcp_test_strategy/inventory.py
```

Downstream spikes can depend on the library by adding that `lib/` directory to `PYTHONPATH`/`sys.path` in their own analysis scripts. The CLI wrapper remains available only as a convenience binary-compatible entrypoint; it is not the downstream integration contract.

No product/runtime Rust import surface was introduced or changed by this hardening step.

## Regression Check

Baseline/optimized control target from optimize:

- Inventory command elapsed: `0.06s` at command startup/timer floor.
- Checksum: `b090cba04589f3f9a6e980d2314280698d369f698d9a952f1082ae8316198527`.
- Product validation: `cargo check --all-targets` passed; no runtime/product file diff.

Post-decomposition benchmark suite:

```bash
for i in 1 2 3 4 5; do \
  /usr/bin/time -f "run=$i elapsed=%e rss_kb=%M" \
    ./architecture/experiments/reset-biomcp-test-strategy-around-request-contracts/scripts/analyze_test_strategy.py \
    >/tmp/t371-inventory-path.txt; \
done
sha256sum architecture/experiments/reset-biomcp-test-strategy-around-request-contracts/results/test_strategy_inventory.json
```

Observed post-decomposition results:

- Elapsed: `0.05-0.06s` across five runs; no regression from the optimized `0.06s` command-level floor.
- RSS: `23,652-24,608 KB` across five runs; within normal process-startup variation and below/near prior optimize measurements.
- Checksum: unchanged at `b090cba04589f3f9a6e980d2314280698d369f698d9a952f1082ae8316198527`.

Post-decomposition validation suite:

```bash
uv run --script - <<'PY'
# /// script
# requires-python = ">=3.12"
# ///
from pathlib import Path
import sys
repo = Path.cwd()
sys.path.insert(0, str(repo / 'architecture/experiments/reset-biomcp-test-strategy-around-request-contracts/lib'))
from biomcp_test_strategy import build_inventory, default_config, source_contract_inventory, plan_seam_inventory
config = default_config(repo / 'architecture/experiments/reset-biomcp-test-strategy-around-request-contracts/scripts/analyze_test_strategy.py')
payload = build_inventory(config)
assert payload['summary']['representative_spec_sections'] == 39
assert payload['summary']['source_contract_totals']['mock_given'] == 40
assert len(source_contract_inventory(config)) == 6
assert len(plan_seam_inventory(config)) == 6
print('library import smoke passed')
PY
cargo check --all-targets
git diff --name-only -- src Cargo.toml Cargo.lock Makefile spec tests tools docs README.md
```

Validation result: library import smoke passed, `cargo check --all-targets` passed, and there is no product/runtime diff under `src Cargo.toml Cargo.lock Makefile spec tests tools docs README.md`.

## Reusable Assets

Downstream spikes inherit these concrete assets:

- Importable inventory package: `biomcp_test_strategy`.
- Shared configuration/type definitions: `InventoryConfig`, `SpecSection`.
- Stable whole-payload generator: `build_inventory(config)`.
- Stable JSON writer: `write_inventory(payload, results_dir)`.
- Focused inventory helpers for follow-up tickets:
  - `source_contract_inventory(config)` for wiremock/query/header/body/source seam counts.
  - `plan_seam_inventory(config)` for existing CLI/entity plan functions and direct execution coupling.
  - `extract_sections(path, root)` for spec section dependency/proof classification.
  - `validation_profiles(config)` and `makefile_targets(config)` for March/profile reset work.
  - `march_preflight_evidence(config)` for ticket-370 slow/flaky live-source evidence.
- Thin CLI wrapper pattern for future experiment tools: keep `scripts/*.py` as entrypoints and put reusable code under experiment-local `lib/` packages.
- Checksum contract for downstream regression checks: `b090cba04589f3f9a6e980d2314280698d369f698d9a952f1082ae8316198527` for the current inventory JSON when invoked through the wrapper.

## Downstream Consumers

The named downstream consumers are the follow-up tickets recommended by the exploit strategy artifact:

1. OLS4/MyDisease source request-plan primitives and contracts.
2. Disease/discover and article CLI `RequestCommand` seams.
3. March validation profile split into deterministic routine gates and explicit live smoke.
4. Article and variant fixture-backed source-spec conversions.
5. Pruning/relaxing brittle executable assertions after deterministic replacements exist.

Each can import the library directly to reuse the current contract inventory and avoid re-parsing or shelling out to the CLI.
