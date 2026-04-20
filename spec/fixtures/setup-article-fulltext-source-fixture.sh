#!/usr/bin/env bash
set -euo pipefail

workspace_root="${1:-$PWD}"
cache_dir="$workspace_root/.cache"
env_file="$cache_dir/spec-article-fulltext-source-env"

mkdir -p "$cache_dir"

if [ -f "$env_file" ]; then
  # shellcheck disable=SC1090
  . "$env_file"
  if [ -n "${BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_PID:-}" ] \
    && kill -0 "$BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_PID" 2>/dev/null; then
    kill "$BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_PID" 2>/dev/null || true
  fi
  if [ -n "${BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_ROOT:-}" ] \
    && [ -d "${BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_ROOT:-}" ]; then
    rm -rf "$BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_ROOT"
  fi
fi

fixture_root="$(mktemp -d "$cache_dir/spec-article-fulltext-source.XXXXXX")"
ready_file="$fixture_root/base-url"
server_log="$fixture_root/server.log"

python3 - "$ready_file" <<'PY' >"$server_log" 2>&1 &
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse
import json
import sys


ARTICLE_XML = """<article><front><article-meta><title-group><article-title>Europe full text winner</article-title></title-group><abstract><p>Abstract text.</p></abstract></article-meta></front><body><p>Europe PMC body text.</p></body></article>"""


def send_json(handler, status, payload):
    body = json.dumps(payload).encode("utf-8")
    handler.send_response(status)
    handler.send_header("Content-Type", "application/json")
    handler.send_header("Content-Length", str(len(body)))
    handler.end_headers()
    handler.wfile.write(body)


def send_text(handler, status, body, content_type):
    payload = body.encode("utf-8")
    handler.send_response(status)
    handler.send_header("Content-Type", content_type)
    handler.send_header("Content-Length", str(len(payload)))
    handler.end_headers()
    handler.wfile.write(payload)


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        parsed = urlparse(self.path)
        query = parse_qs(parsed.query)

        if parsed.path == "/publications/export/biocjson" and query.get("pmids") == ["22663011"]:
            send_json(
                self,
                200,
                {
                    "PubTator3": [
                        {
                            "pmid": 22663011,
                            "pmcid": "PMC123456",
                            "passages": [
                                {"infons": {"type": "title"}, "text": "Europe full text winner"},
                                {"infons": {"type": "abstract"}, "text": "Abstract text."},
                            ],
                        }
                    ]
                },
            )
            return

        if (
            parsed.path == "/search"
            and query.get("query") == ["EXT_ID:22663011 AND SRC:MED"]
            and query.get("format") == ["json"]
            and query.get("page") == ["1"]
            and query.get("pageSize") == ["1"]
        ):
            send_json(
                self,
                200,
                {
                    "hitCount": 1,
                    "resultList": {
                        "result": [
                            {
                                "id": "22663011",
                                "pmid": "22663011",
                                "pmcid": "PMC123456",
                                "title": "Europe full text winner",
                                "journalTitle": "Journal One",
                                "firstPublicationDate": "2025-01-01",
                            }
                        ]
                    },
                },
            )
            return

        if parsed.path == "/PMC123456/fullTextXML":
            send_text(self, 200, ARTICLE_XML, "application/xml")
            return

        if parsed.path == "/graph/v1/paper/PMID:22663011":
            send_json(self, 200, {"paperId": "paper-1", "title": "Europe full text winner"})
            return

        if parsed.path == "/efetch.fcgi":
            send_text(self, 404, "not found", "text/plain")
            return

        send_json(self, 404, {"error": "not found"})

    def log_message(self, format, *args):
        return


ready_path = Path(sys.argv[1])
server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
ready_path.write_text(f"http://127.0.0.1:{server.server_port}\n", encoding="utf-8")
server.serve_forever()
PY
server_pid=$!

for _ in $(seq 1 50); do
  if [ -s "$ready_file" ]; then
    break
  fi
  if ! kill -0 "$server_pid" 2>/dev/null; then
    cat "$server_log" >&2
    exit 1
  fi
  sleep 0.1
done

test -s "$ready_file"
base_url="$(cat "$ready_file")"

printf 'export BIOMCP_PUBTATOR_BASE=%q\n' "$base_url" >"$env_file"
printf 'export BIOMCP_EUROPEPMC_BASE=%q\n' "$base_url" >>"$env_file"
printf 'export BIOMCP_PUBMED_BASE=%q\n' "$base_url" >>"$env_file"
printf 'export BIOMCP_PMC_OA_BASE=%q\n' "$base_url" >>"$env_file"
printf 'export BIOMCP_S2_BASE=%q\n' "$base_url" >>"$env_file"
printf 'unset NCBI_API_KEY\n' >>"$env_file"
printf 'unset S2_API_KEY\n' >>"$env_file"
printf 'export BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_PID=%q\n' "$server_pid" >>"$env_file"
printf 'export BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_ROOT=%q\n' "$fixture_root" >>"$env_file"
printf 'export BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_READY_FILE=%q\n' "$ready_file" >>"$env_file"

printf '%s\n' "$fixture_root"
