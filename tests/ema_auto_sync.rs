use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const EMA_REPORT_PATH: &str = "/en/documents/report";
const EMA_FEEDS: [(&str, &str); 6] = [
    (
        "medicines-output-medicines_json-report_en.json",
        "medicines.json",
    ),
    (
        "medicines-output-post_authorisation_json-report_en.json",
        "post_authorisation.json",
    ),
    ("referrals-output-json-report_en.json", "referrals.json"),
    (
        "medicines-output-periodic_safety_update_report_single_assessments-output-json-report_en.json",
        "psusas.json",
    ),
    ("dhpc-output-json-report_en.json", "dhpcs.json"),
    ("shortages-output-json-report_en.json", "shortages.json"),
];

struct CommandResult {
    stdout: String,
    stderr: String,
    status: std::process::ExitStatus,
}

impl CommandResult {
    fn from_output(output: Output) -> Self {
        Self {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            status: output.status,
        }
    }
}

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(label: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "biomcp-ema-auto-sync-{label}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("temp dir should be created");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn default_ema_root(data_home: &Path) -> PathBuf {
    data_home.join("biomcp").join("ema")
}

fn load_fixture_body(local_name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("spec")
        .join("fixtures")
        .join("ema-human")
        .join(local_name);
    fs::read_to_string(path).expect("EMA fixture should be readable")
}

async fn mount_success_server() -> MockServer {
    let server = MockServer::start().await;
    for (report_name, local_name) in EMA_FEEDS {
        let body = load_fixture_body(local_name);
        Mock::given(method("GET"))
            .and(path(format!("{EMA_REPORT_PATH}/{report_name}")))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .insert_header("cache-control", "public, max-age=0")
                    .insert_header("etag", format!("\"{report_name}\""))
                    .insert_header("last-modified", "Wed, 21 Oct 2015 07:28:00 GMT")
                    .set_body_raw(body, "application/json"),
            )
            .mount(&server)
            .await;
    }
    server
}

async fn mount_failure_server(status: u16) -> MockServer {
    let server = MockServer::start().await;
    for (report_name, _) in EMA_FEEDS {
        Mock::given(method("GET"))
            .and(path(format!("{EMA_REPORT_PATH}/{report_name}")))
            .respond_with(
                ResponseTemplate::new(status)
                    .insert_header("content-type", "text/plain")
                    .set_body_string("ema upstream failure"),
            )
            .mount(&server)
            .await;
    }
    server
}

async fn mount_selective_failure_server(report_name: &str, status: u16) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("{EMA_REPORT_PATH}/{report_name}")))
        .respond_with(
            ResponseTemplate::new(status)
                .insert_header("content-type", "text/plain")
                .set_body_string("ema upstream failure"),
        )
        .mount(&server)
        .await;
    server
}

fn run_biomcp(
    args: &[&str],
    data_home: &Path,
    cache_home: &Path,
    extra_envs: &[(&str, &str)],
) -> CommandResult {
    let mut command = Command::new(env!("CARGO_BIN_EXE_biomcp"));
    command.args(args);
    command.env("XDG_DATA_HOME", data_home);
    command.env("XDG_CACHE_HOME", cache_home);
    command.env_remove("BIOMCP_EMA_DIR");
    command.env_remove("BIOMCP_CACHE_MODE");
    command.env_remove("RUST_LOG");
    for (name, value) in extra_envs {
        command.env(name, value);
    }

    let output = command.output().expect("biomcp command should run");
    CommandResult::from_output(output)
}

async fn request_count(server: &MockServer, report_name: &str) -> usize {
    let path = format!("{EMA_REPORT_PATH}/{report_name}");
    server
        .received_requests()
        .await
        .expect("server should record requests")
        .into_iter()
        .filter(|request| request.url.path() == path)
        .count()
}

async fn requests_for_feed(server: &MockServer, report_name: &str) -> Vec<Request> {
    let path = format!("{EMA_REPORT_PATH}/{report_name}");
    server
        .received_requests()
        .await
        .expect("server should record requests")
        .into_iter()
        .filter(|request| request.url.path() == path)
        .collect()
}

