use super::*;

#[test]
fn pubmed_only_routes_use_common_finalizer_for_sorting() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    runtime.block_on(async {
        let _guard = lock_env().await;
        let server = MockServer::start().await;
        let _pubmed_base = set_env_var("BIOMCP_PUBMED_BASE", Some(&server.uri()));
        let mut filters = empty_filters();
        filters.gene = Some("BRAF".into());
        filters.sort = ArticleSort::Date;

        Mock::given(method("GET"))
            .and(path("/esearch.fcgi"))
            .and(query_param("db", "pubmed"))
            .and(query_param("retmode", "json"))
            .and(query_param("retstart", "0"))
            .and(query_param("retmax", "100"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "esearchresult": {
                    "count": "2",
                    "idlist": ["1", "2"]
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/esummary.fcgi"))
            .and(query_param("db", "pubmed"))
            .and(query_param("retmode", "json"))
            .and(query_param("id", "1,2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "result": {
                    "uids": ["1", "2"],
                    "1": {
                        "uid": "1",
                        "title": "Older title",
                        "sortpubdate": "2024/01/01 00:00",
                        "fulljournalname": "Nature",
                        "source": "Nature"
                    },
                    "2": {
                        "uid": "2",
                        "title": "Newer title",
                        "sortpubdate": "2025/02/01 00:00",
                        "fulljournalname": "Nature",
                        "source": "Nature"
                    }
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let page = search_page(&filters, 2, 0, ArticleSourceFilter::PubMed)
            .await
            .expect("pubmed search should succeed");
        let pmids: Vec<&str> = page.results.iter().map(|row| row.pmid.as_str()).collect();
        assert_eq!(pmids, vec!["2", "1"]);
    });
}

#[tokio::test]
async fn pubmed_source_search_enriches_citation_count_and_abstract_from_semantic_scholar_batch() {
    let _guard = lock_env().await;
    let pubmed = MockServer::start().await;
    let s2 = MockServer::start().await;
    let _pubmed_base = set_env_var("BIOMCP_PUBMED_BASE", Some(&pubmed.uri()));
    let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&s2.uri()));
    let _s2_key = set_env_var("S2_API_KEY", None);

    Mock::given(method("GET"))
        .and(path("/esearch.fcgi"))
        .and(query_param("db", "pubmed"))
        .and(query_param("retmode", "json"))
        .and(query_param("retstart", "0"))
        .and(query_param("retmax", "100"))
        .and(query_param("term", "GDNF RET Hirschsprung 1996"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "esearchresult": {
                "count": "1",
                "idlist": ["8896569"]
            }
        })))
        .expect(1)
        .mount(&pubmed)
        .await;

    Mock::given(method("GET"))
        .and(path("/esummary.fcgi"))
        .and(query_param("db", "pubmed"))
        .and(query_param("retmode", "json"))
        .and(query_param("id", "8896569"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "result": {
                "uids": ["8896569"],
                "8896569": {
                    "uid": "8896569",
                    "title": "A mutation of the RET proto-oncogene in Hirschsprung's disease increases its sensitivity to glial cell line-derived neurotrophic factor",
                    "sortpubdate": "1996/10/24 00:00",
                    "pubdate": "1996 Oct 24",
                    "fulljournalname": "Nature",
                    "source": "Nature"
                }
            }
        })))
        .expect(1)
        .mount(&pubmed)
        .await;

    Mock::given(method("POST"))
        .and(path("/graph/v1/paper/batch"))
        .and(query_param(
            "fields",
            "paperId,externalIds,citationCount,influentialCitationCount,abstract",
        ))
        .and(body_string_contains("\"PMID:8896569\""))
        .and(|request: &wiremock::Request| !request.headers.contains_key("x-api-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "paperId": "paper-8896569",
                "externalIds": {"PubMed": "8896569"},
                "citationCount": 231,
                "influentialCitationCount": 17,
                "abstract": "Glial cell line-derived neurotrophic factor signaling through RET is increased by the Hirschsprung disease mutation."
            }
        ])))
        .expect(1)
        .mount(&s2)
        .await;

    let page = search_page(
        &ArticleSearchFilters {
            keyword: Some("GDNF RET Hirschsprung 1996".into()),
            ..empty_filters()
        },
        1,
        0,
        ArticleSourceFilter::PubMed,
    )
    .await
    .expect("pubmed source search should succeed");

    assert_eq!(page.results.len(), 1);
    let row = &page.results[0];
    assert_eq!(row.pmid, "8896569");
    assert_eq!(row.source, ArticleSource::PubMed);
    assert_eq!(row.matched_sources, vec![ArticleSource::PubMed]);
    assert_eq!(row.citation_count, Some(231));
    assert_eq!(row.influential_citation_count, Some(17));
    assert!(
        row.abstract_snippet
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
    );
    assert!(!row.normalized_abstract.is_empty());
}

