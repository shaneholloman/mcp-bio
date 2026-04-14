use clap::{CommandFactory, Parser};

use crate::cli::study::StudyCommand;
use crate::cli::{ChartType, Cli, Commands, execute};

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
fn study_download_parses_positional_study_id() {
    let cli = Cli::try_parse_from(["biomcp", "study", "download", "msk_impact_2017"])
        .expect("study download should parse");

    match cli.command {
        Commands::Study {
            cmd: StudyCommand::Download { list, study_id },
        } => {
            assert!(!list);
            assert_eq!(study_id.as_deref(), Some("msk_impact_2017"));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn study_download_parses_list_flag() {
    let cli = Cli::try_parse_from(["biomcp", "study", "download", "--list"])
        .expect("study download list should parse");

    match cli.command {
        Commands::Study {
            cmd: StudyCommand::Download { list, study_id },
        } => {
            assert!(list);
            assert_eq!(study_id, None);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn study_cohort_parses_required_flags() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "study",
        "cohort",
        "--study",
        "brca_tcga_pan_can_atlas_2018",
        "--gene",
        "TP53",
    ])
    .expect("study cohort should parse");

    match cli.command {
        Commands::Study {
            cmd: StudyCommand::Cohort { study, gene },
        } => {
            assert_eq!(study, "brca_tcga_pan_can_atlas_2018");
            assert_eq!(gene, "TP53");
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn study_query_parses_required_flags() {
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
    ])
    .expect("study query should parse");

    match cli.command {
        Commands::Study {
            cmd:
                StudyCommand::Query {
                    study,
                    gene,
                    query_type,
                    ..
                },
        } => {
            assert_eq!(study, "msk_impact_2017");
            assert_eq!(gene, "TP53");
            assert_eq!(query_type, "mutations");
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn study_top_mutated_parses_limit_flag() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "study",
        "top-mutated",
        "--study",
        "msk_impact_2017",
        "--limit",
        "10",
    ])
    .expect("study top-mutated should parse");

    match cli.command {
        Commands::Study {
            cmd: StudyCommand::TopMutated { study, limit },
        } => {
            assert_eq!(study, "msk_impact_2017");
            assert_eq!(limit, 10);
        }
        other => panic!("unexpected command: {other:?}"),
    }
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
        "--terminal",
        "--cols",
        "80",
        "--rows",
        "24",
        "--title",
        "TP53 mutations",
        "--theme",
        "dark",
        "--palette",
        "wong",
    ])
    .expect("study query chart flags should parse");

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
    assert!(chart.terminal);
    assert_eq!(chart.cols, Some(80));
    assert_eq!(chart.rows, Some(24));
    assert_eq!(chart.title.as_deref(), Some("TP53 mutations"));
    assert_eq!(chart.theme.as_deref(), Some("dark"));
    assert_eq!(chart.palette.as_deref(), Some("wong"));
}

#[test]
fn study_query_parses_waterfall_chart_flag() {
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
        "waterfall",
        "--terminal",
    ])
    .expect("study query waterfall chart should parse");

    let Cli {
        command: Commands::Study {
            cmd: StudyCommand::Query { chart, .. },
        },
        ..
    } = cli
    else {
        panic!("expected study query command");
    };

    assert_eq!(chart.chart, Some(ChartType::Waterfall));
    assert!(chart.terminal);
}

#[test]
fn study_co_occurrence_parses_heatmap_chart_flag() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "study",
        "co-occurrence",
        "--study",
        "brca_tcga_pan_can_atlas_2018",
        "--genes",
        "TP53,PIK3CA,GATA3",
        "--chart",
        "heatmap",
        "--terminal",
    ])
    .expect("study co-occurrence heatmap chart should parse");

    let Cli {
        command:
            Commands::Study {
                cmd: StudyCommand::CoOccurrence { chart, .. },
            },
        ..
    } = cli
    else {
        panic!("expected study co-occurrence command");
    };

    assert_eq!(chart.chart, Some(ChartType::Heatmap));
    assert!(chart.terminal);
}

