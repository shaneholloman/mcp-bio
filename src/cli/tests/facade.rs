use clap::{CommandFactory, FromArgMatches, Parser};

use super::super::test_support::{TempDirGuard, lock_env, set_env_var};
use super::super::{ChartArgs, Cli, Commands, McpChartPass, execute, rewrite_mcp_chart_args};

#[test]
fn skill_help_examples_match_installed_surface() {
    let mut command = Cli::command();
    let skill = command
        .find_subcommand_mut("skill")
        .expect("skill subcommand should exist");
    let mut help = Vec::new();
    skill
        .write_long_help(&mut help)
        .expect("skill help should render");
    let help = String::from_utf8(help).expect("help should be utf-8");

    assert!(help.contains("biomcp skill            # show skill overview"));
    assert!(help.contains("biomcp skill install    # install skill to your agent config"));
    assert!(help.contains("Commands:\n  list"));
    assert!(!help.contains("biomcp skill 03"));
    assert!(!help.contains("variant-to-treatment"));
}

#[test]
fn runtime_help_hides_query_only_global_flags() {
    for subcommand_name in super::super::RUNTIME_HELP_SUBCOMMANDS {
        let mut command = super::super::build_cli();
        let runtime = command
            .find_subcommand_mut(subcommand_name)
            .expect("runtime subcommand should exist");
        let mut help = Vec::new();
        runtime
            .write_long_help(&mut help)
            .expect("runtime help should render");
        let help = String::from_utf8(help).expect("help should be utf-8");

        assert!(
            !help.contains("--json"),
            "{subcommand_name} help should not advertise --json"
        );
        assert!(
            !help.contains("--no-cache"),
            "{subcommand_name} help should not advertise --no-cache"
        );
    }
}

#[test]
fn runtime_commands_still_parse_hidden_global_flags() {
    let cli = parse_built_cli([
        "biomcp",
        "serve-http",
        "--json",
        "--no-cache",
        "--host",
        "127.0.0.1",
        "--port",
        "8080",
    ]);
    assert!(cli.json);
    assert!(cli.no_cache);
    assert!(matches!(
        cli.command,
        Commands::ServeHttp(super::super::system::ServeHttpArgs { host, port })
            if host == "127.0.0.1" && port == 8080
    ));

    for args in [
        ["biomcp", "mcp", "--json", "--no-cache"].as_slice(),
        ["biomcp", "serve", "--json", "--no-cache"].as_slice(),
        ["biomcp", "serve-sse", "--json", "--no-cache"].as_slice(),
    ] {
        let cli = parse_built_cli(args);
        assert!(cli.json);
        assert!(cli.no_cache);
    }
}

#[test]
fn serve_sse_help_stays_callable_and_deprecated() {
    let mut command = super::super::build_cli();
    let serve_sse = command
        .find_subcommand_mut("serve-sse")
        .expect("serve-sse subcommand should exist");
    let mut help = Vec::new();
    serve_sse
        .write_long_help(&mut help)
        .expect("serve-sse help should render");
    let help = String::from_utf8(help).expect("help should be utf-8");

    assert!(help.contains("serve-sse"));
    assert!(help.contains("removed"));
    assert!(help.contains("serve-http"));
    assert!(help.contains("/mcp"));
    assert!(!help.contains("--json"));
    assert!(!help.contains("--no-cache"));
}

#[test]
fn top_level_help_hides_serve_sse_but_keeps_serve_http() {
    let mut command = super::super::build_cli();
    let mut help = Vec::new();
    command
        .write_long_help(&mut help)
        .expect("top-level help should render");
    let help = String::from_utf8(help).expect("help should be utf-8");

    assert!(help.contains("serve-http"));
    assert!(!help.contains("serve-sse"));
}

#[test]
fn cache_path_command_parses() {
    Cli::try_parse_from(["biomcp", "cache", "path"]).expect("cache path should parse");
}

#[test]
fn cache_stats_command_parses() {
    Cli::try_parse_from(["biomcp", "cache", "stats"]).expect("cache stats should parse");
}

