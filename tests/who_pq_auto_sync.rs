use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const WHO_EXPORT_PATH: &str =
    "/prequal/medicines/prequalified/finished-pharmaceutical-products/export";
const WHO_API_EXPORT_PATH: &str =
    "/prequal/medicines/prequalified/active-pharmaceutical-ingredients/export";
const WHO_CSV_FILE: &str = "who_pq.csv";
const WHO_API_CSV_FILE: &str = "who_api.csv";

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
            "biomcp-who-auto-sync-{label}-{}-{stamp}",
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

fn default_who_root(data_home: &Path) -> PathBuf {
    data_home.join("biomcp").join("who-pq")
}

fn load_fixture_body(file_name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("spec")
        .join("fixtures")
        .join("who-pq")
        .join(file_name);
    fs::read_to_string(path).expect("WHO fixture should be readable")
}

fn export_url(server: &MockServer) -> String {
    format!("{}{}?page&_format=csv", server.uri(), WHO_EXPORT_PATH)
}

fn export_api_url(server: &MockServer) -> String {
    format!("{}{}?page&_format=csv", server.uri(), WHO_API_EXPORT_PATH)
}

async fn mount_success_server() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(WHO_EXPORT_PATH))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/csv; charset=utf-8")
                .set_body_string(load_fixture_body(WHO_CSV_FILE)),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(WHO_API_EXPORT_PATH))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/csv; charset=utf-8")
                .set_body_string(load_fixture_body(WHO_API_CSV_FILE)),
        )
        .mount(&server)
        .await;
    server
}

async fn mount_failure_server(status: u16) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(WHO_EXPORT_PATH))
        .respond_with(
            ResponseTemplate::new(status)
                .insert_header("content-type", "text/plain")
                .set_body_string("who upstream failure"),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(WHO_API_EXPORT_PATH))
        .respond_with(
            ResponseTemplate::new(status)
                .insert_header("content-type", "text/plain")
                .set_body_string("who upstream failure"),
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
    command.env_remove("BIOMCP_WHO_DIR");
    command.env_remove("BIOMCP_WHO_PQ_URL");
    command.env_remove("BIOMCP_WHO_PQ_API_URL");
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
        .filter(|request| matches!(request.url.path(), WHO_EXPORT_PATH | WHO_API_EXPORT_PATH))
        .count()
}

async fn requests(server: &MockServer) -> Vec<Request> {
    server
        .received_requests()
        .await
        .expect("server should record requests")
        .into_iter()
        .filter(|request| matches!(request.url.path(), WHO_EXPORT_PATH | WHO_API_EXPORT_PATH))
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

fn assert_trastuzumab_search(result: &CommandResult) {
    assert!(
        result.status.success(),
        "expected successful WHO search\nstdout:\n{}\nstderr:\n{}",
        result.stdout,
        result.stderr
    );
    assert!(result.stdout.contains("# Drugs: trastuzumab"));
    assert!(
        result.stdout.contains(
            "|INN|Type|Therapeutic Area|Dosage Form|Applicant|WHO ID|Listing Basis|Date|"
        )
    );
    assert!(result.stdout.contains("Trastuzumab"));
}

#[tokio::test]
async fn clean_who_search_downloads_missing_csv() {
    let server = mount_success_server().await;
    let data_home = TempDirGuard::new("clean-data-home");
    let cache_home = TempDirGuard::new("clean-cache-home");
    let who_export_url = export_url(&server);
    let who_api_export_url = export_api_url(&server);

    let result = run_biomcp(
        &[
            "search",
            "drug",
            "trastuzumab",
            "--region",
            "who",
            "--limit",
            "2",
        ],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_WHO_PQ_URL", &who_export_url),
            ("BIOMCP_WHO_PQ_API_URL", &who_api_export_url),
        ],
    );

    assert_trastuzumab_search(&result);
    assert!(
        result
            .stderr
            .contains("Downloading WHO Prequalification data (~134 KB + ~22 KB)...")
    );
    let who_root = default_who_root(data_home.path());
    assert!(who_root.join(WHO_CSV_FILE).is_file());
    assert!(who_root.join(WHO_API_CSV_FILE).is_file());
    assert_eq!(request_count(&server).await, 2);
}

#[tokio::test]
async fn second_run_within_ttl_skips_download() {
    let server = mount_success_server().await;
    let data_home = TempDirGuard::new("fresh-data-home");
    let cache_home = TempDirGuard::new("fresh-cache-home");
    let who_export_url = export_url(&server);
    let who_api_export_url = export_api_url(&server);

    let first = run_biomcp(
        &[
            "search",
            "drug",
            "trastuzumab",
            "--region",
            "who",
            "--limit",
            "2",
        ],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_WHO_PQ_URL", &who_export_url),
            ("BIOMCP_WHO_PQ_API_URL", &who_api_export_url),
        ],
    );
    assert_trastuzumab_search(&first);

    let second = run_biomcp(
        &[
            "search",
            "drug",
            "trastuzumab",
            "--region",
            "who",
            "--limit",
            "2",
        ],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_WHO_PQ_URL", &who_export_url),
            ("BIOMCP_WHO_PQ_API_URL", &who_api_export_url),
        ],
    );
    assert_trastuzumab_search(&second);
    assert!(
        !second
            .stderr
            .contains("Downloading WHO Prequalification data")
    );
    assert!(
        !second
            .stderr
            .contains("Refreshing stale WHO Prequalification data")
    );
    assert_eq!(request_count(&server).await, 2);
}

