use clap::{CommandFactory, Parser};

use super::GeneCommand;
use crate::cli::test_support::{
    MockServer, lock_env, mount_gene_lookup_miss, mount_ols_alias, set_env_var,
};
use crate::cli::{Cli, Commands, GetEntity, OutputStream};

fn render_gene_get_long_help() -> String {
    let mut command = Cli::command();
    let get = command
        .find_subcommand_mut("get")
        .expect("get subcommand should exist");
    let gene = get
        .find_subcommand_mut("gene")
        .expect("gene get subcommand should exist");
    let mut help = Vec::new();
    gene.write_long_help(&mut help)
        .expect("gene help should render");
    String::from_utf8(help).expect("help should be utf-8")
}

#[test]
fn get_gene_help_includes_when_to_use_guidance() {
    let help = render_gene_get_long_help();

    assert!(help.contains("When to use:"));
    assert!(help.contains("default card"));
    assert!(help.contains("protein, hpa, expression, diseases, or funding"));
    assert!(help.contains("ERBB2 funding"));
}

#[test]
fn gene_get_alias_parses_as_definition_subcommand() {
    let cli = Cli::try_parse_from(["biomcp", "gene", "get", "BRAF"])
        .expect("gene get alias should parse");

    match cli.command {
        Commands::Gene {
            cmd: GeneCommand::Definition { symbol },
        } => assert_eq!(symbol, "BRAF"),
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn gene_bare_symbol_parses_as_external_subcommand() {
    let cli =
        Cli::try_parse_from(["biomcp", "gene", "BRAF"]).expect("bare gene symbol should parse");

    match cli.command {
        Commands::Gene {
            cmd: GeneCommand::External(args),
        } => assert_eq!(args, vec!["BRAF"]),
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn gene_pathways_parses_limit_and_offset() {
    let cli = Cli::try_parse_from([
        "biomcp", "gene", "pathways", "BRAF", "--limit", "5", "--offset", "1",
    ])
    .expect("gene pathways pagination flags should parse");

    match cli.command {
        Commands::Gene {
            cmd:
                GeneCommand::Pathways {
                    symbol,
                    limit,
                    offset,
                },
        } => {
            assert_eq!(symbol, "BRAF");
            assert_eq!(limit, 5);
            assert_eq!(offset, 1);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[tokio::test]
async fn handle_get_gene_alias_fallback_returns_markdown_suggestion() {
    let _guard = lock_env().await;
    let mygene = MockServer::start().await;
    let ols = MockServer::start().await;
    let _mygene_base = set_env_var("BIOMCP_MYGENE_BASE", Some(&format!("{}/v3", mygene.uri())));
    let _ols_base = set_env_var("BIOMCP_OLS4_BASE", Some(&ols.uri()));
    let _umls_base = set_env_var("BIOMCP_UMLS_BASE", None);
    let _umls_key = set_env_var("UMLS_API_KEY", None);

    mount_gene_lookup_miss(&mygene, "ERBB1").await;
    mount_ols_alias(&ols, "ERBB1", "hgnc", "HGNC:3236", "EGFR", &["ERBB1"], 1).await;

    let cli = Cli::try_parse_from(["biomcp", "get", "gene", "ERBB1"]).expect("parse");

    let Cli {
        command: Commands::Get {
            entity: GetEntity::Gene(args),
        },
        json,
        ..
    } = cli
    else {
        panic!("expected get gene command");
    };

    let outcome = super::handle_get(args, json, false)
        .await
        .expect("alias outcome");

    assert_eq!(outcome.stream, OutputStream::Stderr);
    assert_eq!(outcome.exit_code, 1);
    assert!(outcome.text.contains("Error: gene 'ERBB1' not found."));
    assert!(
        outcome
            .text
            .contains("Did you mean: `biomcp get gene EGFR`")
    );
}
