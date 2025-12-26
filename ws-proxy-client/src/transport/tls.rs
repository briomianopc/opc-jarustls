use anyhow::{Context, Result};
use rustls::pki_types::ServerName;
use rustls::ClientConfig;
use std::sync::Arc;
use tracing::{debug, info};

/// Create a TLS client configuration with fingerprint randomization
pub fn create_tls_config(randomize_fingerprint: bool) -> Result<Arc<ClientConfig>> {
    info!("Creating TLS client config with fingerprint randomization: {}", randomize_fingerprint);
    
    // Load system root certificates
    let root_store = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.iter().cloned().collect(),
    };

    // Build client config
    let mut config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth()
        .context("Failed to build TLS client config")?;

    // Enable fingerprint randomization
    config.randomize_fingerprint = randomize_fingerprint;

    if randomize_fingerprint {
        info!("✅ TLS fingerprint randomization enabled");
        debug!("  - Cipher suites will be randomly ordered");
        debug!("  - Padding extension will be added (70% probability)");
        debug!("  - Extensions order is randomized by default");
    }

    Ok(Arc::new(config))
}

/// Parse server name for TLS SNI
pub fn parse_server_name(sni: &str) -> Result<ServerName<'static>> {
    ServerName::try_from(sni)
        .map(|name| name.to_owned())
        .context("Invalid server name for SNI")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_tls_config() {
        let config = create_tls_config(true).unwrap();
        assert!(config.randomize_fingerprint);
    }

    #[test]
    fn test_parse_server_name() {
        let name = parse_server_name("example.com").unwrap();
        assert_eq!(name.to_str(), "example.com");
    }
}
