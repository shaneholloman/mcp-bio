use clap::{CommandFactory, Parser};

use super::dispatch::{drug_search_json, resolve_drug_get_region, resolve_drug_search_region};
use crate::cli::{Cli, Commands, DrugCommand, DrugRegionArg, GetEntity, SearchEntity};
use crate::entities::drug::{DrugRegion, DrugSearchFilters};

fn render_drug_trials_help() -> String {
    let mut command = Cli::command();
    let drug = command
        .find_subcommand_mut("drug")
        .expect("drug subcommand should exist");
    let trials = drug
        .find_subcommand_mut("trials")
        .expect("drug trials subcommand should exist");
    let mut help = Vec::new();
    trials
        .write_long_help(&mut help)
        .expect("drug trials help should render");
    String::from_utf8(help).expect("help should be utf-8")
}

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
    assert!(help.contains("biomcp get drug Dupixent regulatory --region ema"));
    assert!(help.contains("biomcp get drug Ozempic safety --region eu"));
    assert!(
        help.contains(
            "`--region ema` is accepted as an alias for the canonical `eu` region value."
        )
    );
    assert!(help.contains(
        "If you omit `--region` on `biomcp get drug <name> regulatory`, BioMCP checks U.S. and EU regulatory data."
    ));
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
fn get_drug_parses_ema_region_alias_as_eu() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "get",
        "drug",
        "Dupixent",
        "regulatory",
        "--region",
        "ema",
    ])
    .expect("get drug ema alias should parse");

    let Cli {
        command:
            Commands::Get {
                entity:
                    GetEntity::Drug(crate::cli::drug::DrugGetArgs {
                        region, sections, ..
                    }),
            },
        ..
    } = cli
    else {
        panic!("expected get drug command");
    };

    assert_eq!(sections, vec!["regulatory".to_string()]);
    assert_eq!(region, Some(DrugRegionArg::Eu));
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

#[test]
fn drug_trials_help_mentions_alias_expansion_and_opt_out() {
    let help = render_drug_trials_help();

    assert!(help.contains("inherits intervention alias expansion"));
    assert!(help.contains("Matched Intervention"));
    assert!(help.contains("matched_intervention_label"));
    assert!(help.contains("--no-alias-expand"));
}

#[test]
fn drug_trials_parse_no_alias_expand() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "drug",
        "trials",
        "daraxonrasib",
        "--no-alias-expand",
    ])
    .expect("drug trials should parse");

    match cli.command {
        Commands::Drug {
            cmd:
                DrugCommand::Trials {
                    name,
                    no_alias_expand,
                    ..
                },
        } => {
            assert_eq!(name, "daraxonrasib");
            assert!(no_alias_expand);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[tokio::test]
async fn drug_trials_reject_no_alias_expand_for_nci_source() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "drug",
        "trials",
        "daraxonrasib",
        "--source",
        "nci",
        "--no-alias-expand",
    ])
    .expect("drug trials should parse");

    let Cli {
        command: Commands::Drug { cmd },
        json,
        ..
    } = cli
    else {
        panic!("expected drug command");
    };

    let err = super::handle_command(cmd, json, false)
        .await
        .expect_err("nci no-alias-expand should fail");
    assert!(
        err.to_string()
            .contains("--no-alias-expand is only supported for CTGov intervention searches")
    );
}

#[tokio::test]
async fn handle_search_rejects_non_us_structured_region() {
    let cli = Cli::try_parse_from([
        "biomcp", "search", "drug", "--target", "EGFR", "--region", "eu",
    ])
    .expect("search drug should parse");

    let Cli {
        command: Commands::Search {
            entity: SearchEntity::Drug(args),
        },
        json,
        ..
    } = cli
    else {
        panic!("expected search drug command");
    };

    let err = super::handle_search(args, json)
        .await
        .expect_err("explicit EU structured search should fail");
    assert!(
        err.to_string()
            .contains("EMA and all-region search currently support name/alias lookups only")
    );
}

#[tokio::test]
async fn get_drug_raw_rejects_non_label_sections() {
    let cli = Cli::try_parse_from(["biomcp", "get", "drug", "pembrolizumab", "targets", "--raw"])
        .expect("get drug --raw should parse");

    let err = crate::cli::run_outcome(cli)
        .await
        .expect_err("targets --raw should be rejected");
    assert!(
        err.to_string()
            .contains("--raw can only be used with label or all")
    );
}

