use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use flate2::Compression;
use flate2::write::GzEncoder;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const GTR_TEST_VERSION_PATH: &str = "/pub/GTR/data/test_version.gz";
const GTR_CONDITION_GENE_PATH: &str = "/pub/GTR/data/test_condition_gene.txt";
const GTR_TEST_VERSION_FILE: &str = "test_version.gz";
const GTR_CONDITION_GENE_FILE: &str = "test_condition_gene.txt";

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
            "biomcp-gtr-auto-sync-{label}-{}-{stamp}",
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

fn default_gtr_root(data_home: &Path) -> PathBuf {
    data_home.join("biomcp").join("gtr")
}

fn load_test_version_fixture_body() -> Vec<u8> {
    include_bytes!("../spec/fixtures/gtr/test_version.gz").to_vec()
}

fn load_condition_gene_fixture_body() -> String {
    include_str!("../spec/fixtures/gtr/test_condition_gene.txt").to_string()
}

fn invalid_test_version_fixture_body() -> Vec<u8> {
    let payload = "lab_test_name\tnow_current\nBroken panel\t1\n";
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(payload.as_bytes())
        .expect("write invalid gzip fixture");
    encoder.finish().expect("finish invalid gzip fixture")
}

fn invalid_condition_gene_fixture_body() -> String {
    "accession_version\tobject\tobject_name\nGTR000000001.1\tgene\tBRCA1\n".to_string()
}

fn gtr_test_version_url(server: &MockServer) -> String {
    format!("{}{}", server.uri(), GTR_TEST_VERSION_PATH)
}

fn gtr_condition_gene_url(server: &MockServer) -> String {
    format!("{}{}", server.uri(), GTR_CONDITION_GENE_PATH)
}

async fn mount_success_server() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(GTR_TEST_VERSION_PATH))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/gzip")
                .set_body_raw(load_test_version_fixture_body(), "application/gzip"),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(GTR_CONDITION_GENE_PATH))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/tab-separated-values; charset=utf-8")
                .set_body_string(load_condition_gene_fixture_body()),
        )
        .mount(&server)
        .await;
    server
}

async fn mount_parse_failure_server() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(GTR_TEST_VERSION_PATH))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/gzip")
                .set_body_raw(invalid_test_version_fixture_body(), "application/gzip"),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(GTR_CONDITION_GENE_PATH))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/tab-separated-values; charset=utf-8")
                .set_body_string(invalid_condition_gene_fixture_body()),
        )
        .mount(&server)
        .await;
    server
}

async fn mount_download_failure_server() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(GTR_TEST_VERSION_PATH))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/gzip")
                .set_body_raw(load_test_version_fixture_body(), "application/gzip"),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(GTR_CONDITION_GENE_PATH))
        .respond_with(
            ResponseTemplate::new(503)
                .insert_header("content-type", "text/plain; charset=utf-8")
                .set_body_string("gtr upstream failure"),
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
    command.env_remove("BIOMCP_GTR_DIR");
    command.env_remove("BIOMCP_GTR_TEST_VERSION_URL");
    command.env_remove("BIOMCP_GTR_CONDITION_GENE_URL");
    command.env_remove("BIOMCP_CACHE_MODE");
    command.env_remove("RUST_LOG");
    for (name, value) in extra_envs {
        command.env(name, value);
    }

    let output = command.output().expect("biomcp command should run");
    CommandResult::from_output(output)
}

async fn requests_for_path(server: &MockServer, request_path: &str) -> Vec<Request> {
    server
        .received_requests()
        .await
        .expect("server should record requests")
        .into_iter()
        .filter(|request| request.url.path() == request_path)
        .collect()
}

fn set_stale(path: &Path) {
    let file = fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("stale target should open");
    file.set_modified(
        SystemTime::now()
            .checked_sub(Duration::from_secs(8 * 24 * 60 * 60))
            .expect("stale time should be valid"),
    )
    .expect("mtime should update");
}

fn assert_brca1_search(result: &CommandResult) {
    assert!(
        result.status.success(),
        "expected successful diagnostic search\nstdout:\n{}\nstderr:\n{}",
        result.stdout,
        result.stderr
    );
    assert!(result.stdout.contains("# Diagnostic tests: gene=BRCA1"));
    assert!(result.stdout.contains("GTR000000001.1"));
    assert!(result.stdout.contains("BRCA1 Hereditary Cancer Panel"));
}

#[tokio::test]
async fn first_use_search_downloads_missing_bundle_into_default_root() {
    let server = mount_success_server().await;
    let data_home = TempDirGuard::new("clean-data-home");
    let cache_home = TempDirGuard::new("clean-cache-home");
    let test_version_url = gtr_test_version_url(&server);
    let condition_gene_url = gtr_condition_gene_url(&server);

    let result = run_biomcp(
        &["search", "diagnostic", "--gene", "BRCA1", "--limit", "5"],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_GTR_TEST_VERSION_URL", &test_version_url),
            ("BIOMCP_GTR_CONDITION_GENE_URL", &condition_gene_url),
        ],
    );

    assert_brca1_search(&result);
    assert!(result.stderr.contains("Downloading GTR data..."));

    let gtr_root = default_gtr_root(data_home.path());
    assert!(gtr_root.join(GTR_TEST_VERSION_FILE).is_file());
    assert!(gtr_root.join(GTR_CONDITION_GENE_FILE).is_file());
    assert!(
        !requests_for_path(&server, GTR_TEST_VERSION_PATH)
            .await
            .is_empty()
    );
    assert!(
        !requests_for_path(&server, GTR_CONDITION_GENE_PATH)
            .await
            .is_empty()
    );
}

