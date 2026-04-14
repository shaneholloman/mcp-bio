use clap::Parser;

use super::ProteinCommand;
use crate::cli::{Cli, Commands, SearchEntity};

#[test]
fn protein_structures_parses_offset_flag() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "protein",
        "structures",
        "P15056",
        "--limit",
        "5",
        "--offset",
        "5",
    ])
    .expect("protein structures pagination flags should parse");

    match cli.command {
        Commands::Protein {
            cmd:
                ProteinCommand::Structures {
                    accession,
                    limit,
                    offset,
                },
        } => {
            assert_eq!(accession, "P15056");
            assert_eq!(limit, 5);
            assert_eq!(offset, 5);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[tokio::test]
async fn handle_search_rejects_next_page_with_offset() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "search",
        "protein",
        "BRAF",
        "--next-page",
        "cursor-1",
        "--offset",
        "1",
    ])
    .expect("protein search should parse");

    let Cli {
        command: Commands::Search {
            entity: SearchEntity::Protein(args),
        },
        json,
        ..
    } = cli
    else {
        panic!("expected protein search command");
    };

    let err = super::handle_search(args, json)
        .await
        .expect_err("next-page plus offset should fail fast");
    assert!(
        err.to_string()
            .contains("--next-page cannot be used together with --offset")
    );
}
