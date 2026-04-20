use super::super::fulltext::fulltext_cache_key;
#[allow(unused_imports)]
use super::super::test_support::*;
use super::*;
#[allow(unused_imports)]
use wiremock::matchers::{body_string_contains, header, method, path, query_param};
#[allow(unused_imports)]
use wiremock::{Mock, MockServer, ResponseTemplate};

#[test]
fn fulltext_cache_key_is_kind_aware_and_versioned() {
    let key = fulltext_cache_key(
        crate::entities::article::ArticleFulltextKind::JatsXml,
        "22663011",
    );
    assert_eq!(key, "article-fulltext-v3:jats_xml:22663011");
}

#[tokio::test]
async fn get_fulltext_prefers_europepmc_before_ncbi_efetch() {
    let _guard = lock_env().await;
    let pubtator = MockServer::start().await;
    let europepmc = MockServer::start().await;
    let efetch = MockServer::start().await;
    let pmc_oa = MockServer::start().await;
    let s2 = MockServer::start().await;
    let _pubtator_base = set_env_var("BIOMCP_PUBTATOR_BASE", Some(&pubtator.uri()));
    let _europepmc_base = set_env_var("BIOMCP_EUROPEPMC_BASE", Some(&europepmc.uri()));
    let _efetch_base = set_env_var("BIOMCP_PUBMED_BASE", Some(&efetch.uri()));
    let _pmc_oa_base = set_env_var("BIOMCP_PMC_OA_BASE", Some(&pmc_oa.uri()));
    let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&s2.uri()));
    let _s2_key = set_env_var("S2_API_KEY", None);

    Mock::given(method("GET"))
        .and(path("/publications/export/biocjson"))
        .and(query_param("pmids", "22663011"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "PubTator3": [{
                "pmid": 22663011,
                "pmcid": "PMC123456",
                "passages": [
                    {"infons": {"type": "title"}, "text": "Europe full text winner"},
                    {"infons": {"type": "abstract"}, "text": "Abstract text."}
                ]
            }]
        })))
        .expect(1)
        .mount(&pubtator)
        .await;

    Mock::given(method("GET"))
        .and(path("/search"))
        .and(query_param("query", "EXT_ID:22663011 AND SRC:MED"))
        .and(query_param("format", "json"))
        .and(query_param("page", "1"))
        .and(query_param("pageSize", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "hitCount": 1,
            "resultList": {
                "result": [{
                    "id": "22663011",
                    "pmid": "22663011",
                    "pmcid": "PMC123456",
                    "title": "Europe full text winner",
                    "journalTitle": "Journal One",
                    "firstPublicationDate": "2025-01-01"
                }]
            }
        })))
        .expect(1)
        .mount(&europepmc)
        .await;

    Mock::given(method("GET"))
        .and(path("/PMC123456/fullTextXML"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(sample_jats_article_xml(
                "Europe full text winner",
                "Europe PMC body text.",
            )),
        )
        .expect(1)
        .mount(&europepmc)
        .await;

    Mock::given(method("GET"))
        .and(path("/efetch.fcgi"))
        .and(query_param("db", "pmc"))
        .and(query_param("id", "123456"))
        .and(query_param("rettype", "xml"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(sample_pmc_articleset_xml(
                "efetch should not run",
                "efetch should not run.",
            )),
        )
        .expect(0)
        .mount(&efetch)
        .await;

    Mock::given(method("GET"))
        .and(path("/"))
        .and(query_param("id", "PMC123456"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&pmc_oa)
        .await;

    Mock::given(method("GET"))
        .and(path("/graph/v1/paper/PMID:22663011"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "paperId": "paper-1",
            "title": "Europe full text winner"
        })))
        .expect(1)
        .mount(&s2)
        .await;

    let article = get("22663011", &["fulltext".to_string()])
        .await
        .expect("fulltext request should succeed");

    assert!(article.full_text_note.is_none());
    assert_eq!(
        article.full_text_source,
        Some(crate::entities::article::ArticleFulltextSource {
            kind: crate::entities::article::ArticleFulltextKind::JatsXml,
            label: "Europe PMC XML".to_string(),
            source: "Europe PMC".to_string(),
        })
    );
    let path = article.full_text_path.expect("full text path");
    let metadata = std::fs::metadata(path).expect("saved full text metadata");
    assert!(metadata.len() > 0);
}

