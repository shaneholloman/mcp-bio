use clap::{CommandFactory, Parser};

use crate::cli::{Cli, Commands, DrugCommand, DrugRegionArg, GetEntity};

#[test]
fn get_drug_help_lists_region_flag_and_examples() {
    let mut command = Cli::command();
    let get = command
        .find_subcommand_mut("get")
        .expect("get subcommand should exist");
    let drug = get
        .find_subcommand_mut("drug")
        .expect("drug subcommand should exist");
    let mut help = Vec::new();
    drug.write_long_help(&mut help)
        .expect("drug help should render");
    let help = String::from_utf8(help).expect("help should be utf-8");

    assert!(help.contains("--region <REGION>"));
    assert!(help.contains("biomcp get drug Keytruda regulatory --region eu"));
    assert!(help.contains("biomcp get drug Ozempic safety --region eu"));
}

#[test]
fn get_drug_help_mentions_raw_label_mode() {
    let mut command = Cli::command();
    let get = command
        .find_subcommand_mut("get")
        .expect("get subcommand should exist");
    let drug = get
        .find_subcommand_mut("drug")
        .expect("drug subcommand should exist");
    let mut help = Vec::new();
    drug.write_long_help(&mut help)
        .expect("drug help should render");
    let help = String::from_utf8(help).expect("help should be utf-8");

    assert!(help.contains("--raw"));
    assert!(help.contains("biomcp get drug pembrolizumab label --raw"));
}

#[test]
fn search_drug_help_mentions_default_all_and_structured_filter_note() {
    let mut command = Cli::command();
    let search = command
        .find_subcommand_mut("search")
        .expect("search subcommand should exist");
    let drug = search
        .find_subcommand_mut("drug")
        .expect("search drug subcommand should exist");
    let mut help = Vec::new();
    drug.write_long_help(&mut help)
        .expect("search drug help should render");
    let help = String::from_utf8(help).expect("help should be utf-8");

    assert!(help.contains("When to use:"));
    assert!(help.contains("when you know the drug or brand name"));
    assert!(help.contains("--indication, --target, or --mechanism"));
    assert!(help.contains("[default: all]"));
    assert!(
        help.contains(
            "Omitting --region on a plain name/alias search checks U.S., EU, and WHO data."
        )
    );
    assert!(help.contains(
        "If you omit --region while using structured filters such as --target or --indication, BioMCP stays on the U.S. MyChem path."
    ));
    assert!(help.contains(
        "Explicit --region who filters structured U.S. hits through WHO Prequalification."
    ));
    assert!(help.contains("Explicit --region eu|all with structured filters still errors."));
}

#[test]
fn get_drug_parses_region_split_form() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "get",
        "drug",
        "trastuzumab",
        "regulatory",
        "--region",
        "who",
    ])
    .expect("get drug should parse");

    let Cli {
        command:
            Commands::Get {
                entity:
                    GetEntity::Drug(crate::cli::drug::DrugGetArgs {
                        name,
                        sections,
                        region,
                        raw,
                    }),
            },
        json,
        no_cache,
    } = cli
    else {
        panic!("expected get drug command");
    };

    assert_eq!(name, "trastuzumab");
    assert_eq!(sections, vec!["regulatory".to_string()]);
    assert_eq!(region, Some(DrugRegionArg::Who));
    assert!(!raw);
    assert!(!json);
    assert!(!no_cache);
}

#[test]
fn drug_bare_name_parses_as_external_subcommand() {
    let cli =
        Cli::try_parse_from(["biomcp", "drug", "imatinib"]).expect("bare drug name should parse");

    match cli.command {
        Commands::Drug {
            cmd: DrugCommand::External(args),
        } => assert_eq!(args, vec!["imatinib"]),
        other => panic!("unexpected command: {other:?}"),
    }
}
