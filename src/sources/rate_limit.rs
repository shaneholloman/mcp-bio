use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use http::Extensions;
use reqwest::Url;
use reqwest_middleware::{Middleware, Next};
use tokio::sync::Mutex;
use tokio::time::{Instant, sleep_until};

#[derive(Clone, Debug)]
pub(crate) struct RateLimitPolicy {
    pub key: &'static str,
    pub prefix: Cow<'static, str>,
    pub min_interval: Duration,
}

#[derive(Debug)]
pub(crate) struct RateLimiter {
    policies: Vec<RateLimitPolicy>,
    default_min_interval: Duration,
    last_seen: Mutex<HashMap<String, Instant>>,
}

impl RateLimiter {
    pub(crate) fn from_env() -> Self {
        // NCBI_API_KEY enables the higher PubTator request budget (10 req/sec).
        let has_ncbi_api_key = crate::sources::ncbi_api_key().is_some();
        let has_s2_api_key = crate::sources::s2_api_key().is_some();
        let policies = vec![
            policy(
                "pubtator",
                "BIOMCP_PUBTATOR_BASE",
                "https://www.ncbi.nlm.nih.gov/research/pubtator3-api",
                pubtator_min_interval(has_ncbi_api_key),
            ),
            policy(
                "pmc-oa",
                "BIOMCP_PMC_OA_BASE",
                "https://www.ncbi.nlm.nih.gov/pmc/utils/oa/oa.fcgi",
                Duration::from_millis(334),
            ),
            policy(
                "pubmed-eutils",
                "BIOMCP_PUBMED_BASE",
                "https://eutils.ncbi.nlm.nih.gov/entrez/eutils",
                pubmed_eutils_min_interval(has_ncbi_api_key),
            ),
            policy(
                "litsense2",
                "BIOMCP_LITSENSE2_BASE",
                "https://www.ncbi.nlm.nih.gov/research/litsense2-api/api",
                Duration::from_secs(1),
            ),
            policy(
                "ncbi-idconv",
                "BIOMCP_NCBI_IDCONV_BASE",
                "https://pmc.ncbi.nlm.nih.gov/tools/idconv/api/v1/articles",
                Duration::from_millis(334),
            ),
            policy(
                "nih-reporter",
                "BIOMCP_NIH_REPORTER_BASE",
                "https://api.reporter.nih.gov/v2",
                Duration::from_secs(1),
            ),
            policy(
                "opentargets",
                "BIOMCP_OPENTARGETS_BASE",
                "https://api.platform.opentargets.org/api/v4",
                Duration::from_millis(500),
            ),
            policy(
                "civic",
                "BIOMCP_CIVIC_BASE",
                "https://civicdb.org/api",
                Duration::from_millis(334),
            ),
            policy(
                "cpic",
                "BIOMCP_CPIC_BASE",
                "https://api.cpicpgx.org/v1",
                Duration::from_millis(250),
            ),
            policy(
                "pharmgkb",
                "BIOMCP_PHARMGKB_BASE",
                "https://api.pharmgkb.org/v1",
                Duration::from_millis(500),
            ),
            policy(
                "semantic-scholar",
                "BIOMCP_S2_BASE",
                "https://api.semanticscholar.org",
                s2_min_interval(has_s2_api_key),
            ),
            policy(
                "kegg",
                "BIOMCP_KEGG_BASE",
                "https://rest.kegg.jp",
                Duration::from_millis(334),
            ),
        ];
        Self::new(policies, Duration::from_millis(100))
    }

    pub(crate) fn new(policies: Vec<RateLimitPolicy>, default_min_interval: Duration) -> Self {
        Self {
            policies,
            default_min_interval,
            last_seen: Mutex::new(HashMap::new()),
        }
    }

    fn resolve_key_and_interval(&self, url: &Url) -> (String, Duration) {
        let full = url.as_str();

        if let Some(policy) = self
            .policies
            .iter()
            .filter(|p| full.starts_with(p.prefix.as_ref()))
            .max_by_key(|p| p.prefix.len())
        {
            return (format!("policy:{}", policy.key), policy.min_interval);
        }

        let origin = format!(
            "{}://{}",
            url.scheme(),
            url.host_str().unwrap_or("unknown-host")
        );
        (format!("default:{origin}"), self.default_min_interval)
    }

    pub(crate) async fn wait_for_url(&self, url: &Url) {
        let (key, min_interval) = self.resolve_key_and_interval(url);
        loop {
            let now = Instant::now();
            let mut map = self.last_seen.lock().await;
            let wait_until = map.get(&key).map(|last| *last + min_interval);

            match wait_until {
                Some(target) if target > now => {
                    drop(map);
                    sleep_until(target).await;
                }
                _ => {
                    map.insert(key, now);
                    return;
                }
            }
        }
    }

