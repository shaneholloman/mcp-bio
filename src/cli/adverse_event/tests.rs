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
fn search_adverse_event_parses_source_filter() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "search",
        "adverse-event",
        "MMR vaccine",
        "--source",
        "vaers",
    ])
    .expect("adverse-event search should parse source filter");

    let Cli {
        command:
            Commands::Search {
                entity:
                    SearchEntity::AdverseEvent(crate::cli::adverse_event::AdverseEventSearchArgs {
                        positional_query,
                        r#type,
                        source,
                        ..
                    }),
            },
        ..
    } = cli
    else {
        panic!("expected adverse-event search command");
    };

    assert_eq!(positional_query.as_deref(), Some("MMR vaccine"));
    assert_eq!(r#type, "faers");
    assert_eq!(source, "vaers");
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

#[test]
fn search_plan_rejects_positional_drug_alias_for_device() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "search",
        "adverse-event",
        "pembrolizumab",
        "--type",
        "device",
    ])
    .expect("adverse-event device search should parse");

    let Cli {
        command: Commands::Search {
            entity: SearchEntity::AdverseEvent(args),
        },
        json,
        ..
    } = cli
    else {
        panic!("expected adverse-event search command");
    };

    assert!(!json);
    let err = super::dispatch::search_plan_from_args(&args)
        .expect_err("device query should reject positional drug alias");
    assert!(
        err.to_string()
            .contains("--drug cannot be used with --type device")
    );
}

#[test]
fn search_adverse_event_device_rejects_positional_drug_alias() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "search",
        "adverse-event",
        "pembrolizumab",
        "--type",
        "device",
    ])
    .expect("adverse-event device search should parse");
    let Cli {
        command: Commands::Search {
            entity: SearchEntity::AdverseEvent(args),
        },
        ..
    } = cli
    else {
        panic!("expected adverse-event search command");
    };
    let err = super::dispatch::search_plan_from_args(&args)
        .expect_err("device query should reject positional drug alias");
    assert!(
        err.to_string()
            .contains("--drug cannot be used with --type device")
    );
}

#[test]
fn search_plan_rejects_count_for_vaers_source() {
    for count in ["reaction", ""] {
        let cli = Cli::try_parse_from([
            "biomcp",
            "search",
            "adverse-event",
            "MMR vaccine",
            "--source",
            "vaers",
            "--count",
            count,
        ])
        .expect("adverse-event vaers count query should parse");

        let Cli {
            command:
                Commands::Search {
                    entity: SearchEntity::AdverseEvent(args),
                },
            json,
            ..
        } = cli
        else {
            panic!("expected adverse-event search command");
        };

        assert!(!json);
        let err = super::dispatch::search_plan_from_args(&args)
            .expect_err("vaers search should reject count");
        assert!(
            err.to_string()
                .contains("--count is not supported with --source vaers")
        );
    }
}

#[test]
fn search_plan_rejects_nondefault_source_for_recall() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "search",
        "adverse-event",
        "ibuprofen",
        "--type",
        "recall",
        "--source",
        "vaers",
    ])
    .expect("recall search should parse");

    let Cli {
        command: Commands::Search {
            entity: SearchEntity::AdverseEvent(args),
        },
        json,
        ..
    } = cli
    else {
        panic!("expected adverse-event search command");
    };

    assert!(!json);
    let err = super::dispatch::search_plan_from_args(&args)
        .expect_err("recall query should reject non-default source");
    assert!(
        err.to_string()
            .contains("--source is only supported for --type faers adverse-event search")
    );
}

#[test]
fn search_plan_rejects_nondefault_source_for_device() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "search",
        "adverse-event",
        "--device",
        "pump",
        "--type",
        "device",
        "--source",
        "faers",
    ])
    .expect("device search should parse");

    let Cli {
        command: Commands::Search {
            entity: SearchEntity::AdverseEvent(args),
        },
        json,
        ..
    } = cli
    else {
        panic!("expected adverse-event search command");
    };

    assert!(!json);
    let err = super::dispatch::search_plan_from_args(&args)
        .expect_err("device query should reject non-default source");
    assert!(
        err.to_string()
            .contains("--source is only supported for --type faers adverse-event search")
    );
}
