use clap::Parser;

use crate::cli::{Cli, Commands, SearchEntity};

#[test]
fn search_pgx_parses_positional_query() {
    let cli = Cli::try_parse_from(["biomcp", "search", "pgx", "CYP2D6", "--limit", "2"])
        .expect("search pgx should parse");

    let Cli {
        command:
            Commands::Search {
                entity:
                    SearchEntity::Pgx(crate::cli::pgx::PgxSearchArgs {
                        gene,
                        positional_query,
                        drug,
                        cpic_level,
                        pgx_testing,
                        evidence,
                        limit,
                        offset,
                    }),
            },
        ..
    } = cli
    else {
        panic!("expected search pgx command");
    };

    assert_eq!(gene, None);
    assert_eq!(positional_query.as_deref(), Some("CYP2D6"));
    assert_eq!(drug, None);
    assert_eq!(cpic_level, None);
    assert_eq!(pgx_testing, None);
    assert_eq!(evidence, None);
    assert_eq!(limit, 2);
    assert_eq!(offset, 0);
}

#[tokio::test]
async fn handle_search_rejects_zero_limit_before_backend_lookup() {
    let cli = Cli::try_parse_from(["biomcp", "search", "pgx", "CYP2D6", "--limit", "0"])
        .expect("search pgx should parse");

    let Cli {
        command: Commands::Search {
            entity: SearchEntity::Pgx(args),
        },
        json,
        ..
    } = cli
    else {
        panic!("expected search pgx command");
    };

    let err = super::handle_search(args, json)
        .await
        .expect_err("zero pgx limit should fail fast");
    assert!(err.to_string().contains("--limit must be between 1 and 50"));
}
