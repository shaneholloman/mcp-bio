## Spike Question

Can BioMCP support an end-to-end curated biomedical news pipeline covering source discovery, article content extraction, entity/identifier extraction, BioMCP cross-references, and personalized briefing generation?

Success means validating the six target sources at live-source scale, measuring content extraction on real articles, extracting useful biomedical entities from current stories, proving several BioMCP pivots, and producing a sample personalized briefing for an oncologist interested in immunotherapy, KRAS, and melanoma.

Measurements were generated on 2026-04-19:

- `architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/results/discovery_results.json`
- `architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/results/article_extraction_results.json`
- `architecture/experiments/245-biomedical-news-discovery-and-personalized-briefing/results/entity_briefing_results.json`

## Prior Art Summary

The ticket did not point to a prior news implementation, but existing BioMCP source and entity code gives the implementation shape:

- `src/cli/commands.rs` already establishes the stable `search <entity>` / `get <entity>` grammar.
- `src/cli/article/mod.rs` uses the same typed-filter pattern the product docs want for news: `--gene`, `--disease`, `--drug`, `--keyword`, date/source/limit flags.
- `src/entities/article/mod.rs` models compact search cards and richer get cards with IDs, dates, source metadata, annotations, open-access/full-text notes, and enrichment.
- `src/transform/article.rs` keeps upstream parsing/cleanup behind a stable facade.
- `src/sources/mod.rs` provides the source-client discipline to reuse: shared HTTP client, retries, bounded body reads, cache controls, and no-cache/auth-aware behavior.

Reuse the command grammar, typed filters, source-client discipline, and compact-search/rich-get split. Adapt with a declarative publisher registry because publisher news access needs discovery/access/rights/fetch/extract fields that differ from current structured API clients.

## Approaches Tried

Approach 1: RSS/feed-first discovery

Script: `scripts/news_discovery_probe.py`

How: tested explicit RSS candidates plus feed autodiscovery from each source landing page. Measured HTTP status, feed entries, field coverage, latest item age, and sample article links.

Results:

| Source | Best feed | Entries | Latest age | Result |
| --- | --- | ---: | ---: | --- |
| Fierce Biotech | `/rss/xml` | 25 | 37.3h | Good RSS discovery |
| Fierce Pharma | `/rss/xml` | 25 | 50.0h | Good RSS discovery |
| BioPharma Dive | `/feeds/news/` | 10 | 40.3h | Good RSS discovery |
| STAT | `/category/biotech/feed/` | 20 | 4.4h | Good RSS discovery |
| Endpoints News | `/feed/` | 24 | 13.6h | Good RSS discovery |
| GenomeWeb | tested feed paths | 0 | n/a | Blocked/no discovery over unauthenticated direct HTTP |

Finding: RSS is the winning discovery mode for five of six sources. It is fresher, cleaner, and easier to normalize than generic headline-page scraping.

Approach 2: headline-page discovery

Script: `scripts/news_discovery_probe.py`

How: fetched each source landing/news page and extracted candidate article links with generic HTML heuristics and date detection.

Results:

- Fierce Biotech and Fierce Pharma homepages fetched successfully and yielded 12 candidate links each, but no date fields.
- BioPharma Dive yielded 25 candidate links and 4 date fields.
- STAT yielded 25 candidate links and 17 date fields.
- Endpoints yielded 25 links but needed source-specific filtering because generic heuristics can over-rank topic hubs.
- GenomeWeb homepage returned 403.

Finding: headline pages are useful fallback and enrichment surfaces, but not the best default. They need source-specific selectors/rules before shipping.

Approach 3: HTTP article extraction with Trafilatura

Script: `scripts/news_extract_articles.py`

How: fetched discovered article URLs with direct HTTP and ran Trafilatura plus a simple HTML-text fallback. Measured HTTP status, extracted text length, fallback text length, paywall/auth signals, access label, and quality score.

Results:

- 20 articles attempted.
- 8 useful extractions from 2 sources: BioPharma Dive and STAT.
- BioPharma Dive: 4/4 useful, quality 5/5, 2939-4592 extracted characters.
- STAT: 4/4 useful by text threshold, quality 4-5/5, 876-2391 extracted characters. Several were `STAT+`, so rights/access handling must remain conservative even when direct fetch returns readable text.
- Endpoints: 4/4 HTTP 200 but only partial text, max 414 Trafilatura chars and 702 fallback chars.
- Fierce Biotech and Fierce Pharma article pages returned 403 Cloudflare "Just a moment" pages under direct HTTP, even though RSS/headline discovery worked.
- GenomeWeb was already blocked at discovery.

