use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const MYCHEM_QUERY_PATH: &str = "/v1/query";
const CVX_DOWNLOAD_PATH: &str = "/fixtures/cvx/custom-cvx.txt";
const TRADENAME_DOWNLOAD_PATH: &str = "/fixtures/cvx/custom-tradename.txt";
const MVX_DOWNLOAD_PATH: &str = "/fixtures/cvx/custom-mvx.txt";
const CVX_FILE: &str = "cvx.txt";
const TRADENAME_FILE: &str = "TRADENAME.txt";
const MVX_FILE: &str = "mvx.txt";

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
            "biomcp-cvx-auto-sync-{label}-{}-{stamp}",
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

fn default_cvx_root(data_home: &Path) -> PathBuf {
    data_home.join("biomcp").join("cvx")
}

fn repo_fixture_root(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("spec")
        .join("fixtures")
        .join(name)
}

fn load_cvx_fixture_body(file_name: &str) -> String {
    fs::read_to_string(repo_fixture_root("cvx").join(file_name))
        .expect("CVX fixture should be readable")
}

fn seed_ema_fixture_root(label: &str) -> TempDirGuard {
    let root = TempDirGuard::new(label);
    for entry in fs::read_dir(repo_fixture_root("ema-human")).expect("EMA fixture dir should exist")
    {
        let entry = entry.expect("EMA fixture entry should be readable");
        let source = entry.path();
        if !source.is_file() {
            continue;
        }
        let destination = root.path().join(entry.file_name());
        fs::copy(&source, &destination).expect("EMA fixture file should copy");
        let file = fs::OpenOptions::new()
            .write(true)
            .open(&destination)
            .expect("EMA fixture should open for touch");
        file.set_modified(SystemTime::now())
            .expect("EMA fixture mtime should refresh");
    }
    root
}

async fn mount_empty_mychem(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path(MYCHEM_QUERY_PATH))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_json(json!({
                    "total": 0,
                    "hits": [],
                })),
        )
        .mount(server)
        .await;
}

async fn mount_success_server() -> MockServer {
    let server = MockServer::start().await;
    mount_empty_mychem(&server).await;

    for (download_path, file_name) in [
        (CVX_DOWNLOAD_PATH, CVX_FILE),
        (TRADENAME_DOWNLOAD_PATH, TRADENAME_FILE),
        (MVX_DOWNLOAD_PATH, MVX_FILE),
    ] {
        Mock::given(method("GET"))
            .and(path(download_path))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/csv; charset=utf-8")
                    .set_body_string(load_cvx_fixture_body(file_name)),
            )
            .mount(&server)
            .await;
    }

    server
}

async fn mount_parse_failure_server() -> MockServer {
    let server = MockServer::start().await;
    mount_empty_mychem(&server).await;

    Mock::given(method("GET"))
        .and(path(CVX_DOWNLOAD_PATH))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/csv; charset=utf-8")
                .set_body_string(load_cvx_fixture_body(CVX_FILE)),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(TRADENAME_DOWNLOAD_PATH))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/csv; charset=utf-8")
                .set_body_string(load_cvx_fixture_body(TRADENAME_FILE)),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(MVX_DOWNLOAD_PATH))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/csv; charset=utf-8")
                .set_body_string("PFR|Pfizer, Inc|broken\n"),
        )
        .mount(&server)
        .await;

    server
}

async fn mount_download_failure_server() -> MockServer {
    let server = MockServer::start().await;
    mount_empty_mychem(&server).await;

    Mock::given(method("GET"))
        .and(path(CVX_DOWNLOAD_PATH))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/csv; charset=utf-8")
                .set_body_string(load_cvx_fixture_body(CVX_FILE)),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(TRADENAME_DOWNLOAD_PATH))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/csv; charset=utf-8")
                .set_body_string(load_cvx_fixture_body(TRADENAME_FILE)),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(MVX_DOWNLOAD_PATH))
        .respond_with(
            ResponseTemplate::new(503)
                .insert_header("content-type", "text/plain; charset=utf-8")
                .set_body_string("temporary outage"),
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
    command.env_remove("BIOMCP_CVX_DIR");
    command.env_remove("BIOMCP_CVX_URL");
    command.env_remove("BIOMCP_CVX_TRADENAME_URL");
    command.env_remove("BIOMCP_MVX_URL");
    command.env_remove("BIOMCP_EMA_DIR");
    command.env_remove("BIOMCP_MYCHEM_BASE");
    command.env_remove("BIOMCP_CACHE_MODE");
    command.env_remove("RUST_LOG");
    for (name, value) in extra_envs {
        command.env(name, value);
    }

    let output = command.output().expect("biomcp command should run");
    CommandResult::from_output(output)
}

