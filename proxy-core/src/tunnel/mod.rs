pub mod yamux;

use anyhow::{Context, Result};
use futures::{SinkExt, StreamExt};
use rustls::pki_types::ServerName;
use rustls::ClientConfig;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, client_async_tls_with_config};
use tungstenite::protocol::Message;
use tracing::{debug, info, warn};
use url::Url;

use crate::doh::DohResolver;
pub use yamux::{MultiplexConfig, YamuxMultiplexer};

/// WebSocket tunnel configuration
#[derive(Debug, Clone)]
pub struct TunnelConfig {
    /// Real server domain (for SNI and certificate validation)
    pub server_domain: String,
    
    /// Target IP or CNAME for connection (CDN optimized IP)
    /// If None, will connect directly to server_domain
    pub target: Option<String>,
    
    /// Server port (usually 443)
    pub port: u16,
    
    /// WebSocket path (default: "/")
    pub ws_path: String,
    
    /// UUID for authentication
    pub uuid: Option<String>,
}

impl TunnelConfig {
    /// Create a new tunnel config without CDN optimization
    pub fn new(server_domain: String, port: u16) -> Self {
        Self {
            server_domain,
            target: None,
            port,
            ws_path: "/".to_string(),
            uuid: None,
        }
    }

    /// Create a new tunnel config with CDN optimized IP/CNAME
    pub fn with_target(server_domain: String, target: String, port: u16) -> Self {
        Self {
            server_domain,
            target: Some(target),
            port,
            ws_path: "/".to_string(),
            uuid: None,
        }
    }

    pub fn with_ws_path(mut self, path: String) -> Self {
        self.ws_path = path;
        self
    }

    pub fn with_uuid(mut self, uuid: String) -> Self {
        self.uuid = Some(uuid);
        self
    }
}

/// WebSocket tunnel client
pub struct TunnelClient {
    config: TunnelConfig,
    tls_config: Arc<ClientConfig>,
    doh_resolver: DohResolver,
}

impl TunnelClient {
    /// Create a new tunnel client
    pub fn new(config: TunnelConfig, tls_config: Arc<ClientConfig>) -> Result<Self> {
        let doh_resolver = DohResolver::new()?;
        
        Ok(Self {
            config,
            tls_config,
            doh_resolver,
        })
    }

    /// Connect to the server
    ///
    /// If target is specified, implements CDN optimization:
    /// 1. Resolve target (optimized IP or CNAME) to actual IP
    /// 2. Connect TCP to the resolved IP
    /// 3. TLS handshake with SNI = server_domain
    /// 4. Certificate validation against server_domain
    /// 5. WebSocket upgrade with Host = server_domain
    ///
    /// If target is None, connects directly to server_domain
    pub async fn connect(&self) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
        info!("🚀 Connecting to tunnel...");
        info!("  Server Domain: {}", self.config.server_domain);
        
        if let Some(ref target) = self.config.target {
            info!("  Target (CDN optimized): {}", target);
        } else {
            info!("  Target: {} (direct connection)", self.config.server_domain);
        }
        
        info!("  Port: {}", self.config.port);

        // Step 1: Resolve target to IP address
        let target_ip = self.resolve_target().await?;
        info!("✅ Resolved to: {}", target_ip);

        // Step 2: Establish TCP connection to the target IP
        let tcp_stream = self.connect_tcp(target_ip).await?;
        info!("✅ TCP connection established");

        // Step 3: TLS handshake with SNI = server_domain
        let tls_stream = self.tls_handshake(tcp_stream).await?;
        info!("✅ TLS handshake completed (SNI: {})", self.config.server_domain);

        // Step 4: WebSocket upgrade with Host = server_domain
        let ws_stream = self.websocket_upgrade(tls_stream).await?;
        info!("✅ WebSocket connection established");

