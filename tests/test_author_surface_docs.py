from __future__ import annotations

import json
import os
import select
import subprocess
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse

import pytest

ROOT = Path(__file__).resolve().parents[1]


def _read(path: str) -> str:
    return (ROOT / path).read_text()


def test_author_surface_docs_describe_the_shipped_provider_exact_slice() -> None:
    functional = _read("architecture/functional/overview.md")
    runtime = _read("architecture/technical/semantic-scholar-runtime-contract.md")
    ux = _read("architecture/ux/cli-reference.md")
    source_guide = _read("docs/sources/semantic-scholar.md")

    assert "| author | Semantic Scholar provider records |" in functional
    assert "BioMCP does not yet ship an author entity" not in functional

    assert "public provider-exact `search author`" in runtime
    assert "without introducing a public author command" not in runtime

    for text in (ux, source_guide):
        assert "search author" in text
        assert "get author semanticscholar:" in text
        assert "--source semanticscholar" in text

    assert "BioMCP does not yet ship these commands" not in ux


RELEASE_BIN = Path(os.environ.get("BIOMCP_BIN", ROOT / "target" / "debug" / "biomcp"))

RICH_ROW = {
    "paperId": "0123456789abcdef0123456789abcdef01234567",
    "corpusId": 277710284,
    "externalIds": {
        "PubMed": "40215974",
        "DOI": "10.1016/j.fixture.2024.01.001",
        "ORCID": "0000-0002-7433-2740",
    },
    "title": "A rich author paper fixture",
    "abstract": "Source abstract.",
    "venue": "Fixture Medicine",
    "year": 2024,
    "publicationDate": "2024-01-31",
    "citationCount": 17,
    "referenceCount": 23,
    "influentialCitationCount": 2,
    "isOpenAccess": False,
    "openAccessPdf": {"url": "https://example.invalid/paper.pdf", "status": "HYBRID", "license": None},
    "fieldsOfStudy": ["Medicine"],
    "publicationTypes": ["JournalArticle"],
    "authors": [{"authorId": "1716151", "name": "A. Butte"}],
}

PAPERS_BODY = {"offset": 0, "next": 1, "data": [RICH_ROW]}


class _RecordingHandler(BaseHTTPRequestHandler):
    def do_GET(self) -> None:  # noqa: N802 - http.server naming
        parsed = urlparse(self.path)
        query = parse_qs(parsed.query)
        self.server.requests.append((parsed.path, query))  # type: ignore[attr-defined]
        if parsed.path == "/graph/v1/author/1716151/papers":
            body = json.dumps(PAPERS_BODY).encode("utf-8")
        else:
            body = json.dumps({"error": f"unexpected path {parsed.path}"}).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_args: object) -> None:
        return


class _FixtureServer:
    def __init__(self) -> None:
        self._server = ThreadingHTTPServer(("127.0.0.1", 0), _RecordingHandler)
        self._server.daemon_threads = True
        self._server.requests = []  # type: ignore[attr-defined]
        self._thread = threading.Thread(target=self._server.serve_forever, daemon=True)
        self._thread.start()
        import urllib.request

        for _ in range(20):
            try:
                urllib.request.urlopen(f"{self.base}/health", timeout=0.5).read()
                break
            except Exception:
                import time

                time.sleep(0.05)
        self._server.requests.clear()  # type: ignore[attr-defined]

    @property
    def base(self) -> str:
        host, port = self._server.server_address[:2]
        return f"http://{host}:{port}"

    @property
    def requests(self) -> list[tuple[str, dict[str, list[str]]]]:
        return list(self._server.requests)  # type: ignore[attr-defined]

    def close(self) -> None:
        self._server.shutdown()
        self._server.server_close()
        self._thread.join(timeout=2)


@pytest.fixture
def fixture() -> _FixtureServer:
    server = _FixtureServer()
    try:
        yield server
    finally:
        server.close()