    #[cfg(test)]
    fn resolve_key_for_str(&self, raw: &str) -> Option<String> {
        let url = Url::parse(raw).ok()?;
        Some(self.resolve_key_and_interval(&url).0)
    }
}

fn pubtator_min_interval(has_ncbi_api_key: bool) -> Duration {
    if has_ncbi_api_key {
        Duration::from_millis(100)
    } else {
        Duration::from_millis(334)
    }
}

fn pubmed_eutils_min_interval(has_ncbi_api_key: bool) -> Duration {
    if has_ncbi_api_key {
        Duration::from_millis(100)
    } else {
        Duration::from_millis(334)
    }
}

fn s2_min_interval(has_s2_api_key: bool) -> Duration {
    if has_s2_api_key {
        Duration::from_secs(1)
    } else {
        Duration::from_secs(2)
    }
}

fn policy(
    key: &'static str,
    env_var: &'static str,
    default_prefix: &'static str,
    min_interval: Duration,
) -> RateLimitPolicy {
    RateLimitPolicy {
        key,
        prefix: crate::sources::env_base(default_prefix, env_var),
        min_interval,
    }
}

static GLOBAL_RATE_LIMITER: OnceLock<Arc<RateLimiter>> = OnceLock::new();

pub(crate) fn global_limiter() -> Arc<RateLimiter> {
    GLOBAL_RATE_LIMITER
        .get_or_init(|| Arc::new(RateLimiter::from_env()))
        .clone()
}

#[derive(Clone, Debug)]
pub(crate) struct RateLimitMiddleware {
    limiter: Arc<RateLimiter>,
}

impl RateLimitMiddleware {
    pub(crate) fn new() -> Self {
        Self {
            limiter: global_limiter(),
        }
    }
}

