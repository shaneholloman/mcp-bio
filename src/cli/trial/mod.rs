//! Trial CLI payloads.

use clap::Args;

#[derive(Args, Debug)]
pub struct TrialSearchArgs {
    /// Filter by condition/disease
    #[arg(short = 'c', long, num_args = 1..)]
    pub condition: Vec<String>,
    /// Optional positional query alias for -c/--condition
    #[arg(value_name = "QUERY")]
    pub positional_query: Option<String>,
    /// Filter by intervention/drug
    #[arg(short = 'i', long, num_args = 1..)]
    pub intervention: Vec<String>,
    /// Filter by institution/facility name (text-search mode by default).
    ///
    /// Without `--lat`/`--lon`/`--distance`, this uses cheap CTGov
    /// `query.locn` text-search mode. With all three geo flags, it enters
    /// geo-verify mode and performs extra per-study location fetches to
    /// confirm the facility match within the requested distance. Geo-verify
    /// mode is materially more expensive, especially with `--count-only`.
    #[arg(long, num_args = 1..)]
    pub facility: Vec<String>,
    /// Filter by phase. Canonical CLI forms: NA, 1, 1/2, 2, 3, 4.
    /// Accepted aliases: EARLY_PHASE1, PHASE1, PHASE2, PHASE3, PHASE4.
    ///
    /// `1/2` matches the ClinicalTrials.gov combined Phase 1/Phase 2 label
    /// (studies tagged as both phases), not Phase 1 OR Phase 2.
    #[arg(short = 'p', long)]
    pub phase: Option<String>,
    /// Study type (e.g., interventional, observational)
    #[arg(long = "study-type")]
    pub study_type: Option<String>,
    /// Patient age in years for eligibility matching (decimals accepted, e.g. 0.5 for 6 months).
    ///
    /// With `--count-only`, age-only CTGov searches report an approximate
    /// upstream total because BioMCP applies the age filter during full
    /// search, not the fast count path.
    #[arg(long)]
    pub age: Option<f32>,
    /// Eligible sex filter [values: female, male, all].
    ///
    /// `all` (also `any`/`both`) resolves to no sex restriction, so no sex
    /// filter is sent to ClinicalTrials.gov. Use `female` or `male` to
    /// apply an actual restriction.
    #[arg(long)]
    pub sex: Option<String>,
    /// Filter by trial status [values: recruiting, not_yet_recruiting, enrolling_by_invitation, active_not_recruiting, completed, suspended, terminated, withdrawn]
    #[arg(short = 's', long)]
    pub status: Option<String>,
    /// Search mutation-related ClinicalTrials.gov text fields (best-effort)
    #[arg(long, num_args = 1..)]
    pub mutation: Vec<String>,
    /// Search eligibility criteria with free-text terms (best-effort)
    #[arg(long, num_args = 1..)]
    pub criteria: Vec<String>,
    /// Biomarker filter (NCI CTS; best-effort for ctgov)
    #[arg(long, num_args = 1..)]
    pub biomarker: Vec<String>,
    /// Prior therapy mentioned in eligibility
    #[arg(long, num_args = 1..)]
    pub prior_therapies: Vec<String>,
    /// Drug/therapy patient progressed on
    #[arg(long, num_args = 1..)]
    pub progression_on: Vec<String>,
    /// Line of therapy: 1L, 2L, 3L+
    #[arg(long)]
    pub line_of_therapy: Option<String>,
    /// Filter by sponsor (best-effort)
    #[arg(long, num_args = 1..)]
    pub sponsor: Vec<String>,
    /// Sponsor/funder category [values: nih, industry, fed, other]
    #[arg(long = "sponsor-type")]
    pub sponsor_type: Option<String>,
    /// Trials updated after date (YYYY-MM-DD)
    #[arg(long = "date-from", alias = "since")]
    pub date_from: Option<String>,
    /// Trials updated before date (YYYY-MM-DD)
    #[arg(long = "date-to", alias = "until")]
    pub date_to: Option<String>,
    /// Latitude for geographic search
    #[arg(long, allow_hyphen_values = true)]
    pub lat: Option<f64>,
    /// Longitude for geographic search
    #[arg(long, allow_hyphen_values = true)]
    pub lon: Option<f64>,
    /// Distance (miles) for geographic search
    #[arg(long)]
    pub distance: Option<u32>,
    /// Only return trials with posted results (default: off, include trials with/without posted results)
    #[arg(long = "has-results", visible_alias = "results-available")]
    pub results_available: bool,
    /// Return only total count (no result table)
    #[arg(long = "count-only")]
    pub count_only: bool,
    /// Trial data source (ctgov or nci)
    #[arg(long, default_value = "ctgov")]
    pub source: String,
    /// Skip the first N results (pagination)
    #[arg(long, default_value = "0")]
    pub offset: usize,
    /// Cursor token from a previous response
    #[arg(long = "next-page")]
    pub next_page: Option<String>,
    /// Maximum results (default: 10)
    #[arg(short, long, default_value = "10")]
    pub limit: usize,
}

