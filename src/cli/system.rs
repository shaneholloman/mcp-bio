//! Top-level CLI payloads and subcommands that stay outside the per-entity families.

use clap::{Args, Subcommand};

#[derive(Args, Debug)]
pub struct HealthArgs {
    /// Check external APIs only
    #[arg(long)]
    pub apis_only: bool,
}

#[derive(Subcommand, Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmaCommand {
    /// Force refresh the EMA local data feeds
    Sync,
}

#[derive(Subcommand, Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhoCommand {
    /// Force refresh the WHO Prequalification local CSV
    Sync,
}

#[derive(Args, Debug)]
pub struct ServeHttpArgs {
    /// Host address to bind
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
    /// Port to listen on
    #[arg(long, default_value = "8080")]
    pub port: u16,
}

#[derive(Args, Debug)]
pub struct UpdateArgs {
    /// Check for updates, but do not install
    #[arg(long)]
    pub check: bool,
}

#[derive(Args, Debug)]
pub struct ListArgs {
    /// Optional entity name (gene, variant, article, trial, drug, disease, pgx, gwas, pathway, protein, study, adverse-event, search-all)
    pub entity: Option<String>,
}

#[derive(Args, Debug)]
pub struct BatchArgs {
    /// Entity type (gene, variant, article, trial, drug, disease, pgx, pathway, protein, adverse-event)
    pub entity: String,
    /// Comma-separated IDs (max 10)
    pub ids: String,
    /// Optional comma-separated sections to request on each get call
    #[arg(long)]
    pub sections: Option<String>,
    /// Trial source when entity=trial (ctgov or nci)
    #[arg(long, default_value = "ctgov")]
    pub source: String,
}

#[derive(Args, Debug)]
pub struct EnrichArgs {
    /// Comma-separated HGNC symbols (e.g., BRAF,KRAS,NRAS)
    pub genes: String,
    /// Maximum enrichment terms (default: 10)
    #[arg(short, long, default_value = "10")]
    pub limit: usize,
}

#[derive(Args, Debug)]
pub struct DiscoverArgs {
    /// Free-text biomedical query
    pub query: String,
}