Finding: Trafilatura is good enough for open HTML and should be promoted for MVP extraction. Direct HTTP is not enough for all target sources; exploit should support article-level status values such as `discovered`, `http_blocked`, `partial`, `auth_required`, and `extracted`.

Approach 4: heuristic entities, BioMCP pivots, and personalized briefing

Script: `scripts/news_entity_briefing.py`

How: ran regex/dictionary extraction for DOI, PMID, NCT IDs, genes, drugs, diseases, companies, trial/approval cues, then validated pivots through the installed `biomcp` CLI. Ranked articles against the sample profile: oncologist interested in immunotherapy, KRAS, melanoma.

Results:

- 10 articles analyzed; 9 had at least one entity/company/disease/gene/drug signal.
- 5/5 BioMCP pivot attempts succeeded:
  - `biomcp get gene MET`
  - `biomcp get drug daraxonrasib`
  - `biomcp get gene KRAS`
  - `biomcp get disease "pancreatic cancer"`
  - `biomcp search trial -i daraxonrasib --limit 1`
- The sample profile ranked a STAT KRAS/pancreatic cancer/daraxonrasib story first with score 20.
- False positives/weak relevance appeared in generic business stories because broad terms like `oncology`, `cancer`, and `phase` are not specific enough on their own.

Finding: keyword/entity heuristics are sufficient for MVP personalization if the first release is transparent and conservative. Embeddings are not needed for the first build, but ranking should penalize generic matches and boost typed entities that match the profile.

## Decision

Promote a narrowed MVP build.

The winning build path is:

- RSS-first discovery for default sources.
- Headline-page fallback only with source-specific rules.
- Trafilatura for HTTP article extraction.
- Explicit access/extraction status per article.
- Heuristic entity extraction for MVP, focused on high-precision identifiers and curated dictionaries.
- BioMCP pivot suggestions attached to news records, with canonical objects preferred over publisher prose when DOI/PMID/NCT/drug/gene/disease signals are found.
- Keyword/entity profile scoring for the first personalized briefing implementation.

Recommended MVP source scope:

- Default discovery enabled: Fierce Biotech, Fierce Pharma, BioPharma Dive, STAT, Endpoints.
- Default content extraction enabled with high confidence: BioPharma Dive.
- Content extraction enabled but marked conservative/metered: STAT.
- Discovery-only or partial extraction status until browser/auth support: Fierce Biotech, Fierce Pharma, Endpoints.
- Defer GenomeWeb to a browser/auth or registered-access follow-up; unauthenticated direct HTTP returned 403 in this spike.

Recommended MVP feature scope:

- `biomcp search news` with `--keyword`, `--source`, `--since`, `--until`, `--access`, `--limit`, plus typed filters `--drug`, `--gene`, `--disease`, `--trial`, `--company`.
- `biomcp get news <id>` returning metadata, source/access status, extracted summary when available, entities, and suggested BioMCP pivots.
- Local cache of metadata and structured extraction. Do not sync/export premium full text.
- Defer Playwright auth/login flows, embeddings, broad NER, and arbitrary user-added source crawling.

## Outcome

promote

The feature is feasible, but only as a curated, source-aware subsystem. Do not promise uniform full-text reading across the six default sources in the first exploit.

## Risks for Exploit

- Fierce article pages returned Cloudflare 403 under direct HTTP; exploit needs browser-fetch design or must mark them discovery-only initially.
- GenomeWeb returned 403 for unauthenticated direct HTTP discovery; defer or require authenticated/browser path.
- STAT returned readable text for several `STAT+` pages, but rights/access rules should still treat it as metered/subscriber and avoid syncing full text.
- Endpoints RSS is good, but direct extraction produced short partial text. Needs source-specific extraction or browser testing.
- Generic headline-page scraping can over-rank topic hubs and navigation-like pages; ship source-specific selectors instead of one global HTML heuristic.
- Heuristic entity extraction has known false positives for short gene symbols and drug suffixes. Use high-precision identifiers first and expose confidence/status rather than pretending full NER.
- Personalization keyword scoring works for obvious profile matches, but generic terms can rank weak stories too high. Add specificity weighting and source/category penalties before user-facing output.
- Live publisher feeds and anti-bot behavior can change without notice. Source tests should become part of a maintenance command like `biomcp news sources test`.
