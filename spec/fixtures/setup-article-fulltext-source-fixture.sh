#!/usr/bin/env bash
set -euo pipefail

workspace_root="${1:-$PWD}"
repo_root="$(git -C "$workspace_root" rev-parse --show-toplevel 2>/dev/null || printf '%s\n' "$workspace_root")"
cache_dir="$workspace_root/.cache"
env_file="$cache_dir/spec-article-fulltext-source-env"
lock_file="$cache_dir/spec-article-fulltext-source.lock"

mkdir -p "$cache_dir"
exec 9>"$lock_file"
flock 9

if [ -f "$env_file" ]; then
  # shellcheck disable=SC1090
  . "$env_file"
  if [ -n "${BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_PID:-}" ] \
    && kill -0 "$BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_PID" 2>/dev/null; then
    printf '%s\n' "${BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_ROOT:-$cache_dir}"
    exit 0
  fi
  if [ -n "${BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_ROOT:-}" ] \
    && [ -d "${BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_ROOT:-}" ]; then
    rm -rf "$BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_ROOT"
  fi
fi

fixture_root="$(mktemp -d "$cache_dir/spec-article-fulltext-source.XXXXXX")"
ready_file="$fixture_root/base-url"
server_log="$fixture_root/server.log"

python3 - "$ready_file" "$repo_root/tests/fixtures/article/fulltext" <<'PY' >"$server_log" 2>&1 &
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, unquote, urlparse
import json
import sys


FIXTURE_DIR = Path(sys.argv[2])
HTML_FALLBACK = (FIXTURE_DIR / "pmc-html-fallback.html").read_text(encoding="utf-8")
PDF_FALLBACK = (
    FIXTURE_DIR / "semantic-scholar-fallback.pdf"
).read_bytes()


ARTICLE_XML = """<article><front><article-meta><title-group><article-title>Europe full text winner</article-title></title-group><abstract><p>Abstract text.</p></abstract></article-meta></front><body><p>Europe PMC body text.</p></body></article>"""


ARTICLES = {
    "22663011": {
        "pmcid": "PMC123456",
        "title": "Europe full text winner",
        "abstract": "Abstract text.",
        "paper_id": "paper-1",
    },
    "22663012": {
        "pmcid": "PMC123457",
        "title": "PMC HTML fallback winner",
        "abstract": "Abstract text.",
        "paper_id": "paper-2",
    },
    "22663013": {
        "pmcid": "PMC123458",
        "title": "Open access PDF fallback winner",
        "abstract": "Abstract text.",
        "paper_id": "paper-3",
    },
}


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


def send_bytes(handler, status, body, content_type):
    handler.send_response(status)
    handler.send_header("Content-Type", content_type)
    handler.send_header("Content-Length", str(len(body)))
    handler.end_headers()
    handler.wfile.write(body)


def pubtator_payload(pmid):
    article = ARTICLES[pmid]
    return {
        "PubTator3": [
            {
                "pmid": int(pmid),
                "pmcid": article["pmcid"],
                "passages": [
                    {"infons": {"type": "title"}, "text": article["title"]},
                    {"infons": {"type": "abstract"}, "text": article["abstract"]},
                ],
            }
        ]
    }


def europepmc_search_payload(pmid):
    article = ARTICLES[pmid]
    return {
        "hitCount": 1,
        "resultList": {
            "result": [
                {
                    "id": pmid,
                    "pmid": pmid,
                    "pmcid": article["pmcid"],
                    "title": article["title"],
                    "journalTitle": "Journal One",
                    "firstPublicationDate": "2025-01-01",
                }
            ]
        },
    }


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        parsed = urlparse(self.path)
        decoded_path = unquote(parsed.path)
        query = parse_qs(parsed.query)

        pmids = query.get("pmids")
        if decoded_path == "/publications/export/biocjson" and pmids and pmids[0] in ARTICLES:
            send_json(self, 200, pubtator_payload(pmids[0]))
            return

        search_query = query.get("query")
        if (
            decoded_path == "/search"
            and search_query
            and query.get("format") == ["json"]
            and query.get("page") == ["1"]
            and query.get("pageSize") == ["1"]
        ):
            for pmid in ARTICLES:
                if search_query == [f"EXT_ID:{pmid} AND SRC:MED"]:
                    send_json(self, 200, europepmc_search_payload(pmid))
                    return

        if decoded_path == "/PMC123456/fullTextXML":
            send_text(self, 200, ARTICLE_XML, "application/xml")
            return

        if decoded_path in {"/PMC123457/fullTextXML", "/PMC123458/fullTextXML", "/22663012/fullTextXML", "/22663013/fullTextXML"}:
            send_text(self, 404, "not found", "text/plain")
            return

        if decoded_path == "/" and query.get("id") in (["PMC123457"], ["PMC123458"]):
            send_text(self, 200, "<records></records>", "application/xml")
            return

        if decoded_path == "/articles/PMC123457/":
            send_text(self, 200, HTML_FALLBACK, "text/html; charset=utf-8")
            return

        if decoded_path == "/articles/PMC123458/":
            send_text(self, 404, "not found", "text/plain")
            return

        if decoded_path.startswith("/graph/v1/paper/PMID:"):
            pmid = decoded_path.rsplit(":", 1)[-1]
            article = ARTICLES.get(pmid)
            if article is None:
                send_json(self, 404, {"error": "not found"})
                return
            payload = {
                "paperId": article["paper_id"],
                "title": article["title"],
            }
            if pmid == "22663013":
                payload["openAccessPdf"] = {
                    "url": f"http://127.0.0.1:{self.server.server_port}/pdf/22663013.pdf",
                    "status": "GREEN",
                    "license": "CC BY",
                }
            send_json(self, 200, payload)
            return

        if decoded_path == "/pdf/22663013.pdf":
            send_bytes(self, 200, PDF_FALLBACK, "application/pdf")
            return

        if decoded_path == "/efetch.fcgi":
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
printf 'export BIOMCP_PMC_HTML_BASE=%q\n' "$base_url" >>"$env_file"
printf 'export BIOMCP_S2_BASE=%q\n' "$base_url" >>"$env_file"
printf 'unset NCBI_API_KEY\n' >>"$env_file"
printf 'unset S2_API_KEY\n' >>"$env_file"
printf 'export BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_PID=%q\n' "$server_pid" >>"$env_file"
printf 'export BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_ROOT=%q\n' "$fixture_root" >>"$env_file"
printf 'export BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_READY_FILE=%q\n' "$ready_file" >>"$env_file"

printf '%s\n' "$fixture_root"
