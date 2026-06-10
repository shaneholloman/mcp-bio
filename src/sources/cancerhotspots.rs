use std::borrow::Cow;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::BioMcpError;

const CANCERHOTSPOTS_BASE: &str = "https://www.cancerhotspots.org";
const CANCERHOTSPOTS_API: &str = "cancerhotspots.org";
const CANCERHOTSPOTS_BASE_ENV: &str = "BIOMCP_CANCERHOTSPOTS_BASE";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancerHotspotRecurrence {
    pub source: String,
    pub position_count: Option<u32>,
    pub same_aa_count: Option<u32>,
    pub matched_transcript: Option<String>,
}

impl CancerHotspotRecurrence {
    fn checked_absent() -> Self {
        Self {
            source: CANCERHOTSPOTS_API.to_string(),
            position_count: None,
            same_aa_count: None,
            matched_transcript: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancerHotspotRow {
    #[allow(dead_code)]
    pub hugo_symbol: Option<String>,
    pub residue: Option<String>,
    pub tumor_count: Option<u32>,
    pub transcript_id: Option<String>,
    #[allow(dead_code)]
    pub amino_acid_position: Option<serde_json::Value>,
    #[serde(default)]
    pub variant_amino_acid: HashMap<String, u32>,
}

pub struct CancerHotspotsClient {
    client: reqwest_middleware::ClientWithMiddleware,
    base: Cow<'static, str>,
}

impl CancerHotspotsClient {
    pub fn new() -> Result<Self, BioMcpError> {
        Ok(Self {
            client: crate::sources::shared_client()?,
            base: crate::sources::env_base(CANCERHOTSPOTS_BASE, CANCERHOTSPOTS_BASE_ENV),
        })
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(base: String) -> Result<Self, BioMcpError> {
        Ok(Self {
            client: crate::sources::test_client()?,
            base: Cow::Owned(base),
        })
    }

    fn endpoint(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.base.as_ref().trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    pub(crate) fn by_gene_url(&self, gene: &str) -> String {
        self.endpoint(&format!(
            "api/hotspots/single/byGene/{}",
            encode_path_segment(gene.trim())
        ))
    }

    pub async fn by_gene(&self, gene: &str) -> Result<Vec<CancerHotspotRow>, BioMcpError> {
        let url = self.by_gene_url(gene);
        let resp = crate::sources::apply_cache_mode(self.client.get(&url))
            .send()
            .await?;
        let status = resp.status();
        let content_type = resp.headers().get(reqwest::header::CONTENT_TYPE).cloned();
        let bytes = crate::sources::read_limited_body(resp, CANCERHOTSPOTS_API).await?;
        if !status.is_success() {
            let excerpt = crate::sources::summarize_http_error_body(content_type.as_ref(), &bytes);
            return Err(BioMcpError::Api {
                api: CANCERHOTSPOTS_API.to_string(),
                message: format!("HTTP {status}: {excerpt}"),
            });
        }
        crate::sources::ensure_json_content_type(
            CANCERHOTSPOTS_API,
            content_type.as_ref(),
            &bytes,
        )?;
        serde_json::from_slice(&bytes).map_err(|source| BioMcpError::ApiJson {
            api: CANCERHOTSPOTS_API.to_string(),
            source,
        })
    }
}

fn encode_path_segment(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

pub(crate) fn recurrence_for_change(
    rows: &[CancerHotspotRow],
    normalized_change: &str,
) -> CancerHotspotRecurrence {
    let Some((requested_residue, requested_alt)) = residue_and_alt(normalized_change) else {
        return CancerHotspotRecurrence::checked_absent();
    };

    rows.iter()
        .find(|row| {
            row.residue
                .as_deref()
                .map(normalize_residue)
                .is_some_and(|residue| residue == requested_residue)
        })
        .and_then(|row| {
            let same_aa_count = same_aa_count(row, &requested_alt)?;
            Some(CancerHotspotRecurrence {
                source: CANCERHOTSPOTS_API.to_string(),
                position_count: row.tumor_count,
                same_aa_count: Some(same_aa_count),
                matched_transcript: row
                    .transcript_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(str::to_string),
            })
        })
        .unwrap_or_else(CancerHotspotRecurrence::checked_absent)
}

fn same_aa_count(row: &CancerHotspotRow, requested_alt: &str) -> Option<u32> {
    row.variant_amino_acid
        .get(requested_alt)
        .copied()
        .or_else(|| {
            row.variant_amino_acid
                .get(&requested_alt.to_ascii_uppercase())
                .copied()
        })
}

fn normalize_residue(value: &str) -> String {
    value.trim().to_ascii_uppercase()
}

fn residue_and_alt(normalized_change: &str) -> Option<(String, String)> {
    let change = normalized_change.trim().trim_start_matches("p.");
    let alt = change.chars().last()?;
    if !(alt.is_ascii_alphabetic() || alt == '*') {
        return None;
    }
    let residue = &change[..change.len() - alt.len_utf8()];
    if residue.is_empty() || !residue.chars().any(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some((normalize_residue(residue), alt.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn by_gene_request_uses_encoded_path_and_no_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/hotspots/single/byGene/ALK%20FUSION"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .expect(1)
            .mount(&server)
            .await;

        let client = CancerHotspotsClient::new_for_test(server.uri()).unwrap();
        let rows = client.by_gene("ALK FUSION").await.unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn recurrence_maps_counts_and_transcript_for_exact_alt() {
        let rows: Vec<CancerHotspotRow> = serde_json::from_value(serde_json::json!([
            {
                "hugoSymbol": "BRAF",
                "residue": "V600",
                "tumorCount": 897,
                "transcriptId": "ENST00000288602",
                "aminoAcidPosition": 600,
                "variantAminoAcid": {"E": 833, "K": 64}
            }
        ]))
        .unwrap();

        let recurrence = recurrence_for_change(&rows, "V600E");
        assert_eq!(recurrence.source, "cancerhotspots.org");
        assert_eq!(recurrence.position_count, Some(897));
        assert_eq!(recurrence.same_aa_count, Some(833));
        assert_eq!(
            recurrence.matched_transcript.as_deref(),
            Some("ENST00000288602")
        );
    }

    #[test]
    fn recurrence_serializes_checked_absence_with_nulls() {
        let recurrence = recurrence_for_change(&[], "G12D");
        let json = serde_json::to_value(&recurrence).unwrap();
        assert_eq!(json["source"], "cancerhotspots.org");
        assert!(json.get("position_count").is_some());
        assert!(json["position_count"].is_null());
        assert!(json["same_aa_count"].is_null());
        assert!(json["matched_transcript"].is_null());
    }

    #[test]
    fn recurrence_treats_missing_exact_alt_as_checked_absence() {
        let rows: Vec<CancerHotspotRow> = serde_json::from_value(serde_json::json!([
            {
                "hugoSymbol": "KRAS",
                "residue": "G12",
                "tumorCount": 100,
                "transcriptId": "ENST00000256078",
                "aminoAcidPosition": 12,
                "variantAminoAcid": {"D": 25}
            }
        ]))
        .unwrap();

        let json = serde_json::to_value(recurrence_for_change(&rows, "G12V")).unwrap();
        assert!(json["position_count"].is_null());
        assert!(json["same_aa_count"].is_null());
        assert!(json["matched_transcript"].is_null());
    }
}
