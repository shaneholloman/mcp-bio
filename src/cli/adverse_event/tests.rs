use clap::Parser;

use crate::cli::{Cli, Commands, GetEntity, SearchEntity, execute};

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

#[tokio::test]
async fn handle_search_rejects_positional_drug_alias_for_device() {
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

    let err = super::handle_search(args, json)
        .await
        .expect_err("device query should reject positional drug alias");
    assert!(
        err.to_string()
            .contains("--drug cannot be used with --type device")
    );
}

#[tokio::test]
async fn search_adverse_event_device_rejects_positional_drug_alias() {
    let err = execute(vec![
        "biomcp".to_string(),
        "search".to_string(),
        "adverse-event".to_string(),
        "pembrolizumab".to_string(),
        "--type".to_string(),
        "device".to_string(),
    ])
    .await
    .expect_err("device query should reject positional drug alias");
    assert!(
        err.to_string()
            .contains("--drug cannot be used with --type device")
    );
}

#[tokio::test]
async fn handle_search_rejects_count_for_vaers_source() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "search",
        "adverse-event",
        "MMR vaccine",
        "--source",
        "vaers",
        "--count",
        "reaction",
    ])
    .expect("adverse-event vaers count query should parse");

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

    let err = super::handle_search(args, json)
        .await
        .expect_err("vaers search should reject count");
    assert!(
        err.to_string()
            .contains("--count is not supported with --source vaers")
    );
}

#[tokio::test]
async fn handle_search_rejects_nondefault_source_for_recall() {
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

    let err = super::handle_search(args, json)
        .await
        .expect_err("recall query should reject non-default source");
    assert!(
        err.to_string()
            .contains("--source is only supported for --type faers adverse-event search")
    );
}

#[tokio::test]
async fn handle_search_rejects_nondefault_source_for_device() {
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

    let err = super::handle_search(args, json)
        .await
        .expect_err("device query should reject non-default source");
    assert!(
        err.to_string()
            .contains("--source is only supported for --type faers adverse-event search")
    );
}
