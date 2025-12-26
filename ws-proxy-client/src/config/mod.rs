use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub tls: TlsConfig,
    pub proxy: ProxyConfig,
    pub multiplexing: MultiplexingConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub address: String,
    pub port: u16,
    pub uuid: String,
    pub ws_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub randomize_fingerprint: bool,
    pub enable_ech: bool,
    pub sni: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub socks5_enabled: bool,
    pub socks5_bind: String,
    pub socks5_port: u16,
    pub http_enabled: bool,
    pub http_bind: String,
    pub http_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiplexingConfig {
    pub max_streams: usize,
    pub window_size: u32,
    pub keep_alive_interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .context("Failed to read config file")?;
        let config: Config = toml::from_str(&content)
            .context("Failed to parse config file")?;
        Ok(config)
    }

    pub fn default() -> Self {
        Self {
            server: ServerConfig {
                address: "example.com".to_string(),
                port: 443,
                uuid: "d342d11e-d424-4583-b36e-524ab1f0afa4".to_string(),
                ws_path: "/".to_string(),
            },
            tls: TlsConfig {
                randomize_fingerprint: true,
                enable_ech: true,
                sni: "example.com".to_string(),
            },
            proxy: ProxyConfig {
                socks5_enabled: true,
                socks5_bind: "127.0.0.1".to_string(),
                socks5_port: 1080,
                http_enabled: true,
                http_bind: "127.0.0.1".to_string(),
                http_port: 8080,
            },
            multiplexing: MultiplexingConfig {
                max_streams: 256,
                window_size: 1048576,
                keep_alive_interval: 30,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
            },
        }
    }
}