#[tokio::test]
async fn second_run_within_ttl_skips_redownload() {
    let server = mount_success_server().await;
    let data_home = TempDirGuard::new("fresh-data-home");
    let cache_home = TempDirGuard::new("fresh-cache-home");
    let test_version_url = gtr_test_version_url(&server);
    let condition_gene_url = gtr_condition_gene_url(&server);

    let first = run_biomcp(
        &["search", "diagnostic", "--gene", "BRCA1", "--limit", "5"],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_GTR_TEST_VERSION_URL", &test_version_url),
            ("BIOMCP_GTR_CONDITION_GENE_URL", &condition_gene_url),
        ],
    );
    assert_brca1_search(&first);

    let second = run_biomcp(
        &["search", "diagnostic", "--gene", "BRCA1", "--limit", "5"],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_GTR_TEST_VERSION_URL", &test_version_url),
            ("BIOMCP_GTR_CONDITION_GENE_URL", &condition_gene_url),
        ],
    );
    assert_brca1_search(&second);
    assert!(!second.stderr.contains("Downloading GTR data..."));
    assert!(!second.stderr.contains("Refreshing stale GTR data..."));
    assert!(
        !requests_for_path(&server, GTR_TEST_VERSION_PATH)
            .await
            .is_empty()
    );
    assert!(
        !requests_for_path(&server, GTR_CONDITION_GENE_PATH)
            .await
            .is_empty()
    );
}

#[tokio::test]
async fn stale_bundle_refreshes_on_next_search() {
    let server = mount_success_server().await;
    let data_home = TempDirGuard::new("stale-data-home");
    let cache_home = TempDirGuard::new("stale-cache-home");
    let test_version_url = gtr_test_version_url(&server);
    let condition_gene_url = gtr_condition_gene_url(&server);

    let first = run_biomcp(
        &["search", "diagnostic", "--gene", "BRCA1", "--limit", "5"],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_GTR_TEST_VERSION_URL", &test_version_url),
            ("BIOMCP_GTR_CONDITION_GENE_URL", &condition_gene_url),
        ],
    );
    assert_brca1_search(&first);

    set_stale(&default_gtr_root(data_home.path()).join(GTR_TEST_VERSION_FILE));

    let second = run_biomcp(
        &["search", "diagnostic", "--gene", "BRCA1", "--limit", "5"],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_GTR_TEST_VERSION_URL", &test_version_url),
            ("BIOMCP_GTR_CONDITION_GENE_URL", &condition_gene_url),
        ],
    );
    assert_brca1_search(&second);
    assert!(second.stderr.contains("Refreshing stale GTR data..."));
    assert_eq!(
        requests_for_path(&server, GTR_TEST_VERSION_PATH)
            .await
            .len(),
        2
    );
    assert_eq!(
        requests_for_path(&server, GTR_CONDITION_GENE_PATH)
            .await
            .len(),
        2
    );
}

#[tokio::test]
async fn gtr_sync_force_refreshes_and_honors_custom_root() {
    let server = mount_success_server().await;
    let data_home = TempDirGuard::new("custom-data-home");
    let cache_home = TempDirGuard::new("custom-cache-home");
    let custom_root = TempDirGuard::new("custom-gtr-root");
    let custom_root_string = custom_root.path().display().to_string();
    let test_version_url = gtr_test_version_url(&server);
    let condition_gene_url = gtr_condition_gene_url(&server);

    let first = run_biomcp(
        &["gtr", "sync"],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_GTR_DIR", &custom_root_string),
            ("BIOMCP_GTR_TEST_VERSION_URL", &test_version_url),
            ("BIOMCP_GTR_CONDITION_GENE_URL", &condition_gene_url),
        ],
    );

    assert!(
        first.status.success(),
        "expected successful gtr sync\nstdout:\n{}\nstderr:\n{}",
        first.stdout,
        first.stderr
    );
    assert!(
        first
            .stdout
            .contains("GTR local diagnostic data synchronized successfully.")
    );
    assert!(first.stderr.contains("Refreshing GTR data..."));
    assert!(custom_root.path().join(GTR_TEST_VERSION_FILE).is_file());
    assert!(custom_root.path().join(GTR_CONDITION_GENE_FILE).is_file());
    assert!(
        !default_gtr_root(data_home.path())
            .join(GTR_TEST_VERSION_FILE)
            .exists(),
        "default GTR root should remain unused when BIOMCP_GTR_DIR is set"
    );

    let second = run_biomcp(
        &["gtr", "sync"],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_GTR_DIR", &custom_root_string),
            ("BIOMCP_GTR_TEST_VERSION_URL", &test_version_url),
            ("BIOMCP_GTR_CONDITION_GENE_URL", &condition_gene_url),
        ],
    );

    assert!(
        second.status.success(),
        "expected second successful gtr sync\nstdout:\n{}\nstderr:\n{}",
        second.stdout,
        second.stderr
    );
    assert!(second.stderr.contains("Refreshing GTR data..."));
    assert_eq!(
        requests_for_path(&server, GTR_TEST_VERSION_PATH)
            .await
            .len(),
        2
    );
    assert_eq!(
        requests_for_path(&server, GTR_CONDITION_GENE_PATH)
            .await
            .len(),
        2
    );
}

