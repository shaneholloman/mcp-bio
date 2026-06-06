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
request_log="$fixture_root/request-log.txt"
: > "$request_log"

python3 - "$ready_file" "$repo_root/tests/fixtures/article/fulltext" "$request_log" <<'PY' >"$server_log" 2>&1 &
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, unquote, urlparse
import io
import json
import sys
import tarfile
import threading


FIXTURE_DIR = Path(sys.argv[2])
REQUEST_LOG = Path(sys.argv[3])
REQUEST_LOG_LOCK = threading.Lock()
HTML_FALLBACK = (
    FIXTURE_DIR / "html" / "pmc_article_page.html"
).read_text(encoding="utf-8")
PDF_FALLBACK = (
    FIXTURE_DIR / "pdf" / "pmc_oa_article_pdf.pdf"
).read_bytes()
FIGSHARE_SUPPLEMENT = b"%PDF-1.4\nFigshare supplemental fixture bytes\n%%EOF\n"


ARTICLE_XML = """<article xmlns:xlink="http://www.w3.org/1999/xlink">
  <front>
    <article-meta>
      <title-group><article-title>Europe full text winner</article-title></title-group>
      <abstract><p>Abstract text.</p></abstract>
    </article-meta>
  </front>
  <body>
    <sec>
      <title>Fixture results</title>
      <p>Europe PMC body text with callout (<xref ref-type="fig" rid="fig2">Figure 2</xref>) and B-RAF<sup>V600E</sup>.PLX4032 boundary text.</p>
      <fig id="fig1">
        <label>Figure 1</label>
        <caption><p>Inline figure caption preserves n=10 cell counts.</p></caption>
        <graphic xlink:href="figure-inline.png" />
      </fig>
      <table-wrap id="t1">
        <label>Table 1</label>
        <caption><p>Fixture quality table.</p></caption>
        <table>
          <tr><th>Signal</th><th>Value</th></tr>
          <tr><td>full text</td><td>present</td></tr>
        </table>
      </table-wrap>
      <table-wrap id="t2">
        <label>Table 2</label>
        <caption><p>Merged treatment table.</p></caption>
        <table>
          <tr><th rowspan="2">Cohort</th><th>Baseline</th><th>Week 8</th></tr>
          <tr><td>10</td><td>4</td></tr>
        </table>
      </table-wrap>
      <supplementary-material id="s1" xlink:href="traces-s1.csv">
        <label>Supplementary Data S1</label>
        <caption><p>Measurement traces for the treatment cohort.</p></caption>
        <media xlink:href="traces-s1.csv" />
      </supplementary-material>
    </sec>
  </body>
  <floats-group>
    <fig id="fig2">
      <label>Figure 2</label>
      <caption><p>Floats-group figure reports measurement bar is 70 μm.</p></caption>
      <graphic xlink:href="figure-floats.png" />
    </fig>
  </floats-group>
  <back>
    <ref-list>
      <ref id="R1"><mixed-citation>Fixture reference.</mixed-citation></ref>
    </ref-list>
  </back>
</article>"""


def make_oa_assets_tgz():
    entries = {
        "article.nxml": ARTICLE_XML.encode("utf-8"),
        "figure-inline.png": b"fixture-inline-figure-bytes\n",
        "figure-floats.png": b"fixture-floats-figure-bytes\n",
        "traces-s1.csv": b"time,value\n0,1\n",
        "readme.txt": b"package sidecar\n",
    }
    out = io.BytesIO()
    with tarfile.open(fileobj=out, mode="w:gz") as archive:
        for name, body in entries.items():
            info = tarfile.TarInfo(name)
            info.size = len(body)
            info.mode = 0o644
            archive.addfile(info, io.BytesIO(body))
    return out.getvalue()


OA_ASSETS_TGZ = make_oa_assets_tgz()


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
    "22663014": {
        "pmcid": None,
        "title": "Resolver order miss",
        "abstract": "Abstract text.",
        "paper_id": "paper-4",
    },
    "22663015": {
        "pmcid": None,
        "title": "Figshare asset fallback winner",
        "abstract": "Abstract text.",
        "paper_id": "paper-5",
    },
}


def append_request_log(line):
    with REQUEST_LOG_LOCK:
        with REQUEST_LOG.open("a", encoding="utf-8") as handle:
            handle.write(f"{line}\n")


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
    record = {
        "pmid": int(pmid),
        "passages": [
            {"infons": {"type": "title"}, "text": article["title"]},
            {"infons": {"type": "abstract"}, "text": article["abstract"]},
        ],
    }
    if article["pmcid"]:
        record["pmcid"] = article["pmcid"]
    return {
        "PubTator3": [record]
    }