#[test]
fn cache_clean_command_parses_with_flags() {
    let cli = Cli::try_parse_from([
        "biomcp",
        "cache",
        "clean",
        "--max-age",
        "30d",
        "--max-size",
        "500M",
        "--dry-run",
    ])
    .expect("cache clean should parse");

    let Cli {
        command:
            Commands::Cache {
                cmd:
                    crate::cli::cache::CacheCommand::Clean {
                        max_age,
                        max_size,
                        dry_run,
                    },
            },
        ..
    } = cli
    else {
        panic!("expected cache clean command");
    };

    assert_eq!(
        max_age,
        Some(std::time::Duration::from_secs(30 * 24 * 60 * 60))
    );
    assert_eq!(max_size, Some(500_000_000));
    assert!(dry_run);
}

#[test]
fn cache_clear_command_parses() {
    Cli::try_parse_from(["biomcp", "cache", "clear"]).expect("cache clear should parse");
}

#[test]
fn cache_clear_command_parses_with_yes_flag() {
    Cli::try_parse_from(["biomcp", "cache", "clear", "--yes"])
        .expect("cache clear --yes should parse");
}

#[test]
fn top_level_help_lists_cache_command() {
    let mut command = super::super::build_cli();
    let mut help = Vec::new();
    command
        .write_long_help(&mut help)
        .expect("top-level help should render");
    let help = String::from_utf8(help).expect("help should be utf-8");

    assert!(
        help.lines()
            .any(|line| line.trim_start().starts_with("cache")),
        "top-level help should list the cache family: {help}"
    );
}

#[test]
fn top_level_help_mentions_cache_path_json_exception() {
    let mut command = super::super::build_cli();
    let mut help = Vec::new();
    command
        .write_long_help(&mut help)
        .expect("top-level help should render");
    let help = String::from_utf8(help).expect("help should be utf-8");

    assert!(help.contains("except biomcp cache path"));
    assert!(help.contains("stays plain text"));
}

#[test]
fn cache_path_help_mentions_plain_text_and_ignored_json() {
    let help = render_cache_path_long_help();

    assert!(help.contains("plain text"));
    assert!(help.contains("--json"));
    assert!(help.contains("ignored"));
}

#[test]
fn cache_stats_help_mentions_json_and_cli_only() {
    let help = render_cache_stats_long_help();

    assert!(help.contains("cache statistics"));
    assert!(help.contains("--json"));
    assert!(help.contains("CLI-only"));
    assert!(help.contains("local filesystem paths"));
}

#[test]
fn cache_clean_help_mentions_dry_run_json_and_limits() {
    let help = render_cache_clean_long_help();

    assert!(help.contains("--max-age"));
    assert!(help.contains("--max-size"));
    assert!(help.contains("--dry-run"));
    assert!(help.contains("--json"));
    assert!(help.contains("orphan"));
}

#[test]
fn cache_clear_help_mentions_yes_tty_and_destructive_scope() {
    let help = render_cache_clear_long_help();

    assert!(help.contains("--yes"));
    assert!(help.contains("TTY"));
    assert!(help.contains("downloads"));
    assert!(help.contains("destructive"));
}

#[test]
fn cache_help_lists_clear_subcommand() {
    let help = render_cache_long_help();

    assert!(help.contains("clear"));
}

#[test]
fn top_level_help_describes_cache_family_not_path_only() {
    let mut command = super::super::build_cli();
    let mut help = Vec::new();
    command
        .write_long_help(&mut help)
        .expect("top-level help should render");
    let help = String::from_utf8(help).expect("help should be utf-8");

    assert!(help.contains(
        "Inspect the managed HTTP cache (CLI-only; cache commands reveal workstation-local filesystem paths)"
    ));
    assert!(
        !help
            .contains("Print the managed HTTP cache path (CLI-only; plain text; ignores `--json`)")
    );
}

fn render_cache_path_long_help() -> String {
    let mut command = Cli::command();
    let cache = command
        .find_subcommand_mut("cache")
        .expect("cache subcommand should exist");
    let path = cache
        .find_subcommand_mut("path")
        .expect("cache path subcommand should exist");
    let mut help = Vec::new();
    path.write_long_help(&mut help)
        .expect("cache path help should render");
    String::from_utf8(help).expect("help should be utf-8")
}

fn render_cache_long_help() -> String {
    let mut command = Cli::command();
    let cache = command
        .find_subcommand_mut("cache")
        .expect("cache subcommand should exist");
    let mut help = Vec::new();
    cache
        .write_long_help(&mut help)
        .expect("cache help should render");
    String::from_utf8(help).expect("help should be utf-8")
}

