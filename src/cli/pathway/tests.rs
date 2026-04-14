use clap::{CommandFactory, Parser};

use super::PathwayCommand;
use super::dispatch::should_try_pathway_trial_fallback;
use crate::cli::{Cli, Commands, execute};

fn render_pathway_search_long_help() -> String {
    let mut command = Cli::command();
    let search = command
        .find_subcommand_mut("search")
        .expect("search subcommand should exist");
    let pathway = search
        .find_subcommand_mut("pathway")
        .expect("pathway subcommand should exist");
    let mut help = Vec::new();
    pathway
        .write_long_help(&mut help)
        .expect("pathway help should render");
    String::from_utf8(help).expect("help should be utf-8")
}

#[test]
fn search_pathway_help_describes_conditional_query_contract() {
    let help = render_pathway_search_long_help();

    assert!(help.contains("biomcp search pathway [OPTIONS] <QUERY>"));
    assert!(help.contains("biomcp search pathway [OPTIONS] --top-level [QUERY]"));
    assert!(help.contains("required unless --top-level is present"));
    assert!(help.contains("multi-word queries must be quoted"));
    assert!(help.contains("biomcp search pathway --top-level --limit 5"));
}

#[test]
fn pathway_help_describes_source_aware_section_contract() {
    let mut command = Cli::command();
    let get = command
        .find_subcommand_mut("get")
        .expect("get subcommand should exist");
    let pathway = get
        .find_subcommand_mut("pathway")
        .expect("pathway subcommand should exist");
    let mut help = Vec::new();
    pathway
        .write_long_help(&mut help)
        .expect("pathway help should render");
    let help = String::from_utf8(help).expect("help should be utf-8");

    assert!(help.contains("events (Reactome only)"));
    assert!(help.contains("enrichment (Reactome only)"));
    assert!(help.contains("all = all sections available for the resolved source"));
    assert!(help.contains("biomcp get pathway R-HSA-5673001 events"));
    assert!(!help.contains("biomcp get pathway hsa05200 enrichment"));
}

#[test]
fn pathway_trials_parse_source_and_limit() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "pathway",
        "trials",
        "R-HSA-5673001",
        "--source",
        "nci",
        "--limit",
        "2",
    ])
    .expect("pathway trials should parse");

    match cli.command {
        Commands::Pathway {
            cmd:
                PathwayCommand::Trials {
                    id,
                    limit,
                    offset,
                    source,
                },
        } => {
            assert_eq!(id, "R-HSA-5673001");
            assert_eq!(limit, 2);
            assert_eq!(offset, 0);
            assert_eq!(source, "nci");
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[tokio::test]
async fn handle_command_rejects_zero_limit_before_related_lookup() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "pathway",
        "drugs",
        "R-HSA-5673001",
        "--limit",
        "0",
    ])
    .expect("pathway drugs should parse");

    let Cli {
        command: Commands::Pathway { cmd },
        json,
        ..
    } = cli
    else {
        panic!("expected pathway command");
    };

    let err = super::handle_command(cmd, json)
        .await
        .expect_err("zero pathway drugs limit should fail fast");
    assert!(err.to_string().contains("--limit must be between 1 and 50"));
}

#[test]
fn pathway_trial_fallback_allows_no_match_on_first_page() {
    assert!(should_try_pathway_trial_fallback(0, 0, Some(0)));
    assert!(should_try_pathway_trial_fallback(0, 0, None));
}

#[test]
fn pathway_trial_fallback_skips_offset_or_known_matches() {
    assert!(!should_try_pathway_trial_fallback(0, 5, Some(2)));
    assert!(!should_try_pathway_trial_fallback(0, 0, Some(7)));
    assert!(!should_try_pathway_trial_fallback(1, 0, Some(1)));
}

#[tokio::test]
async fn search_pathway_requires_query_unless_top_level() {
    let err = execute(vec![
        "biomcp".to_string(),
        "search".to_string(),
        "pathway".to_string(),
    ])
    .await
    .expect_err("search pathway should require query unless --top-level");
    assert!(
        err.to_string()
            .contains("Query is required. Example: biomcp search pathway -q \"MAPK signaling\"")
    );
}
