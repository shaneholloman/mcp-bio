use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

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

fn unique_temp_dir(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("biomcp-{label}-{}-{stamp}", std::process::id()));
    fs::create_dir_all(&path).expect("temp dir should be created");
    path
}

fn run_pathway_search(
    reactome_base: &str,
    kegg_base: &str,
    wikipathways_base: &str,
) -> CommandResult {
    let cache_home = unique_temp_dir("pathway-search-stderr-cache");
    let mut command = Command::new(env!("CARGO_BIN_EXE_biomcp"));
    command.args(["search", "pathway", "apoptosis", "--limit", "3"]);
    command.env("BIOMCP_REACTOME_BASE", reactome_base);
    command.env("BIOMCP_KEGG_BASE", kegg_base);
    command.env("BIOMCP_WIKIPATHWAYS_BASE", wikipathways_base);
    command.env("BIOMCP_CACHE_MODE", "off");
    command.env("XDG_CACHE_HOME", &cache_home);
    command.env_remove("BIOMCP_DISABLE_KEGG");
    command.env_remove("RUST_LOG");

    let output = command.output().expect("pathway search command should run");
    let _ = fs::remove_dir_all(cache_home);
    CommandResult::from_output(output)
}

#[tokio::test]
async fn pathway_search_sanitizes_wikipathways_html_warning() {
    let reactome = MockServer::start().await;
    let kegg = MockServer::start().await;
    let wikipathways = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/search/query"))
        .and(query_param("query", "apoptosis"))
        .and(query_param("species", "Homo sapiens"))
        .and(query_param("pageSize", "3"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"{"results":[{"entries":[{"stId":"R-HSA-109581","name":"Apoptosis"}]}],"totalResults":1}"#,
            "application/json",
        ))
        .expect(1)
        .mount(&reactome)
        .await;

    Mock::given(method("GET"))
        .and(path("/find/pathway/apoptosis"))
        .respond_with(ResponseTemplate::new(200).set_body_string(""))
        .expect(1)
        .mount(&kegg)
        .await;

    Mock::given(method("GET"))
        .and(path("/findPathwaysByText.json"))
        .respond_with(ResponseTemplate::new(404).set_body_raw(
            "<!DOCTYPE html><html><head><title>404</title></head><body>File not found</body></html>",
            "text/html; charset=utf-8",
        ))
        .expect(1)
        .mount(&wikipathways)
        .await;

    let result = run_pathway_search(&reactome.uri(), &kegg.uri(), &wikipathways.uri());

    assert!(
        result.status.success(),
        "pathway search should succeed with surviving Reactome results\nstdout:\n{}\nstderr:\n{}",
        result.stdout,
        result.stderr
    );
    assert!(
        result.stdout.contains("Reactome"),
        "stdout should contain surviving pathway rows\nstdout:\n{}",
        result.stdout
    );
    assert!(
        result.stdout.contains("R-HSA-109581"),
        "stdout should contain the Reactome hit\nstdout:\n{}",
        result.stdout
    );
    assert!(
        result.stderr.contains(
            "WikiPathways search unavailable: API error from wikipathways: HTTP 404 Not Found"
        ),
        "stderr should surface the source warning\nstdout:\n{}\nstderr:\n{}",
        result.stdout,
        result.stderr
    );
    assert!(
        result.stderr.contains("HTML error page"),
        "stderr should contain the sanitized HTML summary\nstderr:\n{}",
        result.stderr
    );
    for forbidden in ["<!DOCTYPE", "<html", "<head"] {
        assert!(
            !result.stderr.contains(forbidden),
            "stderr should not leak raw HTML tags: {forbidden}\nstderr:\n{}",
            result.stderr
        );
        assert!(
            !result.stdout.contains(forbidden),
            "stdout should not leak raw HTML tags: {forbidden}\nstdout:\n{}",
            result.stdout
        );
    }
}
