use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rustls_pki_types::EchConfigListBytes;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use tracing::{debug, info, warn};

const ALIDNS_DOH_URL: &str = "https://dns.alidns.com/dns-query";
const CLOUDFLARE_ECH_DOMAIN: &str = "cloudflare-ech.com";

/// DNS-over-HTTPS resolver using Alibaba Cloud DoH
pub struct DohResolver {
    client: reqwest::Client,
    doh_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct DohRequest {
    name: String,
    #[serde(rename = "type")]
    record_type: String,
}

#[derive(Debug, Deserialize)]
struct DohResponse {
    #[serde(rename = "Status")]
    status: i32,
    #[serde(rename = "Answer")]
    answer: Option<Vec<DohAnswer>>,
}

#[derive(Debug, Deserialize)]
struct DohAnswer {
    name: String,
    #[serde(rename = "type")]
    record_type: u16,
    #[serde(rename = "TTL")]
    ttl: u32,
    data: String,
}

impl DohResolver {
    /// Create a new DoH resolver with default Alibaba Cloud DoH endpoint
    pub fn new() -> Result<Self> {
        Self::with_url(ALIDNS_DOH_URL)
    }

    /// Create a new DoH resolver with custom DoH endpoint
    /// 
    /// Note: Uses native-tls to avoid conflicts with local rustls fork.
    /// DoH requests don't need fingerprint randomization.
    pub fn with_url(doh_url: impl Into<String>) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            client,
            doh_url: doh_url.into(),
        })
    }

    /// Fetch ECH configuration from cloudflare-ech.com via DoH
    ///
    /// This queries the TXT record for cloudflare-ech.com which contains
    /// Cloudflare's public ECH configuration in base64 format.
    pub async fn fetch_ech_config(&self) -> Result<EchConfigListBytes<'static>> {
        info!("Fetching ECH config from {} via DoH", CLOUDFLARE_ECH_DOMAIN);

        let txt_records = self.query_txt(CLOUDFLARE_ECH_DOMAIN).await
            .context("Failed to query ECH config from DNS")?;

        if txt_records.is_empty() {
            anyhow::bail!("No TXT records found for {}", CLOUDFLARE_ECH_DOMAIN);
        }

        // Try each TXT record until we find a valid ECH config
        for (i, record) in txt_records.iter().enumerate() {
            debug!("Trying TXT record {}: {}", i + 1, record);

            // Remove quotes if present
            let record = record.trim_matches('"');

            // Try to decode as base64
            match BASE64.decode(record) {
                Ok(decoded) => {
                    info!("✅ Successfully decoded ECH config ({} bytes)", decoded.len());
                    return Ok(EchConfigListBytes::from(decoded));
                }
                Err(e) => {
                    warn!("Failed to decode TXT record {} as base64: {}", i + 1, e);
                    continue;
                }
            }
        }

        anyhow::bail!("No valid ECH config found in TXT records")
    }

    /// Query TXT records for a domain
    pub async fn query_txt(&self, domain: &str) -> Result<Vec<String>> {
        debug!("Querying TXT records for {} via DoH", domain);

        let response = self
            .client
            .get(&self.doh_url)
            .query(&[("name", domain), ("type", "TXT")])
            .header("Accept", "application/dns-json")
            .send()
            .await
            .context("DoH request failed")?;

        if !response.status().is_success() {
            anyhow::bail!("DoH request failed with status: {}", response.status());
        }

        let doh_response: DohResponse = response
            .json()
            .await
            .context("Failed to parse DoH response")?;

        if doh_response.status != 0 {
            anyhow::bail!("DoH query failed with status: {}", doh_response.status);
        }

        let answers = doh_response.answer.unwrap_or_default();
        let txt_records: Vec<String> = answers
            .into_iter()
            .filter(|a| a.record_type == 16) // TXT record type
            .map(|a| a.data)
            .collect();

        debug!("Found {} TXT records", txt_records.len());
        Ok(txt_records)
    }

    /// Resolve A/AAAA records for a domain
    ///
    /// This is used to resolve the target IP when a CNAME is provided
    pub async fn resolve_ip(&self, domain: &str) -> Result<Vec<IpAddr>> {
        debug!("Resolving IP addresses for {} via DoH", domain);

        // Try A records first
        let mut ips = Vec::new();

        // Query A records (IPv4)
        match self.query_a(domain).await {
            Ok(addrs) => ips.extend(addrs),
            Err(e) => warn!("Failed to query A records: {}", e),
        }

        // Query AAAA records (IPv6)
        match self.query_aaaa(domain).await {
            Ok(addrs) => ips.extend(addrs),
            Err(e) => warn!("Failed to query AAAA records: {}", e),
        }

        if ips.is_empty() {
            anyhow::bail!("No IP addresses found for {}", domain);
        }

        info!("Resolved {} to {} IP address(es)", domain, ips.len());
        Ok(ips)
    }

    /// Query A records (IPv4)
    async fn query_a(&self, domain: &str) -> Result<Vec<IpAddr>> {
        let response = self
            .client
            .get(&self.doh_url)
            .query(&[("name", domain), ("type", "A")])
            .header("Accept", "application/dns-json")
            .send()
            .await?;

        let doh_response: DohResponse = response.json().await?;

        if doh_response.status != 0 {
            anyhow::bail!("DoH A query failed with status: {}", doh_response.status);
        }

        let answers = doh_response.answer.unwrap_or_default();
        let ips: Vec<IpAddr> = answers
            .into_iter()
            .filter(|a| a.record_type == 1) // A record type
            .filter_map(|a| a.data.parse().ok())
            .collect();

        Ok(ips)
    }

    /// Query AAAA records (IPv6)
    async fn query_aaaa(&self, domain: &str) -> Result<Vec<IpAddr>> {
        let response = self
            .client
            .get(&self.doh_url)
            .query(&[("name", domain), ("type", "AAAA")])
            .header("Accept", "application/dns-json")
            .send()
            .await?;

        let doh_response: DohResponse = response.json().await?;

        if doh_response.status != 0 {
            anyhow::bail!("DoH AAAA query failed with status: {}", doh_response.status);
        }

        let answers = doh_response.answer.unwrap_or_default();
        let ips: Vec<IpAddr> = answers
            .into_iter()
            .filter(|a| a.record_type == 28) // AAAA record type
            .filter_map(|a| a.data.parse().ok())
            .collect();

        Ok(ips)
    }
}

impl Default for DohResolver {
    fn default() -> Self {
        Self::new().expect("Failed to create default DoH resolver")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_ech_config() {
        let resolver = DohResolver::new().unwrap();
        let result = resolver.fetch_ech_config().await;
        assert!(result.is_ok(), "Failed to fetch ECH config: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_resolve_ip() {
        let resolver = DohResolver::new().unwrap();
        let ips = resolver.resolve_ip("cloudflare.com").await.unwrap();
        assert!(!ips.is_empty());
    }
}
