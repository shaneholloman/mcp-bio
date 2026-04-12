//! Protein CLI payloads and subcommands.

use clap::{Args, Subcommand};

#[derive(Args, Debug)]
pub struct ProteinSearchArgs {
    /// Free text query (protein name, accession, keyword)
    #[arg(short, long)]
    pub query: Option<String>,
    /// Optional positional query alias for -q/--query
    #[arg(value_name = "QUERY")]
    pub positional_query: Option<String>,
    /// Include all species (default: off, human-only)
    #[arg(long)]
    pub all_species: bool,
    /// Restrict to reviewed entries
    #[arg(long)]
    pub reviewed: bool,
    /// Filter by disease text
    #[arg(long)]
    pub disease: Option<String>,
    /// Filter by protein existence level (1-5)
    #[arg(long)]
    pub existence: Option<u8>,
    /// Maximum results (default: 10)
    #[arg(short, long, default_value = "10")]
    pub limit: usize,
    /// Skip the first N results
    #[arg(long, default_value = "0")]
    pub offset: usize,
    /// Cursor token from a previous response
    #[arg(long = "next-page")]
    pub next_page: Option<String>,
}

#[derive(Args, Debug)]
pub struct ProteinGetArgs {
    /// UniProt accession or HGNC symbol (e.g., P15056 or BRAF)
    pub accession: String,
    /// Sections to include (domains, interactions, complexes, structures, all)
    #[arg(trailing_var_arg = true)]
    pub sections: Vec<String>,
}

#[derive(Subcommand, Debug)]
pub enum ProteinCommand {
    /// Show protein structural identifiers
    #[command(after_help = "\
EXAMPLES:
  biomcp protein structures P15056
  biomcp protein structures P15056 --limit 25 --offset 5

See also: biomcp list protein")]
    Structures {
        /// UniProt accession or HGNC symbol (e.g., P15056 or BRAF)
        accession: String,
        /// Maximum structures to show (default: 10)
        #[arg(short, long, default_value = "10")]
        limit: usize,
        /// Skip the first N results
        #[arg(long, default_value = "0")]
        offset: usize,
    },
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::ProteinCommand;
    use crate::cli::{Cli, Commands};

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
}