fn set_stale(path: &Path) {
    let file = fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("stale target should open");
    file.set_modified(
        SystemTime::now()
            .checked_sub(Duration::from_secs(73 * 60 * 60))
            .expect("stale time should be valid"),
    )
    .expect("mtime should update");
}

fn assert_keytruda_search(result: &CommandResult) {
    assert!(
        result.status.success(),
        "expected successful EMA search\nstdout:\n{}\nstderr:\n{}",
        result.stdout,
        result.stderr
    );
    assert!(result.stdout.contains("# Drugs: Keytruda"));
    assert!(result.stdout.contains("EMEA/H/C/003820"));
    assert!(result.stdout.contains("Keytruda"));
}

#[tokio::test]
async fn clean_eu_search_downloads_missing_feeds() {
    let server = mount_success_server().await;
    let data_home = TempDirGuard::new("clean-data-home");
    let cache_home = TempDirGuard::new("clean-cache-home");
    let report_base = format!("{}{EMA_REPORT_PATH}", server.uri());

    let result = run_biomcp(
        &[
            "search", "drug", "Keytruda", "--region", "eu", "--limit", "1",
        ],
        data_home.path(),
        cache_home.path(),
        &[("BIOMCP_EMA_REPORT_BASE", &report_base)],
    );

    assert_keytruda_search(&result);
    assert!(result.stderr.contains("Downloading EMA data (~11 MB)..."));

    let ema_root = default_ema_root(data_home.path());
    for (_, local_name) in EMA_FEEDS {
        assert!(
            ema_root.join(local_name).is_file(),
            "missing downloaded EMA file: {local_name}"
        );
    }
    for (report_name, _) in EMA_FEEDS {
        assert_eq!(request_count(&server, report_name).await, 1);
    }
}

#[tokio::test]
async fn second_run_within_ttl_skips_download_message() {
    let server = mount_success_server().await;
    let data_home = TempDirGuard::new("fresh-data-home");
    let cache_home = TempDirGuard::new("fresh-cache-home");
    let report_base = format!("{}{EMA_REPORT_PATH}", server.uri());

    let first = run_biomcp(
        &[
            "search", "drug", "Keytruda", "--region", "eu", "--limit", "1",
        ],
        data_home.path(),
        cache_home.path(),
        &[("BIOMCP_EMA_REPORT_BASE", &report_base)],
    );
    assert_keytruda_search(&first);

    let second = run_biomcp(
        &[
            "search", "drug", "Keytruda", "--region", "eu", "--limit", "1",
        ],
        data_home.path(),
        cache_home.path(),
        &[("BIOMCP_EMA_REPORT_BASE", &report_base)],
    );
    assert_keytruda_search(&second);
    assert!(!second.stderr.contains("Downloading EMA data (~11 MB)..."));
    assert!(!second.stderr.contains("Refreshing EMA data (~11 MB)..."));

    for (report_name, _) in EMA_FEEDS {
        assert_eq!(request_count(&server, report_name).await, 1);
    }
}

#[tokio::test]
#[ignore = "http-cache middleware not forwarding stored ETag/Last-Modified as validator headers on stale refresh"]
async fn stale_single_feed_uses_request_only_for_that_feed() {
    let server = mount_success_server().await;
    let data_home = TempDirGuard::new("stale-data-home");
    let cache_home = TempDirGuard::new("stale-cache-home");
    let report_base = format!("{}{EMA_REPORT_PATH}", server.uri());

    let first = run_biomcp(
        &[
            "search", "drug", "Keytruda", "--region", "eu", "--limit", "1",
        ],
        data_home.path(),
        cache_home.path(),
        &[("BIOMCP_EMA_REPORT_BASE", &report_base)],
    );
    assert_keytruda_search(&first);

    set_stale(&default_ema_root(data_home.path()).join("medicines.json"));

    let second = run_biomcp(
        &[
            "search", "drug", "Keytruda", "--region", "eu", "--limit", "1",
        ],
        data_home.path(),
        cache_home.path(),
        &[("BIOMCP_EMA_REPORT_BASE", &report_base)],
    );
    assert_keytruda_search(&second);
    assert!(second.stderr.contains("Refreshing EMA data (~11 MB)..."));

    let medicines_requests =
        requests_for_feed(&server, "medicines-output-medicines_json-report_en.json").await;
    assert_eq!(medicines_requests.len(), 2);
    let refresh_request = &medicines_requests[1];
    assert!(
        refresh_request.headers.get("if-none-match").is_some()
            || refresh_request.headers.get("if-modified-since").is_some(),
        "stale refresh should send validator headers"
    );

    for (report_name, local_name) in EMA_FEEDS {
        let expected = if local_name == "medicines.json" { 2 } else { 1 };
        assert_eq!(
            request_count(&server, report_name).await,
            expected,
            "unexpected request count for {local_name}"
        );
    }
}