fn mychem_base(server: &MockServer) -> String {
    format!("{}/v1", server.uri())
}

fn download_url(server: &MockServer, download_path: &str) -> String {
    format!("{}{}", server.uri(), download_path)
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

async fn cvx_request_count(server: &MockServer) -> usize {
    server
        .received_requests()
        .await
        .expect("server should record requests")
        .into_iter()
        .filter(|request| {
            matches!(
                request.url.path(),
                CVX_DOWNLOAD_PATH | TRADENAME_DOWNLOAD_PATH | MVX_DOWNLOAD_PATH
            )
        })
        .count()
}

fn set_stale(path: &Path) {
    let file = fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("stale target should open");
    file.set_modified(
        SystemTime::now()
            .checked_sub(Duration::from_secs(31 * 24 * 60 * 60))
            .expect("stale time should be valid"),
    )
    .expect("mtime should update");
}

fn assert_prevnar_search(result: &CommandResult) {
    assert!(
        result.status.success(),
        "expected successful EMA vaccine bridge search\nstdout:\n{}\nstderr:\n{}",
        result.stdout,
        result.stderr
    );
    assert!(result.stdout.contains("# Drugs: prevnar"));
    assert!(result.stdout.contains("|Prevenar 13|"));
}

#[tokio::test]
async fn first_use_search_downloads_missing_bundle_into_default_root_using_url_overrides() {
    let server = mount_success_server().await;
    let data_home = TempDirGuard::new("clean-data-home");
    let cache_home = TempDirGuard::new("clean-cache-home");
    let ema_root = seed_ema_fixture_root("ema-root");
    let ema_root_string = ema_root.path().display().to_string();
    let mychem_base = mychem_base(&server);
    let cvx_url = download_url(&server, CVX_DOWNLOAD_PATH);
    let tradename_url = download_url(&server, TRADENAME_DOWNLOAD_PATH);
    let mvx_url = download_url(&server, MVX_DOWNLOAD_PATH);

    let result = run_biomcp(
        &[
            "search", "drug", "prevnar", "--region", "eu", "--limit", "5",
        ],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_EMA_DIR", &ema_root_string),
            ("BIOMCP_MYCHEM_BASE", &mychem_base),
            ("BIOMCP_CVX_URL", &cvx_url),
            ("BIOMCP_CVX_TRADENAME_URL", &tradename_url),
            ("BIOMCP_MVX_URL", &mvx_url),
        ],
    );

    assert_prevnar_search(&result);
    assert!(result.stderr.contains("Downloading CDC CVX/MVX data..."));

    let cvx_root = default_cvx_root(data_home.path());
    assert!(cvx_root.join(CVX_FILE).is_file());
    assert!(cvx_root.join(TRADENAME_FILE).is_file());
    assert!(cvx_root.join(MVX_FILE).is_file());
    assert_eq!(cvx_request_count(&server).await, 3);
    assert_eq!(requests_for_path(&server, CVX_DOWNLOAD_PATH).await.len(), 1);
    assert_eq!(
        requests_for_path(&server, TRADENAME_DOWNLOAD_PATH)
            .await
            .len(),
        1
    );
    assert_eq!(requests_for_path(&server, MVX_DOWNLOAD_PATH).await.len(), 1);
}

