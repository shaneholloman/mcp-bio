use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use bytesize::ByteSize;
use futures::future::join_all;

use crate::error::BioMcpError;

#[derive(Debug, Clone, serde::Serialize)]
pub struct HealthRow {
    pub api: String,
    pub status: String,
    pub latency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affects: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_configured: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct HealthReport {
    pub healthy: usize,
    pub warning: usize,
    pub excluded: usize,
    pub total: usize,
    pub rows: Vec<HealthRow>,
}

impl HealthReport {
    pub fn all_healthy(&self) -> bool {
        self.healthy + self.warning + self.excluded == self.total
    }

    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        let show_affects = self.rows.iter().any(|row| row.affects.is_some());
        let errors = self
            .total
            .saturating_sub(self.healthy + self.warning + self.excluded);

        out.push_str("# BioMCP Health Check\n\n");
        if show_affects {
            out.push_str("| API | Status | Latency | Affects |\n");
            out.push_str("|-----|--------|---------|---------|\n");
            for row in &self.rows {
                let affects = row.affects.as_deref().unwrap_or("-");
                let status = markdown_status(row);
                out.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    row.api, status, row.latency, affects
                ));
            }
        } else {
            out.push_str("| API | Status | Latency |\n");
            out.push_str("|-----|--------|---------|\n");
            for row in &self.rows {
                let status = markdown_status(row);
                out.push_str(&format!("| {} | {} | {} |\n", row.api, status, row.latency));
            }
        }

        out.push_str(&format!(
            "\nStatus: {} ok, {} error, {} excluded",
            self.healthy, errors, self.excluded
        ));
        if self.warning > 0 {
            out.push_str(&format!(", {} warning", self.warning));
        }
        out.push('\n');
        out
    }
}

fn markdown_status(row: &HealthRow) -> String {
    match (row.status.as_str(), row.key_configured) {
        ("ok", Some(true)) => "ok (key configured)".to_string(),
        ("error", Some(true)) => "error (key configured)".to_string(),
        ("error", Some(false)) => "error (key not configured)".to_string(),
        _ => row.status.clone(),
    }
}