def _env_with_base(base: str) -> dict[str, str]:
    env = dict(os.environ)
    env["BIOMCP_S2_BASE"] = base
    env["BIOMCP_TEST_UNPACED_ORIGIN"] = base
    env.pop("S2_API_KEY", None)
    for proxy in ("http_proxy", "https_proxy", "HTTP_PROXY", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"):
        env.pop(proxy, None)
    return env


def _run_cli(args: list[str], base: str) -> subprocess.CompletedProcess[str]:
    assert RELEASE_BIN.exists(), f"missing BioMCP binary: {RELEASE_BIN}"
    return subprocess.run(
        [str(RELEASE_BIN), *args],
        cwd=ROOT,
        env=_env_with_base(base),
        capture_output=True,
        text=True,
        timeout=60,
    )


class _StdioMcp:
    def __init__(self, base: str) -> None:
        assert RELEASE_BIN.exists(), f"missing BioMCP binary: {RELEASE_BIN}"
        self.process = subprocess.Popen(
            [str(RELEASE_BIN), "serve"],
            cwd=ROOT,
            env=_env_with_base(base),
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )

    def call(self, request: dict[str, object]) -> dict[str, object]:
        assert self.process.stdin is not None
        assert self.process.stdout is not None
        self.process.stdin.write(json.dumps(request, separators=(",", ":")) + "\n")
        self.process.stdin.flush()
        ready, _, _ = select.select([self.process.stdout], [], [], 30)
        if not ready:
            if self.process.poll() is not None:
                assert self.process.stderr is not None
                pytest.fail(
                    f"BioMCP serve exited ({self.process.returncode}): "
                    f"{self.process.stderr.read()[:500]}"
                )
            pytest.fail("timed out waiting for an MCP response")
        line = self.process.stdout.readline()
        assert line, "BioMCP exited before returning an MCP response"
        return json.loads(line)  # type: ignore[no-any-return]

    def notify(self, request: dict[str, object]) -> None:
        assert self.process.stdin is not None
        self.process.stdin.write(json.dumps(request, separators=(",", ":")) + "\n")
        self.process.stdin.flush()

    def tool(self, command: str) -> dict[str, object]:
        response = self.call(
            {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": {"name": "biomcp", "arguments": {"command": command}},
            }
        )
        return response["result"]  # type: ignore[no-any-return]

    def close(self) -> None:
        if self.process.poll() is None:
            self.process.terminate()
            self.process.wait(timeout=5)


RICH_COMMAND = "biomcp --json author papers semanticscholar:1716151 --full"
RICH_MARKDOWN_COMMAND = "biomcp author papers semanticscholar:1716151 --full"


def test_rich_cli_json_and_markdown_are_well_formed(fixture: _FixtureServer) -> None:
    result = _run_cli(
        ["--json", "author", "papers", "semanticscholar:1716151", "--full"],
        fixture.base,
    )
    assert result.returncode == 0, result.stderr
    page = json.loads(result.stdout)
    assert page["papers"][0]["abstract"] == "Source abstract."
    assert page["papers"][0]["citation_count"] == 17
    assert page["papers"][0]["open_access_pdf"]["status"] == "HYBRID"
    assert page["pagination"]["next"] == 1
    paths = [path for path, _ in fixture.requests]
    assert paths == ["/graph/v1/author/1716151/papers"], fixture.requests

    markdown = _run_cli(
        ["author", "papers", "semanticscholar:1716151", "--full"],
        fixture.base,
    )
    assert markdown.returncode == 0, markdown.stderr
    assert "### Authors" in markdown.stdout
    assert "- Open access: false" in markdown.stdout
    assert markdown.stdout.endswith("\n")


def test_rich_raw_mcp_matches_cli_byte_for_byte(fixture: _FixtureServer) -> None:
    cli_json = _run_cli(
        ["--json", "author", "papers", "semanticscholar:1716151", "--full"],
        fixture.base,
    ).stdout
    cli_markdown = _run_cli(
        ["author", "papers", "semanticscholar:1716151", "--full"],
        fixture.base,
    ).stdout

    server = _StdioMcp(fixture.base)
    try:
        server.call(
            {
                "jsonrpc": "2.0",
                "id": 0,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-03-26",
                    "capabilities": {},
                    "clientInfo": {"name": "parity-test", "version": "0"},
                },
            }
        )
        server.notify(
            {"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}}
        )

        result = server.tool(RICH_COMMAND)
        assert not result.get("isError", False), result
        assert result["content"][0]["text"].rstrip("\n") == cli_json.rstrip("\n")

        result = server.tool(RICH_MARKDOWN_COMMAND)
        assert not result.get("isError", False), result
        assert result["content"][0]["text"].rstrip("\n") == cli_markdown.rstrip("\n")
    finally:
        server.close()

    paths = [path for path, _ in fixture.requests]
    assert paths.count("/graph/v1/author/1716151/papers") == 4, fixture.requests
    for path, _ in fixture.requests:
        assert "/references" not in path
        assert "/citations" not in path
        assert "/batch" not in path
        assert "europepmc" not in path.lower()


def test_rich_provider_error_is_an_mcp_error_not_an_empty_page(
    fixture: _FixtureServer,
) -> None:
    server = _StdioMcp(fixture.base)
    try:
        server.call(
            {
                "jsonrpc": "2.0",
                "id": 0,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-03-26",
                    "capabilities": {},
                    "clientInfo": {"name": "parity-test", "version": "0"},
                },
            }
        )
        result = server.tool(
            "biomcp --json author papers semanticscholar:unknown-author --full"
        )
        assert result.get("isError") is True
    finally:
        server.close()
