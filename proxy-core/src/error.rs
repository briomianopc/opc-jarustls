use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProxyError {
    #[error("DoH query failed: {0}")]
    DohQueryFailed(String),

    #[error("ECH config not found or invalid")]
    EchConfigInvalid,

    #[error("TLS handshake failed: {0}")]
    TlsHandshakeFailed(String),

    #[error("WebSocket upgrade failed: {0}")]
    WebSocketUpgradeFailed(String),

    #[error("Target resolution failed: {0}")]
    TargetResolutionFailed(String),

    #[error("TCP connection failed: {0}")]
    TcpConnectionFailed(String),

    #[error("SOCKS5 protocol error: {0}")]
    Socks5ProtocolError(String),

    #[error("HTTP protocol error: {0}")]
    HttpProtocolError(String),

    #[error("Tunnel error: {0}")]
    TunnelError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),
}

pub type Result<T> = std::result::Result<T, ProxyError>;
