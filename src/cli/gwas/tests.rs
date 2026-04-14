use clap::Parser;

use crate::cli::{Cli, Commands, SearchEntity};

#[test]
fn search_gwas_parses_positional_query() {
    let cli = Cli::try_parse_from(["biomcp", "search", "gwas", "BRAF", "--limit", "2"])
        .expect("search gwas should parse");

    let Cli {
        command:
            Commands::Search {
                entity:
                    SearchEntity::Gwas(crate::cli::gwas::GwasSearchArgs {
                        gene,
                        positional_query,
                        trait_query,
                        region,
                        p_value,
                        limit,
                        offset,
                    }),
            },
        ..
    } = cli
    else {
        panic!("expected search gwas command");
    };

    assert_eq!(gene, None);
    assert_eq!(positional_query.as_deref(), Some("BRAF"));
    assert_eq!(trait_query, None);
    assert_eq!(region, None);
    assert_eq!(p_value, None);
    assert_eq!(limit, 2);
    assert_eq!(offset, 0);
}

#[tokio::test]
async fn handle_search_rejects_zero_limit_before_backend_lookup() {
    let cli = Cli::try_parse_from(["biomcp", "search", "gwas", "BRAF", "--limit", "0"])
        .expect("search gwas should parse");

    let Cli {
        command: Commands::Search {
            entity: SearchEntity::Gwas(args),
        },
        json,
        ..
    } = cli
    else {
        panic!("expected search gwas command");
    };

    let err = super::handle_search(args, json)
        .await
        .expect_err("zero gwas limit should fail fast");
    assert!(err.to_string().contains("--limit must be between 1 and 50"));
}
