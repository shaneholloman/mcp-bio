//! Search-all CLI payloads for the cross-entity routing card.

use clap::Args;

#[derive(Args, Debug)]
pub struct SearchAllArgs {
    /// Gene slot (e.g., BRAF)
    #[arg(short = 'g', long)]
    pub gene: Option<String>,
    /// Variant slot (e.g., "BRAF V600E")
    #[arg(short = 'v', long)]
    pub variant: Option<String>,
    /// Disease slot (e.g., melanoma)
    #[arg(short = 'd', long)]
    pub disease: Option<String>,
    /// Drug slot (e.g., dabrafenib)
    #[arg(long)]
    pub drug: Option<String>,
    /// Keyword slot
    #[arg(short = 'k', long)]
    pub keyword: Option<String>,
    /// Optional positional query alias for -k/--keyword
    #[arg(value_name = "QUERY")]
    pub positional_query: Option<String>,
    /// Date lower bound for date-capable sections (YYYY, YYYY-MM, or YYYY-MM-DD)
    #[arg(long)]
    pub since: Option<String>,
    /// Maximum rows per section (default: 3)
    #[arg(short, long, default_value = "3")]
    pub limit: usize,
    /// Render counts per section only (skip section rows)
    #[arg(long = "counts-only")]
    pub counts_only: bool,
    /// Include the executed multi-leg routing plan in markdown or JSON output
    #[arg(long = "debug-plan")]
    pub debug_plan: bool,
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::cli::{Cli, Commands, SearchEntity};

    #[test]
    fn search_all_parses_slot_flags() {
        let cli = Cli::try_parse_from([
            "biomcp",
            "search",
            "all",
            "--gene",
            "BRAF",
            "--disease",
            "melanoma",
            "--limit",
            "2",
        ])
        .expect("search all should parse");

        let Cli {
            command:
                Commands::Search {
                    entity:
                        SearchEntity::All(crate::cli::search_all_command::SearchAllArgs {
                            gene,
                            variant,
                            disease,
                            drug,
                            keyword,
                            positional_query,
                            since,
                            limit,
                            counts_only,
                            debug_plan,
                        }),
                },
            ..
        } = cli
        else {
            panic!("expected search all command");
        };

        assert_eq!(gene.as_deref(), Some("BRAF"));
        assert_eq!(variant, None);
        assert_eq!(disease.as_deref(), Some("melanoma"));
        assert_eq!(drug, None);
        assert_eq!(keyword, None);
        assert_eq!(positional_query, None);
        assert_eq!(since, None);
        assert_eq!(limit, 2);
        assert!(!counts_only);
        assert!(!debug_plan);
    }

    #[test]
    fn search_all_parses_positional_keyword() {
        let cli = Cli::try_parse_from(["biomcp", "search", "all", "BRAF", "--limit", "2"])
            .expect("search all should parse");

        let Cli {
            command:
                Commands::Search {
                    entity:
                        SearchEntity::All(crate::cli::search_all_command::SearchAllArgs {
                            keyword,
                            positional_query,
                            limit,
                            ..
                        }),
                },
            ..
        } = cli
        else {
            panic!("expected search all command");
        };

        assert_eq!(keyword, None);
        assert_eq!(positional_query.as_deref(), Some("BRAF"));
        assert_eq!(limit, 2);
    }

    #[test]
    fn search_all_without_typed_slots_still_parses_for_runtime_validation() {
        let cli = Cli::try_parse_from(["biomcp", "search", "all", "--limit", "2"])
            .expect("search all without slots should still parse");

        let Cli {
            command:
                Commands::Search {
                    entity:
                        SearchEntity::All(crate::cli::search_all_command::SearchAllArgs {
                            gene,
                            variant,
                            disease,
                            drug,
                            keyword,
                            positional_query,
                            since,
                            limit,
                            counts_only,
                            debug_plan,
                        }),
                },
            ..
        } = cli
        else {
            panic!("expected search all command");
        };

        assert_eq!(gene, None);
        assert_eq!(variant, None);
        assert_eq!(disease, None);
        assert_eq!(drug, None);
        assert_eq!(keyword, None);
        assert_eq!(positional_query, None);
        assert_eq!(since, None);
        assert_eq!(limit, 2);
        assert!(!counts_only);
        assert!(!debug_plan);
    }
}