fn render_cache_stats_long_help() -> String {
    let mut command = Cli::command();
    let cache = command
        .find_subcommand_mut("cache")
        .expect("cache subcommand should exist");
    let stats = cache
        .find_subcommand_mut("stats")
        .expect("cache stats subcommand should exist");
    let mut help = Vec::new();
    stats
        .write_long_help(&mut help)
        .expect("cache stats help should render");
    String::from_utf8(help).expect("help should be utf-8")
}

fn render_cache_clean_long_help() -> String {
    let mut command = Cli::command();
    let cache = command
        .find_subcommand_mut("cache")
        .expect("cache subcommand should exist");
    let clean = cache
        .find_subcommand_mut("clean")
        .expect("cache clean subcommand should exist");
    let mut help = Vec::new();
    clean
        .write_long_help(&mut help)
        .expect("cache clean help should render");
    String::from_utf8(help).expect("help should be utf-8")
}

fn render_cache_clear_long_help() -> String {
    let mut command = Cli::command();
    let cache = command
        .find_subcommand_mut("cache")
        .expect("cache subcommand should exist");
    let clear = cache
        .find_subcommand_mut("clear")
        .expect("cache clear subcommand should exist");
    let mut help = Vec::new();
    clear
        .write_long_help(&mut help)
        .expect("cache clear help should render");
    String::from_utf8(help).expect("help should be utf-8")
}

fn parse_built_cli<I, T>(args: I) -> Cli
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let matches = super::super::build_cli()
        .try_get_matches_from(args)
        .expect("args should parse with canonical CLI");
    Cli::from_arg_matches(&matches).expect("matches should decode into Cli")
}

#[test]
fn chart_args_default_to_no_chart() {
    let args = ChartArgs {
        chart: None,
        terminal: false,
        output: None,
        title: None,
        theme: None,
        palette: None,
        cols: None,
        rows: None,
        width: None,
        height: None,
        scale: None,
        mcp_inline: false,
    };
    assert_eq!(args.chart, None);
    assert!(!args.terminal);
    assert!(!args.mcp_inline);
    assert_eq!(args.cols, None);
    assert_eq!(args.rows, None);
    assert_eq!(args.width, None);
    assert_eq!(args.height, None);
    assert_eq!(args.scale, None);
}

#[test]
fn chart_dimension_flags_validate_positive_values() {
    let cols_err = Cli::try_parse_from([
        "biomcp",
        "study",
        "query",
        "--study",
        "msk_impact_2017",
        "--gene",
        "TP53",
        "--type",
        "mutations",
        "--chart",
        "bar",
        "--cols",
        "0",
    ])
    .expect_err("zero columns should fail");
    assert!(cols_err.to_string().contains("--cols must be >= 1"));

    let scale_err = Cli::try_parse_from([
        "biomcp",
        "study",
        "query",
        "--study",
        "msk_impact_2017",
        "--gene",
        "TP53",
        "--type",
        "mutations",
        "--chart",
        "bar",
        "--scale",
        "0",
    ])
    .expect_err("zero scale should fail");
    assert!(scale_err.to_string().contains("--scale must be > 0"));

    let nan_err = Cli::try_parse_from([
        "biomcp",
        "study",
        "query",
        "--study",
        "msk_impact_2017",
        "--gene",
        "TP53",
        "--type",
        "mutations",
        "--chart",
        "bar",
        "--scale",
        "NaN",
        "-o",
        "chart.png",
    ])
    .expect_err("non-finite scale should fail");
    assert!(
        nan_err
            .to_string()
            .contains("--scale must be a finite number > 0")
    );
}

#[test]
fn rewrite_mcp_chart_args_preserves_svg_sizing_flags() {
    let args = vec![
        "biomcp".to_string(),
        "study".to_string(),
        "query".to_string(),
        "--study".to_string(),
        "demo".to_string(),
        "--gene".to_string(),
        "TP53".to_string(),
        "--type".to_string(),
        "mutations".to_string(),
        "--chart".to_string(),
        "bar".to_string(),
        "--width".to_string(),
        "1200".to_string(),
        "--height".to_string(),
        "600".to_string(),
        "--title".to_string(),
        "Example".to_string(),
    ];

    let text = rewrite_mcp_chart_args(&args, McpChartPass::Text).expect("text rewrite");
    assert!(!text.iter().any(|value| value == "--chart"));
    assert!(!text.iter().any(|value| value == "--width"));
    assert!(!text.iter().any(|value| value == "--height"));

    let svg = rewrite_mcp_chart_args(&args, McpChartPass::Svg).expect("svg rewrite");
    assert!(svg.iter().any(|value| value == "--chart"));
    assert!(svg.iter().any(|value| value == "--width"));
    assert!(svg.iter().any(|value| value == "--height"));
    assert!(svg.iter().any(|value| value == "--mcp-inline"));
}

