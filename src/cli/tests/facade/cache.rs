use super::*;

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
