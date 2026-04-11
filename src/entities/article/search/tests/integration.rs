use super::*;

#[tokio::test]
async fn source_specific_pubtator_search_uses_default_retraction_filter() {
    let _guard = lock_env().await;
    let server = MockServer::start().await;
    let _pubtator_base = set_env_var("BIOMCP_PUBTATOR_BASE", Some(&server.uri()));

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
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(query_param("page", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "results": [],
            "count": 1,
            "total_pages": 1,
            "current": 2,
            "page_size": 25,
            "facets": {}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let page = search_page(
        &ArticleSearchFilters {
            keyword: Some("alternative microexon splicing metastasis".into()),
            exclude_retracted: true,
            ..empty_filters()
        },
        3,
        0,
        ArticleSourceFilter::PubTator,
    )
    .await
    .expect("pubtator search should succeed");

    assert_eq!(page.results.len(), 1);
    assert_eq!(page.results[0].source, ArticleSource::PubTator);
    assert_eq!(page.results[0].pmid, "22663011");
}

#[tokio::test]
async fn federated_search_keeps_non_europepmc_matches_under_default_retraction_filter() {
    let _guard = lock_env().await;
    let pubtator = MockServer::start().await;
    let europepmc = MockServer::start().await;
    let s2 = MockServer::start().await;
    let _pubtator_base = set_env_var("BIOMCP_PUBTATOR_BASE", Some(&pubtator.uri()));
    let _europepmc_base = set_env_var("BIOMCP_EUROPEPMC_BASE", Some(&europepmc.uri()));
    let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&s2.uri()));
    let _s2_key = set_env_var("S2_API_KEY", None);

    // S2 is now enabled without a key; return empty results so S2 doesn't
    // interfere with the PubTator/EuropePMC assertion below.
    Mock::given(method("GET"))
        .and(path("/graph/v1/paper/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total": 0,
            "data": []
        })))
        .mount(&s2)
        .await;

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

    Mock::given(method("GET"))
        .and(query_param("page", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "results": [],
            "count": 1,
            "total_pages": 1,
            "current": 2,
            "page_size": 25,
            "facets": {}
        })))
        .expect(1)
        .mount(&pubtator)
        .await;

    Mock::given(method("GET"))
        .and(path("/search"))
        .and(query_param(
            "query",
            "alternative microexon splicing metastasis AND NOT PUB_TYPE:\"retracted publication\"",
        ))
        .and(query_param("format", "json"))
        .and(query_param("page", "1"))
        .and(query_param("pageSize", "25"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "hitCount": 1,
            "resultList": {
                "result": [{
                    "id": "EP-1",
                    "pmid": "22663012",
                    "title": "Europe PMC match",
                    "journalTitle": "Nature",
                    "firstPublicationDate": "2024-01-01",
                    "citedByCount": 25,
                    "pubType": "journal article"
                }]
            }
        })))
        .expect(1)
        .mount(&europepmc)
        .await;

    Mock::given(method("GET"))
        .and(path("/search"))
        .and(query_param("page", "2"))
        .and(query_param("format", "json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "hitCount": 1,
            "resultList": {
                "result": []
            }
        })))
        .expect(1)
        .mount(&europepmc)
        .await;

    let page = search_page(
        &ArticleSearchFilters {
            keyword: Some("alternative microexon splicing metastasis".into()),
            exclude_retracted: true,
            ..empty_filters()
        },
        5,
        0,
        ArticleSourceFilter::All,
    )
    .await
    .expect("federated search should succeed");

    assert!(!page.results.is_empty());
    assert!(page.results.iter().any(|row| {
        row.source == ArticleSource::PubTator
            || row.matched_sources.contains(&ArticleSource::PubTator)
    }));
}

#[tokio::test]
async fn federated_search_keyword_includes_litsense2_matches() {
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
        .and(path("/graph/v1/paper/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total": 0,
            "data": []
        })))
        .mount(&s2)
        .await;

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
        .and(query_param("page", "1"))
        .and(query_param("format", "json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "hitCount": 0,
            "resultList": {
                "result": []
            }
        })))
        .expect(1)
        .mount(&europepmc)
        .await;

    Mock::given(method("GET"))
        .and(path("/esearch.fcgi"))
        .and(query_param("db", "pubmed"))
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
        .and(path("/sentences/"))
        .and(query_param("query", "Hirschsprung disease"))
        .and(query_param("rerank", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "pmid": 22663011,
                "pmcid": "PMC9984800",
                "text": "Hirschsprung disease semantic hit",
                "score": 0.8,
                "section": "INTRO",
                "annotations": []
            }
        ])))
        .expect(1)
        .mount(&litsense2)
        .await;

    Mock::given(method("GET"))
        .and(path("/esummary.fcgi"))
        .and(query_param("db", "pubmed"))
        .and(query_param("retmode", "json"))
        .and(query_param("id", "22663011"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "result": {
                "uids": ["22663011"],
                "22663011": {
                    "uid": "22663011",
                    "title": "Hydrated LitSense2 federated title",
                    "sortpubdate": "2024/01/15 00:00",
                    "pubdate": "2024 Jan 15",
                    "fulljournalname": "Journal One",
                    "source": "J1"
                }
            }
        })))
        .expect(1)
        .mount(&pubmed)
        .await;

    let page = search_page(
        &ArticleSearchFilters {
            keyword: Some("Hirschsprung disease".into()),
            exclude_retracted: true,
            ..empty_filters()
        },
        5,
        0,
        ArticleSourceFilter::All,
    )
    .await
    .expect("federated search should succeed");

    assert_eq!(page.results.len(), 1);
    assert_eq!(page.results[0].source, ArticleSource::LitSense2);
    assert_eq!(
        page.results[0].matched_sources,
        vec![ArticleSource::LitSense2]
    );
    assert_eq!(page.results[0].score, Some(0.8));
}