#[test]
fn search_drug_region_defaults_to_all_for_name_only_queries() {
    let filters = DrugSearchFilters {
        query: Some("Keytruda".into()),
        ..Default::default()
    };

    let region = resolve_drug_search_region(None, &filters).expect("name-only default");
    assert_eq!(region, DrugRegion::All);
}

#[test]
fn search_drug_region_defaults_to_us_for_structured_queries() {
    let filters = DrugSearchFilters {
        target: Some("EGFR".into()),
        ..Default::default()
    };

    let region = resolve_drug_search_region(None, &filters).expect("structured default");
    assert_eq!(region, DrugRegion::Us);
}

#[test]
fn search_drug_region_rejects_explicit_non_us_for_structured_queries() {
    let filters = DrugSearchFilters {
        target: Some("EGFR".into()),
        ..Default::default()
    };

    let err = resolve_drug_search_region(Some(crate::cli::DrugRegionArg::Eu), &filters)
        .expect_err("explicit eu should be rejected");
    assert!(format!("{err}").contains(
        "EMA and all-region search currently support name/alias lookups only; use --region us for structured MyChem filters or --region who to filter structured U.S. hits through WHO prequalification."
    ));

    let err = resolve_drug_search_region(Some(crate::cli::DrugRegionArg::All), &filters)
        .expect_err("explicit all should be rejected");
    assert!(format!("{err}").contains(
        "EMA and all-region search currently support name/alias lookups only; use --region us for structured MyChem filters or --region who to filter structured U.S. hits through WHO prequalification."
    ));
}

#[test]
fn search_drug_region_allows_explicit_who_for_structured_queries() {
    let filters = DrugSearchFilters {
        indication: Some("malaria".into()),
        ..Default::default()
    };

    let region =
        resolve_drug_search_region(Some(crate::cli::DrugRegionArg::Who), &filters).expect("who");
    assert_eq!(region, DrugRegion::Who);
}

#[test]
fn get_drug_region_defaults_to_all_for_regulatory_only_queries() {
    let region = resolve_drug_get_region(&["regulatory".to_string()], None);
    assert_eq!(region, DrugRegion::All);
}

#[test]
fn get_drug_region_keeps_non_regulatory_no_flag_shapes_on_us_default() {
    assert_eq!(
        resolve_drug_get_region(&["all".to_string()], None),
        DrugRegion::Us
    );
    assert_eq!(
        resolve_drug_get_region(&["regulatory".to_string(), "safety".to_string()], None),
        DrugRegion::Us
    );
}

#[test]
fn get_drug_region_respects_explicit_region() {
    let region = resolve_drug_get_region(&["regulatory".to_string()], Some(DrugRegion::Who));
    assert_eq!(region, DrugRegion::Who);
}

#[test]
fn drug_search_json_single_region_keeps_selected_bucket_and_who_fields() {
    let json = drug_search_json(
        crate::entities::drug::DrugSearchPageWithRegion::Who(crate::entities::SearchPage::offset(
            vec![crate::entities::drug::WhoPrequalificationSearchResult {
                inn: "Trastuzumab".to_string(),
                therapeutic_area: "Oncology".to_string(),
                dosage_form: "Powder for concentrate for solution for infusion".to_string(),
                applicant: "Samsung Bioepis NL B.V.".to_string(),
                who_reference_number: "BT-ON001".to_string(),
                listing_basis: "Prequalification - Abridged".to_string(),
                prequalification_date: Some("2019-12-18".to_string()),
            }],
            Some(1),
        )),
        Some("trastuzumab"),
        0,
        5,
    )
    .expect("WHO search json");

    let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    assert_eq!(value["region"], "who");
    assert_eq!(
        value["regions"].as_object().map(|regions| regions.len()),
        Some(1)
    );
    assert!(value.get("pagination").is_none());
    assert!(value.get("count").is_none());
    assert!(value.get("results").is_none());
    assert!(value.get("query").is_none());
    assert_eq!(value["regions"]["who"]["count"], 1);
    assert_eq!(value["regions"]["who"]["pagination"]["returned"], 1);
    assert_eq!(
        value["regions"]["who"]["results"][0]["who_reference_number"],
        "BT-ON001"
    );
    assert_eq!(
        value["regions"]["who"]["results"][0]["listing_basis"],
        "Prequalification - Abridged"
    );
    assert_eq!(
        value["regions"]["who"]["results"][0]["prequalification_date"],
        "2019-12-18"
    );
    assert_eq!(
        value["_meta"]["next_commands"][0],
        serde_json::Value::String("biomcp get drug Trastuzumab".into())
    );
    assert_eq!(
        value["_meta"]["next_commands"][1],
        serde_json::Value::String("biomcp list drug".into())
    );
}