#[derive(Args, Debug)]
pub struct TrialGetArgs {
    /// ClinicalTrials.gov identifier (e.g., NCT02693535)
    pub nct_id: String,
    /// Sections to include (eligibility, locations, outcomes, arms, references, all)
    #[arg(trailing_var_arg = true)]
    pub sections: Vec<String>,
    /// Trial data source (ctgov or nci)
    #[arg(long, default_value = "ctgov")]
    pub source: String,
}

mod dispatch;
pub(super) use self::dispatch::{handle_get, handle_search};

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser};

    use crate::cli::{Cli, Commands, GetEntity, SearchEntity};

    fn render_trial_search_long_help() -> String {
        let mut command = Cli::command();
        let search = command
            .find_subcommand_mut("search")
            .expect("search subcommand should exist");
        let trial = search
            .find_subcommand_mut("trial")
            .expect("trial subcommand should exist");
        let mut help = Vec::new();
        trial
            .write_long_help(&mut help)
            .expect("trial help should render");
        String::from_utf8(help).expect("help should be utf-8")
    }

    #[test]
    fn trial_facility_help_names_text_search_and_geo_verify_modes() {
        let help = render_trial_search_long_help();

        assert!(help.contains("text-search mode"));
        assert!(help.contains("geo-verify mode"));
        assert!(help.contains("materially more expensive"));
    }

    #[test]
    fn trial_phase_help_explains_combined_phase_label() {
        let help = render_trial_search_long_help();

        assert!(help.contains("1/2"));
        assert!(help.contains("combined Phase 1/Phase 2 label"));
        assert!(help.contains("not Phase 1 OR Phase 2"));
    }

    #[test]
    fn trial_sex_help_explains_all_means_no_restriction() {
        let help = render_trial_search_long_help();

        assert!(help.contains("all"));
        assert!(help.contains("no sex restriction"));
    }

    #[test]
    fn trial_phase_help_explains_canonical_numeric_forms_and_aliases() {
        let help = render_trial_search_long_help();

        assert!(help.contains("Canonical CLI forms: NA, 1, 1/2, 2, 3, 4."));
        assert!(help.contains("Accepted aliases: EARLY_PHASE1, PHASE1, PHASE2, PHASE3, PHASE4."));
    }

    #[test]
    fn trial_help_documents_nci_source_specific_notes() {
        let help = render_trial_search_long_help();

        assert!(help.contains("Source-specific notes"));
        assert!(help.contains("grounds to an NCI disease ID when available"));
        assert!(help.contains("one mapped status at a time"));
        assert!(help.contains("I_II"));
        assert!(help.contains("early_phase1"));
        assert!(help.contains("sites.org_coordinates"));
        assert!(help.contains("no separate NCI keyword flag"));
    }

    #[test]
    fn trial_age_help_explains_age_only_count_is_approximate() {
        let help = render_trial_search_long_help();

        assert!(help.contains("age-only CTGov searches report an approximate upstream total"));
    }

    #[test]
    fn search_trial_parses_positional_query() {
        let cli = Cli::try_parse_from(["biomcp", "search", "trial", "melanoma", "--limit", "2"])
            .expect("search trial should parse");

        let Cli {
            command:
                Commands::Search {
                    entity:
                        SearchEntity::Trial(crate::cli::trial::TrialSearchArgs {
                            condition,
                            positional_query,
                            intervention,
                            facility,
                            phase,
                            study_type,
                            age,
                            sex,
                            status,
                            mutation,
                            criteria,
                            biomarker,
                            prior_therapies,
                            progression_on,
                            line_of_therapy,
                            sponsor,
                            sponsor_type,
                            date_from,
                            date_to,
                            lat,
                            lon,
                            distance,
                            results_available,
                            count_only,
                            source,
                            offset,
                            next_page,
                            limit,
                        }),
                },
            ..
        } = cli
        else {
            panic!("expected search trial command");
        };

        assert!(condition.is_empty());
        assert_eq!(positional_query.as_deref(), Some("melanoma"));
        assert!(intervention.is_empty());
        assert!(facility.is_empty());
        assert_eq!(phase, None);
        assert_eq!(study_type, None);
        assert_eq!(age, None);
        assert_eq!(sex, None);
        assert_eq!(status, None);
        assert!(mutation.is_empty());
        assert!(criteria.is_empty());
        assert!(biomarker.is_empty());
        assert!(prior_therapies.is_empty());
        assert!(progression_on.is_empty());
        assert_eq!(line_of_therapy, None);
        assert!(sponsor.is_empty());
        assert_eq!(sponsor_type, None);
        assert_eq!(date_from, None);
        assert_eq!(date_to, None);
        assert_eq!(lat, None);
        assert_eq!(lon, None);
        assert_eq!(distance, None);
        assert!(!results_available);
        assert!(!count_only);
        assert_eq!(source, "ctgov");
        assert_eq!(offset, 0);
        assert_eq!(next_page, None);
        assert_eq!(limit, 2);
    }

    #[test]
    fn search_trial_parses_multi_word_positional_query() {
        let cli = Cli::try_parse_from([
            "biomcp",
            "search",
            "trial",
            "endometrial cancer",
            "--status",
            "recruiting",
        ])
        .expect("search trial should parse");

        let Cli {
            command:
                Commands::Search {
                    entity:
                        SearchEntity::Trial(crate::cli::trial::TrialSearchArgs {
                            positional_query,
                            status,
                            ..
                        }),
                },
            ..
        } = cli
        else {
            panic!("expected search trial command");
        };

        assert_eq!(positional_query.as_deref(), Some("endometrial cancer"));
        assert_eq!(status.as_deref(), Some("recruiting"));
    }

    #[test]
    fn search_trial_parses_positional_query_with_status_flag() {
        let cli = Cli::try_parse_from([
            "biomcp",
            "search",
            "trial",
            "melanoma",
            "--status",
            "recruiting",
            "--limit",
            "2",
        ])
        .expect("search trial should parse");

        let Cli {
            command:
                Commands::Search {
                    entity:
                        SearchEntity::Trial(crate::cli::trial::TrialSearchArgs {
                            positional_query,
                            status,
                            limit,
                            ..
                        }),
                },
            ..
        } = cli
        else {
            panic!("expected search trial command");
        };

        assert_eq!(positional_query.as_deref(), Some("melanoma"));
        assert_eq!(status.as_deref(), Some("recruiting"));
        assert_eq!(limit, 2);
    }

    #[test]
    fn search_trial_parses_new_filter_flags() {
        let cli = Cli::try_parse_from([
            "biomcp",
            "search",
            "trial",
            "--age",
            "0.5",
            "--study-type",
            "interventional",
            "--has-results",
        ])
        .expect("search trial should parse");

        let Cli {
            command:
                Commands::Search {
                    entity:
                        SearchEntity::Trial(crate::cli::trial::TrialSearchArgs {
                            age,
                            study_type,
                            results_available,
                            ..
                        }),
                },
            ..
        } = cli
        else {
            panic!("expected search trial command");
        };

        assert_eq!(age, Some(0.5));
        assert_eq!(study_type.as_deref(), Some("interventional"));
        assert!(results_available);
    }

    #[test]
    fn search_trial_rejects_non_numeric_age() {
        let err =
            Cli::try_parse_from(["biomcp", "search", "trial", "--age", "abc", "--count-only"])
                .expect_err("invalid age should fail");
        let rendered = err.to_string();

        assert!(rendered.contains("invalid value 'abc' for '--age <AGE>'"));
        assert!(rendered.contains("invalid float literal"));
    }

    #[test]
    fn search_trial_parses_unquoted_multi_token_mutation() {
        let cli = Cli::try_parse_from([
            "biomcp",
            "search",
            "trial",
            "--mutation",
            "BRAF",
            "V600E",
            "--limit",
            "5",
        ])
        .expect("search trial should parse");

        let Cli {
            command:
                Commands::Search {
                    entity:
                        SearchEntity::Trial(crate::cli::trial::TrialSearchArgs {
                            mutation, limit, ..
                        }),
                },
            ..
        } = cli
        else {
            panic!("expected search trial command");
        };

        assert_eq!(mutation, vec!["BRAF".to_string(), "V600E".to_string()]);
        assert_eq!(limit, 5);
    }

    #[test]
    fn get_trial_parses_source_before_sections() {
        let cli = Cli::try_parse_from([
            "biomcp",
            "get",
            "trial",
            "NCT02576665",
            "--source",
            "ctgov",
            "eligibility",
        ])
        .expect("get trial should parse");

        let Cli {
            command:
                Commands::Get {
                    entity:
                        GetEntity::Trial(crate::cli::trial::TrialGetArgs {
                            nct_id,
                            sections,
                            source,
                        }),
                },
            ..
        } = cli
        else {
            panic!("expected get trial command");
        };

        assert_eq!(nct_id, "NCT02576665");
        assert_eq!(sections, vec!["eligibility".to_string()]);
        assert_eq!(source, "ctgov");
    }
}
