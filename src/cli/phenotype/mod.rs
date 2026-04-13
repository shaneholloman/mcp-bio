//! Phenotype CLI payloads.

use clap::Args;

#[derive(Args, Debug)]
pub struct PhenotypeSearchArgs {
    /// HPO IDs (space- or comma-separated) or one symptom phrase / comma-separated symptom phrases
    pub terms: String,
    /// Maximum results (default: 10)
    #[arg(short, long, default_value = "10")]
    pub limit: usize,
    /// Skip the first N results
    #[arg(long, default_value = "0")]
    pub offset: usize,
}

mod dispatch;
pub(super) use self::dispatch::handle_search;

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser};

    use crate::cli::{Cli, Commands, SearchEntity};

    #[test]
    fn search_phenotype_help_mentions_hpo_ids_and_symptom_phrases() {
        let mut command = Cli::command();
        let search = command
            .find_subcommand_mut("search")
            .expect("search subcommand should exist");
        let phenotype = search
            .find_subcommand_mut("phenotype")
            .expect("phenotype subcommand should exist");
        let mut help = Vec::new();
        phenotype
            .write_long_help(&mut help)
            .expect("phenotype help should render");
        let help = String::from_utf8(help).expect("help should be utf-8");

        assert!(help.contains("HPO IDs"));
        assert!(help.contains("space- or comma-separated"));
        assert!(help.contains("one symptom phrase"));
        assert!(help.contains("comma-separated symptom phrases"));
        assert!(help.contains("seizure, developmental delay"));
        assert!(help.contains("biomcp list phenotype"));
    }

    #[tokio::test]
    async fn handle_search_rejects_zero_limit_before_backend_lookup() {
        let cli = Cli::try_parse_from(["biomcp", "search", "phenotype", "seizure", "--limit", "0"])
            .expect("search phenotype should parse");

        let Cli {
            command:
                Commands::Search {
                    entity: SearchEntity::Phenotype(args),
                },
            json,
            ..
        } = cli
        else {
            panic!("expected search phenotype command");
        };

        let err = super::handle_search(args, json)
            .await
            .expect_err("zero phenotype limit should fail fast");
        assert!(err.to_string().contains("--limit must be between 1 and 50"));
    }
}
