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
