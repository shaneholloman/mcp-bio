use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, SystemTime};

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const WHO_IVD_EXPORT_PATH: &str =
    "/prequal/vitro-diagnostics/prequalified/in-vitro-diagnostics/export";
const WHO_IVD_CSV_FILE: &str = "who_ivd.csv";

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

fn temp_dir(label: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("biomcp-who-ivd-auto-sync-{label}-"))
        .tempdir()
        .expect("temp dir should be created")
}

fn default_who_ivd_root(data_home: &Path) -> PathBuf {
    data_home.join("biomcp").join("who-ivd")
}

fn load_fixture_body() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("spec")
        .join("fixtures")
        .join("who-ivd")
        .join(WHO_IVD_CSV_FILE);
    fs::read_to_string(path).expect("WHO IVD fixture should be readable")
}

fn invalid_fixture_body() -> String {
    "\"Product name\",\"WHO Product ID\",\"Assay Format\",\"Regulatory Version\",\"Manufacturer name\",\"Pathogen/Disease/Marker\",\"Year prequalification\"\n\
\"ONE STEP Anti-HIV (1&2) Test\",\"0372-017-00\",\"Immunochromatographic (lateral flow)\",\"Rest-of-World\",\"InTec Products, Inc.\",\"HIV\",\"2019\"\n"
        .to_string()
}

fn export_url(server: &MockServer) -> String {
    format!("{}{}?page&_format=csv", server.uri(), WHO_IVD_EXPORT_PATH)
}

async fn mount_success_server() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(WHO_IVD_EXPORT_PATH))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/csv; charset=utf-8")
                .set_body_string(load_fixture_body()),
        )
        .mount(&server)
        .await;
    server
}

async fn mount_failure_server(status: u16) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(WHO_IVD_EXPORT_PATH))
        .respond_with(
            ResponseTemplate::new(status)
                .insert_header("content-type", "text/plain; charset=utf-8")
                .set_body_string("who ivd upstream failure"),
        )
        .mount(&server)
        .await;
    server
}

async fn mount_header_failure_server() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(WHO_IVD_EXPORT_PATH))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/csv; charset=utf-8")
                .set_body_string(invalid_fixture_body()),
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
    command.env_remove("BIOMCP_WHO_IVD_DIR");
    command.env_remove("BIOMCP_WHO_IVD_URL");
    command.env_remove("BIOMCP_CACHE_MODE");
    command.env_remove("RUST_LOG");
    for (name, value) in extra_envs {
        command.env(name, value);
    }

    let output = command.output().expect("biomcp command should run");
    CommandResult::from_output(output)
}

async fn request_count(server: &MockServer) -> usize {
    server
        .received_requests()
        .await
        .expect("server should record requests")
        .into_iter()
        .filter(|request| request.url.path() == WHO_IVD_EXPORT_PATH)
        .count()
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

fn assert_hiv_search(result: &CommandResult) {
    assert!(
        result.status.success(),
        "expected successful WHO IVD search\nstdout:\n{}\nstderr:\n{}",
        result.stdout,
        result.stderr
    );
    assert!(result.stdout.contains("# Diagnostic tests: disease=HIV"));
    assert!(result.stdout.contains("ITPW02232- TC40"));
    assert!(result.stdout.contains("WHO Prequalified IVD"));
}

#[tokio::test]
async fn first_use_search_downloads_missing_csv_into_default_root() {
    let server = mount_success_server().await;
    let data_home = temp_dir("clean-data-home");
    let cache_home = temp_dir("clean-cache-home");
    let who_ivd_url = export_url(&server);

    let result = run_biomcp(
        &[
            "search",
            "diagnostic",
            "--disease",
            "HIV",
            "--source",
            "who-ivd",
            "--limit",
            "5",
        ],
        data_home.path(),
        cache_home.path(),
        &[("BIOMCP_WHO_IVD_URL", &who_ivd_url)],
    );

    assert_hiv_search(&result);
    assert!(result.stderr.contains("Downloading WHO IVD data"));
    assert!(
        default_who_ivd_root(data_home.path())
            .join(WHO_IVD_CSV_FILE)
            .is_file()
    );
    assert_eq!(request_count(&server).await, 1);
}

#[tokio::test]
async fn second_run_within_ttl_skips_redownload() {
    let server = mount_success_server().await;
    let data_home = temp_dir("fresh-data-home");
    let cache_home = temp_dir("fresh-cache-home");
    let who_ivd_url = export_url(&server);

    let first = run_biomcp(
        &[
            "search",
            "diagnostic",
            "--disease",
            "HIV",
            "--source",
            "who-ivd",
            "--limit",
            "5",
        ],
        data_home.path(),
        cache_home.path(),
        &[("BIOMCP_WHO_IVD_URL", &who_ivd_url)],
    );
    assert_hiv_search(&first);

    let second = run_biomcp(
        &[
            "search",
            "diagnostic",
            "--disease",
            "HIV",
            "--source",
            "who-ivd",
            "--limit",
            "5",
        ],
        data_home.path(),
        cache_home.path(),
        &[("BIOMCP_WHO_IVD_URL", &who_ivd_url)],
    );
    assert_hiv_search(&second);
    assert!(!second.stderr.contains("Downloading WHO IVD data"));
    assert!(!second.stderr.contains("Refreshing stale WHO IVD data"));
    assert_eq!(request_count(&server).await, 1);
}

#[tokio::test]
async fn stale_csv_refreshes_on_next_search() {
    let server = mount_success_server().await;
    let data_home = temp_dir("stale-data-home");
    let cache_home = temp_dir("stale-cache-home");
    let who_ivd_url = export_url(&server);

    let first = run_biomcp(
        &[
            "search",
            "diagnostic",
            "--disease",
            "HIV",
            "--source",
            "who-ivd",
            "--limit",
            "5",
        ],
        data_home.path(),
        cache_home.path(),
        &[("BIOMCP_WHO_IVD_URL", &who_ivd_url)],
    );
    assert_hiv_search(&first);

    set_stale(&default_who_ivd_root(data_home.path()).join(WHO_IVD_CSV_FILE));

    let second = run_biomcp(
        &[
            "search",
            "diagnostic",
            "--disease",
            "HIV",
            "--source",
            "who-ivd",
            "--limit",
            "5",
        ],
        data_home.path(),
        cache_home.path(),
        &[("BIOMCP_WHO_IVD_URL", &who_ivd_url)],
    );
    assert_hiv_search(&second);
    assert!(second.stderr.contains("Refreshing stale WHO IVD data"));
    assert_eq!(request_count(&server).await, 2);
}

#[tokio::test]
async fn who_ivd_sync_force_refreshes_and_honors_custom_root() {
    let server = mount_success_server().await;
    let data_home = temp_dir("custom-data-home");
    let cache_home = temp_dir("custom-cache-home");
    let custom_root = temp_dir("custom-who-ivd-root");
    let custom_root_string = custom_root.path().display().to_string();
    let who_ivd_url = export_url(&server);

    let first = run_biomcp(
        &["who-ivd", "sync"],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_WHO_IVD_DIR", &custom_root_string),
            ("BIOMCP_WHO_IVD_URL", &who_ivd_url),
        ],
    );

    assert!(
        first.status.success(),
        "expected successful who-ivd sync\nstdout:\n{}\nstderr:\n{}",
        first.stdout,
        first.stderr
    );
    assert!(
        first
            .stdout
            .contains("WHO IVD local diagnostic data synchronized successfully.")
    );
    assert!(first.stderr.contains("Refreshing WHO IVD data"));
    assert!(custom_root.path().join(WHO_IVD_CSV_FILE).is_file());
    assert!(
        !default_who_ivd_root(data_home.path())
            .join(WHO_IVD_CSV_FILE)
            .exists(),
        "default WHO IVD root should remain unused when BIOMCP_WHO_IVD_DIR is set"
    );

    let second = run_biomcp(
        &["who-ivd", "sync"],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_WHO_IVD_DIR", &custom_root_string),
            ("BIOMCP_WHO_IVD_URL", &who_ivd_url),
        ],
    );

    assert!(
        second.status.success(),
        "expected second successful who-ivd sync\nstdout:\n{}\nstderr:\n{}",
        second.stdout,
        second.stderr
    );
    assert!(second.stderr.contains("Refreshing WHO IVD data"));
    assert_eq!(request_count(&server).await, 2);
}

