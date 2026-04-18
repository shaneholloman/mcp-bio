use clap::{CommandFactory, Parser};

use crate::cli::{Cli, Commands, GetEntity, SearchEntity};
use crate::test_support::{TempDirGuard, env_lock, set_env_var};

fn write_gtr_fixture(root: &std::path::Path) {
    std::fs::write(
        root.join(crate::sources::gtr::GTR_TEST_VERSION_FILE),
        include_bytes!("../../../spec/fixtures/gtr/test_version.gz"),
    )
    .expect("write test_version.gz");
    std::fs::write(
        root.join(crate::sources::gtr::GTR_CONDITION_GENE_FILE),
        include_str!("../../../spec/fixtures/gtr/test_condition_gene.txt"),
    )
    .expect("write test_condition_gene.txt");
}

#[test]
fn search_diagnostic_parses_filter_only_flags() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "search",
        "diagnostic",
        "--gene",
        "BRCA1",
        "--type",
        "molecular",
        "--limit",
        "2",
    ])
    .expect("search diagnostic should parse");

    let Cli {
        command:
            Commands::Search {
                entity:
                    SearchEntity::Diagnostic(crate::cli::diagnostic::DiagnosticSearchArgs {
                        gene,
                        disease,
                        test_type,
                        manufacturer,
                        limit,
                        offset,
                    }),
            },
        ..
    } = cli
    else {
        panic!("expected search diagnostic command");
    };

    assert_eq!(gene.as_deref(), Some("BRCA1"));
    assert_eq!(disease, None);
    assert_eq!(test_type.as_deref(), Some("molecular"));
    assert_eq!(manufacturer, None);
    assert_eq!(limit, 2);
    assert_eq!(offset, 0);
}

#[test]
fn get_diagnostic_help_mentions_supported_sections() {
    let mut command = Cli::command();
    let get = command
        .find_subcommand_mut("get")
        .expect("get subcommand should exist");
    let diagnostic = get
        .find_subcommand_mut("diagnostic")
        .expect("diagnostic subcommand should exist");
    let mut help = Vec::new();
    diagnostic
        .write_long_help(&mut help)
        .expect("diagnostic help should render");
    let help = String::from_utf8(help).expect("help should be utf-8");

    assert!(help.contains("genes, conditions, methods, all"));
    assert!(help.contains("biomcp get diagnostic GTR000000001.1 methods"));
}

#[test]
fn search_diagnostic_help_uses_live_gtr_type_example() {
    let mut command = Cli::command();
    let search = command
        .find_subcommand_mut("search")
        .expect("search subcommand should exist");
    let diagnostic = search
        .find_subcommand_mut("diagnostic")
        .expect("diagnostic subcommand should exist");
    let mut help = Vec::new();
    diagnostic
        .write_long_help(&mut help)
        .expect("diagnostic help should render");
    let help = String::from_utf8(help).expect("help should be utf-8");

    assert!(help.contains("biomcp search diagnostic --gene EGFR --type Clinical --limit 5"));
    assert!(help.contains("`--type` values come from the current GTR export"));
    assert!(!help.contains("--type molecular"));
}

#[tokio::test]
async fn handle_search_rejects_zero_limit_before_gtr_lookup() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "search",
        "diagnostic",
        "--gene",
        "BRCA1",
        "--limit",
        "0",
    ])
    .expect("search diagnostic should parse");

    let Cli {
        command: Commands::Search {
            entity: SearchEntity::Diagnostic(args),
        },
        json,
        ..
    } = cli
    else {
        panic!("expected search diagnostic command");
    };

    let err = super::handle_search(args, json)
        .await
        .expect_err("zero diagnostic limit should fail fast");
    assert!(err.to_string().contains("--limit must be between 1 and 50"));
}

#[tokio::test]
async fn handle_get_honors_trailing_json_flag_after_sections() {
    let _lock = env_lock().lock().await;
    let root = TempDirGuard::new("cli-diagnostic");
    write_gtr_fixture(root.path());
    let _env = set_env_var(
        "BIOMCP_GTR_DIR",
        Some(root.path().to_str().expect("utf-8 path")),
    );

    let cli = Cli::try_parse_from([
        "biomcp",
        "get",
        "diagnostic",
        "GTR000000001.1",
        "genes",
        "--json",
    ])
    .expect("get diagnostic should parse");

    let Cli {
        command: Commands::Get {
            entity: GetEntity::Diagnostic(args),
        },
        json,
        ..
    } = cli
    else {
        panic!("expected get diagnostic command");
    };

    let outcome = super::handle_get(args, json)
        .await
        .expect("get diagnostic json");
    let value: serde_json::Value = serde_json::from_str(&outcome.text).expect("valid json");

    assert_eq!(value["accession"], "GTR000000001.1");
    assert!(
        value.get("genes").is_some(),
        "genes section should be present"
    );
    assert!(
        value["_meta"]["next_commands"]
            .as_array()
            .is_some_and(|commands| commands
                .iter()
                .any(|cmd| cmd == "biomcp get diagnostic GTR000000001.1 conditions"))
    );
    assert!(
        value["_meta"]["next_commands"]
            .as_array()
            .is_some_and(|commands| commands
                .iter()
                .all(|cmd| cmd != "biomcp get diagnostic GTR000000001.1 genes"))
    );
}