def europepmc_search_payload(pmid):
    article = ARTICLES[pmid]
    result = {
        "id": pmid,
        "pmid": pmid,
        "title": article["title"],
        "journalTitle": "Journal One",
        "firstPublicationDate": "2025-01-01",
    }
    if article["pmcid"]:
        result["pmcid"] = article["pmcid"]
        result["isOpenAccess"] = "Y"
        result["fullTextIdList"] = {"fullTextId": [article["pmcid"]]}
        result["fullTextUrlList"] = {
            "fullTextUrl": [
                {
                    "availability": "Open access",
                    "availabilityCode": "OA",
                    "documentStyle": "html",
                    "site": "Europe PMC",
                    "url": f"https://europepmc.org/articles/{article['pmcid']}",
                }
            ]
        }
    if pmid == "22663011":
        result["license"] = "CC BY"
    if pmid == "22663015":
        result["doi"] = "10.1158/fixture.figshare"
    return {
        "hitCount": 1,
        "resultList": {
            "result": [result]
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

        if (
            decoded_path == "/"
            and query.get("idtype") == ["pmid"]
            and query.get("ids") == ["22663014"]
        ):
            append_request_log("fulltext:identity:ncbi-idconv")
            send_json(self, 200, {"records": [{"pmid": 22663014, "pmcid": "PMC123459"}]})
            return

        if (
            decoded_path == "/"
            and query.get("idtype") == ["pmid"]
            and query.get("ids") == ["22663015"]
        ):
            send_json(self, 200, {"records": [{"pmid": 22663015}]})
            return

        if (
            decoded_path == "/"
            and query.get("idtype") == ["doi"]
            and query.get("ids") == ["10.1158/fixture.figshare"]
        ):
            send_json(self, 200, {"records": [{"doi": "10.1158/fixture.figshare"}]})
            return

        if decoded_path == "/PMC123456/fullTextXML":
            send_text(self, 200, ARTICLE_XML, "application/xml")
            return

        if decoded_path == "/PMC123459/fullTextXML":
            append_request_log("fulltext:xml:europepmc-pmc")
            send_text(self, 404, "not found", "text/plain")
            return

        if decoded_path == "/22663014/fullTextXML":
            append_request_log("fulltext:xml:europepmc-med")
            send_text(self, 404, "not found", "text/plain")
            return

        if decoded_path in {"/PMC123457/fullTextXML", "/PMC123458/fullTextXML", "/22663012/fullTextXML", "/22663013/fullTextXML"}:
            send_text(self, 404, "not found", "text/plain")
            return

        if decoded_path == "/" and query.get("id") == ["PMC123456"]:
            send_text(self, 200, f"""<records><record license=\"CC BY\" retracted=\"no\"><link format=\"tgz\" href=\"http://127.0.0.1:{self.server.server_port}/oa-assets-22663011.tgz\" /></record></records>""", "application/xml")
            return

        if decoded_path == "/oa-assets-22663011.tgz":
            send_bytes(self, 200, OA_ASSETS_TGZ, "application/gzip")
            return

        if decoded_path == "/" and query.get("id") == ["PMC123459"]:
            append_request_log("fulltext:xml:pmc-oa-archive")
            send_text(self, 200, "<records></records>", "application/xml")
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

        if decoded_path == "/articles/PMC123459/":
            append_request_log("fulltext:html:pmc")
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
            if pmid == "22663014":
                payload["openAccessPdf"] = {
                    "url": f"http://127.0.0.1:{self.server.server_port}/pdf/22663014.pdf",
                    "status": "GREEN",
                    "license": "CC BY",
                }
            if pmid == "22663015":
                payload["openAccessPdf"] = {
                    "url": "https://aacr.figshare.com/articles/journal_contribution/Fixture_Figshare_supplement/22474820?file=39926318",
                    "status": "GREEN",
                    "license": "CC BY 4.0",
                }
            send_json(self, 200, payload)
            return

        if decoded_path == "/v2/articles/22474820":
            send_json(self, 200, {
                "id": 22474820,
                "doi": "10.1158/fixture.figshare",
                "url_public_html": "https://aacr.figshare.com/articles/journal_contribution/Fixture_Figshare_supplement/22474820",
                "url_api": f"http://127.0.0.1:{self.server.server_port}/v2/articles/22474820",
                "license": {
                    "name": "CC BY 4.0",
                    "url": "https://creativecommons.org/licenses/by/4.0/",
                },
                "files": [
                    {
                        "id": 39926318,
                        "name": "figshare-supplement.pdf",
                        "size": len(FIGSHARE_SUPPLEMENT),
                        "md5": "0123456789abcdef0123456789abcdef",
                        "mimetype": "application/pdf",
                        "download_url": f"http://127.0.0.1:{self.server.server_port}/figshare/files/39926318/figshare-supplement.pdf",
                    }
                ],
            })
            return

        if decoded_path == "/figshare/files/39926318/figshare-supplement.pdf":
            send_bytes(self, 200, FIGSHARE_SUPPLEMENT, "application/pdf")
            return

        if decoded_path == "/pdf/22663013.pdf":
            send_bytes(self, 200, PDF_FALLBACK, "application/pdf")
            return

        if decoded_path == "/pdf/22663014.pdf":
            append_request_log("fulltext:pdf:semantic-scholar")
            send_text(self, 404, "not found", "text/plain")
            return

        if decoded_path == "/efetch.fcgi":
            if query.get("id") == ["123459"]:
                append_request_log("fulltext:xml:ncbi-efetch-pmc")
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
printf 'export BIOMCP_NCBI_IDCONV_BASE=%q\n' "$base_url" >>"$env_file"
printf 'export BIOMCP_S2_BASE=%q\n' "$base_url" >>"$env_file"
printf 'export BIOMCP_FIGSHARE_BASE=%q\n' "$base_url" >>"$env_file"
printf 'unset NCBI_API_KEY\n' >>"$env_file"
printf 'unset S2_API_KEY\n' >>"$env_file"
printf 'export BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_PID=%q\n' "$server_pid" >>"$env_file"
printf 'export BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_ROOT=%q\n' "$fixture_root" >>"$env_file"
printf 'export BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_READY_FILE=%q\n' "$ready_file" >>"$env_file"
printf 'export BIOMCP_ARTICLE_FULLTEXT_SOURCE_FIXTURE_REQUEST_LOG=%q\n' "$request_log" >>"$env_file"

printf '%s\n' "$fixture_root"