#[tokio::test]
async fn get_fulltext_falls_back_to_ncbi_efetch_before_pmc_oa() {
    let _guard = lock_env().await;
    let pubtator = MockServer::start().await;
    let europepmc = MockServer::start().await;
    let efetch = MockServer::start().await;
    let pmc_oa = MockServer::start().await;
    let s2 = MockServer::start().await;
    let _pubtator_base = set_env_var("BIOMCP_PUBTATOR_BASE", Some(&pubtator.uri()));
    let _europepmc_base = set_env_var("BIOMCP_EUROPEPMC_BASE", Some(&europepmc.uri()));
    let _efetch_base = set_env_var("BIOMCP_PUBMED_BASE", Some(&efetch.uri()));
    let _pmc_oa_base = set_env_var("BIOMCP_PMC_OA_BASE", Some(&pmc_oa.uri()));
    let _s2_base = set_env_var("BIOMCP_S2_BASE", Some(&s2.uri()));
    let _s2_key = set_env_var("S2_API_KEY", None);

    Mock::given(method("GET"))
        .and(path("/publications/export/biocjson"))
        .and(query_param("pmids", "22663012"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "PubTator3": [{
                "pmid": 22663012,
                "pmcid": "PMC123457",
                "passages": [
                    {"infons": {"type": "title"}, "text": "efetch fallback winner"},
                    {"infons": {"type": "abstract"}, "text": "Abstract text."}
                ]
            }]
        })))
        .expect(1)
        .mount(&pubtator)
        .await;

    Mock::given(method("GET"))
        .and(path("/search"))
        .and(query_param("query", "EXT_ID:22663012 AND SRC:MED"))
        .and(query_param("format", "json"))
        .and(query_param("page", "1"))
        .and(query_param("pageSize", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "hitCount": 1,
            "resultList": {
                "result": [{
                    "id": "22663012",
                    "pmid": "22663012",
                    "pmcid": "PMC123457",
                    "title": "efetch fallback winner",
                    "journalTitle": "Journal One",
                    "firstPublicationDate": "2025-01-01"
                }]
            }
        })))
        .expect(1)
        .mount(&europepmc)
        .await;

    Mock::given(method("GET"))
        .and(path("/PMC123457/fullTextXML"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&europepmc)
        .await;

    Mock::given(method("GET"))
        .and(path("/efetch.fcgi"))
        .and(query_param("db", "pmc"))
        .and(query_param("id", "123457"))
        .and(query_param("rettype", "xml"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(sample_pmc_articleset_xml(
                "efetch fallback winner",
                "NCBI efetch body text.",
            )),
        )
        .expect(1)
        .mount(&efetch)
        .await;

    Mock::given(method("GET"))
        .and(path("/"))
        .and(query_param("id", "PMC123457"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&pmc_oa)
        .await;

    Mock::given(method("GET"))
        .and(path("/graph/v1/paper/PMID:22663012"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "paperId": "paper-1",
            "title": "efetch fallback winner"
        })))
        .expect(1)
        .mount(&s2)
        .await;

    let article = get("22663012", &["fulltext".to_string()])
        .await
        .expect("fulltext request should succeed");

    assert!(article.full_text_note.is_none());
    assert_eq!(
        article.full_text_source,
        Some(crate::entities::article::ArticleFulltextSource {
            kind: crate::entities::article::ArticleFulltextKind::JatsXml,
            label: "NCBI EFetch PMC XML".to_string(),
            source: "NCBI EFetch".to_string(),
        })
    );
    let path = article.full_text_path.expect("full text path");
    let metadata = std::fs::metadata(path).expect("saved full text metadata");
    assert!(metadata.len() > 0);
}