#[test]
fn rewrite_mcp_chart_args_rejects_terminal_and_png_only_flags() {
    let cols_err = rewrite_mcp_chart_args(
        &[
            "biomcp".to_string(),
            "study".to_string(),
            "query".to_string(),
            "--study".to_string(),
            "demo".to_string(),
            "--gene".to_string(),
            "TP53".to_string(),
            "--type".to_string(),
            "mutations".to_string(),
            "--chart".to_string(),
            "bar".to_string(),
            "--cols".to_string(),
            "80".to_string(),
        ],
        McpChartPass::Svg,
    )
    .expect_err("mcp svg rewrite should reject terminal sizing");
    assert!(
        cols_err
            .to_string()
            .contains("--cols/--rows require terminal chart output"),
        "{cols_err}"
    );

    let scale_err = rewrite_mcp_chart_args(
        &[
            "biomcp".to_string(),
            "study".to_string(),
            "query".to_string(),
            "--study".to_string(),
            "demo".to_string(),
            "--gene".to_string(),
            "TP53".to_string(),
            "--type".to_string(),
            "mutations".to_string(),
            "--chart".to_string(),
            "bar".to_string(),
            "--scale".to_string(),
            "2.0".to_string(),
        ],
        McpChartPass::Svg,
    )
    .expect_err("mcp svg rewrite should reject png scale");
    assert!(
        scale_err
            .to_string()
            .contains("--scale requires PNG chart output"),
        "{scale_err}"
    );
}

#[tokio::test]
async fn search_all_requires_at_least_one_typed_slot() {
    let err = execute(vec![
        "biomcp".to_string(),
        "search".to_string(),
        "all".to_string(),
    ])
    .await
    .expect_err("search all should require typed slots");
    assert!(err.to_string().contains("at least one typed slot"));
    assert!(err.to_string().contains("--gene"));
}

#[tokio::test]
async fn json_cache_path_still_returns_plain_text() {
    let _guard = lock_env().await;
    let root = TempDirGuard::new("cache-path-json");
    let cache_home = root.path().join("cache-home");
    let config_home = root.path().join("config-home");
    std::fs::create_dir_all(&cache_home).expect("create cache home");
    std::fs::create_dir_all(&config_home).expect("create config home");
    let _cache_home = set_env_var("XDG_CACHE_HOME", Some(&cache_home.to_string_lossy()));
    let _config_home = set_env_var("XDG_CONFIG_HOME", Some(&config_home.to_string_lossy()));
    let _cache_dir = set_env_var("BIOMCP_CACHE_DIR", None);
    let _cache_size = set_env_var("BIOMCP_CACHE_MAX_SIZE", None);

    let output = execute(vec![
        "biomcp".to_string(),
        "--json".to_string(),
        "cache".to_string(),
        "path".to_string(),
    ])
    .await
    .expect("cache path should execute");

    assert_eq!(
        output.trim(),
        cache_home.join("biomcp").join("http").display().to_string()
    );
    assert!(!output.trim_start().starts_with('{'));
}

#[tokio::test]
async fn cache_stats_execute_returns_markdown_table() {
    let _guard = lock_env().await;
    let root = TempDirGuard::new("cache-stats-text");
    let cache_home = root.path().join("cache-home");
    let config_home = root.path().join("config-home");
    std::fs::create_dir_all(&cache_home).expect("create cache home");
    std::fs::create_dir_all(&config_home).expect("create config home");
    let _cache_home = set_env_var("XDG_CACHE_HOME", Some(&cache_home.to_string_lossy()));
    let _config_home = set_env_var("XDG_CONFIG_HOME", Some(&config_home.to_string_lossy()));
    let _cache_dir = set_env_var("BIOMCP_CACHE_DIR", None);
    let _cache_size = set_env_var("BIOMCP_CACHE_MAX_SIZE", None);
    let _cache_age = set_env_var("BIOMCP_CACHE_MAX_AGE", None);

    let output = execute(vec![
        "biomcp".to_string(),
        "cache".to_string(),
        "stats".to_string(),
    ])
    .await
    .expect("cache stats should execute");

    for row in [
        "| Path |",
        "| Blob bytes |",
        "| Blob files |",
        "| Orphan blobs |",
        "| Age range |",
        "| Max size |",
        "| Max age |",
    ] {
        assert!(output.contains(row), "missing row {row}: {output}");
    }
    assert!(!output.trim_start().starts_with('{'));
}