#[tokio::test]
async fn stale_local_pair_survives_refresh_failure_with_warning() {
    let success_server = mount_success_server().await;
    let failing_server = mount_download_failure_server().await;
    let data_home = TempDirGuard::new("fallback-data-home");
    let cache_home = TempDirGuard::new("fallback-cache-home");
    let success_test_version_url = gtr_test_version_url(&success_server);
    let success_condition_gene_url = gtr_condition_gene_url(&success_server);
    let failing_test_version_url = gtr_test_version_url(&failing_server);
    let failing_condition_gene_url = gtr_condition_gene_url(&failing_server);

    let first = run_biomcp(
        &["search", "diagnostic", "--gene", "BRCA1", "--limit", "5"],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_GTR_TEST_VERSION_URL", &success_test_version_url),
            ("BIOMCP_GTR_CONDITION_GENE_URL", &success_condition_gene_url),
        ],
    );
    assert_brca1_search(&first);

    set_stale(&default_gtr_root(data_home.path()).join(GTR_TEST_VERSION_FILE));

    let second = run_biomcp(
        &["search", "diagnostic", "--gene", "BRCA1", "--limit", "5"],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_GTR_TEST_VERSION_URL", &failing_test_version_url),
            ("BIOMCP_GTR_CONDITION_GENE_URL", &failing_condition_gene_url),
        ],
    );
    assert_brca1_search(&second);
    assert!(second.stderr.contains("Warning: GTR refresh failed:"));
    assert!(second.stderr.contains("test_condition_gene.txt: HTTP 503"));
    assert!(second.stderr.contains("Using existing data."));
    assert!(
        !requests_for_path(&failing_server, GTR_TEST_VERSION_PATH)
            .await
            .is_empty()
    );
    assert!(
        !requests_for_path(&failing_server, GTR_CONDITION_GENE_PATH)
            .await
            .is_empty()
    );
}

#[tokio::test]
async fn gtr_sync_parse_failure_mentions_recovery_paths() {
    let server = mount_parse_failure_server().await;
    let data_home = TempDirGuard::new("parse-failure-data-home");
    let cache_home = TempDirGuard::new("parse-failure-cache-home");
    let test_version_url = gtr_test_version_url(&server);
    let condition_gene_url = gtr_condition_gene_url(&server);

    let result = run_biomcp(
        &["gtr", "sync"],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_GTR_TEST_VERSION_URL", &test_version_url),
            ("BIOMCP_GTR_CONDITION_GENE_URL", &condition_gene_url),
        ],
    );

    assert!(
        !result.status.success(),
        "gtr sync should fail on malformed payloads\nstdout:\n{}\nstderr:\n{}",
        result.stdout,
        result.stderr
    );
    assert!(
        result
            .stderr
            .contains("test_version.gz is missing required column: test_accession_ver")
    );
    assert!(result.stderr.contains("biomcp gtr sync"));
    assert!(result.stderr.contains("BIOMCP_GTR_DIR"));
    assert!(
        !requests_for_path(&server, GTR_TEST_VERSION_PATH)
            .await
            .is_empty()
    );
    assert!(
        !requests_for_path(&server, GTR_CONDITION_GENE_PATH)
            .await
            .is_empty()
    );
}

#[tokio::test]
async fn gtr_sync_download_failure_mentions_recovery_paths() {
    let server = mount_download_failure_server().await;
    let data_home = TempDirGuard::new("download-failure-data-home");
    let cache_home = TempDirGuard::new("download-failure-cache-home");
    let test_version_url = gtr_test_version_url(&server);
    let condition_gene_url = gtr_condition_gene_url(&server);

    let result = run_biomcp(
        &["gtr", "sync"],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_GTR_TEST_VERSION_URL", &test_version_url),
            ("BIOMCP_GTR_CONDITION_GENE_URL", &condition_gene_url),
        ],
    );

    assert!(
        !result.status.success(),
        "gtr sync should fail when a download returns an error\nstdout:\n{}\nstderr:\n{}",
        result.stdout,
        result.stderr
    );
    assert!(result.stderr.contains("test_condition_gene.txt: HTTP 503"));
    assert!(result.stderr.contains("biomcp gtr sync"));
    assert!(result.stderr.contains("BIOMCP_GTR_DIR"));
    assert!(
        !requests_for_path(&server, GTR_TEST_VERSION_PATH)
            .await
            .is_empty()
    );
    assert!(
        !requests_for_path(&server, GTR_CONDITION_GENE_PATH)
            .await
            .is_empty()
    );
}
