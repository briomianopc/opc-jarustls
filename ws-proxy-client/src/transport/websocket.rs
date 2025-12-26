use anyhow::{Context, Result};
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async_tls_with_config, tungstenite::protocol::Message, Connector, MaybeTlsStream,
    WebSocketStream,
};
use tracing::{debug, info, warn};
use url::Url;

use super::tls::{create_tls_config, parse_server_name};
use crate::config::Config;

pub type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// WebSocket client for connecting to the proxy server
pub struct WebSocketClient {
    config: Config,
}

impl WebSocketClient {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Connect to the WebSocket server with TLS fingerprint randomization
    pub async fn connect(&self) -> Result<WsStream> {
        let server_url = self.build_server_url()?;
        info!("Connecting to WebSocket server: {}", server_url);

        // Create TLS config with fingerprint randomization
        let tls_config = create_tls_config(self.config.tls.randomize_fingerprint)?;

        // Parse server name for SNI
        let server_name = parse_server_name(&self.config.tls.sni)?;

        // Create TLS connector
        let connector = Connector::Rustls(tls_config);

        // Build WebSocket request with UUID authentication
        let mut request = server_url.into_client_request()?;
        
        // Add UUID to Sec-WebSocket-Protocol header for authentication
        request.headers_mut().insert(
            "Sec-WebSocket-Protocol",
            self.config.server.uuid.parse()?,
        );

        debug!("WebSocket request headers: {:?}", request.headers());

        // Connect with TLS
        let (ws_stream, response) = connect_async_tls_with_config(
            request,
            None,
            false,
            Some(connector),
        )
        .await
        .context("Failed to connect to WebSocket server")?;

        info!("✅ WebSocket connected successfully");
        debug!("Response status: {}", response.status());
        debug!("Response headers: {:?}", response.headers());

        Ok(ws_stream)
    }

    /// Build the WebSocket server URL
    fn build_server_url(&self) -> Result<Url> {
        let scheme = if self.config.server.port == 443 {
            "wss"
        } else {
            "ws"
        };

        let url = format!(
            "{}://{}:{}{}{}",
            scheme,
            self.config.server.address,
            self.config.server.port,
            self.config.server.ws_path,
            self.config.server.uuid
        );

        Url::parse(&url).context("Invalid server URL")
    }

    /// Send a message to the WebSocket server
    pub async fn send_message(ws: &mut WsStream, msg: Message) -> Result<()> {
        ws.send(msg)
            .await
            .context("Failed to send WebSocket message")
    }

    /// Receive a message from the WebSocket server
    pub async fn receive_message(ws: &mut WsStream) -> Result<Option<Message>> {
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
        ws: &mut WsStream,
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
    fn test_build_server_url() {
        let config = Config::default();
        let client = WebSocketClient::new(config);
        let url = client.build_server_url().unwrap();
        assert!(url.to_string().starts_with("wss://"));
    }
}