#[tokio::test]
async fn cache_stats_execute_json_returns_structured_report() {
    let _guard = lock_env().await;
    let root = TempDirGuard::new("cache-stats-json");
    let cache_home = root.path().join("cache-home");
    let config_home = root.path().join("config-home");
    std::fs::create_dir_all(&cache_home).expect("create cache home");
    std::fs::create_dir_all(&config_home).expect("create config home");
    let _cache_home = set_env_var("XDG_CACHE_HOME", Some(&cache_home.to_string_lossy()));
    let _config_home = set_env_var("XDG_CONFIG_HOME", Some(&config_home.to_string_lossy()));
    let _cache_dir = set_env_var("BIOMCP_CACHE_DIR", None);
    let _cache_size = set_env_var("BIOMCP_CACHE_MAX_SIZE", None);
    let _cache_age = set_env_var("BIOMCP_CACHE_MAX_AGE", None);

    let output = execute(vec![
        "biomcp".to_string(),
        "--json".to_string(),
        "cache".to_string(),
        "stats".to_string(),
    ])
    .await
    .expect("cache stats json should execute");

    let value: serde_json::Value =
        serde_json::from_str(&output).expect("cache stats json should be valid");
    for key in [
        "path",
        "blob_bytes",
        "blob_count",
        "orphan_count",
        "age_range",
        "max_size_bytes",
        "max_size_origin",
        "max_age_secs",
        "max_age_origin",
    ] {
        assert!(value.get(key).is_some(), "missing key {key}: {value}");
    }
    assert!(!output.contains("| Path |"));
    assert!(!output.contains("| Blob bytes |"));
}

#[tokio::test]
async fn cache_clean_execute_returns_single_line_summary() {
    let _guard = lock_env().await;
    let root = TempDirGuard::new("cache-clean-text");
    let cache_home = root.path().join("cache-home");
    let config_home = root.path().join("config-home");
    std::fs::create_dir_all(&cache_home).expect("create cache home");
    std::fs::create_dir_all(&config_home).expect("create config home");
    let _cache_home = set_env_var("XDG_CACHE_HOME", Some(&cache_home.to_string_lossy()));
    let _config_home = set_env_var("XDG_CONFIG_HOME", Some(&config_home.to_string_lossy()));
    let _cache_dir = set_env_var("BIOMCP_CACHE_DIR", None);
    let _cache_size = set_env_var("BIOMCP_CACHE_MAX_SIZE", None);

    let output = execute(vec![
        "biomcp".to_string(),
        "cache".to_string(),
        "clean".to_string(),
    ])
    .await
    .expect("cache clean should execute");

    assert!(output.starts_with("Cache clean:"));
    assert!(output.contains("dry_run=false"));
    assert_eq!(output.lines().count(), 1);
}

#[tokio::test]
async fn cache_clean_execute_json_returns_structured_report() {
    let _guard = lock_env().await;
    let root = TempDirGuard::new("cache-clean-json");
    let cache_home = root.path().join("cache-home");
    let config_home = root.path().join("config-home");
    std::fs::create_dir_all(&cache_home).expect("create cache home");
    std::fs::create_dir_all(&config_home).expect("create config home");
    let _cache_home = set_env_var("XDG_CACHE_HOME", Some(&cache_home.to_string_lossy()));
    let _config_home = set_env_var("XDG_CONFIG_HOME", Some(&config_home.to_string_lossy()));
    let _cache_dir = set_env_var("BIOMCP_CACHE_DIR", None);
    let _cache_size = set_env_var("BIOMCP_CACHE_MAX_SIZE", None);

    let output = execute(vec![
        "biomcp".to_string(),
        "--json".to_string(),
        "cache".to_string(),
        "clean".to_string(),
    ])
    .await
    .expect("cache clean json should execute");

    let value: serde_json::Value =
        serde_json::from_str(&output).expect("cache clean json should be valid");
    for key in [
        "dry_run",
        "orphans_removed",
        "entries_removed",
        "bytes_freed",
        "errors",
    ] {
        assert!(value.get(key).is_some(), "missing key {key}: {value}");
    }
}
