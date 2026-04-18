# Harden: 25-gene-all-latency

## Decomposition

The optimized `get gene all` implementation is now library-first:

- `src/gene.rs` is the public library facade. Downstream Rust consumers import
  this module instead of shelling out to `biomcp`.
- `src/entities/gene.rs` owns the reusable data model, typed section selection,
  get options, timing report structs, provider fanout, short-circuit behavior,
  and strategy selection.
- `src/cli/gene/dispatch.rs` is back to a thin wrapper for `get` and search
  dispatch. It is 121 lines after extraction and calls `crate::gene::get` for
  gene retrieval.
- `src/cli/gene/related.rs` holds non-get related command rendering for trials,
  drugs, articles, and pathways, keeping the primary dispatch wrapper small.

The refactor did not change the optimized fetch algorithm. The default
strategy remains `ParallelTop`, ClinGen prefetch still overlaps MyGene
resolution, and independent optional sections still fan out concurrently.

## Public API

Downstream code should import `biomcp_cli::gene`.

Primary types:

- `Gene`: structured gene result with optional section payloads.
- `GeneSection`: typed selector for optional sections.
- `GeneGetStrategy`: `ParallelTop` default, plus `Baseline` and
  `OpenTargetsEnsembl` for regression controls.
- `GeneGetOptions`: sections, strategy, optional section timeout, and optional
  timing report path.
- `GeneGetResult`: `Gene` plus `GeneTimingReport`.
- `GeneTimingReport` and `GeneTimingEntry`: structured timing output for
  benchmark harnesses and agent diagnostics.

Primary functions:

- `gene::get(symbol, sections)`: CLI-compatible string-section wrapper that
  preserves existing env controls.
- `gene::get_with_options(symbol, &GeneGetOptions)`: preferred library call for
  downstream consumers that only need the `Gene`.
- `gene::get_with_report(symbol, &GeneGetOptions)`: same production path, with
  timing returned in memory.
- `gene::parse_sections(symbol, sections)`: parses CLI-compatible section names
  into `Vec<GeneSection>`.
- `GeneSection::all_default()`: expands `all` exactly as the CLI does today,
  intentionally excluding `Disgenet` and `Funding`.

Example: get the default `all` payload without a shell:

```rust
use biomcp_cli::gene::{self, GeneGetOptions, GeneSection};

let options = GeneGetOptions::default()
    .with_sections(GeneSection::all_default());
let gene = gene::get_with_options("BRAF", &options).await?;

assert_eq!(gene.symbol, "BRAF");
assert!(gene.clingen.is_some());
```

Example: collect timing in memory for an agent workflow:

```rust
use biomcp_cli::gene::{self, GeneGetOptions, GeneSection};

let options = GeneGetOptions::default()
    .with_sections(vec![GeneSection::ClinGen, GeneSection::Druggability]);
let result = gene::get_with_report("TP53", &options).await?;

for section in result.timing.sections {
    eprintln!("{}: {} ms ({})", section.section, section.elapsed_ms, section.outcome);
}
```

Example: parse a CLI-compatible section list once, then call the library:

```rust
use biomcp_cli::gene::{self, GeneGetOptions};

let raw_sections = vec!["all".to_string()];
let sections = gene::parse_sections("CFTR", &raw_sections)?;
let result = gene::get_with_report(
    "CFTR",
    &GeneGetOptions::default().with_sections(sections),
).await?;
```

## Build System

This spike is Rust, not Zig. There is no `build.zig` in the repository.
Cargo already produces:

- library target: `biomcp_cli` from `src/lib.rs`
- binary target: `biomcp` from `src/main.rs`
- binary target: `biomcp-cli` from `src/main_biomcp_cli.rs`

Downstream Rust code can depend on the library target with Cargo:

```toml
[dependencies]
biomcp-cli = { path = "../biomcp" }
```

Then import the library crate as:

```rust
use biomcp_cli::gene::{self, GeneGetOptions, GeneSection};
```

No downstream spike needs a process boundary or copied `get gene all` code.

## Regression Check

Release binary rebuilt after the final source change:

```bash
cargo build --release
```

Benchmark suite:

```bash
python3 architecture/experiments/25-gene-all-latency/scripts/gene_all_latency_probe.py \
  --approach harden \
  --gene BRAF \
  --gene TP53 \
  --gene CFTR \
  --runs 5 \
  --timeout-seconds 180 \
  --output architecture/experiments/25-gene-all-latency/results/harden_regression_control.json
```

Primary harden matrix, 30/30 successful commands:

| Gene | Mode | p50 wall clock | p95 wall clock |
|---|---|---:|---:|
| BRAF | markdown | 1811.51 ms | 12516.26 ms |
| BRAF | JSON | 1756.57 ms | 1851.33 ms |
| TP53 | markdown | 1784.69 ms | 1865.52 ms |
| TP53 | JSON | 1667.79 ms | 2024.91 ms |
| CFTR | markdown | 1669.77 ms | 1717.11 ms |
| CFTR | JSON | 1743.43 ms | 1798.80 ms |

The BRAF markdown p95 was a single live QuickGO tail (`go` 14848 ms) in an
unchanged section. The command still stayed under the 30s ticket budget and the
20s stretch ceiling. A same-code rerun
(`harden_regression_control_rerun.json`) repeated QuickGO provider tails while
all 30 commands still succeeded and remained under 20s p95, confirming the tail
was live upstream variance rather than a harden refactor failure.

Validation suite:

```bash
cargo fmt --check
RUST_MIN_STACK=16777216 cargo test --lib
cargo clippy --lib --tests -- -D warnings
uv run --extra dev pytest spec/18-source-labels.md --mustmatch-lang bash --mustmatch-timeout 60 -v
python3 architecture/experiments/25-gene-all-latency/scripts/gene_all_output_diff.py \
  --baseline-approach baseline:BIOMCP_GENE_GET_STRATEGY=baseline \
  --candidate-approach harden:BIOMCP_GENE_GET_STRATEGY=parallel-top \
  --gene BRAF \
  --gene TP53 \
  --gene CFTR \
  --timeout-seconds 180 \
  --output architecture/experiments/25-gene-all-latency/results/harden_output_diff.json
```

Validation results:

- `cargo fmt --check`: passed.
- `RUST_MIN_STACK=16777216 cargo test --lib`: passed, 1762 tests.
- `cargo clippy --lib --tests -- -D warnings`: passed.
- `spec/18-source-labels.md`: passed, 16 passed and 4 skipped.
- Output diff: passed for `BRAF`, `TP53`, and `CFTR`; markdown identical,
  canonical JSON identical, mismatch count 0.

## Reusable Assets

Downstream spikes inherit:

- Public `biomcp_cli::gene` facade.
- Structured `Gene` result and section payload types.
- Typed `GeneSection` selectors, including exact CLI `all` expansion.
- `GeneGetOptions` for section selection, strategy control, optional timeout,
  and timing file output.
- `GeneGetResult`, `GeneTimingReport`, and `GeneTimingEntry` for direct
  in-process instrumentation.
- Optimized `ParallelTop` section fanout and ClinGen prefetch path.
- CLI-compatible `parse_sections` for code that accepts user-facing section
  names but still calls Rust directly.
- Cargo dependency pattern for consuming the library crate from another Rust
  module.