#[tokio::test]
async fn pubmed_search_falls_back_to_article_base_when_s2_returns_null_abstract() {
    let _guard = lock_env().await;
    let pubmed = MockServer::start().await;
    let pubtator = MockServer::start().await;
    let europepmc = MockServer::start().await;
    let s2 = MockServer::start().await;
    let _pubmed_base = set_env_var("BIOMCP_PUBMED_BASE", Some(&pubmed.uri()));
    let _pubtator_base = set_env_var("BIOMCP_PUBTATOR_BASE", Some(&pubtator.uri()));
    let _europepmc_base = set_env_var("BIOMCP_EUROPEPMC_BASE", Some(&europepmc.uri()));
    let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&s2.uri()));
    let _s2_key = set_env_var("S2_API_KEY", None);

    Mock::given(method("GET"))
        .and(path("/esearch.fcgi"))
        .and(query_param("db", "pubmed"))
        .and(query_param("retmode", "json"))
        .and(query_param("retstart", "0"))
        .and(query_param("retmax", "100"))
        .and(query_param("term", "GDNF RET Hirschsprung 1996"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "esearchresult": {"count": "1", "idlist": ["8896569"]}
        })))
        .expect(1)
        .mount(&pubmed)
        .await;

    Mock::given(method("GET"))
        .and(path("/esummary.fcgi"))
        .and(query_param("db", "pubmed"))
        .and(query_param("retmode", "json"))
        .and(query_param("id", "8896569"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "result": {
                "uids": ["8896569"],
                "8896569": {
                    "uid": "8896569",
                    "title": "A mutation of the RET proto-oncogene in Hirschsprung's disease",
                    "sortpubdate": "1996/10/24 00:00",
                    "pubdate": "1996 Oct 24",
                    "fulljournalname": "Nature",
                    "source": "Nature"
                }
            }
        })))
        .expect(1)
        .mount(&pubmed)
        .await;

    // S2 batch returns citation count but no abstract (null)
    Mock::given(method("POST"))
        .and(path("/graph/v1/paper/batch"))
        .and(query_param(
            "fields",
            "paperId,externalIds,citationCount,influentialCitationCount,abstract",
        ))
        .and(body_string_contains("\"PMID:8896569\""))
        .and(|request: &wiremock::Request| !request.headers.contains_key("x-api-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "paperId": "paper-8896569",
                "externalIds": {"PubMed": "8896569"},
                "citationCount": 231,
                "influentialCitationCount": 17,
                "abstract": null
            }
        ])))
        .expect(1)
        .mount(&s2)
        .await;

    // Article-base fallback: PubTator provides the abstract
    // Note: PubTatorInfons uses `#[serde(rename = "type")]` for the `kind` field,
    // so the JSON key must be "type", not "kind".
    Mock::given(method("GET"))
        .and(path("/publications/export/biocjson"))
        .and(query_param("pmids", "8896569"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "PubTator3": [{
                "pmid": 8896569,
                "passages": [
                    {
                        "infons": {"type": "abstract"},
                        "text": "Hirschsprung disease is a congenital malformation of the enteric nervous system."
                    }
                ]
            }]
        })))
        .expect(1)
        .mount(&pubtator)
        .await;

    // EuropePMC lookup triggered by resolve_article_from_pmid to merge citation metadata
    Mock::given(method("GET"))
        .and(path("/search"))
        .and(query_param("query", "EXT_ID:8896569 AND SRC:MED"))
        .and(query_param("format", "json"))
        .and(query_param("page", "1"))
        .and(query_param("pageSize", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "hitCount": 0,
            "resultList": {"result": []}
        })))
        .mount(&europepmc)
        .await;

    let page = search_page(
        &ArticleSearchFilters {
            keyword: Some("GDNF RET Hirschsprung 1996".into()),
            ..empty_filters()
        },
        1,
        0,
        ArticleSourceFilter::PubMed,
    )
    .await
    .expect("search should succeed with article-base abstract fallback");

    assert_eq!(page.results.len(), 1);
    let row = &page.results[0];
    assert_eq!(row.pmid, "8896569");
    assert_eq!(row.source, ArticleSource::PubMed);
    assert_eq!(row.matched_sources, vec![ArticleSource::PubMed]);
    // S2 citation count is preserved
    assert_eq!(row.citation_count, Some(231));
    assert_eq!(row.influential_citation_count, Some(17));
    // Abstract comes from PubTator fallback since S2 returned null
    assert!(
        row.abstract_snippet
            .as_deref()
            .is_some_and(|value| value.contains("Hirschsprung"))
    );
    assert!(row.normalized_abstract.contains("hirschsprung"));
}

