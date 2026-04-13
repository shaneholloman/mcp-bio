use clap::Parser;

use crate::cli::{Cli, Commands, GetEntity, OutputStream, SearchEntity, VariantCommand};

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

#[tokio::test]
async fn handle_get_returns_guidance_json_for_shorthand_variant() {
    let cli = Cli::try_parse_from(["biomcp", "--json", "get", "variant", "R620W"]).expect("parse");

    let Cli {
        command: Commands::Get {
            entity: GetEntity::Variant(args),
        },
        json,
        ..
    } = cli
    else {
        panic!("expected get variant command");
    };

    let outcome = super::handle_get(args, json, false)
        .await
        .expect("guidance outcome");

    assert_eq!(outcome.stream, OutputStream::Stdout);
    assert_eq!(outcome.exit_code, 1);
    let value: serde_json::Value =
        serde_json::from_str(&outcome.text).expect("valid variant guidance json");
    assert_eq!(
        value["_meta"]["alias_resolution"]["kind"],
        "protein_change_only"
    );
    assert_eq!(
        value["_meta"]["next_commands"][0],
        "biomcp search variant --hgvsp R620W --limit 10"
    );
}