        Ok(ws_stream)
    }

    /// Resolve target to IP address
    ///
    /// If target is specified:
    /// - An IP address (1.1.1.1) - use directly
    /// - A CNAME domain (optimized.cf-cdn.com) - resolve via DoH
    ///
    /// If target is None, resolve server_domain
    async fn resolve_target(&self) -> Result<IpAddr> {
        let target_to_resolve = self.config.target.as_ref()
            .unwrap_or(&self.config.server_domain);
        
        debug!("Resolving target: {}", target_to_resolve);

        // Try to parse as IP address first
        if let Ok(ip) = target_to_resolve.parse::<IpAddr>() {
            debug!("Target is already an IP address: {}", ip);
            return Ok(ip);
        }

        // Target is a domain, resolve via DoH
        debug!("Target is a domain, resolving via DoH...");
        let ips = self.doh_resolver.resolve_ip(target_to_resolve).await
            .context("Failed to resolve target domain")?;

        if ips.is_empty() {
            anyhow::bail!("No IP addresses found for target: {}", target_to_resolve);
        }

        // Use the first IP address
        let ip = ips[0];
        debug!("Selected IP: {} (from {} candidates)", ip, ips.len());
        
        Ok(ip)
    }

    /// Establish TCP connection to the target IP
    ///
    /// CRITICAL: Connect to the target IP, NOT the server domain
    async fn connect_tcp(&self, target_ip: IpAddr) -> Result<TcpStream> {
        let addr = SocketAddr::new(target_ip, self.config.port);
        debug!("Connecting TCP to: {}", addr);

        let stream = TcpStream::connect(addr)
            .await
            .context("TCP connection failed")?;

        // Set TCP options for better performance
        stream.set_nodelay(true)?;

        Ok(stream)
    }

    /// Perform TLS handshake with SNI = server_domain
    ///
    /// CRITICAL: Use server_domain for SNI and certificate validation,
    /// even though we're connected to a different IP
    async fn tls_handshake(&self, tcp_stream: TcpStream) -> Result<tokio_rustls::client::TlsStream<TcpStream>> {
        debug!("Starting TLS handshake with SNI: {}", self.config.server_domain);

        // Parse server name for SNI
        let server_name = ServerName::try_from(self.config.server_domain.as_str())
            .context("Invalid server domain")?
            .to_owned();

        // Create TLS connector
        let connector = TlsConnector::from(self.tls_config.clone());

        // Perform TLS handshake
        // The certificate will be validated against server_domain
        let tls_stream = connector
            .connect(server_name, tcp_stream)
            .await
            .context("TLS handshake failed")?;

        debug!("TLS handshake completed successfully");
        Ok(tls_stream)
    }

    /// Upgrade to WebSocket with Host = server_domain
    ///
    /// CRITICAL: Use server_domain in Host header and URL
    async fn websocket_upgrade(
        &self,
        tls_stream: tokio_rustls::client::TlsStream<TcpStream>,
    ) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
        debug!("Upgrading to WebSocket...");

        // Build WebSocket URL with server_domain
        let ws_url = self.build_websocket_url()?;
        debug!("WebSocket URL: {}", ws_url);

        // Create WebSocket request
        let mut request = ws_url.into_client_request()?;

        // Add UUID authentication if provided
        if let Some(uuid) = &self.config.uuid {
            request.headers_mut().insert(
                "Sec-WebSocket-Protocol",
                uuid.parse()?,
            );
            debug!("Added UUID authentication header");
        }

        // Ensure Host header is set to server_domain
        request.headers_mut().insert(
            "Host",
            self.config.server_domain.parse()?,
        );

        debug!("WebSocket request headers: {:?}", request.headers());

        // Wrap TLS stream in MaybeTlsStream
        let maybe_tls_stream = MaybeTlsStream::Rustls(tls_stream);

        // Perform WebSocket handshake
        let (ws_stream, response) = client_async_tls_with_config(
            request,
            maybe_tls_stream,
            None,
            false,
        )
        .await
        .context("WebSocket upgrade failed")?;

        debug!("WebSocket upgrade completed");
        debug!("Response status: {}", response.status());
        debug!("Response headers: {:?}", response.headers());

        Ok(ws_stream)
    }

    /// Build WebSocket URL
    ///
    /// Uses wss:// scheme and server_domain as host
    fn build_websocket_url(&self) -> Result<Url> {
        let url = format!(
            "wss://{}:{}{}",
            self.config.server_domain,
            self.config.port,
            self.config.ws_path
        );

        Url::parse(&url).context("Invalid WebSocket URL")
    }

    /// Send a message through the tunnel
    pub async fn send_message(
        ws: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
        msg: Message,
    ) -> Result<()> {
        ws.send(msg)
            .await
            .context("Failed to send WebSocket message")
    }

    /// Receive a message from the tunnel
    pub async fn receive_message(
        ws: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
    ) -> Result<Option<Message>> {
        match ws.next().await {
            Some(Ok(msg)) => Ok(Some(msg)),
            Some(Err(e)) => {
                warn!("WebSocket receive error: {}", e);
                Err(e.into())
            }
            None => {
                debug!("WebSocket stream closed");
                Ok(None)
            }
        }
    }

    /// Send CONNECT command to establish tunnel
    pub async fn send_connect(
        ws: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
        target_host: &str,
        target_port: u16,
        initial_data: Option<&[u8]>,
    ) -> Result<()> {
        let connect_msg = if let Some(data) = initial_data {
            format!(
                "CONNECT:{}:{}|{}",
                target_host,
                target_port,
                String::from_utf8_lossy(data)
            )
        } else {
            format!("CONNECT:{}:{}|", target_host, target_port)
        };

        debug!("Sending CONNECT: {}:{}", target_host, target_port);
        Self::send_message(ws, Message::Text(connect_msg)).await?;

        // Wait for CONNECTED response
        match Self::receive_message(ws).await? {
            Some(Message::Text(text)) if text == "CONNECTED" => {
                debug!("✅ Tunnel established: {}:{}", target_host, target_port);
                Ok(())
            }
            Some(msg) => {
                warn!("Unexpected response: {:?}", msg);
                anyhow::bail!("Failed to establish tunnel: unexpected response")
            }
            None => anyhow::bail!("Connection closed before tunnel established"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tunnel_config_with_target() {
        let config = TunnelConfig::with_target(
            "example.com".to_string(),
            "1.1.1.1".to_string(),
            443,
        )
        .with_ws_path("/ws".to_string())
        .with_uuid("test-uuid".to_string());

        assert_eq!(config.server_domain, "example.com");
        assert_eq!(config.target, Some("1.1.1.1".to_string()));
        assert_eq!(config.port, 443);
        assert_eq!(config.ws_path, "/ws");
        assert_eq!(config.uuid, Some("test-uuid".to_string()));
    }

    #[test]
    fn test_tunnel_config_without_target() {
        let config = TunnelConfig::new("example.com".to_string(), 443)
            .with_ws_path("/ws".to_string());

        assert_eq!(config.server_domain, "example.com");
        assert_eq!(config.target, None);
        assert_eq!(config.port, 443);
    }

    #[tokio::test]
    async fn test_resolve_ip_address() {
        let config = TunnelConfig::with_target(
            "example.com".to_string(),
            "1.1.1.1".to_string(),
            443,
        );

        let tls_config = crate::tls::create_tls_config_without_ech(false).unwrap();
        let client = TunnelClient::new(config, tls_config).unwrap();

        let ip = client.resolve_target().await.unwrap();
        assert_eq!(ip.to_string(), "1.1.1.1");
    }
}
