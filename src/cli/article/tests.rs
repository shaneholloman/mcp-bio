use clap::CommandFactory;

use crate::cli::Cli;

fn render_article_search_long_help() -> String {
    let mut command = Cli::command();
    let search = command
        .find_subcommand_mut("search")
        .expect("search subcommand should exist");
    let article = search
        .find_subcommand_mut("article")
        .expect("article subcommand should exist");
    let mut help = Vec::new();
    article
        .write_long_help(&mut help)
        .expect("article help should render");
    String::from_utf8(help).expect("help should be utf-8")
}

#[test]
fn search_article_help_includes_when_to_use_guidance() {
    let help = render_article_search_long_help();

    assert!(help.contains("When to use:"));
    assert!(help.contains("keyword search to scan a topic"));
    assert!(help.contains("Prefer --type review"));
}

#[test]
fn search_article_help_includes_query_formulation_guidance() {
    let help = render_article_search_long_help();

    assert!(help.contains("QUERY FORMULATION:"));
    assert!(help.contains(
        "Known gene/disease/drug anchors belong in `-g/--gene`, `-d/--disease`, or `--drug`."
    ));
    assert!(help.contains(
        "Use `-k/--keyword` for mechanisms, phenotypes, datasets, outcomes, and other free-text concepts."
    ));
    assert!(
        help.contains(
            "Unknown-entity questions should stay keyword-first or start with `discover`."
        )
    );
    assert!(help.contains(
        "Adding `-k/--keyword` on the default route brings in LitSense2 and default `hybrid` relevance."
    ));
    assert!(help.contains(
        "`semantic` sorts by the LitSense2-derived semantic signal and falls back to lexical ties."
    ));
    assert!(help.contains(
        "Hybrid score = `0.4*semantic + 0.3*lexical + 0.2*citations + 0.1*position` by default, using the same LitSense2-derived semantic signal and `semantic=0` when LitSense2 did not match."
    ));
    assert!(
        help.contains("biomcp search article -g TP53 -k \"apoptosis gene regulation\" --limit 5")
    );
    assert!(help.contains(
        "biomcp search article -k '\"cafe-au-lait spots\" neurofibromas disease' --type review --limit 5"
    ));
}

#[test]
fn article_date_help_advertises_shared_accepted_formats() {
    let help = render_article_search_long_help();

    assert!(help.contains("Published after date (YYYY, YYYY-MM, or YYYY-MM-DD)"));
    assert!(help.contains("Published before date (YYYY, YYYY-MM, or YYYY-MM-DD)"));
    assert!(help.contains("[aliases: --since]"));
    assert!(help.contains("[aliases: --until]"));
    assert!(help.contains("--max-per-source <N>"));
    assert!(help.contains(
        "Cap each federated source's contribution after deduplication and before ranking."
    ));
    assert!(help.contains(
        "Default: 40% of `--limit` on federated pools with at least three surviving primary sources."
    ));
    assert!(help.contains("`0` uses the default cap."));
    assert!(help.contains("Setting it equal to `--limit` disables capping."));
}
