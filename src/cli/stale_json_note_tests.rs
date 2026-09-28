//! End-to-end stale-cache note tests for the JSON search bodies
//! (ticket 1263). Article search, GWAS search, search-all, and the
//! ClinGen prefetch inside `get gene` must all carry the note a
//! clinician or JSON consumer sees, not only the log line. The
//! fixture shape follows the disease stale-serve test: an axum
//! server answering 200 with a one-second freshness window, then
//! killed so the next command serves stale from the cache.

/// Answer every request with `body` as JSON, fresh-cacheable for one
/// second: after the window, with the server gone, the cache serves
/// stale and stamps the age marker the note reads.
async fn stale_note_fixture_server(body: &'static str) -> (String, tokio::task::JoinHandle<()>) {
    use axum::http::{header, StatusCode};
    use axum::response::IntoResponse;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind stale-note fixture");
    let base = format!(
        "http://{}",
        listener.local_addr().expect("fixture address")
    );
    let app = axum::Router::new().fallback(move || async move {
        (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "application/json"),
                (header::CACHE_CONTROL, "max-age=1"),
            ],
            body,
        )
            .into_response()
    });
    let task = tokio::spawn(async move { axum::serve(listener, app).await.expect("fixture serves") });
    (base, task)
}

/// One command's stdout, driven through the real CLI entry.
async fn run(args: &[&str]) -> String {
    let argv = std::iter::once("biomcp")
        .chain(args.iter().copied())
        .map(str::to_string)
        .collect();
    crate::cli::execute(argv)
        .await
        .expect("command succeeds against the fixture")
}

struct StaleNoteEnv {
    root: crate::test_support::TempDirGuard,
    /// (key, previous value) pairs restored on drop, so one test's
    /// fixture bases never leak into the next.
    previous: Vec<(&'static str, Option<String>)>,
}

impl StaleNoteEnv {
    fn new(base: &str, keys: &[&'static str]) -> Self {
        let root = crate::test_support::TempDirGuard::new("stale-json-notes");
        let mut previous = Vec::new();
        for key in keys.iter().copied().chain(["BIOMCP_CACHE_DIR"]) {
            // SAFETY: serialized on the source_env key; values restore on drop.
            unsafe {
                previous.push((key, std::env::var(key).ok()));
                std::env::set_var(key, if key == "BIOMCP_CACHE_DIR" {
                    root.path().to_str().expect("utf-8 cache root")
                } else {
                    base
                });
            }
        }
        Self { root, previous }
    }
}

impl Drop for StaleNoteEnv {
    fn drop(&mut self) {
        for (key, value) in self.previous.drain(..) {
            // SAFETY: serialized on the source_env key.
            unsafe {
                match value {
                    Some(previous) => std::env::set_var(key, previous),
                    None => std::env::remove_var(key),
                }
            }
        }
    }
}

const EPMC_BODY: &str = r#"{"hitCount":1,"resultList":{"result":[{"id":"32794606","source":"MED","title":"Aspirin","firstPublicationDate":"1990-01-01"}]}}"#;
const GWAS_BODY: &str = r#"{"_embedded":{"associations":[{"snps":[{"rsId":"rs1000000"}],"efoTraits":[{"trait":"Aspirin response"}]}]}}"#;

#[tokio::test]
#[serial_test::serial(source_env)]
async fn a_stale_article_search_json_states_the_cache_age() {
    let (base, server) = stale_note_fixture_server(EPMC_BODY).await;
    let _env = StaleNoteEnv::new(&base, &["BIOMCP_EUROPEPMC_BASE"]);
    let fresh = run(&["--json", "search", "article", "--keyword", "aspirin", "--limit", "1", "--source", "europepmc"]).await;
    assert!(!fresh.contains("Cache note"), "fresh serve: {fresh}");
    assert!(!fresh.contains("older than the provider's freshness window"), "{fresh}");

    tokio::time::sleep(std::time::Duration::from_millis(1600)).await; // watchdog: freshness-window wait, bounded at 1.6 s
    server.abort();

    let stale = run(&["--json", "search", "article", "--keyword", "aspirin", "--limit", "1", "--source", "europepmc"]).await;
    assert!(
        stale.contains("older than the provider's freshness window"),
        "the stale article search JSON carries the note: {stale}"
    );
}

#[tokio::test]
#[serial_test::serial(source_env)]
async fn a_stale_gwas_search_json_states_the_cache_age() {
    let (base, server) = stale_note_fixture_server(GWAS_BODY).await;
    let _env = StaleNoteEnv::new(&base, &["BIOMCP_GWAS_BASE"]);
    let fresh = run(&["--json", "search", "gwas", "--trait", "aspirin", "--limit", "1"]).await;
    assert!(!fresh.contains("older than the provider's freshness window"), "fresh serve: {fresh}");

    tokio::time::sleep(std::time::Duration::from_millis(1600)).await; // watchdog: freshness-window wait, bounded at 1.6 s
    server.abort();

    let stale = run(&["--json", "search", "gwas", "--trait", "aspirin", "--limit", "1"]).await;
    assert!(
        stale.contains("older than the provider's freshness window"),
        "the stale GWAS search JSON carries the note: {stale}"
    );
}

#[tokio::test]
#[serial_test::serial(source_env)]
async fn a_stale_search_all_json_states_the_cache_age() {
    let (base, server) = stale_note_fixture_server(EPMC_BODY).await;
    let _env = StaleNoteEnv::new(&base, &["BIOMCP_EUROPEPMC_BASE"]);
    let fresh = run(&["--json", "search", "all", "--keyword", "aspirin", "--limit", "1"]).await;
    assert!(!fresh.contains("older than the provider's freshness window"), "fresh serve: {fresh}");

    tokio::time::sleep(std::time::Duration::from_millis(1600)).await; // watchdog: freshness-window wait, bounded at 1.6 s
    server.abort();

    let stale = run(&["--json", "search", "all", "--keyword", "aspirin", "--limit", "1"]).await;
    assert!(
        stale.contains("older than the provider's freshness window"),
        "the stale search-all JSON carries the note: {stale}"
    );
    assert!(
        stale.contains("\"_meta\""),
        "the note rides the _meta channel like every other search body: {stale}"
    );
}