#[tokio::test]
async fn second_run_within_ttl_skips_download() {
    let server = mount_success_server().await;
    let data_home = TempDirGuard::new("fresh-data-home");
    let cache_home = TempDirGuard::new("fresh-cache-home");
    let ema_root = seed_ema_fixture_root("ema-root");
    let ema_root_string = ema_root.path().display().to_string();
    let mychem_base = mychem_base(&server);
    let cvx_url = download_url(&server, CVX_DOWNLOAD_PATH);
    let tradename_url = download_url(&server, TRADENAME_DOWNLOAD_PATH);
    let mvx_url = download_url(&server, MVX_DOWNLOAD_PATH);

    let first = run_biomcp(
        &[
            "search", "drug", "prevnar", "--region", "eu", "--limit", "5",
        ],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_EMA_DIR", &ema_root_string),
            ("BIOMCP_MYCHEM_BASE", &mychem_base),
            ("BIOMCP_CVX_URL", &cvx_url),
            ("BIOMCP_CVX_TRADENAME_URL", &tradename_url),
            ("BIOMCP_MVX_URL", &mvx_url),
        ],
    );
    assert_prevnar_search(&first);

    let second = run_biomcp(
        &[
            "search", "drug", "prevnar", "--region", "eu", "--limit", "5",
        ],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_EMA_DIR", &ema_root_string),
            ("BIOMCP_MYCHEM_BASE", &mychem_base),
            ("BIOMCP_CVX_URL", &cvx_url),
            ("BIOMCP_CVX_TRADENAME_URL", &tradename_url),
            ("BIOMCP_MVX_URL", &mvx_url),
        ],
    );
    assert_prevnar_search(&second);
    assert!(!second.stderr.contains("Downloading CDC CVX/MVX data..."));
    assert!(
        !second
            .stderr
            .contains("Refreshing stale CDC CVX/MVX data...")
    );
    assert_eq!(cvx_request_count(&server).await, 3);
}

#[tokio::test]
async fn stale_bundle_refreshes_on_next_search() {
    let server = mount_success_server().await;
    let data_home = TempDirGuard::new("stale-data-home");
    let cache_home = TempDirGuard::new("stale-cache-home");
    let ema_root = seed_ema_fixture_root("ema-root");
    let ema_root_string = ema_root.path().display().to_string();
    let mychem_base = mychem_base(&server);
    let cvx_url = download_url(&server, CVX_DOWNLOAD_PATH);
    let tradename_url = download_url(&server, TRADENAME_DOWNLOAD_PATH);
    let mvx_url = download_url(&server, MVX_DOWNLOAD_PATH);

    let first = run_biomcp(
        &[
            "search", "drug", "prevnar", "--region", "eu", "--limit", "5",
        ],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_EMA_DIR", &ema_root_string),
            ("BIOMCP_MYCHEM_BASE", &mychem_base),
            ("BIOMCP_CVX_URL", &cvx_url),
            ("BIOMCP_CVX_TRADENAME_URL", &tradename_url),
            ("BIOMCP_MVX_URL", &mvx_url),
        ],
    );
    assert_prevnar_search(&first);

    set_stale(&default_cvx_root(data_home.path()).join(MVX_FILE));

    let second = run_biomcp(
        &[
            "search", "drug", "prevnar", "--region", "eu", "--limit", "5",
        ],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_EMA_DIR", &ema_root_string),
            ("BIOMCP_MYCHEM_BASE", &mychem_base),
            ("BIOMCP_CVX_URL", &cvx_url),
            ("BIOMCP_CVX_TRADENAME_URL", &tradename_url),
            ("BIOMCP_MVX_URL", &mvx_url),
        ],
    );
    assert_prevnar_search(&second);
    assert!(
        second
            .stderr
            .contains("Refreshing stale CDC CVX/MVX data...")
    );
    assert_eq!(cvx_request_count(&server).await, 6);
}

#[tokio::test]
async fn cvx_sync_honors_custom_root() {
    let server = mount_success_server().await;
    let data_home = TempDirGuard::new("custom-data-home");
    let cache_home = TempDirGuard::new("custom-cache-home");
    let custom_root = TempDirGuard::new("custom-cvx-root");
    let custom_root_string = custom_root.path().display().to_string();
    let mychem_base = mychem_base(&server);
    let cvx_url = download_url(&server, CVX_DOWNLOAD_PATH);
    let tradename_url = download_url(&server, TRADENAME_DOWNLOAD_PATH);
    let mvx_url = download_url(&server, MVX_DOWNLOAD_PATH);

    let result = run_biomcp(
        &["cvx", "sync"],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_CVX_DIR", &custom_root_string),
            ("BIOMCP_MYCHEM_BASE", &mychem_base),
            ("BIOMCP_CVX_URL", &cvx_url),
            ("BIOMCP_CVX_TRADENAME_URL", &tradename_url),
            ("BIOMCP_MVX_URL", &mvx_url),
        ],
    );

    assert!(
        result.status.success(),
        "expected successful cvx sync\nstdout:\n{}\nstderr:\n{}",
        result.stdout,
        result.stderr
    );
    assert!(
        result
            .stdout
            .contains("CDC CVX/MVX local data bundle synchronized successfully.")
    );
    assert!(custom_root.path().join(CVX_FILE).is_file());
    assert!(custom_root.path().join(TRADENAME_FILE).is_file());
    assert!(custom_root.path().join(MVX_FILE).is_file());
    assert!(
        !default_cvx_root(data_home.path()).join(CVX_FILE).exists(),
        "default CVX root should remain unused when BIOMCP_CVX_DIR is set"
    );
    assert_eq!(cvx_request_count(&server).await, 3);
}

