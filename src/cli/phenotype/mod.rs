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
    use clap::CommandFactory;

    use crate::cli::Cli;

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
}