#[tokio::test]
async fn federated_search_enrichment_overwrites_europepmc_zero_citation_count() {
    let _guard = lock_env().await;
    let pubtator = MockServer::start().await;
    let europepmc = MockServer::start().await;
    let pubmed = MockServer::start().await;
    let s2 = MockServer::start().await;
    let litsense2 = MockServer::start().await;
    let _pubtator_base = set_env_var("BIOMCP_PUBTATOR_BASE", Some(&pubtator.uri()));
    let _europepmc_base = set_env_var("BIOMCP_EUROPEPMC_BASE", Some(&europepmc.uri()));
    let _pubmed_base = set_env_var("BIOMCP_PUBMED_BASE", Some(&pubmed.uri()));
    let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&s2.uri()));
    let _litsense2_base = set_env_var("BIOMCP_LITSENSE2_BASE", Some(&litsense2.uri()));
    let _s2_key = set_env_var("S2_API_KEY", None);

    Mock::given(method("GET"))
        .and(query_param("page", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "results": [],
            "count": 0,
            "total_pages": 1,
            "current": 1,
            "page_size": 25,
            "facets": {}
        })))
        .expect(1)
        .mount(&pubtator)
        .await;

    Mock::given(method("GET"))
        .and(path("/search"))
        .and(query_param("format", "json"))
        .and(query_param("page", "1"))
        .and(query_param("pageSize", "25"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "hitCount": 2,
            "resultList": {
                "result": [
                    {
                        "id": "EP-1",
                        "pmid": "8896569",
                        "title": "RET mutation study",
                        "journalTitle": "Nature",
                        "firstPublicationDate": "1996-10-24",
                        "citedByCount": 0,
                        "abstractText": "Primary source abstract.",
                        "pubType": "journal article"
                    },
                    {
                        "id": "EP-2",
                        "pmid": "99900001",
                        "title": "Comparator study",
                        "journalTitle": "Science",
                        "firstPublicationDate": "1995-01-01",
                        "citedByCount": 5,
                        "abstractText": "Comparator abstract.",
                        "pubType": "journal article"
                    }
                ]
            }
        })))
        .expect(1)
        .mount(&europepmc)
        .await;

    Mock::given(method("GET"))
        .and(path("/esearch.fcgi"))
        .and(query_param("db", "pubmed"))
        .and(query_param("retmode", "json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "esearchresult": {
                "count": "0",
                "idlist": []
            }
        })))
        .expect(1)
        .mount(&pubmed)
        .await;

    Mock::given(method("GET"))
        .and(path("/graph/v1/paper/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total": 0,
            "data": []
        })))
        .expect(1)
        .mount(&s2)
        .await;

    Mock::given(method("GET"))
        .and(path("/sentences/"))
        .and(query_param("query", "GDNF RET Hirschsprung 1996"))
        .and(query_param("rerank", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(1)
        .mount(&litsense2)
        .await;

    Mock::given(method("POST"))
        .and(path("/graph/v1/paper/batch"))
        .and(query_param(
            "fields",
            "paperId,externalIds,citationCount,influentialCitationCount,abstract",
        ))
        .and(body_string_contains("\"PMID:8896569\""))
        .and(body_string_contains("\"PMID:99900001\""))
        .and(|request: &wiremock::Request| !request.headers.contains_key("x-api-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "paperId": "paper-8896569",
                "externalIds": {"PubMed": "8896569"},
                "citationCount": 231,
                "influentialCitationCount": 17,
                "abstract": "Semantic Scholar abstract for RET mutation study."
            },
            {
                "paperId": "paper-99900001",
                "externalIds": {"PubMed": "99900001"},
                "citationCount": 5,
                "influentialCitationCount": 1,
                "abstract": "Semantic Scholar abstract for comparator study."
            }
        ])))
        .expect(1)
        .mount(&s2)
        .await;

    let page = search_page(
        &ArticleSearchFilters {
            keyword: Some("GDNF RET Hirschsprung 1996".into()),
            sort: ArticleSort::Citations,
            ..empty_filters()
        },
        2,
        0,
        ArticleSourceFilter::All,
    )
    .await
    .expect("federated search should succeed");

    assert_eq!(
        page.results
            .iter()
            .map(|row| row.pmid.as_str())
            .collect::<Vec<_>>(),
        vec!["8896569", "99900001"]
    );
    assert_eq!(page.results[0].source, ArticleSource::EuropePmc);
    assert_eq!(
        page.results[0].matched_sources,
        vec![ArticleSource::EuropePmc]
    );
    assert_eq!(page.results[0].citation_count, Some(231));
}