#[tokio::test]
async fn federated_search_gene_only_skips_litsense2() {
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
        .and(path("/graph/v1/paper/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total": 0,
            "data": []
        })))
        .mount(&s2)
        .await;

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
        .and(query_param("page", "1"))
        .and(query_param("format", "json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "hitCount": 0,
            "resultList": {
                "result": []
            }
        })))
        .expect(1)
        .mount(&europepmc)
        .await;

    Mock::given(method("GET"))
        .and(path("/esearch.fcgi"))
        .and(query_param("db", "pubmed"))
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
        .and(path("/sentences/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&litsense2)
        .await;

    let page = search_page(
        &ArticleSearchFilters {
            gene: Some("BRAF".into()),
            exclude_retracted: true,
            ..empty_filters()
        },
        5,
        0,
        ArticleSourceFilter::All,
    )
    .await
    .expect("federated search should succeed");

    assert!(page.results.is_empty());
}

#[tokio::test]
async fn federated_search_includes_pubmed_rows_in_matched_sources() {
    let _guard = lock_env().await;
    let pubtator = MockServer::start().await;
    let europepmc = MockServer::start().await;
    let pubmed = MockServer::start().await;
    let s2 = MockServer::start().await;
    let _pubtator_base = set_env_var("BIOMCP_PUBTATOR_BASE", Some(&pubtator.uri()));
    let _europepmc_base = set_env_var("BIOMCP_EUROPEPMC_BASE", Some(&europepmc.uri()));
    let _pubmed_base = set_env_var("BIOMCP_PUBMED_BASE", Some(&pubmed.uri()));
    let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&s2.uri()));
    let _s2_key = set_env_var("S2_API_KEY", None);

    Mock::given(method("GET"))
        .and(path("/graph/v1/paper/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total": 0,
            "data": []
        })))
        .mount(&s2)
        .await;

    Mock::given(method("GET"))
        .and(query_param("page", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "results": [{
                "_id": "pt-1",
                "pmid": 22663011,
                "title": "PubTator match",
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

    Mock::given(method("GET"))
        .and(query_param("page", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "results": [],
            "count": 1,
            "total_pages": 1,
            "current": 2,
            "page_size": 25,
            "facets": {}
        })))
        .expect(1)
        .mount(&pubtator)
        .await;

    Mock::given(method("GET"))
        .and(path("/search"))
        .and(query_param(
            "query",
            "alternative microexon splicing metastasis AND NOT PUB_TYPE:\"retracted publication\"",
        ))
        .and(query_param("format", "json"))
        .and(query_param("page", "1"))
        .and(query_param("pageSize", "25"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "hitCount": 1,
            "resultList": {
                "result": [{
                    "id": "EP-1",
                    "pmid": "22663012",
                    "title": "Europe PMC match",
                    "journalTitle": "Nature",
                    "firstPublicationDate": "2024-01-01",
                    "citedByCount": 25,
                    "pubType": "journal article"
                }]
            }
        })))
        .expect(1)
        .mount(&europepmc)
        .await;

    Mock::given(method("GET"))
        .and(path("/search"))
        .and(query_param("page", "2"))
        .and(query_param("format", "json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "hitCount": 1,
            "resultList": {
                "result": []
            }
        })))
        .expect(1)
        .mount(&europepmc)
        .await;

    Mock::given(method("GET"))
        .and(path("/esearch.fcgi"))
        .and(query_param("db", "pubmed"))
        .and(query_param("retmode", "json"))
        .and(query_param("retstart", "0"))
        .and(query_param("retmax", "100"))
        .and(query_param(
            "term",
            "alternative microexon splicing metastasis NOT retracted publication[pt]",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "esearchresult": {
                "count": "1",
                "idlist": ["22663013"]
            }
        })))
        .expect(1)
        .mount(&pubmed)
        .await;

    Mock::given(method("GET"))
        .and(path("/esummary.fcgi"))
        .and(query_param("db", "pubmed"))
        .and(query_param("retmode", "json"))
        .and(query_param("id", "22663013"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "result": {
                "uids": ["22663013"],
                "22663013": {
                    "uid": "22663013",
                    "title": "PubMed visible match",
                    "sortpubdate": "2025/03/01 00:00",
                    "fulljournalname": "Science",
                    "source": "Science"
                }
            }
        })))
        .expect(1)
        .mount(&pubmed)
        .await;

    Mock::given(method("GET"))
        .and(path("/esearch.fcgi"))
        .and(query_param("db", "pubmed"))
        .and(query_param("retmode", "json"))
        .and(query_param("retstart", "1"))
        .and(query_param("retmax", "100"))
        .and(query_param(
            "term",
            "alternative microexon splicing metastasis NOT retracted publication[pt]",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "esearchresult": {
                "count": "1",
                "idlist": []
            }
        })))
        .expect(1)
        .mount(&pubmed)
        .await;

    let page = search_page(
        &ArticleSearchFilters {
            keyword: Some("alternative microexon splicing metastasis".into()),
            exclude_retracted: true,
            ..empty_filters()
        },
        5,
        0,
        ArticleSourceFilter::All,
    )
    .await
    .expect("federated search should succeed");

    assert!(!page.results.is_empty());
    assert!(page.results.iter().any(|row| {
        row.source == ArticleSource::PubMed || row.matched_sources.contains(&ArticleSource::PubMed)
    }));
}
