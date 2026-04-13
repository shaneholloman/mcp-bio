//! Pathway CLI payloads and subcommands.

use clap::{Args, Subcommand};

#[derive(Args, Debug)]
pub struct PathwaySearchArgs {
    /// Free text query (pathway name, process, keyword)
    #[arg(short, long)]
    pub query: Option<String>,
    /// Positional alias for -q/--query; required unless --top-level is present, and multi-word queries must be quoted
    #[arg(value_name = "QUERY")]
    pub positional_query: Option<String>,
    /// Entity type filter (e.g., pathway)
    #[arg(long = "type")]
    pub pathway_type: Option<String>,
    /// Include top-level pathways
    #[arg(long = "top-level")]
    pub top_level: bool,
    /// Maximum results (default: 10)
    #[arg(short, long, default_value = "10")]
    pub limit: usize,
    /// Skip the first N results
    #[arg(long, default_value = "0")]
    pub offset: usize,
}

#[derive(Args, Debug)]
pub struct PathwayGetArgs {
    /// Pathway ID (e.g., R-HSA-5673001, hsa05200)
    pub id: String,
    /// Sections to include (genes, events (Reactome only), enrichment (Reactome only), all = all sections available for the resolved source)
    #[arg(trailing_var_arg = true)]
    pub sections: Vec<String>,
}

#[derive(Subcommand, Debug)]
pub enum PathwayCommand {
    /// Search drugs linked to genes in this pathway (best-effort)
    #[command(after_help = "\
EXAMPLES:
  biomcp pathway drugs R-HSA-5673001 --limit 5
  biomcp pathway drugs hsa05200 --limit 5
  biomcp pathway drugs R-HSA-6802957 --limit 5

Note: Searches free-text fields (e.g., eligibility criteria). Results depend on source document wording.
See also: biomcp list pathway")]
    Drugs {
        /// Pathway ID (e.g., R-HSA-5673001, hsa05200)
        id: String,
        /// Maximum results (default: 10)
        #[arg(short, long, default_value = "10")]
        limit: usize,
        /// Skip the first N results
        #[arg(long, default_value = "0")]
        offset: usize,
    },
    /// Search articles linked to this pathway (best-effort)
    #[command(after_help = "\
EXAMPLES:
  biomcp pathway articles R-HSA-5673001 --limit 5
  biomcp pathway articles hsa05200 --limit 5
  biomcp pathway articles R-HSA-6802957 --limit 5

Note: Searches free-text fields (e.g., eligibility criteria). Results depend on source document wording.
See also: biomcp list pathway")]
    Articles {
        /// Pathway ID (e.g., R-HSA-5673001, hsa05200)
        id: String,
        /// Maximum results (default: 10)
        #[arg(short, long, default_value = "10")]
        limit: usize,
        /// Skip the first N results
        #[arg(long, default_value = "0")]
        offset: usize,
    },
    /// Search trials linked to this pathway (best-effort)
    #[command(after_help = "\
EXAMPLES:
  biomcp pathway trials R-HSA-5673001 --limit 5
  biomcp pathway trials hsa05200 --limit 5
  biomcp pathway trials R-HSA-5673001 --source nci --limit 5

Note: Searches free-text fields (e.g., eligibility criteria). Results depend on source document wording.
See also: biomcp list pathway")]
    Trials {
        /// Pathway ID (e.g., R-HSA-5673001, hsa05200)
        id: String,
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
}

mod dispatch;
pub(super) use self::dispatch::{handle_command, handle_get, handle_search};

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser};

    use super::PathwayCommand;
    use crate::cli::{Cli, Commands};

    fn render_pathway_search_long_help() -> String {
        let mut command = Cli::command();
        let search = command
            .find_subcommand_mut("search")
            .expect("search subcommand should exist");
        let pathway = search
            .find_subcommand_mut("pathway")
            .expect("pathway subcommand should exist");
        let mut help = Vec::new();
        pathway
            .write_long_help(&mut help)
            .expect("pathway help should render");
        String::from_utf8(help).expect("help should be utf-8")
    }

    #[test]
    fn search_pathway_help_describes_conditional_query_contract() {
        let help = render_pathway_search_long_help();

        assert!(help.contains("biomcp search pathway [OPTIONS] <QUERY>"));
        assert!(help.contains("biomcp search pathway [OPTIONS] --top-level [QUERY]"));
        assert!(help.contains("required unless --top-level is present"));
        assert!(help.contains("multi-word queries must be quoted"));
        assert!(help.contains("biomcp search pathway --top-level --limit 5"));
    }

    #[test]
    fn pathway_help_describes_source_aware_section_contract() {
        let mut command = Cli::command();
        let get = command
            .find_subcommand_mut("get")
            .expect("get subcommand should exist");
        let pathway = get
            .find_subcommand_mut("pathway")
            .expect("pathway subcommand should exist");
        let mut help = Vec::new();
        pathway
            .write_long_help(&mut help)
            .expect("pathway help should render");
        let help = String::from_utf8(help).expect("help should be utf-8");

        assert!(help.contains("events (Reactome only)"));
        assert!(help.contains("enrichment (Reactome only)"));
        assert!(help.contains("all = all sections available for the resolved source"));
        assert!(help.contains("biomcp get pathway R-HSA-5673001 events"));
        assert!(!help.contains("biomcp get pathway hsa05200 enrichment"));
    }

    #[test]
    fn pathway_trials_parse_source_and_limit() {
        let cli = Cli::try_parse_from([
            "biomcp",
            "pathway",
            "trials",
            "R-HSA-5673001",
            "--source",
            "nci",
            "--limit",
            "2",
        ])
        .expect("pathway trials should parse");

        match cli.command {
            Commands::Pathway {
                cmd:
                    PathwayCommand::Trials {
                        id,
                        limit,
                        offset,
                        source,
                    },
            } => {
                assert_eq!(id, "R-HSA-5673001");
                assert_eq!(limit, 2);
                assert_eq!(offset, 0);
                assert_eq!(source, "nci");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[tokio::test]
    async fn handle_command_rejects_zero_limit_before_related_lookup() {
        let cli = Cli::try_parse_from([
            "biomcp",
            "pathway",
            "drugs",
            "R-HSA-5673001",
            "--limit",
            "0",
        ])
        .expect("pathway drugs should parse");

        let Cli {
            command: Commands::Pathway { cmd },
            json,
            ..
        } = cli
        else {
            panic!("expected pathway command");
        };

        let err = super::handle_command(cmd, json)
            .await
            .expect_err("zero pathway drugs limit should fail fast");
        assert!(err.to_string().contains("--limit must be between 1 and 50"));
    }
}