#[tokio::test]
async fn article_search_semantic_scholar_batch_failure_is_non_fatal() {
    let _guard = lock_env().await;
    let pubtator = MockServer::start().await;
    let s2 = MockServer::start().await;
    let _pubtator_base = set_env_var("BIOMCP_PUBTATOR_BASE", Some(&pubtator.uri()));
    let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&s2.uri()));
    let _s2_key = set_env_var("S2_API_KEY", None);

    Mock::given(method("GET"))
        .and(query_param("page", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "results": [{
                "_id": "pt-1",
                "pmid": 22663011,
                "title": "Alternative microexon splicing in metastasis",
                "journal": "Cancer Cell",
                "date": "2025-01-01",
                "score": 42.0
            }],
            "count": 1,
            "total_pages": 1,
            "current": 1,
            "page_size": 25,
            "facets": {}
        })))
        .expect(1)
        .mount(&pubtator)
        .await;

    Mock::given(method("POST"))
        .and(path("/graph/v1/paper/batch"))
        .and(query_param(
            "fields",
            "paperId,externalIds,citationCount,influentialCitationCount,abstract",
        ))
        .and(body_string_contains("\"PMID:22663011\""))
        .and(|request: &wiremock::Request| !request.headers.contains_key("x-api-key"))
        .respond_with(ResponseTemplate::new(429).set_body_string("shared rate limit"))
        .expect(1)
        .mount(&s2)
        .await;

    let page = search_page(
        &ArticleSearchFilters {
            keyword: Some("alternative microexon splicing metastasis".into()),
            sort: ArticleSort::Date,
            ..empty_filters()
        },
        1,
        0,
        ArticleSourceFilter::PubTator,
    )
    .await
    .expect("search should survive Semantic Scholar batch failures");

    assert_eq!(page.results.len(), 1);
    assert_eq!(page.results[0].source, ArticleSource::PubTator);
    assert_eq!(
        page.results[0].matched_sources,
        vec![ArticleSource::PubTator]
    );
    assert_eq!(page.results[0].citation_count, None);
    assert_eq!(page.results[0].abstract_snippet, None);
}