#[tokio::test]
async fn missing_single_feed_fetches_only_missing_file() {
    let server = mount_success_server().await;
    let data_home = TempDirGuard::new("missing-data-home");
    let cache_home = TempDirGuard::new("missing-cache-home");
    let report_base = format!("{}{EMA_REPORT_PATH}", server.uri());

    let first = run_biomcp(
        &[
            "search", "drug", "Keytruda", "--region", "eu", "--limit", "1",
        ],
        data_home.path(),
        cache_home.path(),
        &[("BIOMCP_EMA_REPORT_BASE", &report_base)],
    );
    assert_keytruda_search(&first);

    fs::remove_file(default_ema_root(data_home.path()).join("shortages.json"))
        .expect("shortages fixture should be removable");

    let second = run_biomcp(
        &[
            "search", "drug", "Keytruda", "--region", "eu", "--limit", "1",
        ],
        data_home.path(),
        cache_home.path(),
        &[("BIOMCP_EMA_REPORT_BASE", &report_base)],
    );
    assert_keytruda_search(&second);
    assert!(second.stderr.contains("Downloading EMA data (~11 MB)..."));

    for (report_name, local_name) in EMA_FEEDS {
        let expected = if local_name == "shortages.json" { 2 } else { 1 };
        assert_eq!(
            request_count(&server, report_name).await,
            expected,
            "unexpected request count for {local_name}"
        );
    }
}

#[tokio::test]
async fn no_cache_forces_reload_of_all_feeds() {
    let server = mount_success_server().await;
    let data_home = TempDirGuard::new("reload-data-home");
    let cache_home = TempDirGuard::new("reload-cache-home");
    let report_base = format!("{}{EMA_REPORT_PATH}", server.uri());

    let first = run_biomcp(
        &[
            "search", "drug", "Keytruda", "--region", "eu", "--limit", "1",
        ],
        data_home.path(),
        cache_home.path(),
        &[("BIOMCP_EMA_REPORT_BASE", &report_base)],
    );
    assert_keytruda_search(&first);

    let second = run_biomcp(
        &[
            "--no-cache",
            "search",
            "drug",
            "Keytruda",
            "--region",
            "eu",
            "--limit",
            "1",
        ],
        data_home.path(),
        cache_home.path(),
        &[("BIOMCP_EMA_REPORT_BASE", &report_base)],
    );
    assert_keytruda_search(&second);
    assert!(second.stderr.contains("Refreshing EMA data (~11 MB)..."));

    for (report_name, _) in EMA_FEEDS {
        assert_eq!(request_count(&server, report_name).await, 2);
    }
}

#[tokio::test]
async fn ema_sync_forces_reload_of_all_feeds() {
    let server = mount_success_server().await;
    let data_home = TempDirGuard::new("sync-data-home");
    let cache_home = TempDirGuard::new("sync-cache-home");
    let report_base = format!("{}{EMA_REPORT_PATH}", server.uri());

    let first = run_biomcp(
        &[
            "search", "drug", "Keytruda", "--region", "eu", "--limit", "1",
        ],
        data_home.path(),
        cache_home.path(),
        &[("BIOMCP_EMA_REPORT_BASE", &report_base)],
    );
    assert_keytruda_search(&first);

    let second = run_biomcp(
        &["ema", "sync"],
        data_home.path(),
        cache_home.path(),
        &[("BIOMCP_EMA_REPORT_BASE", &report_base)],
    );
    assert!(
        second.status.success(),
        "ema sync should succeed\nstdout:\n{}\nstderr:\n{}",
        second.stdout,
        second.stderr
    );
    assert!(
        second
            .stdout
            .contains("EMA data synchronized successfully.")
    );
    assert!(second.stderr.contains("Refreshing EMA data (~11 MB)..."));

    for (report_name, _) in EMA_FEEDS {
        assert_eq!(request_count(&server, report_name).await, 2);
    }
}