#[test]
fn study_compare_mutations_parses_stacked_bar_chart_flag() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "study",
        "compare",
        "--study",
        "brca_tcga_pan_can_atlas_2018",
        "--gene",
        "TP53",
        "--type",
        "mutations",
        "--target",
        "PIK3CA",
        "--chart",
        "stacked-bar",
        "--terminal",
    ])
    .expect("study compare stacked-bar chart should parse");

    let Cli {
        command: Commands::Study {
            cmd: StudyCommand::Compare { chart, .. },
        },
        ..
    } = cli
    else {
        panic!("expected study compare command");
    };

    assert_eq!(chart.chart, Some(ChartType::StackedBar));
    assert!(chart.terminal);
}

#[test]
fn study_compare_expression_parses_scatter_chart_with_file_dimensions() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "study",
        "compare",
        "--study",
        "brca_tcga_pan_can_atlas_2018",
        "--gene",
        "TP53",
        "--type",
        "expression",
        "--target",
        "ERBB2",
        "--chart",
        "scatter",
        "--width",
        "1200",
        "--height",
        "600",
        "-o",
        "scatter.svg",
    ])
    .expect("study compare scatter chart should parse");

    let Cli {
        command: Commands::Study {
            cmd: StudyCommand::Compare { chart, .. },
        },
        ..
    } = cli
    else {
        panic!("expected study compare command");
    };

    assert_eq!(chart.chart, Some(ChartType::Scatter));
    assert_eq!(chart.width, Some(1200));
    assert_eq!(chart.height, Some(600));
    assert_eq!(
        chart.output.as_deref(),
        Some(std::path::Path::new("scatter.svg"))
    );
}

#[test]
fn study_survival_parses_survival_chart_flag() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "study",
        "survival",
        "--study",
        "brca_tcga_pan_can_atlas_2018",
        "--gene",
        "TP53",
        "--chart",
        "survival",
        "--terminal",
    ])
    .expect("study survival chart flags should parse");

    let Cli {
        command: Commands::Study {
            cmd: StudyCommand::Survival { chart, .. },
        },
        ..
    } = cli
    else {
        panic!("expected study survival command");
    };

    assert_eq!(chart.chart, Some(ChartType::Survival));
    assert!(chart.terminal);
}

#[test]
fn chart_auxiliary_flags_require_chart() {
    let err = Cli::try_parse_from([
        "biomcp",
        "study",
        "query",
        "--study",
        "msk_impact_2017",
        "--gene",
        "TP53",
        "--type",
        "mutations",
        "--terminal",
    ])
    .expect_err("--terminal without --chart should fail");

    assert!(err.to_string().contains("--chart"));
}

#[test]
fn short_help_hides_chart_flags_but_long_help_shows_them() {
    let mut command = Cli::command();
    let study = command
        .find_subcommand_mut("study")
        .expect("study subcommand should exist");
    let query = study
        .find_subcommand_mut("query")
        .expect("study query subcommand should exist");

    let mut short_help = Vec::new();
    query
        .write_help(&mut short_help)
        .expect("short help should render");
    let short_help = String::from_utf8(short_help).expect("short help should be utf-8");
    assert!(!short_help.contains("--theme"));
    assert!(!short_help.contains("--palette"));

    let mut long_help = Vec::new();
    query
        .write_long_help(&mut long_help)
        .expect("long help should render");
    let long_help = String::from_utf8(long_help).expect("long help should be utf-8");
    assert!(long_help.contains("--theme <THEME>"));
    assert!(long_help.contains("--palette <PALETTE>"));
}

#[tokio::test]
async fn handle_command_rejects_invalid_expression_chart() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "study",
        "compare",
        "--study",
        "msk_impact_2017",
        "--gene",
        "TP53",
        "--type",
        "expression",
        "--target",
        "ERBB2",
        "--chart",
        "pie",
        "--terminal",
    ])
    .expect("study compare should parse");

    let Cli {
        command: Commands::Study { cmd },
        json,
        ..
    } = cli
    else {
        panic!("expected study compare command");
    };

    let err = super::handle_command(cmd, json)
        .await
        .expect_err("expression compare should reject pie");
    let msg = err.to_string();
    assert!(msg.contains("study compare --type expression"));
    assert!(msg.contains("box"));
    assert!(msg.contains("violin"));
    assert!(msg.contains("ridgeline"));
    assert!(msg.contains("scatter"));
}