#[tokio::test]
async fn cvx_sync_parse_failure_mentions_recovery_paths() {
    let server = mount_parse_failure_server().await;
    let data_home = TempDirGuard::new("failure-data-home");
    let cache_home = TempDirGuard::new("failure-cache-home");
    let cvx_url = download_url(&server, CVX_DOWNLOAD_PATH);
    let tradename_url = download_url(&server, TRADENAME_DOWNLOAD_PATH);
    let mvx_url = download_url(&server, MVX_DOWNLOAD_PATH);

    let result = run_biomcp(
        &["cvx", "sync"],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_CVX_URL", &cvx_url),
            ("BIOMCP_CVX_TRADENAME_URL", &tradename_url),
            ("BIOMCP_MVX_URL", &mvx_url),
        ],
    );

    assert!(
        !result.status.success(),
        "cvx sync should fail on malformed payload\nstdout:\n{}\nstderr:\n{}",
        result.stdout,
        result.stderr
    );
    assert!(result.stderr.contains("expected at least 5 fields"));
    assert!(result.stderr.contains("biomcp cvx sync"));
    assert!(result.stderr.contains("BIOMCP_CVX_DIR"));
    assert!(result.stderr.contains(&cvx_url));
    assert!(result.stderr.contains(&tradename_url));
    assert!(result.stderr.contains(&mvx_url));
    assert!(
        !requests_for_path(&server, CVX_DOWNLOAD_PATH)
            .await
            .is_empty()
    );
    assert!(
        !requests_for_path(&server, TRADENAME_DOWNLOAD_PATH)
            .await
            .is_empty()
    );
    assert!(
        !requests_for_path(&server, MVX_DOWNLOAD_PATH)
            .await
            .is_empty()
    );
}

#[tokio::test]
async fn cvx_sync_download_failure_mentions_recovery_paths() {
    let server = mount_download_failure_server().await;
    let data_home = TempDirGuard::new("download-failure-data-home");
    let cache_home = TempDirGuard::new("download-failure-cache-home");
    let cvx_url = download_url(&server, CVX_DOWNLOAD_PATH);
    let tradename_url = download_url(&server, TRADENAME_DOWNLOAD_PATH);
    let mvx_url = download_url(&server, MVX_DOWNLOAD_PATH);

    let result = run_biomcp(
        &["cvx", "sync"],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_CVX_URL", &cvx_url),
            ("BIOMCP_CVX_TRADENAME_URL", &tradename_url),
            ("BIOMCP_MVX_URL", &mvx_url),
        ],
    );

    assert!(
        !result.status.success(),
        "cvx sync should fail when a download returns an error\nstdout:\n{}\nstderr:\n{}",
        result.stdout,
        result.stderr
    );
    assert!(result.stderr.contains("mvx.txt: HTTP 503"));
    assert!(result.stderr.contains("biomcp cvx sync"));
    assert!(result.stderr.contains("BIOMCP_CVX_DIR"));
    assert!(result.stderr.contains(&cvx_url));
    assert!(result.stderr.contains(&tradename_url));
    assert!(result.stderr.contains(&mvx_url));
    assert!(
        !requests_for_path(&server, CVX_DOWNLOAD_PATH)
            .await
            .is_empty()
    );
    assert!(
        !requests_for_path(&server, TRADENAME_DOWNLOAD_PATH)
            .await
            .is_empty()
    );
    assert!(
        !requests_for_path(&server, MVX_DOWNLOAD_PATH)
            .await
            .is_empty()
    );
}
