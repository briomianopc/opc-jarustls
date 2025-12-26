use anyhow::{Context, Result};
use bytes::BytesMut;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info, warn};

use crate::config::ProxyConfig;
use super::socks5::TargetAddr;

/// HTTP proxy server
pub struct HttpProxyServer {
    config: ProxyConfig,
}

impl HttpProxyServer {
    pub fn new(config: ProxyConfig) -> Self {
        Self { config }
    }

    /// Start the HTTP proxy server
    pub async fn run<F, Fut>(self, handler: F) -> Result<()>
    where
        F: Fn(TcpStream, TargetAddr, Option<Vec<u8>>) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        let bind_addr = format!("{}:{}", self.config.http_bind, self.config.http_port);
        let listener = TcpListener::bind(&bind_addr)
            .await
            .context("Failed to bind HTTP proxy server")?;

        info!("🚀 HTTP proxy listening on {}", bind_addr);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    debug!("New HTTP connection from {}", addr);
                    let handler = handler.clone();
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_client(stream, handler).await {
                            error!("HTTP client error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept HTTP connection: {}", e);
                }
            }
        }
    }

    /// Handle an HTTP client connection
    async fn handle_client<F, Fut>(stream: TcpStream, handler: F) -> Result<()>
    where
        F: Fn(TcpStream, TargetAddr, Option<Vec<u8>>) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let mut reader = BufReader::new(stream);
        let mut request_line = String::new();
        
        // Read the request line
        reader.read_line(&mut request_line).await?;
        
        let parts: Vec<&str> = request_line.trim().split_whitespace().collect();
        if parts.len() < 3 {
            anyhow::bail!("Invalid HTTP request line");
        }

        let method = parts[0];
        let url = parts[1];
        let version = parts[2];

        debug!("HTTP request: {} {} {}", method, url, version);

        if method == "CONNECT" {
            // HTTPS CONNECT tunnel
            Self::handle_connect(reader, url, handler).await
        } else {
            // Regular HTTP request
            Self::handle_http(reader, method, url, version, request_line, handler).await
        }
    }

    /// Handle HTTP CONNECT method (for HTTPS)
    async fn handle_connect<F, Fut>(
        mut reader: BufReader<TcpStream>,
        url: &str,
        handler: F,
    ) -> Result<()>
    where
        F: Fn(TcpStream, TargetAddr, Option<Vec<u8>>) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        // Parse host:port
        let (host, port) = Self::parse_host_port(url)?;
        
        debug!("HTTP CONNECT to {}:{}", host, port);

        // Read and discard headers
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).await?;
            if line.trim().is_empty() {
                break;
            }
        }

        // Send 200 Connection Established
        let mut stream = reader.into_inner();
        stream
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await?;

        let target = TargetAddr::Domain(host, port);
        
        // Forward to handler (no initial data for CONNECT)
        handler(stream, target, None).await
    }

    /// Handle regular HTTP request
    async fn handle_http<F, Fut>(
        mut reader: BufReader<TcpStream>,
        method: &str,
        url: &str,
        version: &str,
        request_line: String,
        handler: F,
    ) -> Result<()>
    where
        F: Fn(TcpStream, TargetAddr, Option<Vec<u8>>) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        // Parse URL to get host
        let (host, port, path) = Self::parse_http_url(url)?;
        
        debug!("HTTP {} to {}:{}{}", method, host, port, path);

        // Read headers
        let mut headers = Vec::new();
        let mut content_length = 0usize;
        
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).await?;
            
            if line.trim().is_empty() {
                break;
            }
            
            // Check for Content-Length
            if line.to_lowercase().starts_with("content-length:") {
                if let Some(len_str) = line.split(':').nth(1) {
                    content_length = len_str.trim().parse().unwrap_or(0);
                }
            }
            
            headers.push(line);
        }

        // Reconstruct the HTTP request
        let mut request_data = BytesMut::new();
        
        // Request line (modify to use path only)
        request_data.extend_from_slice(format!("{} {} {}\r\n", method, path, version).as_bytes());
        
        // Headers
        for header in headers {
            request_data.extend_from_slice(header.as_bytes());
        }
        request_data.extend_from_slice(b"\r\n");

        // Body (if any)
        if content_length > 0 {
            let mut body = vec![0u8; content_length];
            reader.read_exact(&mut body).await?;
            request_data.extend_from_slice(&body);
        }

        let stream = reader.into_inner();
        let target = TargetAddr::Domain(host, port);
        
        // Forward to handler with initial HTTP request data
        handler(stream, target, Some(request_data.to_vec())).await
    }

    /// Parse host:port from CONNECT URL
    fn parse_host_port(url: &str) -> Result<(String, u16)> {
        let parts: Vec<&str> = url.split(':').collect();
        if parts.len() != 2 {
            anyhow::bail!("Invalid CONNECT URL: {}", url);
        }

        let host = parts[0].to_string();
        let port = parts[1].parse::<u16>()
            .context("Invalid port number")?;

        Ok((host, port))
    }

    /// Parse HTTP URL to extract host, port, and path
    fn parse_http_url(url: &str) -> Result<(String, u16, String)> {
        if url.starts_with("http://") {
            let without_scheme = &url[7..];
            let parts: Vec<&str> = without_scheme.splitn(2, '/').collect();
            
            let host_port = parts[0];
            let path = if parts.len() > 1 {
                format!("/{}", parts[1])
            } else {
                "/".to_string()
            };

            let (host, port) = if host_port.contains(':') {
                let hp: Vec<&str> = host_port.split(':').collect();
                (hp[0].to_string(), hp[1].parse::<u16>()?)
            } else {
                (host_port.to_string(), 80)
            };

            Ok((host, port, path))
        } else {
            // Relative URL, extract from Host header (handled by caller)
            anyhow::bail!("Relative URL not supported: {}", url);
        }
    }
}
