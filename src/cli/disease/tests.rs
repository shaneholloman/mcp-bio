use clap::{CommandFactory, Parser};

use super::DiseaseCommand;
use super::dispatch::disease_search_json;
use crate::cli::{Cli, Commands, PaginationMeta};

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
    assert!(help.contains("diagnostics, funding, or survival"));
    assert!(help.contains("tuberculosis diagnostics"));
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

#[tokio::test]
async fn handle_command_rejects_zero_limit_before_related_lookup() {
    let cli = Cli::try_parse_from(["biomcp", "disease", "articles", "melanoma", "--limit", "0"])
        .expect("disease articles should parse");

    let Cli {
        command: Commands::Disease { cmd },
        json,
        ..
    } = cli
    else {
        panic!("expected disease command");
    };

    let err = super::handle_command(cmd, json)
        .await
        .expect_err("zero disease articles limit should fail fast");
    assert!(err.to_string().contains("--limit must be between 1 and 50"));
}

#[test]
fn disease_search_json_includes_fallback_meta_and_provenance() {
    let pagination = PaginationMeta::offset(0, 10, 1, Some(1));
    let results = vec![crate::entities::disease::DiseaseSearchResult {
        id: "MONDO:0000115".into(),
        name: "Arnold-Chiari malformation".into(),
        synonyms_preview: Some("Chiari malformation".into()),
        resolved_via: Some("MESH crosswalk".into()),
        source_id: Some("MESH:D001139".into()),
    }];
    let next_commands = crate::render::markdown::search_next_commands_disease(&results);
    let json = disease_search_json(results, pagination, true, next_commands)
        .expect("disease search json should render");

    let value: serde_json::Value =
        serde_json::from_str(&json).expect("json should parse successfully");
    assert_eq!(value["results"][0]["resolved_via"], "MESH crosswalk");
    assert_eq!(value["results"][0]["source_id"], "MESH:D001139");
    assert_eq!(
        value["_meta"]["next_commands"][0],
        serde_json::Value::String("biomcp get disease MONDO:0000115".into())
    );
    assert_eq!(
        value["_meta"]["next_commands"][1],
        serde_json::Value::String("biomcp list disease".into())
    );
    assert_eq!(value["_meta"]["fallback_used"], true);
}

#[test]
fn disease_search_json_includes_next_commands_for_direct_hits() {
    let pagination = PaginationMeta::offset(0, 10, 1, Some(1));
    let results = vec![crate::entities::disease::DiseaseSearchResult {
        id: "MONDO:0005105".into(),
        name: "melanoma".into(),
        synonyms_preview: Some("malignant melanoma".into()),
        resolved_via: None,
        source_id: None,
    }];
    let next_commands = crate::render::markdown::search_next_commands_disease(&results);
    let json = disease_search_json(results, pagination, false, next_commands)
        .expect("disease search json should render");

    let value: serde_json::Value =
        serde_json::from_str(&json).expect("json should parse successfully");
    assert_eq!(
        value["_meta"]["next_commands"][0],
        serde_json::Value::String("biomcp get disease MONDO:0005105".into())
    );
    assert_eq!(
        value["_meta"]["next_commands"][1],
        serde_json::Value::String("biomcp list disease".into())
    );
    assert!(value["_meta"].get("fallback_used").is_none());
    assert!(value["results"][0].get("resolved_via").is_none());
    assert!(value["results"][0].get("source_id").is_none());
}