#[test]
fn parse_sections_supports_tldr_and_all() {
    let tldr_only = parse_sections(&["tldr".to_string()]).expect("tldr should parse");
    assert!(tldr_only.include_tldr);
    assert!(!tldr_only.include_annotations);
    assert!(!tldr_only.include_fulltext);

    let all = parse_sections(&["all".to_string()]).expect("all should parse");
    assert!(all.include_tldr);
    assert!(all.include_annotations);
    assert!(all.include_fulltext);
}

#[test]
fn is_doi_basic() {
    assert!(is_doi("10.1056/NEJMoa1203421"));
    assert!(is_doi("10.1056/nejmoa1203421"));
    assert!(!is_doi("22663011"));
    assert!(!is_doi("doi:10.1056/NEJMoa1203421"));
}

#[test]
fn parse_pmid_basic() {
    assert_eq!(parse_pmid("22663011"), Some(22663011));
    assert_eq!(parse_pmid(" 22663011 "), Some(22663011));
    assert_eq!(parse_pmid(""), None);
    assert_eq!(parse_pmid("10.1056/NEJMoa1203421"), None);
    assert_eq!(parse_pmid("abc"), None);
}

#[test]
fn parse_pmcid_basic() {
    assert_eq!(parse_pmcid("PMC9984800"), Some("PMC9984800".into()));
    assert_eq!(parse_pmcid("pmc9984800"), Some("PMC9984800".into()));
    assert_eq!(parse_pmcid("PMCID:PMC9984800"), Some("PMC9984800".into()));
    assert_eq!(parse_pmcid(" PMC9984800 "), Some("PMC9984800".into()));
    assert_eq!(parse_pmcid("PMC"), None);
    assert_eq!(parse_pmcid("PMCX"), None);
    assert_eq!(parse_pmcid("PMC-123"), None);
    assert_eq!(parse_pmcid("22663011"), None);
}

#[test]
fn parse_article_id_basic() {
    match parse_article_id("PMC9984800") {
        ArticleIdType::Pmc(v) => assert_eq!(v, "PMC9984800"),
        _ => panic!("expected PMCID"),
    }
    match parse_article_id("10.1056/NEJMoa1203421") {
        ArticleIdType::Doi(v) => assert_eq!(v, "10.1056/NEJMoa1203421"),
        _ => panic!("expected DOI"),
    }
    match parse_article_id("22663011") {
        ArticleIdType::Pmid(v) => assert_eq!(v, 22663011),
        _ => panic!("expected PMID"),
    }
    assert!(matches!(
        parse_article_id("doi:10.1056/NEJMoa1203421"),
        ArticleIdType::Invalid
    ));
}

#[test]
fn parse_article_id_publisher_pii_is_invalid() {
    assert!(matches!(
        parse_article_id("S1535610826000103"),
        ArticleIdType::Invalid
    ));
}

#[test]
fn pubtator_lag_error_is_400_or_404_only() {
    let err_400 = BioMcpError::Api {
        api: "pubtator3".into(),
        message: "HTTP 400 Bad Request: pending".into(),
    };
    let err_404 = BioMcpError::Api {
        api: "pubtator3".into(),
        message: "HTTP 404 Not Found: pending".into(),
    };
    let err_500 = BioMcpError::Api {
        api: "pubtator3".into(),
        message: "HTTP 500 Internal Server Error".into(),
    };
    let other_api_400 = BioMcpError::Api {
        api: "europepmc".into(),
        message: "HTTP 400 Bad Request".into(),
    };

    assert!(is_pubtator_lag_error(&err_400));
    assert!(is_pubtator_lag_error(&err_404));
    assert!(!is_pubtator_lag_error(&err_500));
    assert!(!is_pubtator_lag_error(&other_api_400));
}