#[derive(Debug, Clone, Copy)]
struct SourceDescriptor {
    api: &'static str,
    affects: Option<&'static str>,
    probe: ProbeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProbeClass {
    Healthy,
    Warning,
    Error,
    Excluded,
}

#[derive(Debug, Clone)]
struct ProbeOutcome {
    row: HealthRow,
    class: ProbeClass,
}

#[derive(Debug, Clone, Copy)]
enum ProbeKind {
    Get {
        url: &'static str,
    },
    PostJson {
        url: &'static str,
        payload: &'static str,
    },
    AuthGet {
        url: &'static str,
        env_var: &'static str,
        header_name: &'static str,
        header_value_prefix: &'static str,
    },
    OptionalAuthGet {
        url: &'static str,
        env_var: &'static str,
        header_name: &'static str,
        header_value_prefix: &'static str,
        unauthenticated_ok_status: &'static str,
        authenticated_ok_status: &'static str,
        unauthenticated_rate_limited_status: Option<&'static str>,
    },
    AuthQueryParam {
        url: &'static str,
        env_var: &'static str,
        param_name: &'static str,
    },
    #[allow(dead_code)]
    AuthPostJson {
        url: &'static str,
        payload: &'static str,
        env_var: &'static str,
        header_name: &'static str,
        header_value_prefix: &'static str,
    },
    AlphaGenomeConnect {
        env_var: &'static str,
    },
}

const HEALTH_SOURCES: &[SourceDescriptor] = &[
    SourceDescriptor {
        api: "MyGene",
        affects: Some("get/search gene and gene helper commands"),
        probe: ProbeKind::Get {
            url: "https://mygene.info/v3/query?q=BRAF&size=1",
        },
    },
    SourceDescriptor {
        api: "MyVariant",
        affects: Some("get/search variant and variant helper commands"),
        probe: ProbeKind::Get {
            url: "https://myvariant.info/v1/query?q=rs113488022&size=1",
        },
    },
    SourceDescriptor {
        api: "MyChem",
        affects: Some("get/search drug and drug helper commands"),
        probe: ProbeKind::Get {
            url: "https://mychem.info/v1/query?q=aspirin&size=1",
        },
    },
    SourceDescriptor {
        api: "PubTator3",
        affects: Some("article annotations and entity extraction"),
        probe: ProbeKind::Get {
            url: "https://www.ncbi.nlm.nih.gov/research/pubtator3-api/publications/export/biocjson?pmids=22663011",
        },
    },
    SourceDescriptor {
        api: "PubMed",
        affects: Some("PubMed-backed article search foundation"),
        probe: ProbeKind::Get {
            url: "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esearch.fcgi?db=pubmed&retmode=json&retmax=1&term=BRAF",
        },
    },
    SourceDescriptor {
        api: "Europe PMC",
        affects: Some("article search coverage"),
        probe: ProbeKind::Get {
            url: "https://www.ebi.ac.uk/europepmc/webservices/rest/search?query=BRAF&format=json&pageSize=1",
        },
    },
    SourceDescriptor {
        api: "LitSense2",
        affects: Some("keyword-gated semantic article search"),
        probe: ProbeKind::Get {
            url: "https://www.ncbi.nlm.nih.gov/research/litsense2-api/api/sentences/?query=test&rerank=true",
        },
    },
    SourceDescriptor {
        api: "PMC OA",
        affects: Some("article fulltext resolution"),
        probe: ProbeKind::Get {
            url: "https://www.ncbi.nlm.nih.gov/pmc/utils/oa/oa.fcgi?id=PMC9984800",
        },
    },
    SourceDescriptor {
        api: "NCBI ID Converter",
        affects: Some("article fulltext resolution and identifier bridging"),
        probe: ProbeKind::Get {
            url: "https://pmc.ncbi.nlm.nih.gov/tools/idconv/api/v1/articles/?format=json&idtype=pmid&ids=22663011",
        },
    },
    SourceDescriptor {
        api: "ClinicalTrials.gov",
        affects: Some("search/get trial and trial helper commands"),
        probe: ProbeKind::Get {
            url: "https://clinicaltrials.gov/api/v2/studies?query.term=cancer&pageSize=1",
        },
    },
    SourceDescriptor {
        api: "NCI CTS",
        affects: Some("trial --source nci"),
        probe: ProbeKind::AuthGet {
            url: "https://clinicaltrialsapi.cancer.gov/api/v2/trials?size=1&keyword=melanoma",
            env_var: "NCI_API_KEY",
            header_name: "X-API-KEY",
            header_value_prefix: "",
        },
    },
    SourceDescriptor {
        api: "Enrichr",
        affects: Some("gene/pathway enrichment sections"),
        probe: ProbeKind::Get {
            url: "https://maayanlab.cloud/Enrichr/datasetStatistics",
        },
    },
    SourceDescriptor {
        api: "OpenFDA",
        affects: Some("adverse-event search"),
        probe: ProbeKind::Get {
            url: "https://api.fda.gov/drug/event.json?limit=1",
        },
    },
    SourceDescriptor {
        api: "OncoKB",
        affects: Some("variant oncokb command and variant evidence section"),
        probe: ProbeKind::AuthGet {
            url: "https://www.oncokb.org/api/v1/annotate/mutations/byProteinChange?hugoSymbol=BRAF&alteration=V600E",
            env_var: "ONCOKB_TOKEN",
            header_name: "Authorization",
            header_value_prefix: "Bearer ",
        },
    },
    SourceDescriptor {
        api: "DisGeNET",
        affects: Some("gene and disease disgenet sections"),
        probe: ProbeKind::AuthGet {
            url: "https://api.disgenet.com/api/v1/gda/summary?gene_ncbi_id=7157&page_number=0",
            env_var: "DISGENET_API_KEY",
            header_name: "Authorization",
            header_value_prefix: "",
        },
    },
    SourceDescriptor {
        api: "AlphaGenome",
        affects: Some("variant predict section"),
        probe: ProbeKind::AlphaGenomeConnect {
            env_var: "ALPHAGENOME_API_KEY",
        },
    },
    SourceDescriptor {
        api: "Semantic Scholar",
        affects: Some("Semantic Scholar features"),
        probe: ProbeKind::OptionalAuthGet {
            url: "https://api.semanticscholar.org/graph/v1/paper/search?query=BRAF&fields=paperId,title&limit=1",
            env_var: "S2_API_KEY",
            header_name: "x-api-key",
            header_value_prefix: "",
            unauthenticated_ok_status: "available (unauthenticated, shared rate limit)",
            authenticated_ok_status: "configured (authenticated)",
            unauthenticated_rate_limited_status: Some(
                "unavailable (set S2_API_KEY for reliable access)",
            ),
        },
    },
    SourceDescriptor {
        api: "CPIC",
        affects: Some("pgx recommendations and annotations"),
        probe: ProbeKind::Get {
            url: "https://api.cpicpgx.org/v1/pair_view?select=pairid&limit=1",
        },
    },
    SourceDescriptor {
        api: "PharmGKB",
        affects: Some("pgx recommendations and annotations"),
        probe: ProbeKind::Get {
            url: "https://api.pharmgkb.org/v1/data/labelAnnotation?relatedChemicals.name=warfarin&view=min",
        },
    },
    SourceDescriptor {
        api: "Monarch",
        affects: Some("disease genes, phenotypes, and models"),
        probe: ProbeKind::Get {
            url: "https://api-v3.monarchinitiative.org/v3/api/association?object=MONDO:0007739&subject_category=biolink:Gene&limit=1",
        },
    },
    SourceDescriptor {
        api: "HPO",
        affects: Some("phenotype search and disease ranking"),
        probe: ProbeKind::Get {
            url: "https://ontology.jax.org/api/hp/terms/HP:0001250",
        },
    },
    SourceDescriptor {
        api: "MyDisease",
        affects: Some("disease search and normalization"),
        probe: ProbeKind::Get {
            url: "https://mydisease.info/v1/query?q=melanoma&size=1&fields=disease_ontology.name,mondo.label",
        },
    },
    SourceDescriptor {
        api: "CIViC",
        affects: Some("disease genes and variants sections"),
        probe: ProbeKind::PostJson {
            url: "https://civicdb.org/api/graphql",
            payload: r#"{"query":"query { evidenceItems(first: 1) { totalCount } }"}"#,
        },
    },
    SourceDescriptor {
        api: "GWAS Catalog",
        affects: Some("gwas search and variant gwas context"),
        probe: ProbeKind::Get {
            url: "https://www.ebi.ac.uk/gwas/rest/api/singleNucleotidePolymorphisms/rs7903146",
        },
    },
    SourceDescriptor {
        api: "GTEx",
        affects: Some("gene expression section"),
        probe: ProbeKind::Get {
            url: "https://gtexportal.org/api/v2/",
        },
    },
    SourceDescriptor {
        api: "DGIdb",
        affects: Some("gene druggability section"),
        probe: ProbeKind::PostJson {
            url: "https://dgidb.org/api/graphql",
            payload: r#"{"query":"query { __typename }"}"#,
        },
    },
    SourceDescriptor {
        api: "ClinGen",
        affects: Some("gene clingen section"),
        probe: ProbeKind::Get {
            url: "https://search.clinicalgenome.org/api/genes/look/BRAF",
        },
    },
    SourceDescriptor {
        api: "gnomAD",
        affects: Some("gene constraint section"),
        probe: ProbeKind::PostJson {
            url: "https://gnomad.broadinstitute.org/api",
            payload: r#"{"query":"query { __typename }"}"#,
        },
    },
    SourceDescriptor {
        api: "UniProt",
        affects: Some("gene protein summary and protein detail sections"),
        probe: ProbeKind::Get {
            url: "https://rest.uniprot.org/uniprotkb/P15056.json",
        },
    },
    SourceDescriptor {
        api: "QuickGO",
        affects: Some("gene go terms and protein annotation sections"),
        probe: ProbeKind::Get {
            url: "https://www.ebi.ac.uk/QuickGO/services/annotation/search?geneProductId=P15056&limit=5",
        },
    },
    SourceDescriptor {
        api: "STRING",
        affects: Some("gene interactions and protein interaction sections"),
        probe: ProbeKind::Get {
            url: "https://string-db.org/api/json/network?identifiers=BRAF&species=9606&limit=5",
        },
    },
    SourceDescriptor {
        api: "Reactome",
        affects: Some("pathway search and disease pathway sections"),
        probe: ProbeKind::Get {
            url: "https://reactome.org/ContentService/search/query?query=MAPK&species=Homo%20sapiens&pageSize=1",
        },
    },
    SourceDescriptor {
        api: "KEGG",
        affects: Some("pathway search and detail sections"),
        probe: ProbeKind::Get {
            url: "https://rest.kegg.jp/find/pathway/MAPK",
        },
    },
    SourceDescriptor {
        api: "WikiPathways",
        affects: Some("pathway search and WikiPathways detail/genes sections"),
        probe: ProbeKind::Get {
            url: "https://www.wikipathways.org/json/findPathwaysByText.json",
        },
    },
    SourceDescriptor {
        api: "g:Profiler",
        affects: Some("gene enrichment (biomcp enrich)"),
        probe: ProbeKind::PostJson {
            url: "https://biit.cs.ut.ee/gprofiler/api/gost/profile/",
            payload: r#"{"organism":"hsapiens","query":["BRAF"]}"#,
        },
    },
    SourceDescriptor {
        api: "OpenTargets",
        affects: Some("gene druggability, drug target, and disease association sections"),
        probe: ProbeKind::PostJson {
            url: "https://api.platform.opentargets.org/api/v4/graphql",
            payload: r#"{"query":"query { drug(chemblId: \"CHEMBL25\") { id name } }"}"#,
        },
    },
    SourceDescriptor {
        api: "ChEMBL",
        affects: Some("drug targets and indications sections"),
        probe: ProbeKind::Get {
            url: "https://www.ebi.ac.uk/chembl/api/data/molecule/CHEMBL25.json",
        },
    },
    SourceDescriptor {
        api: "HPA",
        affects: Some("gene protein tissue expression and localization section"),
        probe: ProbeKind::Get {
            url: "https://www.proteinatlas.org/ENSG00000157764.xml",
        },
    },
    SourceDescriptor {
        api: "InterPro",
        affects: Some("protein domains section"),
        probe: ProbeKind::Get {
            url: "https://www.ebi.ac.uk/interpro/api/entry/interpro/protein/uniprot/P15056/?page_size=5",
        },
    },
    SourceDescriptor {
        api: "ComplexPortal",
        affects: Some("protein complex membership section"),
        probe: ProbeKind::Get {
            url: "https://www.ebi.ac.uk/intact/complex-ws/search/P15056?number=25&filters=species_f:(%22Homo%20sapiens%22)",
        },
    },
    SourceDescriptor {
        api: "OLS4",
        affects: Some("discover command concept resolution"),
        probe: ProbeKind::Get {
            url: "https://www.ebi.ac.uk/ols4/api/search?q=BRCA1&rows=1&groupField=iri&ontology=hgnc",
        },
    },
    SourceDescriptor {
        api: "UMLS",
        affects: Some("discover command clinical crosswalk enrichment"),
        probe: ProbeKind::AuthQueryParam {
            url: "https://uts-ws.nlm.nih.gov/rest/search/current?string=BRCA1&pageSize=1",
            env_var: "UMLS_API_KEY",
            param_name: "apiKey",
        },
    },
    SourceDescriptor {
        api: "MedlinePlus",
        affects: Some("discover command plain-language disease and symptom context"),
        probe: ProbeKind::Get {
            url: "https://wsearch.nlm.nih.gov/ws/query?db=healthTopics&term=chest+pain&retmax=1",
        },
    },
    SourceDescriptor {
        api: "cBioPortal",
        affects: Some("cohort frequency section"),
        probe: ProbeKind::Get {
            url: "https://www.cbioportal.org/api/studies?projection=SUMMARY&pageSize=1",
        },
    },
];

const EMA_LOCAL_DATA_AFFECTS: &str =
    "search/get drug --region eu|all and EU regulatory/safety/shortage sections";

fn health_sources() -> &'static [SourceDescriptor] {
    HEALTH_SOURCES
}

#[cfg_attr(not(test), allow(dead_code))]
fn affects_for_api(api: &str) -> Option<&'static str> {
    health_sources()
        .iter()
        .find(|source| source.api == api)
        .and_then(|source| source.affects)
}

fn health_row(
    api: &str,
    status: String,
    latency: String,
    affects: Option<&'static str>,
    key_configured: Option<bool>,
) -> HealthRow {
    HealthRow {
        api: api.to_string(),
        status,
        latency,
        affects: affects.map(str::to_string),
        key_configured,
    }
}

fn outcome(row: HealthRow, class: ProbeClass) -> ProbeOutcome {
    ProbeOutcome { row, class }
}

