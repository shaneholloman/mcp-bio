use super::*;

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
