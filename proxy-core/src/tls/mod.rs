use anyhow::{Context, Result};
use rustls::client::{EchConfig, EchMode};
use rustls::crypto::aws_lc_rs;
use rustls::pki_types::{EchConfigListBytes, ServerName};
use rustls::ClientConfig;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::doh::DohResolver;

/// TLS configuration builder with ECH support
pub struct TlsConfigBuilder {
    enable_ech: bool,
    enable_fingerprint_randomization: bool,
    ech_config: Option<EchConfigListBytes<'static>>,
}

impl TlsConfigBuilder {
    /// Create a new TLS config builder
    pub fn new() -> Self {
        Self {
            enable_ech: true,
            enable_fingerprint_randomization: true,
            ech_config: None,
        }
    }

    /// Enable or disable ECH
    pub fn with_ech(mut self, enable: bool) -> Self {
        self.enable_ech = enable;
        self
    }

    /// Enable or disable TLS fingerprint randomization
    pub fn with_fingerprint_randomization(mut self, enable: bool) -> Self {
        self.enable_fingerprint_randomization = enable;
        self
    }

    /// Set ECH configuration manually
    pub fn with_ech_config(mut self, config: EchConfigListBytes<'static>) -> Self {
        self.ech_config = Some(config);
        self
    }

    /// Fetch ECH configuration from cloudflare-ech.com via DoH
    pub async fn fetch_ech_config(mut self) -> Result<Self> {
        if !self.enable_ech {
            debug!("ECH disabled, skipping config fetch");
            return Ok(self);
        }

        info!("Fetching ECH configuration via DoH...");
        let resolver = DohResolver::new()?;
        let ech_config = resolver.fetch_ech_config().await
            .context("Failed to fetch ECH config")?;

        self.ech_config = Some(ech_config);
        info!("✅ ECH configuration fetched successfully");
        Ok(self)
    }

    /// Build the TLS client configuration
    pub fn build(self) -> Result<Arc<ClientConfig>> {
        info!("Building TLS client configuration...");
        info!("  ECH: {}", if self.enable_ech { "enabled" } else { "disabled" });
        info!("  Fingerprint randomization: {}", 
              if self.enable_fingerprint_randomization { "enabled" } else { "disabled" });

        // Load system root certificates
        let root_store = rustls::RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.iter().cloned().collect(),
        };

        // Build config with ECH if enabled and config is available
        let mut config = if self.enable_ech && self.ech_config.is_some() {
            info!("Creating ECH configuration...");
            
            // Get ECH config bytes
            let ech_config_bytes = self.ech_config.unwrap();
            
            // Create EchConfig with all supported HPKE suites from aws-lc-rs
            let ech_config = EchConfig::new(
                ech_config_bytes,
                aws_lc_rs::hpke::ALL_SUPPORTED_SUITES
            ).context("Failed to create ECH config - no compatible HPKE suite found")?;
            
            info!("✅ ECH config created successfully");
            
            // Convert EchConfig to EchMode
            let ech_mode = EchMode::from(ech_config);
            
            // Build ClientConfig with ECH using aws-lc-rs TLS 1.3 only provider
            // IMPORTANT: ECH requires TLS 1.3 only - using DEFAULT_TLS13_PROVIDER
            // which has empty tls12_cipher_suites
            ClientConfig::builder(Arc::new(aws_lc_rs::DEFAULT_TLS13_PROVIDER))
                .with_ech(ech_mode)
                .with_root_certificates(root_store)
                .with_no_client_auth()
                .context("Failed to build TLS config with ECH")?
        } else {
            // Build standard config without ECH
            if self.enable_ech {
                warn!("ECH enabled but no config available, building without ECH");
            }
            
            ClientConfig::builder(Arc::new(aws_lc_rs::default_provider()))
                .with_root_certificates(root_store)
                .with_no_client_auth()
                .context("Failed to build TLS config")?
        };

        // Enable fingerprint randomization if requested
        config.randomize_fingerprint = self.enable_fingerprint_randomization;

        if self.enable_fingerprint_randomization {
            info!("✅ TLS fingerprint randomization enabled");
        }

        if self.enable_ech && self.ech_config.is_some() {
            info!("✅ ECH fully integrated and enabled");
        }

        info!("✅ TLS client configuration built successfully");
        Ok(Arc::new(config))
    }
}

impl Default for TlsConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// Note: get_hpke_suites is no longer needed as we use ALL_SUPPORTED_SUITES directly

/// Create a TLS connector with the given configuration
pub struct TlsConnector {
    config: Arc<ClientConfig>,
}

impl TlsConnector {
    /// Create a new TLS connector with the given configuration
    pub fn new(config: Arc<ClientConfig>) -> Self {
        Self { config }
    }

    /// Get the TLS configuration
    pub fn config(&self) -> &Arc<ClientConfig> {
        &self.config
    }

    /// Create a TLS connection to the given server
    ///
    /// This is a helper method that wraps the rustls ClientConnection
    pub fn connect(
        &self,
        server_name: ServerName<'static>,
        stream: tokio::net::TcpStream,
    ) -> Result<tokio_rustls::client::TlsStream<tokio::net::TcpStream>> {
        use tokio_rustls::TlsConnector as TokioTlsConnector;

        let connector = TokioTlsConnector::from(self.config.clone());
        
        // Note: This is a synchronous wrapper - in practice you'd use the async version
        // This is just to show the API structure
        unimplemented!("Use async connect_async instead")
    }

    /// Create a TLS connection asynchronously
    pub async fn connect_async(
        &self,
        server_name: ServerName<'static>,
        stream: tokio::net::TcpStream,
    ) -> Result<tokio_rustls::client::TlsStream<tokio::net::TcpStream>> {
        use tokio_rustls::TlsConnector as TokioTlsConnector;

        let connector = TokioTlsConnector::from(self.config.clone());
        
        connector
            .connect(server_name, stream)
            .await
            .context("TLS connection failed")
    }
}

/// Helper function to create a quick TLS config with ECH
pub async fn create_tls_config_with_ech(
    enable_fingerprint_randomization: bool,
) -> Result<Arc<ClientConfig>> {
    TlsConfigBuilder::new()
        .with_ech(true)
        .with_fingerprint_randomization(enable_fingerprint_randomization)
        .fetch_ech_config()
        .await?
        .build()
}

/// Helper function to create a quick TLS config without ECH
pub fn create_tls_config_without_ech(
    enable_fingerprint_randomization: bool,
) -> Result<Arc<ClientConfig>> {
    TlsConfigBuilder::new()
        .with_ech(false)
        .with_fingerprint_randomization(enable_fingerprint_randomization)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_build_config_without_ech() {
        let config = TlsConfigBuilder::new()
            .with_ech(false)
            .build();
        assert!(config.is_ok());
    }

    #[tokio::test]
    async fn test_fetch_and_build_config_with_ech() {
        let config = TlsConfigBuilder::new()
            .with_ech(true)
            .fetch_ech_config()
            .await
            .unwrap()
            .build();
        assert!(config.is_ok());
    }
}