#[tokio::test]
async fn stale_local_csv_survives_refresh_failure_with_warning() {
    let success_server = mount_success_server().await;
    let failing_server = mount_failure_server(503).await;
    let data_home = temp_dir("fallback-data-home");
    let cache_home = temp_dir("fallback-cache-home");
    let success_url = export_url(&success_server);
    let failing_url = export_url(&failing_server);

    let first = run_biomcp(
        &[
            "search",
            "diagnostic",
            "--disease",
            "HIV",
            "--source",
            "who-ivd",
            "--limit",
            "5",
        ],
        data_home.path(),
        cache_home.path(),
        &[("BIOMCP_WHO_IVD_URL", &success_url)],
    );
    assert_hiv_search(&first);

    set_stale(&default_who_ivd_root(data_home.path()).join(WHO_IVD_CSV_FILE));

    let second = run_biomcp(
        &[
            "search",
            "diagnostic",
            "--disease",
            "HIV",
            "--source",
            "who-ivd",
            "--limit",
            "5",
        ],
        data_home.path(),
        cache_home.path(),
        &[("BIOMCP_WHO_IVD_URL", &failing_url)],
    );
    assert_hiv_search(&second);
    assert!(second.stderr.contains("Warning: WHO IVD refresh failed:"));
    assert!(second.stderr.contains("who_ivd.csv: HTTP 503"));
    assert!(second.stderr.contains("Using existing data."));
    assert!(
        request_count(&failing_server).await > 0,
        "expected stale refresh failure path to hit the WHO IVD export"
    );
}

#[tokio::test]
async fn who_ivd_sync_header_validation_failure_mentions_recovery_paths() {
    let server = mount_header_failure_server().await;
    let data_home = temp_dir("header-failure-data-home");
    let cache_home = temp_dir("header-failure-cache-home");
    let who_ivd_url = export_url(&server);

    let result = run_biomcp(
        &["who-ivd", "sync"],
        data_home.path(),
        cache_home.path(),
        &[("BIOMCP_WHO_IVD_URL", &who_ivd_url)],
    );

    assert!(
        !result.status.success(),
        "who-ivd sync should fail on malformed CSV headers\nstdout:\n{}\nstderr:\n{}",
        result.stdout,
        result.stderr
    );
    assert!(
        result
            .stderr
            .contains("who_ivd.csv is missing required column: product code")
    );
    assert!(result.stderr.contains("biomcp who-ivd sync"));
    assert!(result.stderr.contains("BIOMCP_WHO_IVD_DIR"));
    assert_eq!(request_count(&server).await, 1);
    assert!(
        !default_who_ivd_root(data_home.path())
            .join(WHO_IVD_CSV_FILE)
            .exists(),
        "invalid WHO IVD payload should not replace the local file"
    );
}