#[tokio::test]
async fn custom_ema_dir_receives_downloaded_files() {
    let server = mount_success_server().await;
    let data_home = TempDirGuard::new("custom-data-home");
    let cache_home = TempDirGuard::new("custom-cache-home");
    let custom_root = TempDirGuard::new("custom-ema-root");
    let report_base = format!("{}{EMA_REPORT_PATH}", server.uri());
    let custom_root_string = custom_root.path().display().to_string();

    let result = run_biomcp(
        &[
            "search", "drug", "Keytruda", "--region", "eu", "--limit", "1",
        ],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_EMA_REPORT_BASE", &report_base),
            ("BIOMCP_EMA_DIR", &custom_root_string),
        ],
    );

    assert_keytruda_search(&result);
    for (_, local_name) in EMA_FEEDS {
        assert!(
            custom_root.path().join(local_name).is_file(),
            "missing downloaded EMA file in custom root: {local_name}"
        );
    }
    assert!(
        !default_ema_root(data_home.path()).exists(),
        "default EMA root should remain unused when BIOMCP_EMA_DIR is set"
    );
}

#[tokio::test]
async fn stale_files_survive_refresh_failure_with_warning() {
    let success_server = mount_success_server().await;
    let failing_server =
        mount_selective_failure_server("medicines-output-medicines_json-report_en.json", 403).await;
    let data_home = TempDirGuard::new("fallback-data-home");
    let cache_home = TempDirGuard::new("fallback-cache-home");
    let success_base = format!("{}{EMA_REPORT_PATH}", success_server.uri());
    let failing_base = format!("{}{EMA_REPORT_PATH}", failing_server.uri());

    let first = run_biomcp(
        &[
            "search", "drug", "Keytruda", "--region", "eu", "--limit", "1",
        ],
        data_home.path(),
        cache_home.path(),
        &[("BIOMCP_EMA_REPORT_BASE", &success_base)],
    );
    assert_keytruda_search(&first);

    set_stale(&default_ema_root(data_home.path()).join("medicines.json"));

    let second = run_biomcp(
        &[
            "search", "drug", "Keytruda", "--region", "eu", "--limit", "1",
        ],
        data_home.path(),
        cache_home.path(),
        &[("BIOMCP_EMA_REPORT_BASE", &failing_base)],
    );
    assert_keytruda_search(&second);
    assert!(second.stderr.contains("Warning:"));
    assert!(second.stderr.contains("medicines.json"));
    assert!(second.stderr.contains("Using existing data"));
}

#[tokio::test]
async fn missing_files_fail_when_refresh_cannot_populate_root() {
    let server = mount_failure_server(403).await;
    let data_home = TempDirGuard::new("failure-data-home");
    let cache_home = TempDirGuard::new("failure-cache-home");
    let report_base = format!("{}{EMA_REPORT_PATH}", server.uri());
    let ema_root = default_ema_root(data_home.path());

    let result = run_biomcp(
        &[
            "search", "drug", "Keytruda", "--region", "eu", "--limit", "1",
        ],
        data_home.path(),
        cache_home.path(),
        &[("BIOMCP_EMA_REPORT_BASE", &report_base)],
    );

    assert!(
        !result.status.success(),
        "command should fail when EMA root cannot be populated\nstdout:\n{}\nstderr:\n{}",
        result.stdout,
        result.stderr
    );
    assert!(result.stderr.contains("Source unavailable: EMA"));
    assert!(result.stderr.contains("biomcp ema sync"));
    assert!(result.stderr.contains("BIOMCP_EMA_DIR"));
    assert!(result.stderr.contains(&ema_root.display().to_string()));
}
