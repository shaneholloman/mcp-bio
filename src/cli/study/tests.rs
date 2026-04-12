use clap::Parser;

use crate::cli::study::StudyCommand;
use crate::cli::{ChartType, Cli, Commands};

#[test]
fn study_list_parses_subcommand() {
    let cli = Cli::try_parse_from(["biomcp", "study", "list"]).expect("study list should parse");

    assert!(matches!(
        cli.command,
        Commands::Study {
            cmd: StudyCommand::List
        }
    ));
}

#[test]
fn study_chart_subcommand_parses_specific_topic() {
    Cli::try_parse_from(["biomcp", "chart", "violin"]).expect("chart docs should parse");
}

#[test]
fn study_query_parses_chart_flags() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "study",
        "query",
        "--study",
        "msk_impact_2017",
        "--gene",
        "TP53",
        "--type",
        "mutations",
        "--chart",
        "bar",
    ])
    .expect("study query should parse");

    let Cli {
        command: Commands::Study {
            cmd: StudyCommand::Query { chart, .. },
        },
        ..
    } = cli
    else {
        panic!("expected study query command");
    };

    assert_eq!(chart.chart, Some(ChartType::Bar));
}
