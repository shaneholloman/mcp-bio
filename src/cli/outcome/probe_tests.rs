use super::run_outcome_inner;
use crate::cli::Cli;
use clap::Parser;

/// Future-size probe (ticket 1243). `run_outcome_inner`'s future is the
/// biggest state machine in the CLI: every dispatch arm's locals live in
/// it, and the 8 MiB execute stack's headroom is set by its size (the
/// ticket 1225 pin). Future construction is lazy — parsing a minimal CLI
/// and calling the async fn runs no I/O — so the assert's failure
/// message reports the measured size without side effects.
///
/// Measured history (bytes, debug test profile):
///   2026-09-26 base fe691c48 (async with_no_cache, unboxed arms):
///     dispatch 225_312, run 227_856
///   2026-09-26 scoped no-cache return (item 1):
///     dispatch 113_520, run 114_512
///   2026-09-26 boxed dispatch arms and join (item 2):
///     dispatch 2_240, run 2_656
/// The ceilings carry roughly 2x margin over the boxed sizes so an
/// unrelated arm change surfaces here first.
#[test]
fn the_dispatch_future_stays_under_the_size_ceiling() {
    let cli =
        Cli::try_parse_from(["biomcp", "get", "gene", "BRAF"]).expect("minimal gene get parses");
    let fut = run_outcome_inner(cli, false);
    let bytes = std::mem::size_of_val(&fut);
    assert!(
        bytes <= 4_096,
        "run_outcome_inner's future is {bytes} bytes, over the 4_096-byte ceiling; \
         a dispatch arm grew its state machine (see the measured history above)"
    );
}

/// The `run` fallthrough wraps the same arms and must not regrow them:
/// its future is bounded separately because it constructs its own Cli.
#[test]
fn the_run_fallthrough_future_stays_under_the_size_ceiling() {
    let cli = Cli::try_parse_from(["biomcp", "cache", "path"]).expect("cache path parses");
    let fut = super::super::run(cli);
    let bytes = std::mem::size_of_val(&fut);
    assert!(
        bytes <= 8_192,
        "the run future is {bytes} bytes, over the 8_192-byte ceiling; \
         see the measured history in the dispatch-future probe"
    );
}
