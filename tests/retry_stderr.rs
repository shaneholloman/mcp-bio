use std::process::{Command, Output};

use wiremock::matchers::{body_string_contains, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const BATCH_PAPER_FIELDS: &str = "paperId,externalIds,title,venue,year";
const CITATION_EDGE_FIELDS: &str = "contexts,intents,isInfluential,citingPaper.paperId,citingPaper.externalIds,citingPaper.title,citingPaper.venue,citingPaper.year";

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

fn run_article_citations(
    mock_base: &str,
    rust_log: Option<&str>,
    api_key: Option<&str>,
) -> CommandResult {
    let cache_home = tempfile::Builder::new()
        .prefix("biomcp-retry-stderr-cache-")
        .tempdir()
        .expect("temp dir should be created");
    let mut command = Command::new(env!("CARGO_BIN_EXE_biomcp"));
    command.args(["article", "citations", "22663011", "--limit", "1"]);
    command.env("BIOMCP_S2_BASE", mock_base);
    command.env("BIOMCP_CACHE_MODE", "off");
    command.env("XDG_CACHE_HOME", cache_home.path());
    match api_key {
        Some(api_key) => {
            command.env("S2_API_KEY", api_key);
        }
        None => {
            command.env_remove("S2_API_KEY");
        }
    }

    if let Some(rust_log) = rust_log {
        command.env("RUST_LOG", rust_log);
    } else {
        command.env_remove("RUST_LOG");
    }

    let output = command
        .output()
        .expect("article citations command should run");
    CommandResult::from_output(output)
}

async fn semantic_scholar_retry_server() -> MockServer {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/graph/v1/paper/batch"))
        .and(query_param("fields", BATCH_PAPER_FIELDS))
        .and(header("x-api-key", "test-key"))
        .and(body_string_contains("\"PMID:22663011\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "paperId": "paper-1",
                "externalIds": {"PubMed": "22663011"},
                "title": "Seed paper",
                "venue": "Science",
                "year": 2012
            }
        ])))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/graph/v1/paper/paper-1/citations"))
        .and(query_param("fields", CITATION_EDGE_FIELDS))
        .and(query_param("limit", "1"))
        .and(header("x-api-key", "test-key"))
        .respond_with(ResponseTemplate::new(429))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/graph/v1/paper/paper-1/citations"))
        .and(query_param("fields", CITATION_EDGE_FIELDS))
        .and(query_param("limit", "1"))
        .and(header("x-api-key", "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [{
                "contexts": ["Example retry context"],
                "intents": ["Background"],
                "isInfluential": false,
                "citingPaper": {
                    "paperId": "paper-2",
                    "externalIds": {"PubMed": "24200969"},
                    "title": "Recovered after retry",
                    "venue": "Nature",
                    "year": 2024
                }
            }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    server
}

async fn semantic_scholar_unauthenticated_rate_limit_server() -> MockServer {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/graph/v1/paper/batch"))
        .and(query_param("fields", BATCH_PAPER_FIELDS))
        .and(|request: &wiremock::Request| !request.headers.contains_key("x-api-key"))
        .and(body_string_contains("\"PMID:22663011\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "paperId": "paper-1",
                "externalIds": {"PubMed": "22663011"},
                "title": "Seed paper",
                "venue": "Science",
                "year": 2012
            }
        ])))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/graph/v1/paper/paper-1/citations"))
        .and(query_param("fields", CITATION_EDGE_FIELDS))
        .and(query_param("limit", "1"))
        .and(|request: &wiremock::Request| !request.headers.contains_key("x-api-key"))
        .respond_with(ResponseTemplate::new(429).set_body_string("shared pool"))
        .expect(1)
        .mount(&server)
        .await;

    server
}

#[tokio::test]
async fn article_citations_suppresses_retry_warnings_on_default_stderr_but_keeps_debug_logs() {
    let default_server = semantic_scholar_retry_server().await;
    let debug_server = semantic_scholar_retry_server().await;

    let default_result = run_article_citations(&default_server.uri(), None, Some("test-key"));
    assert!(
        default_result.status.success(),
        "expected success with default logs\nstdout:\n{}\nstderr:\n{}",
        default_result.stdout,
        default_result.stderr
    );
    assert!(
        !default_result.stderr.contains("Retry attempt"),
        "default stderr should not contain retry attempts\nstderr:\n{}",
        default_result.stderr
    );
    assert!(
        !default_result.stderr.contains("reqwest_retry"),
        "default stderr should not contain reqwest_retry target logs\nstderr:\n{}",
        default_result.stderr
    );

    let debug_result = run_article_citations(&debug_server.uri(), Some("debug"), Some("test-key"));
    assert!(
        debug_result.status.success(),
        "expected success with debug logs\nstdout:\n{}\nstderr:\n{}",
        debug_result.stdout,
        debug_result.stderr
    );
    assert!(
        debug_result.stderr.contains("Retry attempt #0"),
        "debug stderr should retain retry diagnostics\nstderr:\n{}",
        debug_result.stderr
    );
}

#[tokio::test]
async fn article_citations_without_api_key_fails_fast_on_429_without_retry_logs() {
    let default_server = semantic_scholar_unauthenticated_rate_limit_server().await;
    let debug_server = semantic_scholar_unauthenticated_rate_limit_server().await;

    let default_result = run_article_citations(&default_server.uri(), None, None);
    assert!(
        !default_result.status.success(),
        "expected no-key 429 to fail\nstdout:\n{}\nstderr:\n{}",
        default_result.stdout,
        default_result.stderr
    );
    assert!(
        default_result.stderr.contains(
            "Rate limited by Semantic Scholar. Set S2_API_KEY for a dedicated rate limit."
        ),
        "default stderr should contain the no-key guidance\nstderr:\n{}",
        default_result.stderr
    );
    assert!(
        !default_result.stderr.contains("Retry attempt"),
        "default stderr should not contain retry attempts\nstderr:\n{}",
        default_result.stderr
    );

    let debug_result = run_article_citations(&debug_server.uri(), Some("debug"), None);
    assert!(
        !debug_result.status.success(),
        "expected no-key 429 to fail with debug logs\nstdout:\n{}\nstderr:\n{}",
        debug_result.stdout,
        debug_result.stderr
    );
    assert!(
        debug_result.stderr.contains(
            "Rate limited by Semantic Scholar. Set S2_API_KEY for a dedicated rate limit."
        ),
        "debug stderr should contain the no-key guidance\nstderr:\n{}",
        debug_result.stderr
    );
    assert!(
        !debug_result.stderr.contains("Retry attempt"),
        "debug stderr should not contain retry attempts when shared-pool 429s are fatal\nstderr:\n{}",
        debug_result.stderr
    );
}
