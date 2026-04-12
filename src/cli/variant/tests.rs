use clap::Parser;

use crate::cli::{Cli, Commands, SearchEntity, VariantCommand};

#[test]
fn search_variant_parses_single_token_positional_query() {
    let cli = Cli::try_parse_from(["biomcp", "search", "variant", "BRAF", "--limit", "2"])
        .expect("search variant should parse");

    let Cli {
        command:
            Commands::Search {
                entity:
                    SearchEntity::Variant(crate::cli::variant::VariantSearchArgs {
                        gene,
                        positional_query,
                        limit,
                        offset,
                        ..
                    }),
            },
        ..
    } = cli
    else {
        panic!("expected search variant command");
    };

    assert_eq!(gene, None);
    assert_eq!(positional_query, vec!["BRAF".to_string()]);
    assert_eq!(limit, 2);
    assert_eq!(offset, 0);
}

#[test]
fn search_variant_parses_multi_token_positional_query_and_flag() {
    let cli = Cli::try_parse_from([
        "biomcp", "search", "variant", "-g", "PTPN22", "R620W", "--limit", "5",
    ])
    .expect("search variant should parse");

    let Cli {
        command:
            Commands::Search {
                entity:
                    SearchEntity::Variant(crate::cli::variant::VariantSearchArgs {
                        gene,
                        positional_query,
                        limit,
                        ..
                    }),
            },
        ..
    } = cli
    else {
        panic!("expected search variant command");
    };

    assert_eq!(gene.as_deref(), Some("PTPN22"));
    assert_eq!(positional_query, vec!["R620W".to_string()]);
    assert_eq!(limit, 5);
}

#[test]
fn search_variant_parses_quoted_gene_change_positional_query() {
    let cli = Cli::try_parse_from(["biomcp", "search", "variant", "BRAF V600E", "--limit", "5"])
        .expect("search variant should parse");

    let Cli {
        command:
            Commands::Search {
                entity:
                    SearchEntity::Variant(crate::cli::variant::VariantSearchArgs {
                        positional_query,
                        limit,
                        ..
                    }),
            },
        ..
    } = cli
    else {
        panic!("expected search variant command");
    };

    assert_eq!(positional_query, vec!["BRAF V600E".to_string()]);
    assert_eq!(limit, 5);
}

#[test]
fn variant_bare_id_parses_as_external_subcommand() {
    let cli = Cli::try_parse_from(["biomcp", "variant", "BRAF V600E"])
        .expect("bare variant id should parse");

    match cli.command {
        Commands::Variant {
            cmd: VariantCommand::External(args),
        } => assert_eq!(args, vec!["BRAF V600E"]),
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn variant_trials_parses_source_flag() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "variant",
        "trials",
        "BRAF V600E",
        "--source",
        "nci",
        "--limit",
        "3",
    ])
    .expect("variant trials with --source should parse");

    match cli.command {
        Commands::Variant {
            cmd:
                VariantCommand::Trials {
                    source,
                    limit,
                    offset,
                    ..
                },
        } => {
            assert_eq!(source, "nci");
            assert_eq!(limit, 3);
            assert_eq!(offset, 0);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}
