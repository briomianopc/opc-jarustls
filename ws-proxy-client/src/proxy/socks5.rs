use anyhow::{Context, Result};
use bytes::{Buf, BufMut, BytesMut};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info, warn};

use crate::config::ProxyConfig;

const SOCKS5_VERSION: u8 = 0x05;
const SOCKS5_AUTH_NONE: u8 = 0x00;
const SOCKS5_CMD_CONNECT: u8 = 0x01;
const SOCKS5_ATYP_IPV4: u8 = 0x01;
const SOCKS5_ATYP_DOMAIN: u8 = 0x03;
const SOCKS5_ATYP_IPV6: u8 = 0x04;
const SOCKS5_REP_SUCCESS: u8 = 0x00;
const SOCKS5_REP_GENERAL_FAILURE: u8 = 0x01;

#[derive(Debug, Clone)]
pub enum TargetAddr {
    Ip(SocketAddr),
    Domain(String, u16),
}

impl TargetAddr {
    pub fn host(&self) -> String {
        match self {
            TargetAddr::Ip(addr) => addr.ip().to_string(),
            TargetAddr::Domain(domain, _) => domain.clone(),
        }
    }

    pub fn port(&self) -> u16 {
        match self {
            TargetAddr::Ip(addr) => addr.port(),
            TargetAddr::Domain(_, port) => *port,
        }
    }
}

/// SOCKS5 proxy server
pub struct Socks5Server {
    config: ProxyConfig,
}

impl Socks5Server {
    pub fn new(config: ProxyConfig) -> Self {
        Self { config }
    }

    /// Start the SOCKS5 proxy server
    pub async fn run<F, Fut>(self, handler: F) -> Result<()>
    where
        F: Fn(TcpStream, TargetAddr) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        let bind_addr = format!("{}:{}", self.config.socks5_bind, self.config.socks5_port);
        let listener = TcpListener::bind(&bind_addr)
            .await
            .context("Failed to bind SOCKS5 server")?;

        info!("🚀 SOCKS5 proxy listening on {}", bind_addr);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    debug!("New SOCKS5 connection from {}", addr);
                    let handler = handler.clone();
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_client(stream, handler).await {
                            error!("SOCKS5 client error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept SOCKS5 connection: {}", e);
                }
            }
        }
    }

    /// Handle a SOCKS5 client connection
    async fn handle_client<F, Fut>(mut stream: TcpStream, handler: F) -> Result<()>
    where
        F: Fn(TcpStream, TargetAddr) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        // Step 1: Authentication negotiation
        Self::handle_auth(&mut stream).await?;

        // Step 2: Request parsing
        let target = Self::handle_request(&mut stream).await?;

        debug!("SOCKS5 target: {}:{}", target.host(), target.port());

        // Step 3: Send success response
        Self::send_response(&mut stream, SOCKS5_REP_SUCCESS).await?;

        // Step 4: Forward to handler
        handler(stream, target).await?;

        Ok(())
    }

    /// Handle SOCKS5 authentication
    async fn handle_auth(stream: &mut TcpStream) -> Result<()> {
        let mut buf = [0u8; 2];
        stream.read_exact(&mut buf).await?;

        let version = buf[0];
        let nmethods = buf[1];

        if version != SOCKS5_VERSION {
            anyhow::bail!("Unsupported SOCKS version: {}", version);
        }

        // Read methods
        let mut methods = vec![0u8; nmethods as usize];
        stream.read_exact(&mut methods).await?;

        // We only support no authentication
        if !methods.contains(&SOCKS5_AUTH_NONE) {
            stream.write_all(&[SOCKS5_VERSION, 0xFF]).await?;
            anyhow::bail!("No acceptable authentication method");
        }

        // Send auth response
        stream
            .write_all(&[SOCKS5_VERSION, SOCKS5_AUTH_NONE])
            .await?;

        Ok(())
    }

    /// Handle SOCKS5 request
    async fn handle_request(stream: &mut TcpStream) -> Result<TargetAddr> {
        let mut buf = [0u8; 4];
        stream.read_exact(&mut buf).await?;

        let version = buf[0];
        let cmd = buf[1];
        let atyp = buf[3];

        if version != SOCKS5_VERSION {
            anyhow::bail!("Invalid SOCKS version in request: {}", version);
        }

        if cmd != SOCKS5_CMD_CONNECT {
            anyhow::bail!("Unsupported SOCKS command: {}", cmd);
        }

        // Parse target address
        let target = match atyp {
            SOCKS5_ATYP_IPV4 => {
                let mut addr = [0u8; 4];
                stream.read_exact(&mut addr).await?;
                let mut port_buf = [0u8; 2];
                stream.read_exact(&mut port_buf).await?;
                let port = u16::from_be_bytes(port_buf);
                
                let ip = Ipv4Addr::new(addr[0], addr[1], addr[2], addr[3]);
                TargetAddr::Ip(SocketAddr::from((ip, port)))
            }
            SOCKS5_ATYP_IPV6 => {
                let mut addr = [0u8; 16];
                stream.read_exact(&mut addr).await?;
                let mut port_buf = [0u8; 2];
                stream.read_exact(&mut port_buf).await?;
                let port = u16::from_be_bytes(port_buf);
                
                let ip = Ipv6Addr::from(addr);
                TargetAddr::Ip(SocketAddr::from((ip, port)))
            }
            SOCKS5_ATYP_DOMAIN => {
                let mut len_buf = [0u8; 1];
                stream.read_exact(&mut len_buf).await?;
                let len = len_buf[0] as usize;
                
                let mut domain = vec![0u8; len];
                stream.read_exact(&mut domain).await?;
                
                let mut port_buf = [0u8; 2];
                stream.read_exact(&mut port_buf).await?;
                let port = u16::from_be_bytes(port_buf);
                
                let domain = String::from_utf8(domain)
                    .context("Invalid domain name")?;
                
                TargetAddr::Domain(domain, port)
            }
            _ => anyhow::bail!("Unsupported address type: {}", atyp),
        };

        Ok(target)
    }

    /// Send SOCKS5 response
    async fn send_response(stream: &mut TcpStream, rep: u8) -> Result<()> {
        let mut response = BytesMut::with_capacity(10);
        response.put_u8(SOCKS5_VERSION);
        response.put_u8(rep);
        response.put_u8(0x00); // Reserved
        response.put_u8(SOCKS5_ATYP_IPV4);
        response.put_u32(0); // Bind address (0.0.0.0)
        response.put_u16(0); // Bind port (0)

        stream.write_all(&response).await?;
        Ok(())
    }
}