#[test]
fn drug_search_json_all_region_uses_unified_regions_envelope() {
    let json = drug_search_json(
        crate::entities::drug::DrugSearchPageWithRegion::All {
            us: crate::entities::SearchPage::offset(
                vec![crate::entities::drug::DrugSearchResult {
                    name: "pembrolizumab".to_string(),
                    drugbank_id: None,
                    drug_type: None,
                    mechanism: None,
                    target: Some("PDCD1".to_string()),
                }],
                Some(1),
            ),
            eu: crate::entities::SearchPage::offset(
                vec![crate::entities::drug::EmaDrugSearchResult {
                    name: "Keytruda".to_string(),
                    active_substance: "pembrolizumab".to_string(),
                    ema_product_number: "EMEA/H/C/003820".to_string(),
                    status: "Authorised".to_string(),
                }],
                Some(1),
            ),
            who: crate::entities::SearchPage::offset(
                vec![crate::entities::drug::WhoPrequalificationSearchResult {
                    inn: "Pembrolizumab".to_string(),
                    therapeutic_area: "Oncology".to_string(),
                    dosage_form: "Concentrate".to_string(),
                    applicant: "Merck Sharp & Dohme".to_string(),
                    who_reference_number: "BT-ON002".to_string(),
                    listing_basis: "Prequalification".to_string(),
                    prequalification_date: Some("2020-01-01".to_string()),
                }],
                Some(1),
            ),
        },
        Some("keytruda"),
        0,
        5,
    )
    .expect("all-region drug search json");

    let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    assert_eq!(value["region"], "all");
    assert_eq!(
        value["regions"].as_object().map(|regions| regions.len()),
        Some(3)
    );
    assert!(value.get("pagination").is_none());
    assert!(value.get("count").is_none());
    assert!(value.get("results").is_none());
    assert!(value.get("query").is_none());
    assert_eq!(value["regions"]["us"]["count"], 1);
    assert_eq!(value["regions"]["eu"]["count"], 1);
    assert_eq!(value["regions"]["who"]["count"], 1);
    assert_eq!(
        value["regions"]["who"]["results"][0]["who_reference_number"],
        "BT-ON002"
    );
    assert_eq!(
        value["regions"]["eu"]["results"][0]["ema_product_number"],
        "EMEA/H/C/003820"
    );
    assert_eq!(
        value["_meta"]["next_commands"][0],
        serde_json::Value::String("biomcp get drug Keytruda".into())
    );
    assert_eq!(
        value["_meta"]["next_commands"][1],
        serde_json::Value::String("biomcp list drug".into())
    );
}

#[test]
fn drug_search_json_all_region_keeps_empty_buckets() {
    let json = drug_search_json(
        crate::entities::drug::DrugSearchPageWithRegion::All {
            us: crate::entities::SearchPage::offset(Vec::new(), Some(0)),
            eu: crate::entities::SearchPage::offset(
                vec![crate::entities::drug::EmaDrugSearchResult {
                    name: "Keytruda".to_string(),
                    active_substance: "pembrolizumab".to_string(),
                    ema_product_number: "EMEA/H/C/003820".to_string(),
                    status: "Authorised".to_string(),
                }],
                Some(1),
            ),
            who: crate::entities::SearchPage::offset(Vec::new(), Some(0)),
        },
        Some("keytruda"),
        0,
        5,
    )
    .expect("all-region empty bucket json");

    let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    assert_eq!(value["regions"]["us"]["count"], 0);
    assert_eq!(value["regions"]["us"]["results"], serde_json::json!([]));
    assert_eq!(value["regions"]["who"]["count"], 0);
    assert_eq!(value["regions"]["who"]["results"], serde_json::json!([]));
    assert_eq!(
        value["_meta"]["next_commands"][0],
        serde_json::Value::String("biomcp get drug Keytruda".into())
    );
}