#[test]
fn study_survival_parses_endpoint_flag() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "study",
        "survival",
        "--study",
        "brca_tcga_pan_can_atlas_2018",
        "--gene",
        "TP53",
        "--endpoint",
        "dfs",
    ])
    .expect("study survival should parse");
    match cli.command {
        Commands::Study {
            cmd:
                StudyCommand::Survival {
                    study,
                    gene,
                    endpoint,
                    ..
                },
        } => {
            assert_eq!(study, "brca_tcga_pan_can_atlas_2018");
            assert_eq!(gene, "TP53");
            assert_eq!(endpoint, "dfs");
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn study_compare_parses_type_and_target() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "study",
        "compare",
        "--study",
        "brca_tcga_pan_can_atlas_2018",
        "--gene",
        "TP53",
        "--type",
        "expression",
        "--target",
        "ERBB2",
    ])
    .expect("study compare should parse");
    match cli.command {
        Commands::Study {
            cmd:
                StudyCommand::Compare {
                    study,
                    gene,
                    compare_type,
                    target,
                    ..
                },
        } => {
            assert_eq!(study, "brca_tcga_pan_can_atlas_2018");
            assert_eq!(gene, "TP53");
            assert_eq!(compare_type, "expression");
            assert_eq!(target, "ERBB2");
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn study_filter_parses_all_flags_and_repeated_values() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "study",
        "filter",
        "--study",
        "brca_tcga_pan_can_atlas_2018",
        "--mutated",
        "TP53",
        "--mutated",
        "PIK3CA",
        "--amplified",
        "ERBB2",
        "--deleted",
        "PTEN",
        "--expression-above",
        "MYC:1.5",
        "--expression-above",
        "ERBB2:-0.5",
        "--expression-below",
        "ESR1:0.5",
        "--cancer-type",
        "Breast Cancer",
        "--cancer-type",
        "Lung Cancer",
    ])
    .expect("study filter should parse");
    match cli.command {
        Commands::Study {
            cmd:
                StudyCommand::Filter {
                    study,
                    mutated,
                    amplified,
                    deleted,
                    expression_above,
                    expression_below,
                    cancer_type,
                },
        } => {
            assert_eq!(study, "brca_tcga_pan_can_atlas_2018");
            assert_eq!(mutated, vec!["TP53", "PIK3CA"]);
            assert_eq!(amplified, vec!["ERBB2"]);
            assert_eq!(deleted, vec!["PTEN"]);
            assert_eq!(expression_above, vec!["MYC:1.5", "ERBB2:-0.5"]);
            assert_eq!(expression_below, vec!["ESR1:0.5"]);
            assert_eq!(cancer_type, vec!["Breast Cancer", "Lung Cancer"]);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn study_co_occurrence_parses_gene_list() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "study",
        "co-occurrence",
        "--study",
        "brca_tcga_pan_can_atlas_2018",
        "--genes",
        "TP53,PIK3CA,GATA3",
    ])
    .expect("study co-occurrence should parse");
    match cli.command {
        Commands::Study {
            cmd: StudyCommand::CoOccurrence { study, genes, .. },
        } => {
            assert_eq!(study, "brca_tcga_pan_can_atlas_2018");
            assert_eq!(genes, "TP53,PIK3CA,GATA3");
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[tokio::test]
async fn study_co_occurrence_requires_2_to_10_genes() {
    let err = execute(vec![
        "biomcp".to_string(),
        "study".to_string(),
        "co-occurrence".to_string(),
        "--study".to_string(),
        "msk_impact_2017".to_string(),
        "--genes".to_string(),
        "TP53".to_string(),
    ])
    .await
    .expect_err("study co-occurrence should validate gene count");
    assert!(err.to_string().contains("--genes must contain 2 to 10"));
}

#[tokio::test]
async fn study_filter_requires_at_least_one_criterion() {
    let err = execute(vec![
        "biomcp".to_string(),
        "study".to_string(),
        "filter".to_string(),
        "--study".to_string(),
        "brca_tcga_pan_can_atlas_2018".to_string(),
    ])
    .await
    .expect_err("study filter should require criteria");
    assert!(
        err.to_string()
            .contains("At least one filter criterion is required")
    );
}

#[tokio::test]
async fn study_filter_rejects_malformed_expression_threshold() {
    let err = execute(vec![
        "biomcp".to_string(),
        "study".to_string(),
        "filter".to_string(),
        "--study".to_string(),
        "brca_tcga_pan_can_atlas_2018".to_string(),
        "--expression-above".to_string(),
        "MYC:not-a-number".to_string(),
    ])
    .await
    .expect_err("study filter should validate threshold format");
    assert!(err.to_string().contains("--expression-above"));
    assert!(err.to_string().contains("GENE:THRESHOLD"));
}

#[tokio::test]
async fn study_survival_rejects_unknown_endpoint() {
    let err = execute(vec![
        "biomcp".to_string(),
        "study".to_string(),
        "survival".to_string(),
        "--study".to_string(),
        "msk_impact_2017".to_string(),
        "--gene".to_string(),
        "TP53".to_string(),
        "--endpoint".to_string(),
        "foo".to_string(),
    ])
    .await
    .expect_err("study survival should validate endpoint");
    assert!(err.to_string().contains("Unknown survival endpoint"));
}

#[tokio::test]
async fn study_compare_rejects_unknown_type() {
    let err = execute(vec![
        "biomcp".to_string(),
        "study".to_string(),
        "compare".to_string(),
        "--study".to_string(),
        "msk_impact_2017".to_string(),
        "--gene".to_string(),
        "TP53".to_string(),
        "--type".to_string(),
        "foo".to_string(),
        "--target".to_string(),
        "ERBB2".to_string(),
    ])
    .await
    .expect_err("study compare should validate type");
    assert!(err.to_string().contains("Unknown comparison type"));
}

#[tokio::test]
async fn study_co_occurrence_invalid_chart_lists_heatmap() {
    let err = execute(vec![
        "biomcp".to_string(),
        "study".to_string(),
        "co-occurrence".to_string(),
        "--study".to_string(),
        "msk_impact_2017".to_string(),
        "--genes".to_string(),
        "TP53,KRAS".to_string(),
        "--chart".to_string(),
        "violin".to_string(),
        "--terminal".to_string(),
    ])
    .await
    .expect_err("study co-occurrence should reject violin");
    let msg = err.to_string();
    assert!(msg.contains("study co-occurrence"));
    assert!(msg.contains("bar"));
    assert!(msg.contains("pie"));
    assert!(msg.contains("heatmap"));
}

#[tokio::test]
async fn study_query_mutations_invalid_chart_lists_waterfall() {
    let err = execute(vec![
        "biomcp".to_string(),
        "study".to_string(),
        "query".to_string(),
        "--study".to_string(),
        "msk_impact_2017".to_string(),
        "--gene".to_string(),
        "TP53".to_string(),
        "--type".to_string(),
        "mutations".to_string(),
        "--chart".to_string(),
        "violin".to_string(),
        "--terminal".to_string(),
    ])
    .await
    .expect_err("study query mutations should reject violin");
    let msg = err.to_string();
    assert!(msg.contains("study query --type mutations"));
    assert!(msg.contains("bar"));
    assert!(msg.contains("pie"));
    assert!(msg.contains("waterfall"));
}

#[tokio::test]
async fn study_compare_mutations_invalid_chart_lists_stacked_bar() {
    let err = execute(vec![
        "biomcp".to_string(),
        "study".to_string(),
        "compare".to_string(),
        "--study".to_string(),
        "msk_impact_2017".to_string(),
        "--gene".to_string(),
        "TP53".to_string(),
        "--type".to_string(),
        "mutations".to_string(),
        "--target".to_string(),
        "KRAS".to_string(),
        "--chart".to_string(),
        "violin".to_string(),
        "--terminal".to_string(),
    ])
    .await
    .expect_err("mutation compare should reject violin");
    let msg = err.to_string();
    assert!(msg.contains("study compare --type mutations"));
    assert!(msg.contains("bar"));
    assert!(msg.contains("stacked-bar"));
}
