//! Disease CLI payloads and subcommands.

use clap::{Args, Subcommand};

#[derive(Args, Debug)]
pub struct DiseaseSearchArgs {
    /// Free text query (disease name or keyword)
    #[arg(short, long)]
    pub query: Option<String>,
    /// Optional positional query alias for -q/--query
    #[arg(value_name = "QUERY")]
    pub positional_query: Option<String>,
    /// Restrict results by ontology source (mondo, doid, mesh)
    #[arg(long)]
    pub source: Option<String>,
    /// Filter by inheritance pattern
    #[arg(long)]
    pub inheritance: Option<String>,
    /// Filter by phenotype term (e.g., HP:0001250)
    #[arg(long)]
    pub phenotype: Option<String>,
    /// Filter by clinical onset period
    #[arg(long)]
    pub onset: Option<String>,
    /// Disable automatic discover fallback when zero direct disease rows are found
    #[arg(long)]
    pub no_fallback: bool,
    /// Maximum results (default: 10)
    #[arg(short, long, default_value = "10")]
    pub limit: usize,
    /// Skip the first N results
    #[arg(long, default_value = "0")]
    pub offset: usize,
}

#[derive(Args, Debug)]
pub struct DiseaseGetArgs {
    /// Disease name (e.g., melanoma) or ID (e.g., MONDO:0005105)
    pub name_or_id: String,
    /// Sections to include (genes, pathways, phenotypes, variants, models, prevalence, survival, civic, disgenet, funding, all)
    #[arg(trailing_var_arg = true)]
    pub sections: Vec<String>,
}

#[derive(Subcommand, Debug)]
pub enum DiseaseCommand {
    /// Search trials for this disease (best-effort)
    #[command(after_help = "\
EXAMPLES:
  biomcp disease trials melanoma --limit 5
  biomcp disease trials \"lung cancer\" --source nci --limit 5

Note: Searches free-text fields (e.g., eligibility criteria). Results depend on source document wording.
See also: biomcp list disease")]
    Trials {
        /// Disease name (e.g., melanoma)
        name: String,
        /// Maximum results (default: 10)
        #[arg(short, long, default_value = "10")]
        limit: usize,
        /// Skip the first N results
        #[arg(long, default_value = "0")]
        offset: usize,
        /// Trial data source (ctgov or nci)
        #[arg(long, default_value = "ctgov")]
        source: String,
    },
    /// Search articles for this disease (best-effort)
    #[command(after_help = "\
EXAMPLES:
  biomcp disease articles melanoma --limit 5
  biomcp disease articles \"glioblastoma\" --limit 5

Note: Searches free-text fields (e.g., eligibility criteria). Results depend on source document wording.
See also: biomcp list disease")]
    Articles {
        /// Disease name (e.g., melanoma)
        name: String,
        /// Maximum results (default: 10)
        #[arg(short, long, default_value = "10")]
        limit: usize,
        /// Skip the first N results
        #[arg(long, default_value = "0")]
        offset: usize,
    },
    /// Search drugs with this disease as an indication (best-effort)
    #[command(after_help = "\
EXAMPLES:
  biomcp disease drugs melanoma --limit 5
  biomcp disease drugs \"breast cancer\" --limit 5

Note: Searches free-text fields (e.g., eligibility criteria). Results depend on source document wording.
See also: biomcp list disease")]
    Drugs {
        /// Disease name (e.g., melanoma)
        name: String,
        /// Maximum results (default: 10)
        #[arg(short, long, default_value = "10")]
        limit: usize,
        /// Skip the first N results
        #[arg(long, default_value = "0")]
        offset: usize,
    },
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser};

    use super::DiseaseCommand;
    use crate::cli::{Cli, Commands};

    fn render_disease_get_long_help() -> String {
        let mut command = Cli::command();
        let get = command
            .find_subcommand_mut("get")
            .expect("get subcommand should exist");
        let disease = get
            .find_subcommand_mut("disease")
            .expect("disease get subcommand should exist");
        let mut help = Vec::new();
        disease
            .write_long_help(&mut help)
            .expect("disease help should render");
        String::from_utf8(help).expect("help should be utf-8")
    }

    #[test]
    fn get_disease_help_includes_when_to_use_guidance() {
        let help = render_disease_get_long_help();

        assert!(help.contains("When to use:"));
        assert!(help.contains("normalized disease card"));
        assert!(help.contains("funding or survival"));
        assert!(help.contains("search article -d"));
    }

    #[test]
    fn disease_trials_parses_source_and_limit() {
        let cli = Cli::try_parse_from([
            "biomcp", "disease", "trials", "melanoma", "--source", "nci", "--limit", "2",
        ])
        .expect("disease trials should parse");

        match cli.command {
            Commands::Disease {
                cmd:
                    DiseaseCommand::Trials {
                        name,
                        limit,
                        offset,
                        source,
                    },
            } => {
                assert_eq!(name, "melanoma");
                assert_eq!(limit, 2);
                assert_eq!(offset, 0);
                assert_eq!(source, "nci");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }
}