fn configured_key(env_var: &str) -> Option<String> {
    std::env::var(env_var)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn excluded_outcome(api: &str, env_var: &str, affects: Option<&'static str>) -> ProbeOutcome {
    outcome(
        health_row(
            api,
            format!("excluded (set {env_var})"),
            "n/a".into(),
            affects,
            Some(false),
        ),
        ProbeClass::Excluded,
    )
}

fn transport_error_latency(start: Instant, err: &reqwest::Error) -> String {
    let elapsed = start.elapsed().as_millis();
    if err.is_timeout() {
        format!("{elapsed}ms (timeout)")
    } else if err.is_connect() {
        format!("{elapsed}ms (connect)")
    } else {
        format!("{elapsed}ms (error)")
    }
}

fn api_error_latency(start: Instant, err: &BioMcpError) -> String {
    let elapsed = start.elapsed().as_millis();
    match err {
        BioMcpError::Api { message, .. } if message.contains("connect failed") => {
            format!("{elapsed}ms (connect)")
        }
        _ => format!("{elapsed}ms (error)"),
    }
}

async fn send_request(
    api: &str,
    affects: Option<&'static str>,
    request: reqwest::RequestBuilder,
    key_configured: Option<bool>,
) -> ProbeOutcome {
    let start = Instant::now();
    let response = request.send().await;

    match response {
        Ok(response) => {
            let status = response.status();
            let elapsed = start.elapsed().as_millis();
            if status.is_success() {
                outcome(
                    health_row(
                        api,
                        "ok".into(),
                        format!("{elapsed}ms"),
                        None,
                        key_configured,
                    ),
                    ProbeClass::Healthy,
                )
            } else {
                outcome(
                    health_row(
                        api,
                        "error".into(),
                        format!("{elapsed}ms (HTTP {})", status.as_u16()),
                        affects,
                        key_configured,
                    ),
                    ProbeClass::Error,
                )
            }
        }
        Err(err) => outcome(
            health_row(
                api,
                "error".into(),
                transport_error_latency(start, &err),
                affects,
                key_configured,
            ),
            ProbeClass::Error,
        ),
    }
}

async fn check_get(
    client: reqwest::Client,
    api: &str,
    url: &str,
    affects: Option<&'static str>,
) -> ProbeOutcome {
    send_request(api, affects, client.get(url), None).await
}

async fn check_post_json(
    client: reqwest::Client,
    api: &str,
    url: &str,
    payload: &str,
    affects: Option<&'static str>,
) -> ProbeOutcome {
    send_request(
        api,
        affects,
        client
            .post(url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(payload.to_string()),
        None,
    )
    .await
}

async fn check_auth_get(
    client: reqwest::Client,
    api: &str,
    url: &str,
    env_var: &str,
    header_name: &str,
    header_value_prefix: &str,
    affects: Option<&'static str>,
) -> ProbeOutcome {
    let Some(key) = configured_key(env_var) else {
        return excluded_outcome(api, env_var, affects);
    };

    let header_value = format!("{header_value_prefix}{key}");

    send_request(
        api,
        affects,
        client.get(url).header(header_name, header_value),
        Some(true),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn check_optional_auth_get(
    client: reqwest::Client,
    api: &str,
    url: &str,
    env_var: &str,
    header_name: &str,
    header_value_prefix: &str,
    unauthenticated_ok_status: &str,
    authenticated_ok_status: &str,
    unauthenticated_rate_limited_status: Option<&str>,
    affects: Option<&'static str>,
) -> ProbeOutcome {
    let key = configured_key(env_var);
    let key_configured = Some(key.is_some());
    let request = match key {
        Some(key) => client
            .get(url)
            .header(header_name, format!("{header_value_prefix}{key}")),
        None => client.get(url),
    };
    let success_status = if key_configured == Some(true) {
        authenticated_ok_status
    } else {
        unauthenticated_ok_status
    };
    let start = Instant::now();
    let error_outcome = |latency: String| {
        outcome(
            health_row(api, "error".into(), latency, affects, key_configured),
            ProbeClass::Error,
        )
    };

    match request.send().await {
        Ok(response) => {
            let status = response.status();
            let elapsed = start.elapsed().as_millis();
            if status.is_success() {
                outcome(
                    health_row(
                        api,
                        success_status.to_string(),
                        format!("{elapsed}ms"),
                        None,
                        key_configured,
                    ),
                    ProbeClass::Healthy,
                )
            } else if key_configured == Some(false)
                && status == reqwest::StatusCode::TOO_MANY_REQUESTS
                && let Some(status_message) = unauthenticated_rate_limited_status
            {
                outcome(
                    health_row(
                        api,
                        status_message.to_string(),
                        format!("{elapsed}ms"),
                        None,
                        key_configured,
                    ),
                    ProbeClass::Healthy,
                )
            } else {
                error_outcome(format!("{elapsed}ms (HTTP {})", status.as_u16()))
            }
        }
        Err(err) => error_outcome(transport_error_latency(start, &err)),
    }
}

async fn check_auth_query_param(
    client: reqwest::Client,
    api: &str,
    url: &str,
    env_var: &str,
    param_name: &str,
    affects: Option<&'static str>,
) -> ProbeOutcome {
    let Some(key) = configured_key(env_var) else {
        return excluded_outcome(api, env_var, affects);
    };

    let req = match reqwest::Url::parse(url) {
        Ok(mut parsed) => {
            parsed.query_pairs_mut().append_pair(param_name, &key);
            client.get(parsed)
        }
        Err(err) => {
            return outcome(
                health_row(
                    api,
                    "error".into(),
                    format!("invalid url: {err}"),
                    affects,
                    Some(true),
                ),
                ProbeClass::Error,
            );
        }
    };

    send_request(api, affects, req, Some(true)).await
}

#[allow(clippy::too_many_arguments)]
async fn check_auth_post_json(
    client: reqwest::Client,
    api: &str,
    url: &str,
    payload: &str,
    env_var: &str,
    header_name: &str,
    header_value_prefix: &str,
    affects: Option<&'static str>,
) -> ProbeOutcome {
    let Some(key) = configured_key(env_var) else {
        return excluded_outcome(api, env_var, affects);
    };

    let header_value = format!("{header_value_prefix}{key}");

    send_request(
        api,
        affects,
        client
            .post(url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(header_name, header_value)
            .body(payload.to_string()),
        Some(true),
    )
    .await
}

async fn check_alphagenome_connect(
    api: &str,
    env_var: &str,
    affects: Option<&'static str>,
) -> ProbeOutcome {
    let Some(_key) = configured_key(env_var) else {
        return excluded_outcome(api, env_var, affects);
    };

    let start = Instant::now();

    match crate::sources::alphagenome::AlphaGenomeClient::new().await {
        Ok(_) => outcome(
            health_row(
                api,
                "ok".into(),
                format!("{}ms", start.elapsed().as_millis()),
                None,
                Some(true),
            ),
            ProbeClass::Healthy,
        ),
        Err(err) => outcome(
            health_row(
                api,
                "error".into(),
                api_error_latency(start, &err),
                affects,
                Some(true),
            ),
            ProbeClass::Error,
        ),
    }
}

fn ema_local_data_outcome(root: &Path, env_configured: bool) -> ProbeOutcome {
    let api = format!("EMA local data ({})", root.display());
    let missing =
        crate::sources::ema::ema_missing_files(root, crate::sources::ema::EMA_REQUIRED_FILES);

    if missing.is_empty() {
        let status = if env_configured {
            "configured"
        } else {
            "available (default path)"
        };
        return outcome(
            health_row(&api, status.to_string(), "n/a".into(), None, None),
            ProbeClass::Healthy,
        );
    }

    if !env_configured && missing.len() == crate::sources::ema::EMA_REQUIRED_FILES.len() {
        return outcome(
            health_row(
                &api,
                "not configured".into(),
                "n/a".into(),
                Some(EMA_LOCAL_DATA_AFFECTS),
                None,
            ),
            ProbeClass::Excluded,
        );
    }

    outcome(
        health_row(
            &api,
            format!("error (missing: {})", missing.join(", ")),
            "n/a".into(),
            Some(EMA_LOCAL_DATA_AFFECTS),
            None,
        ),
        ProbeClass::Error,
    )
}

fn check_ema_local_data() -> ProbeOutcome {
    let env_configured = configured_key("BIOMCP_EMA_DIR").is_some();
    let root = crate::sources::ema::resolve_ema_root();
    ema_local_data_outcome(&root, env_configured)
}

async fn probe_source(client: reqwest::Client, source: &SourceDescriptor) -> ProbeOutcome {
    match source.probe {
        ProbeKind::Get { url } => check_get(client, source.api, url, source.affects).await,
        ProbeKind::PostJson { url, payload } => {
            check_post_json(client, source.api, url, payload, source.affects).await
        }
        ProbeKind::AuthGet {
            url,
            env_var,
            header_name,
            header_value_prefix,
        } => {
            check_auth_get(
                client,
                source.api,
                url,
                env_var,
                header_name,
                header_value_prefix,
                source.affects,
            )
            .await
        }
        ProbeKind::OptionalAuthGet {
            url,
            env_var,
            header_name,
            header_value_prefix,
            unauthenticated_ok_status,
            authenticated_ok_status,
            unauthenticated_rate_limited_status,
        } => {
            check_optional_auth_get(
                client,
                source.api,
                url,
                env_var,
                header_name,
                header_value_prefix,
                unauthenticated_ok_status,
                authenticated_ok_status,
                unauthenticated_rate_limited_status,
                source.affects,
            )
            .await
        }
        ProbeKind::AuthQueryParam {
            url,
            env_var,
            param_name,
        } => {
            check_auth_query_param(client, source.api, url, env_var, param_name, source.affects)
                .await
        }
        ProbeKind::AuthPostJson {
            url,
            payload,
            env_var,
            header_name,
            header_value_prefix,
        } => {
            check_auth_post_json(
                client,
                source.api,
                url,
                payload,
                env_var,
                header_name,
                header_value_prefix,
                source.affects,
            )
            .await
        }
        ProbeKind::AlphaGenomeConnect { env_var } => {
            check_alphagenome_connect(source.api, env_var, source.affects).await
        }
    }
}

fn health_http_client() -> Result<reqwest::Client, BioMcpError> {
    static HEALTH_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

    if let Some(client) = HEALTH_HTTP_CLIENT.get() {
        return Ok(client.clone());
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .user_agent(concat!("biomcp-cli/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(BioMcpError::HttpClientInit)?;

    match HEALTH_HTTP_CLIENT.set(client.clone()) {
        Ok(()) => Ok(client),
        Err(_) => HEALTH_HTTP_CLIENT
            .get()
            .cloned()
            .ok_or_else(|| BioMcpError::Api {
                api: "health".into(),
                message: "Health HTTP client initialization race".into(),
            }),
    }
}

async fn check_cache_dir() -> ProbeOutcome {
    let dir = match crate::cache::resolve_cache_config() {
        Ok(config) => config.cache_root,
        Err(err) => {
            return outcome(
                HealthRow {
                    api: "Cache dir".into(),
                    status: "error".into(),
                    latency: err.to_string(),
                    affects: Some("local cache-backed lookups and downloads".into()),
                    key_configured: None,
                },
                ProbeClass::Error,
            );
        }
    };
    probe_cache_dir(&dir).await
}

async fn check_cache_limits() -> ProbeOutcome {
    let config = match crate::cache::resolve_cache_config() {
        Ok(config) => config,
        Err(err) => {
            return cache_limits_error_outcome(err.to_string());
        }
    };

    match tokio::task::spawn_blocking(move || {
        check_cache_limits_with(
            || Ok(config),
            crate::cache::snapshot_cache,
            crate::cache::inspect_filesystem_space,
        )
    })
    .await
    {
        Ok(outcome) => outcome,
        Err(err) => cache_limits_error_outcome(err.to_string()),
    }
}

async fn probe_cache_dir(dir: &Path) -> ProbeOutcome {
    let start = Instant::now();
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let probe = dir.join(format!(".biomcp-healthcheck-{suffix}.tmp"));

    let result = async {
        tokio::fs::create_dir_all(&dir).await?;
        tokio::fs::write(&probe, b"ok").await?;
        match tokio::fs::remove_file(&probe).await {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err),
        }
    }
    .await;

    match result {
        Ok(()) => outcome(
            HealthRow {
                api: format!("Cache dir ({})", dir.display()),
                status: "ok".into(),
                latency: format!("{}ms", start.elapsed().as_millis()),
                affects: None,
                key_configured: None,
            },
            ProbeClass::Healthy,
        ),
        Err(err) => outcome(
            HealthRow {
                api: format!("Cache dir ({})", dir.display()),
                status: "error".into(),
                latency: format!("{:?}", err.kind()),
                affects: Some("local cache-backed lookups and downloads".into()),
                key_configured: None,
            },
            ProbeClass::Error,
        ),
    }
}

fn report_from_outcomes(outcomes: Vec<ProbeOutcome>) -> HealthReport {
    let healthy = outcomes
        .iter()
        .filter(|outcome| outcome.class == ProbeClass::Healthy)
        .count();
    let warning = outcomes
        .iter()
        .filter(|outcome| outcome.class == ProbeClass::Warning)
        .count();
    let excluded = outcomes
        .iter()
        .filter(|outcome| outcome.class == ProbeClass::Excluded)
        .count();
    let rows = outcomes
        .into_iter()
        .map(|outcome| outcome.row)
        .collect::<Vec<_>>();

    HealthReport {
        healthy,
        warning,
        excluded,
        total: rows.len(),
        rows,
    }
}

/// Runs connectivity checks for configured upstream APIs and local EMA/cache readiness.
///
/// # Errors
///
/// Returns an error when the shared HTTP client cannot be created.
pub async fn check(apis_only: bool) -> Result<HealthReport, BioMcpError> {
    let client = health_http_client()?;
    let mut outcomes = join_all(
        health_sources()
            .iter()
            .map(|source| probe_source(client.clone(), source)),
    )
    .await;

    if !apis_only {
        outcomes.push(check_ema_local_data());
        outcomes.push(check_cache_dir().await);
        outcomes.push(check_cache_limits().await);
    }

    Ok(report_from_outcomes(outcomes))
}

fn check_cache_limits_with<R, S, I>(
    resolve_config: R,
    snapshotter: S,
    inspect_space: I,
) -> ProbeOutcome
where
    R: FnOnce() -> Result<crate::cache::ResolvedCacheConfig, BioMcpError>,
    S: FnOnce(&Path) -> Result<crate::cache::CacheSnapshot, crate::cache::CachePlannerError>,
    I: FnOnce(&Path) -> Result<crate::cache::FilesystemSpace, BioMcpError>,
{
    let config = match resolve_config() {
        Ok(config) => config,
        Err(err) => return cache_limits_error_outcome(err.to_string()),
    };
    let cache_path = config.cache_root.join("http");
    let snapshot = match snapshotter(&cache_path) {
        Ok(snapshot) => snapshot,
        Err(err) => return cache_limits_error_outcome(err.to_string()),
    };
    let space = match inspect_space(&config.cache_root) {
        Ok(space) => space,
        Err(err) => return cache_limits_error_outcome(err.to_string()),
    };
    let evaluation = crate::cache::evaluate_cache_limits(&snapshot, &config, space);

    if evaluation.over_max_size || evaluation.below_min_disk_free {
        return outcome(
            HealthRow {
                api: "Cache limits".into(),
                status: "warning".into(),
                latency: cache_limits_warning_message(&config, space, &evaluation),
                affects: None,
                key_configured: None,
            },
            ProbeClass::Warning,
        );
    }

    outcome(
        HealthRow {
            api: "Cache limits".into(),
            status: "ok".into(),
            latency: "within limits".into(),
            affects: None,
            key_configured: None,
        },
        ProbeClass::Healthy,
    )
}

fn cache_limits_warning_message(
    config: &crate::cache::ResolvedCacheConfig,
    space: crate::cache::FilesystemSpace,
    evaluation: &crate::cache::CacheLimitEvaluation,
) -> String {
    let mut clauses = Vec::new();
    if evaluation.over_max_size {
        clauses.push(format!(
            "referenced bytes {} exceed max_size {}",
            evaluation.usage.referenced_blob_bytes, config.max_size
        ));
    }
    if evaluation.below_min_disk_free {
        clauses.push(format!(
            "available disk {} is below min_disk_free {}",
            ByteSize(space.available_bytes),
            config.min_disk_free.display()
        ));
    }
    format!("{}; run biomcp cache clean", clauses.join("; "))
}

fn cache_limits_error_outcome(message: String) -> ProbeOutcome {
    outcome(
        HealthRow {
            api: "Cache limits".into(),
            status: "error".into(),
            latency: message,
            affects: None,
            key_configured: None,
        },
        ProbeClass::Error,
    )
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::io;
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use ssri::Integrity;
    use tokio::sync::MutexGuard;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::{
        EMA_LOCAL_DATA_AFFECTS, HealthReport, HealthRow, ProbeClass, ProbeKind, ProbeOutcome,
        SourceDescriptor, affects_for_api, check_cache_dir, check_cache_limits_with,
        ema_local_data_outcome, health_sources, probe_cache_dir, probe_source,
        report_from_outcomes,
    };
    use crate::cache::{
        CacheBlob, CacheConfigOrigins, CacheEntry, CachePlannerError, CacheSnapshot, ConfigOrigin,
        DiskFreeThreshold, FilesystemSpace, ResolvedCacheConfig,
    };

    fn block_on<F: Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("health test runtime")
            .block_on(future)
    }

    fn env_lock() -> MutexGuard<'static, ()> {
        crate::test_support::env_lock().blocking_lock()
    }

    struct EnvVarGuard {
        name: &'static str,
        previous: Option<String>,
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            // Safety: tests serialize environment mutation with `env_lock()`.
            unsafe {
                match &self.previous {
                    Some(value) => std::env::set_var(self.name, value),
                    None => std::env::remove_var(self.name),
                }
            }
        }
    }

    fn set_env_var(name: &'static str, value: Option<&str>) -> EnvVarGuard {
        let previous = std::env::var(name).ok();
        // Safety: tests serialize environment mutation with `env_lock()`.
        unsafe {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
        EnvVarGuard { name, previous }
    }

    struct TempDirGuard {
        path: PathBuf,
    }

    impl TempDirGuard {
        fn new() -> Self {
            let suffix = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "biomcp-health-test-{}-{suffix}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn fixture_ema_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("spec")
            .join("fixtures")
            .join("ema-human")
    }

    fn write_ema_files(root: &Path, files: &[&str]) {
        for file in files {
            std::fs::write(root.join(file), b"{}").expect("write EMA fixture file");
        }
    }

    fn assert_cache_dir_affects(value: Option<&str>) {
        assert_eq!(value, Some("local cache-backed lookups and downloads"));
    }

    fn assert_millisecond_latency(value: &str) {
        let digits = value
            .strip_suffix("ms")
            .expect("latency should end with ms");
        assert!(
            !digits.is_empty() && digits.chars().all(|ch| ch.is_ascii_digit()),
            "unexpected latency: {value}"
        );
    }

    fn semantic_scholar_source(url: &'static str) -> SourceDescriptor {
        let source = health_sources()
            .iter()
            .find(|source| source.api == "Semantic Scholar")
            .expect("semantic scholar health source");
        let ProbeKind::OptionalAuthGet {
            env_var,
            header_name,
            header_value_prefix,
            unauthenticated_ok_status,
            authenticated_ok_status,
            unauthenticated_rate_limited_status,
            ..
        } = source.probe
        else {
            panic!("semantic scholar should use optional auth get");
        };

        SourceDescriptor {
            api: source.api,
            affects: source.affects,
            probe: ProbeKind::OptionalAuthGet {
                url,
                env_var,
                header_name,
                header_value_prefix,
                unauthenticated_ok_status,
                authenticated_ok_status,
                unauthenticated_rate_limited_status,
            },
        }
    }

    fn test_integrity(bytes: &[u8]) -> Integrity {
        Integrity::from(bytes)
    }

    fn test_entry(key: &str, bytes: &[u8], time_ms: u128) -> CacheEntry {
        CacheEntry {
            key: key.to_string(),
            integrity: test_integrity(bytes),
            time_ms,
            size_bytes: bytes.len() as u64,
        }
    }

    fn test_blob(label: &str, bytes: &[u8], refcount: usize) -> CacheBlob {
        CacheBlob {
            integrity: test_integrity(bytes),
            path: PathBuf::from(format!("content-v2/mock/{label}.blob")),
            size_bytes: bytes.len() as u64,
            refcount,
        }
    }

    fn test_snapshot(
        cache_path: impl Into<PathBuf>,
        entries: Vec<CacheEntry>,
        blobs: Vec<CacheBlob>,
    ) -> CacheSnapshot {
        CacheSnapshot {
            cache_path: cache_path.into(),
            entries,
            blobs,
        }
    }

    fn test_config(
        cache_root: impl Into<PathBuf>,
        max_size: u64,
        min_disk_free: DiskFreeThreshold,
    ) -> ResolvedCacheConfig {
        ResolvedCacheConfig {
            cache_root: cache_root.into(),
            max_size,
            min_disk_free,
            max_age: Duration::from_secs(86_400),
            origins: CacheConfigOrigins {
                cache_root: ConfigOrigin::Default,
                max_size: ConfigOrigin::Default,
                min_disk_free: ConfigOrigin::Default,
                max_age: ConfigOrigin::Default,
            },
        }
    }

    #[test]
    fn markdown_shows_affects_column_when_present() {
        let report = HealthReport {
            healthy: 1,
            warning: 0,
            excluded: 0,
            total: 2,
            rows: vec![
                HealthRow {
                    api: "MyGene".into(),
                    status: "ok".into(),
                    latency: "10ms".into(),
                    affects: None,
                    key_configured: None,
                },
                HealthRow {
                    api: "OpenFDA".into(),
                    status: "error".into(),
                    latency: "timeout".into(),
                    affects: Some("adverse-event search".into()),
                    key_configured: None,
                },
            ],
        };
        let md = report.to_markdown();
        assert!(md.contains("| API | Status | Latency | Affects |"));
        assert!(md.contains("adverse-event search"));
    }

    #[test]
    fn markdown_omits_affects_column_when_all_healthy() {
        let report = HealthReport {
            healthy: 2,
            warning: 0,
            excluded: 0,
            total: 2,
            rows: vec![
                HealthRow {
                    api: "MyGene".into(),
                    status: "ok".into(),
                    latency: "10ms".into(),
                    affects: None,
                    key_configured: None,
                },
                HealthRow {
                    api: "MyVariant".into(),
                    status: "ok".into(),
                    latency: "11ms".into(),
                    affects: None,
                    key_configured: None,
                },
            ],
        };
        let md = report.to_markdown();
        assert!(md.contains("| API | Status | Latency |"));
        assert!(!md.contains("| API | Status | Latency | Affects |"));
    }

    #[test]
    fn markdown_decorates_keyed_success_rows_without_changing_status() {
        let report = HealthReport {
            healthy: 1,
            warning: 0,
            excluded: 0,
            total: 1,
            rows: vec![HealthRow {
                api: "OncoKB".into(),
                status: "ok".into(),
                latency: "10ms".into(),
                affects: None,
                key_configured: Some(true),
            }],
        };

        assert_eq!(report.rows[0].status, "ok");
        let md = report.to_markdown();
        assert!(md.contains("| OncoKB | ok (key configured) | 10ms |"));
    }

    #[test]
    fn markdown_decorates_keyed_error_rows_without_changing_status() {
        let report = HealthReport {
            healthy: 0,
            warning: 0,
            excluded: 0,
            total: 1,
            rows: vec![HealthRow {
                api: "OncoKB".into(),
                status: "error".into(),
                latency: "10ms (HTTP 401)".into(),
                affects: Some("variant oncokb command and variant evidence section".into()),
                key_configured: Some(true),
            }],
        };

        assert_eq!(report.rows[0].status, "error");
        let md = report.to_markdown();
        assert!(md.contains(
            "| OncoKB | error (key configured) | 10ms (HTTP 401) | variant oncokb command and variant evidence section |",
        ));
    }

    #[test]
    fn health_inventory_includes_all_expected_sources() {
        let names: Vec<_> = health_sources().iter().map(|source| source.api).collect();

        assert_eq!(
            names,
            vec![
                "MyGene",
                "MyVariant",
                "MyChem",
                "PubTator3",
                "PubMed",
                "Europe PMC",
                "LitSense2",
                "PMC OA",
                "NCBI ID Converter",
                "ClinicalTrials.gov",
                "NCI CTS",
                "Enrichr",
                "OpenFDA",
                "OncoKB",
                "DisGeNET",
                "AlphaGenome",
                "Semantic Scholar",
                "CPIC",
                "PharmGKB",
                "Monarch",
                "HPO",
                "MyDisease",
                "CIViC",
                "GWAS Catalog",
                "GTEx",
                "DGIdb",
                "ClinGen",
                "gnomAD",
                "UniProt",
                "QuickGO",
                "STRING",
                "Reactome",
                "KEGG",
                "WikiPathways",
                "g:Profiler",
                "OpenTargets",
                "ChEMBL",
                "HPA",
                "InterPro",
                "ComplexPortal",
                "OLS4",
                "UMLS",
                "MedlinePlus",
                "cBioPortal",
            ]
        );
    }

    #[test]
    fn ema_local_data_not_configured_when_default_root_is_empty() {
        let root = TempDirGuard::new();

        let outcome = ema_local_data_outcome(root.path(), false);

        assert_eq!(outcome.class, ProbeClass::Excluded);
        assert_eq!(
            outcome.row.api,
            format!("EMA local data ({})", root.path().display())
        );
        assert_eq!(outcome.row.status, "not configured");
        assert_eq!(outcome.row.latency, "n/a");
        assert_eq!(outcome.row.affects.as_deref(), Some(EMA_LOCAL_DATA_AFFECTS));
    }

    #[test]
    fn ema_local_data_errors_when_default_root_is_partial() {
        let root = TempDirGuard::new();
        write_ema_files(root.path(), &[crate::sources::ema::EMA_REQUIRED_FILES[0]]);

        let outcome = ema_local_data_outcome(root.path(), false);

        assert_eq!(outcome.class, ProbeClass::Error);
        assert_eq!(
            outcome.row.status,
            format!(
                "error (missing: {})",
                crate::sources::ema::EMA_REQUIRED_FILES[1..].join(", ")
            )
        );
        assert_eq!(outcome.row.affects.as_deref(), Some(EMA_LOCAL_DATA_AFFECTS));
    }

    #[test]
    fn ema_local_data_errors_when_env_root_is_missing_files() {
        let root = TempDirGuard::new();

        let outcome = ema_local_data_outcome(root.path(), true);

        assert_eq!(outcome.class, ProbeClass::Error);
        assert_eq!(
            outcome.row.status,
            format!(
                "error (missing: {})",
                crate::sources::ema::EMA_REQUIRED_FILES.join(", ")
            )
        );
        assert_eq!(outcome.row.affects.as_deref(), Some(EMA_LOCAL_DATA_AFFECTS));
    }

    #[test]
    fn ema_local_data_reports_available_when_default_root_is_complete() {
        let fixture_root = fixture_ema_root();

        let outcome = ema_local_data_outcome(&fixture_root, false);

        assert_eq!(outcome.class, ProbeClass::Healthy);
        assert_eq!(
            outcome.row.api,
            format!("EMA local data ({})", fixture_root.display())
        );
        assert_eq!(outcome.row.status, "available (default path)");
        assert_eq!(outcome.row.latency, "n/a");
        assert_eq!(outcome.row.affects, None);
    }

    #[test]
    fn ema_local_data_reports_configured_when_env_root_is_complete() {
        let fixture_root = fixture_ema_root();

        let outcome = ema_local_data_outcome(&fixture_root, true);

        assert_eq!(outcome.class, ProbeClass::Healthy);
        assert_eq!(outcome.row.status, "configured");
        assert_eq!(outcome.row.affects, None);
    }

    #[test]
    fn ema_local_data_json_reports_healthy_row_without_affects() {
        let fixture_root = fixture_ema_root();
        let report = report_from_outcomes(vec![ema_local_data_outcome(&fixture_root, false)]);

        let value = serde_json::to_value(&report).expect("serialize health report");
        let rows = value["rows"].as_array().expect("rows array");
        let row = rows.first().expect("EMA row");

        assert_eq!(
            row["api"],
            format!("EMA local data ({})", fixture_root.display())
        );
        assert_eq!(row["status"], "available (default path)");
        assert_eq!(row["latency"], "n/a");
        assert!(row.get("affects").is_none());
        assert!(row.get("key_configured").is_none());
    }

    #[test]
    fn ema_local_data_json_reports_error_row_with_affects() {
        let root = TempDirGuard::new();
        write_ema_files(root.path(), &[crate::sources::ema::EMA_REQUIRED_FILES[0]]);
        let report = report_from_outcomes(vec![ema_local_data_outcome(root.path(), false)]);

        let value = serde_json::to_value(&report).expect("serialize health report");
        let rows = value["rows"].as_array().expect("rows array");
        let row = rows.first().expect("EMA row");

        assert_eq!(
            row["status"],
            format!(
                "error (missing: {})",
                crate::sources::ema::EMA_REQUIRED_FILES[1..].join(", ")
            )
        );
        assert_eq!(row["affects"], EMA_LOCAL_DATA_AFFECTS);
        assert!(row.get("key_configured").is_none());
    }

    #[test]
    fn key_gated_source_is_excluded_when_env_missing() {
        let _lock = env_lock();
        let _env = set_env_var("ONCOKB_TOKEN", None);
        let source = health_sources()
            .iter()
            .find(|source| source.api == "OncoKB")
            .expect("oncokb health source");

        let outcome = block_on(probe_source(reqwest::Client::new(), source));

        assert_eq!(outcome.class, ProbeClass::Excluded);
        assert_eq!(outcome.row.status, "excluded (set ONCOKB_TOKEN)");
        assert_eq!(outcome.row.latency, "n/a");
        assert_eq!(
            outcome.row.affects.as_deref(),
            Some("variant oncokb command and variant evidence section")
        );
        assert_eq!(outcome.row.key_configured, Some(false));
    }

    #[test]
    fn excluded_key_gated_row_serializes_key_configured_false() {
        let report = report_from_outcomes(vec![ProbeOutcome {
            row: HealthRow {
                api: "OncoKB".into(),
                status: "excluded (set ONCOKB_TOKEN)".into(),
                latency: "n/a".into(),
                affects: Some("variant oncokb command and variant evidence section".into()),
                key_configured: Some(false),
            },
            class: ProbeClass::Excluded,
        }]);

        let value = serde_json::to_value(&report).expect("serialize health report");
        let rows = value["rows"].as_array().expect("rows array");
        let row = rows.first().expect("oncokb row");

        assert_eq!(row["status"], "excluded (set ONCOKB_TOKEN)");
        assert_eq!(row["key_configured"], false);
    }

    #[test]
    fn public_row_omits_key_configured_in_json() {
        let report = report_from_outcomes(vec![ProbeOutcome {
            row: HealthRow {
                api: "MyGene".into(),
                status: "ok".into(),
                latency: "10ms".into(),
                affects: None,
                key_configured: None,
            },
            class: ProbeClass::Healthy,
        }]);

        let value = serde_json::to_value(&report).expect("serialize health report");
        let rows = value["rows"].as_array().expect("rows array");
        let row = rows.first().expect("mygene row");

        assert!(row.get("key_configured").is_none());
    }

    #[test]
    fn keyed_row_serializes_raw_status_with_key_configured_true() {
        let value = serde_json::to_value(HealthRow {
            api: "OncoKB".into(),
            status: "ok".into(),
            latency: "10ms".into(),
            affects: None,
            key_configured: Some(true),
        })
        .expect("serialize keyed row");

        assert_eq!(value["status"], "ok");
        assert_eq!(value["key_configured"], true);
    }

    #[test]
    fn empty_key_is_treated_as_missing() {
        let _lock = env_lock();
        let _env = set_env_var("NCI_API_KEY", Some("   "));
        let source = health_sources()
            .iter()
            .find(|source| source.api == "NCI CTS")
            .expect("nci health source");

        let outcome = block_on(probe_source(reqwest::Client::new(), source));

        assert_eq!(outcome.class, ProbeClass::Excluded);
        assert_eq!(outcome.row.status, "excluded (set NCI_API_KEY)");
        assert_eq!(outcome.row.latency, "n/a");
        assert_eq!(outcome.row.key_configured, Some(false));
    }

    #[test]
    fn nci_health_probe_uses_keyword_query() {
        let source = health_sources()
            .iter()
            .find(|source| source.api == "NCI CTS")
            .expect("nci health source");

        let ProbeKind::AuthGet { url, .. } = source.probe else {
            panic!("NCI CTS health source should use an authenticated GET probe");
        };

        assert!(url.contains("keyword=melanoma"));
        assert!(!url.contains("diseases=melanoma"));
    }

    #[test]
    fn alpha_genome_health_probe_connects_without_scoring() {
        let source = health_sources()
            .iter()
            .find(|source| source.api == "AlphaGenome")
            .expect("alphagenome health source");

        assert!(matches!(source.probe, ProbeKind::AlphaGenomeConnect { .. }));
    }

    #[test]
    fn all_healthy_includes_warning_and_excluded_rows() {
        let report = HealthReport {
            healthy: 1,
            warning: 1,
            excluded: 1,
            total: 3,
            rows: vec![
                HealthRow {
                    api: "MyGene".into(),
                    status: "ok".into(),
                    latency: "10ms".into(),
                    affects: None,
                    key_configured: None,
                },
                HealthRow {
                    api: "OncoKB".into(),
                    status: "excluded (set ONCOKB_TOKEN)".into(),
                    latency: "n/a".into(),
                    affects: Some("variant oncokb command and variant evidence section".into()),
                    key_configured: Some(false),
                },
                HealthRow {
                    api: "Cache limits".into(),
                    status: "warning".into(),
                    latency: "referenced bytes 12 exceed max_size 8; run biomcp cache clean".into(),
                    affects: None,
                    key_configured: None,
                },
            ],
        };

        assert!(report.all_healthy());
    }

    #[test]
    fn markdown_summary_reports_ok_error_excluded_and_warning_counts() {
        let report = HealthReport {
            healthy: 1,
            warning: 1,
            excluded: 1,
            total: 4,
            rows: vec![
                HealthRow {
                    api: "MyGene".into(),
                    status: "ok".into(),
                    latency: "10ms".into(),
                    affects: None,
                    key_configured: None,
                },
                HealthRow {
                    api: "OpenFDA".into(),
                    status: "error".into(),
                    latency: "timeout".into(),
                    affects: Some("adverse-event search".into()),
                    key_configured: None,
                },
                HealthRow {
                    api: "OncoKB".into(),
                    status: "excluded (set ONCOKB_TOKEN)".into(),
                    latency: "n/a".into(),
                    affects: Some("variant oncokb command and variant evidence section".into()),
                    key_configured: Some(false),
                },
                HealthRow {
                    api: "Cache limits".into(),
                    status: "warning".into(),
                    latency:
                        "available disk 10 B is below min_disk_free 20 B; run biomcp cache clean"
                            .into(),
                    affects: None,
                    key_configured: None,
                },
            ],
        };

        let md = report.to_markdown();
        assert!(md.contains("Status: 1 ok, 1 error, 1 excluded, 1 warning"));
    }

    #[test]
    fn report_counts_use_probe_class_not_status_prefixes() {
        let report = report_from_outcomes(vec![
            ProbeOutcome {
                row: HealthRow {
                    api: "Semantic Scholar".into(),
                    status: "available (unauthenticated, shared rate limit)".into(),
                    latency: "15ms".into(),
                    affects: None,
                    key_configured: Some(false),
                },
                class: ProbeClass::Healthy,
            },
            ProbeOutcome {
                row: HealthRow {
                    api: "OncoKB".into(),
                    status: "excluded (set ONCOKB_TOKEN)".into(),
                    latency: "n/a".into(),
                    affects: Some("variant oncokb command and variant evidence section".into()),
                    key_configured: Some(false),
                },
                class: ProbeClass::Excluded,
            },
            ProbeOutcome {
                row: HealthRow {
                    api: "Cache limits".into(),
                    status: "warning".into(),
                    latency: "referenced bytes 12 exceed max_size 8; run biomcp cache clean".into(),
                    affects: None,
                    key_configured: None,
                },
                class: ProbeClass::Warning,
            },
        ]);

        assert_eq!(report.healthy, 1);
        assert_eq!(report.warning, 1);
        assert_eq!(report.excluded, 1);
        assert_eq!(report.total, 3);
    }

    #[test]
    fn check_cache_limits_within_limits_returns_healthy_row() {
        let config = test_config("/tmp/cache", 1_024, DiskFreeThreshold::Percent(10));
        let snapshot = test_snapshot(
            "/tmp/cache/http",
            vec![test_entry("retained", b"live-bytes", 100)],
            vec![test_blob("retained", b"live-bytes", 1)],
        );

        let outcome = check_cache_limits_with(
            || Ok(config),
            |_| Ok(snapshot.clone()),
            |_| {
                Ok(FilesystemSpace {
                    available_bytes: 90,
                    total_bytes: 100,
                })
            },
        );

        assert_eq!(outcome.class, ProbeClass::Healthy);
        assert_eq!(outcome.row.api, "Cache limits");
        assert_eq!(outcome.row.status, "ok");
        assert_eq!(outcome.row.latency, "within limits");
    }

    #[test]
    fn check_cache_limits_warns_when_referenced_bytes_exceed_max_size() {
        let config = test_config("/tmp/cache", 5, DiskFreeThreshold::Percent(10));
        let snapshot = test_snapshot(
            "/tmp/cache/http",
            vec![
                test_entry("old", b"abcde", 100),
                test_entry("new", b"fghij", 200),
            ],
            vec![test_blob("old", b"abcde", 1), test_blob("new", b"fghij", 1)],
        );

        let outcome = check_cache_limits_with(
            || Ok(config),
            |_| Ok(snapshot.clone()),
            |_| {
                Ok(FilesystemSpace {
                    available_bytes: 90,
                    total_bytes: 100,
                })
            },
        );

        assert_eq!(outcome.class, ProbeClass::Warning);
        assert_eq!(outcome.row.status, "warning");
        assert!(outcome.row.latency.contains("referenced bytes"));
        assert!(outcome.row.latency.contains("biomcp cache clean"));
    }

    #[test]
    fn check_cache_limits_warns_when_disk_floor_is_violated() {
        let config = test_config("/tmp/cache", 1_024, DiskFreeThreshold::Percent(20));
        let snapshot = test_snapshot(
            "/tmp/cache/http",
            vec![test_entry("retained", b"live-bytes", 100)],
            vec![test_blob("retained", b"live-bytes", 1)],
        );

        let outcome = check_cache_limits_with(
            || Ok(config),
            |_| Ok(snapshot.clone()),
            |_| {
                Ok(FilesystemSpace {
                    available_bytes: 10,
                    total_bytes: 100,
                })
            },
        );

        assert_eq!(outcome.class, ProbeClass::Warning);
        assert_eq!(outcome.row.status, "warning");
        assert!(outcome.row.latency.contains("available disk"));
        assert!(outcome.row.latency.contains("biomcp cache clean"));
    }

    #[test]
    fn check_cache_limits_reports_snapshot_errors_as_error_rows() {
        let config = test_config("/tmp/cache", 1_024, DiskFreeThreshold::Percent(10));

        let outcome = check_cache_limits_with(
            || Ok(config),
            |_| {
                Err(CachePlannerError::Io {
                    path: PathBuf::from("/tmp/cache/http"),
                    source: io::Error::other("boom"),
                })
            },
            |_| {
                Ok(FilesystemSpace {
                    available_bytes: 90,
                    total_bytes: 100,
                })
            },
        );

        assert_eq!(outcome.class, ProbeClass::Error);
        assert_eq!(outcome.row.api, "Cache limits");
        assert_eq!(outcome.row.status, "error");
        assert!(outcome.row.latency.contains("boom"));
    }

    #[test]
    fn optional_auth_get_reports_unauthed_semantic_scholar_as_healthy() {
        let _lock = env_lock();
        let _env = set_env_var("S2_API_KEY", None);
        let server = block_on(MockServer::start());
        let url = Box::leak(format!("{}/health", server.uri()).into_boxed_str());
        let source = semantic_scholar_source(url);

        block_on(async {
            Mock::given(method("GET"))
                .and(path("/health"))
                .and(|request: &wiremock::Request| !request.headers.contains_key("x-api-key"))
                .respond_with(ResponseTemplate::new(200))
                .expect(1)
                .mount(&server)
                .await;
        });

        let outcome = block_on(probe_source(reqwest::Client::new(), &source));
        assert_eq!(outcome.class, ProbeClass::Healthy);
        assert_eq!(
            outcome.row.status,
            "available (unauthenticated, shared rate limit)"
        );
        assert_eq!(outcome.row.key_configured, Some(false));
    }

    #[test]
    fn optional_auth_get_reports_authed_semantic_scholar_as_configured() {
        let _lock = env_lock();
        let _env = set_env_var("S2_API_KEY", Some("test-key-abc"));
        let server = block_on(MockServer::start());
        let url = Box::leak(format!("{}/health", server.uri()).into_boxed_str());
        let source = semantic_scholar_source(url);

        block_on(async {
            Mock::given(method("GET"))
                .and(path("/health"))
                .and(header("x-api-key", "test-key-abc"))
                .respond_with(ResponseTemplate::new(200))
                .expect(1)
                .mount(&server)
                .await;
        });

        let outcome = block_on(probe_source(reqwest::Client::new(), &source));
        assert_eq!(outcome.class, ProbeClass::Healthy);
        assert_eq!(outcome.row.status, "configured (authenticated)");
        assert_eq!(outcome.row.key_configured, Some(true));
    }

    #[test]
    fn optional_auth_get_reports_unauthenticated_429_as_unavailable() {
        let _lock = env_lock();
        let _env = set_env_var("S2_API_KEY", None);
        let server = block_on(MockServer::start());
        let url = Box::leak(format!("{}/health", server.uri()).into_boxed_str());
        let source = semantic_scholar_source(url);

        block_on(async {
            Mock::given(method("GET"))
                .and(path("/health"))
                .and(|request: &wiremock::Request| !request.headers.contains_key("x-api-key"))
                .respond_with(ResponseTemplate::new(429))
                .expect(1)
                .mount(&server)
                .await;
        });

        let outcome = block_on(probe_source(reqwest::Client::new(), &source));
        assert_eq!(outcome.class, ProbeClass::Healthy);
        assert_eq!(
            outcome.row.status,
            "unavailable (set S2_API_KEY for reliable access)"
        );
        assert_millisecond_latency(&outcome.row.latency);
        assert!(!outcome.row.latency.contains("HTTP 429"));
        assert_eq!(outcome.row.affects, None);
        assert_eq!(outcome.row.key_configured, Some(false));

        let report = report_from_outcomes(vec![outcome.clone()]);
        assert_eq!(report.healthy, 1);
        assert_eq!(report.excluded, 0);
        assert_eq!(report.total, 1);
        assert!(report.all_healthy());

        let value = serde_json::to_value(&report).expect("serialize health report");
        let rows = value["rows"].as_array().expect("rows array");
        let row = rows.first().expect("semantic scholar row");
        assert!(row.get("affects").is_none());
        assert_eq!(row["key_configured"], false);

        let md = report_from_outcomes(vec![
            outcome.clone(),
            ProbeOutcome {
                row: HealthRow {
                    api: "OpenFDA".into(),
                    status: "error".into(),
                    latency: "timeout".into(),
                    affects: Some("adverse-event search".into()),
                    key_configured: None,
                },
                class: ProbeClass::Error,
            },
        ])
        .to_markdown();
        assert!(md.contains(&format!(
            "| Semantic Scholar | {} | {} | - |",
            outcome.row.status, outcome.row.latency
        )));
    }

    #[test]
    fn optional_auth_get_reports_unauthenticated_non_429_as_error() {
        let _lock = env_lock();
        let _env = set_env_var("S2_API_KEY", None);
        let server = block_on(MockServer::start());
        let url = Box::leak(format!("{}/health", server.uri()).into_boxed_str());
        let source = semantic_scholar_source(url);

        block_on(async {
            Mock::given(method("GET"))
                .and(path("/health"))
                .and(|request: &wiremock::Request| !request.headers.contains_key("x-api-key"))
                .respond_with(ResponseTemplate::new(403))
                .expect(1)
                .mount(&server)
                .await;
        });

        let outcome = block_on(probe_source(reqwest::Client::new(), &source));
        assert_eq!(outcome.class, ProbeClass::Error);
        assert_eq!(outcome.row.status, "error");
        assert!(outcome.row.latency.contains("HTTP 403"));
        assert_eq!(
            outcome.row.affects.as_deref(),
            Some("Semantic Scholar features")
        );
        assert_eq!(outcome.row.key_configured, Some(false));
    }

    #[test]
    fn optional_auth_get_reports_authenticated_429_as_error() {
        let _lock = env_lock();
        let _env = set_env_var("S2_API_KEY", Some("test-key-abc"));
        let server = block_on(MockServer::start());
        let url = Box::leak(format!("{}/health", server.uri()).into_boxed_str());
        let source = semantic_scholar_source(url);

        block_on(async {
            Mock::given(method("GET"))
                .and(path("/health"))
                .and(header("x-api-key", "test-key-abc"))
                .respond_with(ResponseTemplate::new(429))
                .expect(1)
                .mount(&server)
                .await;
        });

        let outcome = block_on(probe_source(reqwest::Client::new(), &source));
        assert_eq!(outcome.class, ProbeClass::Error);
        assert_eq!(outcome.row.status, "error");
        assert!(outcome.row.latency.contains("HTTP 429"));
        assert_eq!(
            outcome.row.affects.as_deref(),
            Some("Semantic Scholar features")
        );
        assert_eq!(outcome.row.key_configured, Some(true));
    }

    #[test]
    fn check_cache_dir_success_row_uses_resolved_path_and_ok_contract() {
        let _lock = env_lock();
        let root = TempDirGuard::new();
        let cache_home = root.path().join("cache-home");
        let config_home = root.path().join("config-home");
        let _cache_home = set_env_var("XDG_CACHE_HOME", Some(&cache_home.to_string_lossy()));
        let _config_home = set_env_var("XDG_CONFIG_HOME", Some(&config_home.to_string_lossy()));
        let _cache_dir = set_env_var("BIOMCP_CACHE_DIR", None);

        let outcome = block_on(check_cache_dir());

        assert_eq!(outcome.class, ProbeClass::Healthy);
        assert_eq!(
            outcome.row.api,
            format!("Cache dir ({})", cache_home.join("biomcp").display())
        );
        assert_eq!(outcome.row.status, "ok");
        assert_millisecond_latency(&outcome.row.latency);
        assert_eq!(outcome.row.affects, None);
        assert_eq!(outcome.row.key_configured, None);
    }

    #[test]
    fn probe_cache_dir_failure_preserves_error_contract() {
        let root = TempDirGuard::new();
        let blocking_path = root.path().join("not-a-dir");
        std::fs::write(&blocking_path, b"occupied").expect("blocking file should exist");

        let outcome = block_on(probe_cache_dir(&blocking_path));

        assert_eq!(outcome.class, ProbeClass::Error);
        assert_eq!(
            outcome.row.api,
            format!("Cache dir ({})", blocking_path.display())
        );
        assert_eq!(outcome.row.status, "error");
        assert!(
            outcome.row.latency.contains("AlreadyExists")
                || outcome.row.latency.contains("NotADirectory")
                || outcome.row.latency.contains("PermissionDenied"),
            "unexpected latency: {}",
            outcome.row.latency
        );
        assert_cache_dir_affects(outcome.row.affects.as_deref());
        assert_eq!(outcome.row.key_configured, None);
    }

    #[test]
    fn check_cache_dir_config_error_matches_pinned_contract() {
        let _lock = env_lock();
        let root = TempDirGuard::new();
        let cache_home = root.path().join("cache-home");
        let config_home = root.path().join("config-home");
        let config_dir = config_home.join("biomcp");
        std::fs::create_dir_all(&config_dir).expect("config dir should exist");
        let config_path = config_dir.join("cache.toml");
        std::fs::write(&config_path, "[cache]\nmax_size = 0\n").expect("cache.toml should exist");
        let _cache_home = set_env_var("XDG_CACHE_HOME", Some(&cache_home.to_string_lossy()));
        let _config_home = set_env_var("XDG_CONFIG_HOME", Some(&config_home.to_string_lossy()));
        let _cache_dir = set_env_var("BIOMCP_CACHE_DIR", None);

        let outcome = block_on(check_cache_dir());

        assert_eq!(outcome.class, ProbeClass::Error);
        assert_eq!(outcome.row.api, "Cache dir");
        assert_eq!(outcome.row.status, "error");
        assert_eq!(
            outcome.row.latency,
            format!(
                "Invalid argument: {}: [cache].max_size must be greater than 0",
                config_path.display()
            )
        );
        assert_cache_dir_affects(outcome.row.affects.as_deref());
        assert_eq!(outcome.row.key_configured, None);
    }

    #[test]
    fn markdown_shows_new_affects_mappings() {
        assert_eq!(affects_for_api("GTEx"), Some("gene expression section"));
        assert_eq!(affects_for_api("DGIdb"), Some("gene druggability section"));
        assert_eq!(
            affects_for_api("OpenTargets"),
            Some("gene druggability, drug target, and disease association sections")
        );
        assert_eq!(affects_for_api("ClinGen"), Some("gene clingen section"));
        assert_eq!(affects_for_api("gnomAD"), Some("gene constraint section"));
        assert_eq!(
            affects_for_api("KEGG"),
            Some("pathway search and detail sections")
        );
        assert_eq!(
            affects_for_api("HPA"),
            Some("gene protein tissue expression and localization section")
        );
        assert_eq!(
            affects_for_api("ComplexPortal"),
            Some("protein complex membership section")
        );
        assert_eq!(
            affects_for_api("g:Profiler"),
            Some("gene enrichment (biomcp enrich)")
        );
    }
}
