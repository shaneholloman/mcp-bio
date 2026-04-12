//! Adverse-event CLI payloads.

use clap::Args;

#[derive(Args, Debug)]
pub struct AdverseEventSearchArgs {
    /// Drug name (required for FAERS queries)
    #[arg(short = 'd', long)]
    pub drug: Option<String>,
    /// Optional positional query alias for -d/--drug
    #[arg(value_name = "QUERY")]
    pub positional_query: Option<String>,
    /// Device name (required for --type device)
    #[arg(long)]
    pub device: Option<String>,
    /// Device manufacturer name (for --type device)
    #[arg(long)]
    pub manufacturer: Option<String>,
    /// Device product code (for --type device)
    #[arg(long = "product-code")]
    pub product_code: Option<String>,
    /// Filter by reaction term (MedDRA)
    #[arg(long)]
    pub reaction: Option<String>,
    /// Filter by reaction outcome [values: death, hospitalization, disability]
    #[arg(long)]
    pub outcome: Option<String>,
    /// Seriousness filter (optionally specify type: death, hospitalization, lifethreatening, disability, congenital, other)
    #[arg(long, num_args = 0..=1, default_missing_value = "any")]
    pub serious: Option<String>,
    /// Received after year/date (YYYY or YYYY-MM-DD)
    #[arg(long = "date-from", alias = "since")]
    pub date_from: Option<String>,
    /// Received before year/date (YYYY or YYYY-MM-DD)
    #[arg(long = "date-to", alias = "until")]
    pub date_to: Option<String>,
    /// Restrict to suspect drugs only
    #[arg(long = "suspect-only")]
    pub suspect_only: bool,
    /// Patient sex filter (m|f)
    #[arg(long)]
    pub sex: Option<String>,
    /// Minimum patient age
    #[arg(long = "age-min")]
    pub age_min: Option<u32>,
    /// Maximum patient age
    #[arg(long = "age-max")]
    pub age_max: Option<u32>,
    /// Reporter qualification filter
    #[arg(long)]
    pub reporter: Option<String>,
    /// Server-side count aggregation field
    #[arg(long)]
    pub count: Option<String>,
    /// Query type: faers (default), recall, or device
    #[arg(long, default_value = "faers")]
    pub r#type: String,
    /// Filter by recall classification (Class I, Class II, Class III)
    #[arg(long)]
    pub classification: Option<String>,
    /// Maximum results (default: 10)
    #[arg(short, long, default_value = "10")]
    pub limit: usize,
    /// Skip the first N results
    #[arg(long, default_value = "0")]
    pub offset: usize,
}

#[derive(Args, Debug)]
pub struct AdverseEventGetArgs {
    /// FAERS safetyreportid or MAUDE mdr_report_key
    pub report_id: String,
    /// Sections to include (reactions, outcomes, concomitant, guidance, all)
    #[arg(trailing_var_arg = true)]
    pub sections: Vec<String>,
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::cli::{Cli, Commands, GetEntity, SearchEntity};

    #[test]
    fn search_adverse_event_parses_serious_default_and_limit() {
        let cli = Cli::try_parse_from([
            "biomcp",
            "search",
            "adverse-event",
            "-d",
            "ibuprofen",
            "--serious",
            "--limit",
            "2",
        ])
        .expect("adverse-event search should parse");

        let Cli {
            command:
                Commands::Search {
                    entity:
                        SearchEntity::AdverseEvent(crate::cli::adverse_event::AdverseEventSearchArgs {
                            drug,
                            serious,
                            r#type,
                            limit,
                            offset,
                            ..
                        }),
                },
            ..
        } = cli
        else {
            panic!("expected adverse-event search command");
        };

        assert_eq!(drug.as_deref(), Some("ibuprofen"));
        assert_eq!(serious.as_deref(), Some("any"));
        assert_eq!(r#type, "faers");
        assert_eq!(limit, 2);
        assert_eq!(offset, 0);
    }

    #[test]
    fn get_adverse_event_parses_sections() {
        let cli = Cli::try_parse_from(["biomcp", "get", "adverse-event", "10222779", "reactions"])
            .expect("adverse-event get should parse");

        let Cli {
            command:
                Commands::Get {
                    entity:
                        GetEntity::AdverseEvent(crate::cli::adverse_event::AdverseEventGetArgs {
                            report_id,
                            sections,
                        }),
                },
            ..
        } = cli
        else {
            panic!("expected adverse-event get command");
        };

        assert_eq!(report_id, "10222779");
        assert_eq!(sections, vec!["reactions".to_string()]);
    }
}