#[tokio::test]
async fn stale_who_csv_refreshes_on_next_search() {
    let server = mount_success_server().await;
    let data_home = TempDirGuard::new("stale-data-home");
    let cache_home = TempDirGuard::new("stale-cache-home");
    let who_export_url = export_url(&server);
    let who_api_export_url = export_api_url(&server);

    let first = run_biomcp(
        &[
            "search",
            "drug",
            "trastuzumab",
            "--region",
            "who",
            "--limit",
            "2",
        ],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_WHO_PQ_URL", &who_export_url),
            ("BIOMCP_WHO_PQ_API_URL", &who_api_export_url),
        ],
    );
    assert_trastuzumab_search(&first);

    let csv_path = default_who_root(data_home.path()).join(WHO_CSV_FILE);
    set_stale(&csv_path);

    let second = run_biomcp(
        &[
            "search",
            "drug",
            "trastuzumab",
            "--region",
            "who",
            "--limit",
            "2",
        ],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_WHO_PQ_URL", &who_export_url),
            ("BIOMCP_WHO_PQ_API_URL", &who_api_export_url),
        ],
    );
    assert_trastuzumab_search(&second);
    assert!(
        second
            .stderr
            .contains("Refreshing stale WHO Prequalification data (~134 KB + ~22 KB)...")
    );
    assert_eq!(request_count(&server).await, 4);
}

#[tokio::test]
async fn missing_who_csv_redownloads_on_next_search() {
    let server = mount_success_server().await;
    let data_home = TempDirGuard::new("missing-data-home");
    let cache_home = TempDirGuard::new("missing-cache-home");
    let who_export_url = export_url(&server);
    let who_api_export_url = export_api_url(&server);

    let first = run_biomcp(
        &[
            "search",
            "drug",
            "trastuzumab",
            "--region",
            "who",
            "--limit",
            "2",
        ],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_WHO_PQ_URL", &who_export_url),
            ("BIOMCP_WHO_PQ_API_URL", &who_api_export_url),
        ],
    );
    assert_trastuzumab_search(&first);

    let csv_path = default_who_root(data_home.path()).join(WHO_CSV_FILE);
    fs::remove_file(&csv_path).expect("WHO CSV should be removable");

    let second = run_biomcp(
        &[
            "search",
            "drug",
            "trastuzumab",
            "--region",
            "who",
            "--limit",
            "2",
        ],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_WHO_PQ_URL", &who_export_url),
            ("BIOMCP_WHO_PQ_API_URL", &who_api_export_url),
        ],
    );
    assert_trastuzumab_search(&second);
    assert!(
        second
            .stderr
            .contains("Downloading WHO Prequalification data (~134 KB + ~22 KB)...")
    );
    assert!(csv_path.is_file());
    assert!(
        default_who_root(data_home.path())
            .join(WHO_API_CSV_FILE)
            .is_file()
    );
    assert_eq!(request_count(&server).await, 4);
}

#[tokio::test]
async fn explicit_who_sync_honors_custom_root() {
    let server = mount_success_server().await;
    let data_home = TempDirGuard::new("custom-data-home");
    let cache_home = TempDirGuard::new("custom-cache-home");
    let custom_root = TempDirGuard::new("custom-who-root");
    let custom_root_string = custom_root.path().display().to_string();
    let who_export_url = export_url(&server);
    let who_api_export_url = export_api_url(&server);

    let result = run_biomcp(
        &["who", "sync"],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_WHO_PQ_URL", &who_export_url),
            ("BIOMCP_WHO_PQ_API_URL", &who_api_export_url),
            ("BIOMCP_WHO_DIR", &custom_root_string),
        ],
    );

    assert!(
        result.status.success(),
        "expected successful who sync\nstdout:\n{}\nstderr:\n{}",
        result.stdout,
        result.stderr
    );
    assert!(
        result
            .stdout
            .contains("WHO Prequalification data synchronized successfully.")
    );
    assert!(custom_root.path().join(WHO_CSV_FILE).is_file());
    assert!(custom_root.path().join(WHO_API_CSV_FILE).is_file());
    assert!(
        !default_who_root(data_home.path())
            .join(WHO_CSV_FILE)
            .exists(),
        "default WHO root should remain unused when BIOMCP_WHO_DIR is set"
    );
}

#[tokio::test]
async fn who_sync_failure_mentions_recovery_paths() {
    let server = mount_failure_server(500).await;
    let data_home = TempDirGuard::new("failure-data-home");
    let cache_home = TempDirGuard::new("failure-cache-home");
    let who_export_url = export_url(&server);
    let who_api_export_url = export_api_url(&server);

    let result = run_biomcp(
        &[
            "search",
            "drug",
            "trastuzumab",
            "--region",
            "who",
            "--limit",
            "2",
        ],
        data_home.path(),
        cache_home.path(),
        &[
            ("BIOMCP_WHO_PQ_URL", &who_export_url),
            ("BIOMCP_WHO_PQ_API_URL", &who_api_export_url),
        ],
    );

    assert!(
        !result.status.success(),
        "search should fail when WHO download fails"
    );
    assert!(result.stderr.contains("biomcp who sync"));
    assert!(result.stderr.contains("BIOMCP_WHO_DIR"));
    assert!(
        !requests(&server).await.is_empty(),
        "expected WHO failure path to issue at least one export request"
    );
    assert!(result.stderr.contains(&who_export_url));
    assert!(result.stderr.contains(&who_api_export_url));
}