#[async_trait::async_trait]
impl Middleware for RateLimitMiddleware {
    async fn handle(
        &self,
        req: reqwest::Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> reqwest_middleware::Result<reqwest::Response> {
        self.limiter.wait_for_url(req.url()).await;
        next.run(req, extensions).await
    }
}

pub(crate) async fn wait_for_url_str(raw: &str) {
    if let Ok(url) = Url::parse(raw) {
        global_limiter().wait_for_url(&url).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::set_env_var;
    use tokio::sync::MutexGuard;

    fn env_lock() -> MutexGuard<'static, ()> {
        crate::test_support::env_lock().blocking_lock()
    }

    fn test_policy(key: &'static str, prefix: &str, ms: u64) -> RateLimitPolicy {
        RateLimitPolicy {
            key,
            prefix: Cow::Owned(prefix.to_string()),
            min_interval: Duration::from_millis(ms),
        }
    }

    #[tokio::test]
    async fn rate_limit_blocks_second_request_for_same_prefix() {
        let limiter = RateLimiter::new(
            vec![test_policy("strict", "https://api.example.org/strict", 120)],
            Duration::from_millis(1),
        );

        let url = Url::parse("https://api.example.org/strict/resource").unwrap();
        let start = Instant::now();
        limiter.wait_for_url(&url).await;
        limiter.wait_for_url(&url).await;

        assert!(
            start.elapsed() >= Duration::from_millis(100),
            "second request should be throttled for strict prefix"
        );
    }

    #[tokio::test]
    async fn rate_limit_keeps_same_host_prefixes_independent() {
        let limiter = RateLimiter::new(
            vec![
                test_policy("a", "https://www.ebi.ac.uk/europepmc/webservices/rest", 100),
                test_policy("b", "https://www.ebi.ac.uk/chembl/api/data", 100),
            ],
            Duration::from_millis(1),
        );

        let url_a = Url::parse("https://www.ebi.ac.uk/europepmc/webservices/rest/search").unwrap();
        let url_b = Url::parse("https://www.ebi.ac.uk/chembl/api/data/molecule").unwrap();

        let start = Instant::now();
        limiter.wait_for_url(&url_a).await;
        limiter.wait_for_url(&url_b).await;

        assert!(
            start.elapsed() < Duration::from_millis(80),
            "same host, different prefixes should not block each other"
        );
    }

    #[tokio::test]
    async fn rate_limit_uses_default_policy_for_unknown_prefix() {
        let limiter = RateLimiter::new(Vec::new(), Duration::from_millis(80));
        let url = Url::parse("https://unknown.example.org/path").unwrap();

        let start = Instant::now();
        limiter.wait_for_url(&url).await;
        limiter.wait_for_url(&url).await;

        assert!(
            start.elapsed() >= Duration::from_millis(65),
            "default policy should throttle unknown prefixes"
        );
    }

    #[test]
    fn rate_limit_uses_longest_matching_prefix() {
        let limiter = RateLimiter::new(
            vec![
                test_policy("short", "https://example.org/api", 10),
                test_policy("long", "https://example.org/api/v1", 10),
            ],
            Duration::from_millis(1),
        );

        let key = limiter
            .resolve_key_for_str("https://example.org/api/v1/resource")
            .unwrap();
        assert_eq!(key, "policy:long");
    }

    #[test]
    fn pubtator_interval_uses_key_aware_values() {
        assert_eq!(pubtator_min_interval(false), Duration::from_millis(334));
        assert_eq!(pubtator_min_interval(true), Duration::from_millis(100));
    }

    #[test]
    fn pubmed_eutils_interval_uses_key_aware_values() {
        assert_eq!(
            pubmed_eutils_min_interval(false),
            Duration::from_millis(334)
        );
        assert_eq!(pubmed_eutils_min_interval(true), Duration::from_millis(100));
    }

    #[test]
    fn semantic_scholar_policy_uses_two_second_interval_without_api_key() {
        let _lock = env_lock();
        let _env = set_env_var("S2_API_KEY", None);
        let limiter = RateLimiter::from_env();
        let policy = limiter
            .policies
            .iter()
            .find(|policy| policy.key == "semantic-scholar")
            .expect("semantic-scholar policy should be registered");
        assert_eq!(policy.min_interval, Duration::from_secs(2));
        assert_eq!(policy.prefix.as_ref(), "https://api.semanticscholar.org");
    }

    #[test]
    fn semantic_scholar_policy_uses_one_second_interval_with_api_key() {
        let _lock = env_lock();
        let _env = set_env_var("S2_API_KEY", Some("test-key"));
        let limiter = RateLimiter::from_env();
        let policy = limiter
            .policies
            .iter()
            .find(|policy| policy.key == "semantic-scholar")
            .expect("semantic-scholar policy should be registered");
        assert_eq!(policy.min_interval, Duration::from_secs(1));
        assert_eq!(policy.prefix.as_ref(), "https://api.semanticscholar.org");
    }

    #[test]
    fn semantic_scholar_urls_resolve_to_semantic_scholar_policy() {
        let limiter = RateLimiter::from_env();
        let key = limiter
            .resolve_key_for_str("https://api.semanticscholar.org/graph/v1/paper/PMID%3A22663011")
            .expect("semantic scholar URL should parse");
        assert_eq!(key, "policy:semantic-scholar");
    }

    #[test]
    fn kegg_urls_resolve_to_kegg_policy() {
        let limiter = RateLimiter::from_env();
        let key = limiter
            .resolve_key_for_str("https://rest.kegg.jp/find/pathway/MAPK")
            .expect("kegg URL should parse");
        assert_eq!(key, "policy:kegg");
    }

    #[test]
    fn pubmed_eutils_urls_resolve_to_pubmed_policy() {
        let limiter = RateLimiter::from_env();
        let key = limiter
            .resolve_key_for_str(
                "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esearch.fcgi?db=pubmed&term=BRAF",
            )
            .expect("pubmed E-utilities URL should parse");
        assert_eq!(key, "policy:pubmed-eutils");
    }

    #[test]
    fn nih_reporter_policy_uses_one_second_interval() {
        let limiter = RateLimiter::from_env();
        let policy = limiter
            .policies
            .iter()
            .find(|policy| policy.key == "nih-reporter")
            .expect("nih-reporter policy should be registered");
        assert_eq!(policy.min_interval, Duration::from_secs(1));
        assert_eq!(policy.prefix.as_ref(), "https://api.reporter.nih.gov/v2");
    }

    #[test]
    fn nih_reporter_urls_resolve_to_nih_reporter_policy() {
        let limiter = RateLimiter::from_env();
        let key = limiter
            .resolve_key_for_str("https://api.reporter.nih.gov/v2/projects/search")
            .expect("NIH Reporter URL should parse");
        assert_eq!(key, "policy:nih-reporter");
    }

    #[test]
    fn litsense2_policy_uses_one_second_interval() {
        let limiter = RateLimiter::from_env();
        let policy = limiter
            .policies
            .iter()
            .find(|policy| policy.key == "litsense2")
            .expect("litsense2 policy should be registered");
        assert_eq!(policy.min_interval, Duration::from_secs(1));
        assert_eq!(
            policy.prefix.as_ref(),
            "https://www.ncbi.nlm.nih.gov/research/litsense2-api/api"
        );
    }

    #[test]
    fn litsense2_urls_resolve_to_litsense2_policy() {
        let limiter = RateLimiter::from_env();
        let key = limiter
            .resolve_key_for_str(
                "https://www.ncbi.nlm.nih.gov/research/litsense2-api/api/sentences/?query=test&rerank=true",
            )
            .expect("litsense2 URL should parse");
        assert_eq!(key, "policy:litsense2");
    }
}
