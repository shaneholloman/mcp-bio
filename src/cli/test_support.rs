//! Shared CLI test helpers used by sidecar CLI test modules.

#[allow(unused_imports)]
pub(crate) use crate::test_support::{EnvVarGuard, TempDirGuard, set_env_var};
pub(crate) use wiremock::matchers::{method, path, query_param};
pub(crate) use wiremock::{Mock, MockServer, ResponseTemplate};

pub(crate) async fn lock_env() -> tokio::sync::MutexGuard<'static, ()> {
    crate::test_support::env_lock().lock().await
}

pub(crate) async fn mount_gene_lookup_miss(server: &MockServer, symbol: &str) {
    Mock::given(method("GET"))
        .and(path("/v3/query"))
        .and(query_param("q", format!("symbol:\"{symbol}\"")))
        .and(query_param("species", "human"))
        .and(query_param(
            "fields",
            "symbol,name,summary,alias,type_of_gene,ensembl.gene,entrezgene,genomic_pos.chr,genomic_pos.start,genomic_pos.end,genomic_pos.strand,MIM,uniprot,pathway.kegg",
        ))
        .and(query_param("size", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"{"total":0,"hits":[]}"#,
            "application/json",
        ))
        .expect(1)
        .mount(server)
        .await;
}

pub(crate) async fn mount_gene_lookup_hit(
    server: &MockServer,
    symbol: &str,
    name: &str,
    entrez: &str,
) {
    Mock::given(method("GET"))
        .and(path("/v3/query"))
        .and(query_param("q", format!("symbol:\"{symbol}\"")))
        .and(query_param("species", "human"))
        .and(query_param(
            "fields",
            "symbol,name,summary,alias,type_of_gene,ensembl.gene,entrezgene,genomic_pos.chr,genomic_pos.start,genomic_pos.end,genomic_pos.strand,MIM,uniprot,pathway.kegg",
        ))
        .and(query_param("size", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            format!(
                r#"{{
                    "total": 1,
                    "hits": [{{
                        "symbol": "{symbol}",
                        "name": "{name}",
                        "entrezgene": "{entrez}"
                    }}]
                }}"#
            ),
            "application/json",
        ))
        .expect(1)
        .mount(server)
        .await;
}

pub(crate) async fn mount_drug_lookup_miss(server: &MockServer, query: &str) {
    Mock::given(method("GET"))
        .and(path("/v1/query"))
        .and(query_param("q", query))
        .and(query_param("size", "25"))
        .and(query_param("from", "0"))
        .and(query_param(
            "fields",
            crate::sources::mychem::MYCHEM_FIELDS_GET,
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(r#"{"total":0,"hits":[]}"#, "application/json"),
        )
        .expect(1)
        .mount(server)
        .await;
}

pub(crate) async fn mount_ols_alias(
    server: &MockServer,
    query: &str,
    ontology_prefix: &str,
    obo_id: &str,
    label: &str,
    synonyms: &[&str],
    expected_calls: u64,
) {
    Mock::given(method("GET"))
        .and(path("/api/search"))
        .and(query_param("q", query))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "response": {
                "docs": [{
                    "iri": format!("http://example.org/{ontology_prefix}/{}", obo_id.replace(':', "_")),
                    "ontology_name": ontology_prefix,
                    "ontology_prefix": ontology_prefix,
                    "short_form": obo_id.to_ascii_lowercase(),
                    "obo_id": obo_id,
                    "label": label,
                    "description": [],
                    "exact_synonyms": synonyms,
                    "type": "class"
                }]
            }
        })))
        .expect(expected_calls)
        .mount(server)
        .await;
}