#[derive(Args, Debug)]
pub struct VersionArgs {
    /// Include executable provenance and PATH diagnostics
    #[arg(long)]
    pub verbose: bool,
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, FromArgMatches, Parser};

    use super::EmaCommand;
    use super::WhoCommand;
    use crate::cli::{Cli, Commands};

    fn parse_built_cli<I, T>(args: I) -> Cli
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        let matches = crate::cli::build_cli()
            .try_get_matches_from(args)
            .expect("args should parse with canonical CLI");
        Cli::from_arg_matches(&matches).expect("matches should decode into Cli")
    }

    #[test]
    fn ema_sync_parses_subcommand() {
        let cli = parse_built_cli(["biomcp", "ema", "sync"]);
        assert!(matches!(
            cli.command,
            Commands::Ema {
                cmd: EmaCommand::Sync
            }
        ));
    }

    #[test]
    fn ema_help_mentions_sync_example() {
        let mut command = Cli::command();
        let ema = command
            .find_subcommand_mut("ema")
            .expect("ema subcommand should exist");
        let mut help = Vec::new();
        ema.write_long_help(&mut help)
            .expect("ema help should render");
        let help = String::from_utf8(help).expect("help should be utf-8");

        assert!(help.contains("biomcp ema sync"));
    }

    #[test]
    fn who_sync_parses_subcommand() {
        let cli = parse_built_cli(["biomcp", "who", "sync"]);
        assert!(matches!(
            cli.command,
            Commands::Who {
                cmd: WhoCommand::Sync
            }
        ));
    }

    #[test]
    fn who_help_mentions_sync_example() {
        let mut command = Cli::command();
        let who = command
            .find_subcommand_mut("who")
            .expect("who subcommand should exist");
        let mut help = Vec::new();
        who.write_long_help(&mut help)
            .expect("who help should render");
        let help = String::from_utf8(help).expect("help should be utf-8");

        assert!(help.contains("biomcp who sync"));
    }

    #[test]
    fn discover_help_includes_when_to_use_guidance() {
        let mut command = Cli::command();
        let discover = command
            .find_subcommand_mut("discover")
            .expect("discover subcommand should exist");
        let mut help = Vec::new();
        discover
            .write_long_help(&mut help)
            .expect("discover help should render");
        let help = String::from_utf8(help).expect("help should be utf-8");

        assert!(help.contains("When to use:"));
        assert!(help.contains("only have free text"));
        assert!(help.contains("pick the next typed command"));
    }

    #[test]
    fn discover_top_level_command_parses_query() {
        let cli = Cli::try_parse_from(["biomcp", "discover", "ERBB1"]).expect("parse");

        let Cli {
            command: Commands::Discover(crate::cli::system::DiscoverArgs { query }),
            ..
        } = cli
        else {
            panic!("expected discover command");
        };

        assert_eq!(query, "ERBB1");
    }

    #[test]
    fn health_command_parses_apis_only() {
        let cli =
            Cli::try_parse_from(["biomcp", "health", "--apis-only"]).expect("health should parse");

        assert!(matches!(
            cli.command,
            Commands::Health(crate::cli::system::HealthArgs { apis_only: true })
        ));
    }

    #[test]
    fn list_command_parses_entity_name() {
        let cli = Cli::try_parse_from(["biomcp", "list", "drug"]).expect("list should parse");

        let Cli {
            command: Commands::List(crate::cli::system::ListArgs { entity }),
            ..
        } = cli
        else {
            panic!("expected list command");
        };

        assert_eq!(entity.as_deref(), Some("drug"));
    }

    #[test]
    fn batch_command_parses_sections_and_source() {
        let cli = Cli::try_parse_from([
            "biomcp",
            "batch",
            "trial",
            "NCT02576665,NCT02693535",
            "--sections",
            "eligibility,locations",
            "--source",
            "nci",
        ])
        .expect("batch should parse");

        let Cli {
            command:
                Commands::Batch(crate::cli::system::BatchArgs {
                    entity,
                    ids,
                    sections,
                    source,
                }),
            ..
        } = cli
        else {
            panic!("expected batch command");
        };

        assert_eq!(entity, "trial");
        assert_eq!(ids, "NCT02576665,NCT02693535");
        assert_eq!(sections.as_deref(), Some("eligibility,locations"));
        assert_eq!(source, "nci");
    }

    #[test]
    fn enrich_command_parses_limit() {
        let cli = Cli::try_parse_from(["biomcp", "enrich", "BRAF,KRAS", "--limit", "5"])
            .expect("enrich should parse");

        let Cli {
            command: Commands::Enrich(crate::cli::system::EnrichArgs { genes, limit }),
            ..
        } = cli
        else {
            panic!("expected enrich command");
        };

        assert_eq!(genes, "BRAF,KRAS");
        assert_eq!(limit, 5);
    }

    #[test]
    fn version_command_parses_verbose_flag() {
        let cli =
            Cli::try_parse_from(["biomcp", "version", "--verbose"]).expect("version should parse");

        assert!(matches!(
            cli.command,
            Commands::Version(crate::cli::system::VersionArgs { verbose: true })
        ));
    }

    #[test]
    fn serve_http_help_describes_streamable_http() {
        let mut command = crate::cli::build_cli();
        let serve_http = command
            .find_subcommand_mut("serve-http")
            .expect("serve-http subcommand should exist");
        let mut help = Vec::new();
        serve_http
            .write_long_help(&mut help)
            .expect("serve-http help should render");
        let help = String::from_utf8(help).expect("help should be utf-8");

        assert!(help.contains("Streamable HTTP"));
        assert!(help.contains("/mcp"));
        assert!(help.contains("--host <HOST>"));
        assert!(help.contains("--port <PORT>"));
        assert!(!help.contains("SSE transport"));
        assert!(!help.contains("--json"));
        assert!(!help.contains("--no-cache"));
    }
}
