mod doh;
mod proxy;
mod tls;
mod tunnel;

use anyhow::{Context, Result};
use clap::Parser;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use doh::DohResolver;
use proxy::{HttpProxyServer, Socks5Server, TargetAddr};
use tls::TlsConfigBuilder;
use tunnel::{TunnelClient, TunnelConfig, MultiplexConfig, YamuxMultiplexer};

#[derive(Parser, Debug)]
#[command(name = "proxy-core")]
#[command(about = "High-performance proxy client with ECH and CDN optimization", long_about = None)]
struct Args {
    /// Server domain (for SNI and certificate validation)
    #[arg(long, env = "SERVER_DOMAIN")]
    server_domain: String,

    /// Target IP or CNAME (CDN optimized IP, optional)
    /// If not specified, will connect directly to server-domain
    #[arg(long, env = "TARGET")]
    target: Option<String>,

    /// Server port
    #[arg(long, env = "PORT", default_value = "443")]
    port: u16,

    /// WebSocket path
    #[arg(long, env = "WS_PATH", default_value = "/")]
    ws_path: String,

    /// UUID for authentication
    #[arg(long, env = "UUID")]
    uuid: Option<String>,

    /// Enable ECH (Encrypted Client Hello)
    #[arg(long, env = "ENABLE_ECH", default_value = "true")]
    enable_ech: bool,

    /// Enable TLS fingerprint randomization
    #[arg(long, env = "ENABLE_FINGERPRINT_RANDOMIZATION", default_value = "true")]
    enable_fingerprint_randomization: bool,

    /// Custom DoH server URL
    #[arg(long, env = "DOH_URL")]
    doh_url: Option<String>,

    /// SOCKS5 bind address
    #[arg(long, env = "SOCKS5_BIND", default_value = "127.0.0.1:1080")]
    socks5_bind: String,

    /// HTTP proxy bind address
    #[arg(long, env = "HTTP_BIND", default_value = "127.0.0.1:8080")]
    http_bind: String,

    /// Enable SOCKS5 proxy
    #[arg(long, env = "ENABLE_SOCKS5", default_value = "true")]
    enable_socks5: bool,

    /// Enable HTTP proxy
    #[arg(long, env = "ENABLE_HTTP", default_value = "true")]
    enable_http: bool,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, env = "LOG_LEVEL", default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(&args.log_level)),
        )
        .init();

    info!("🚀 Proxy Core starting...");
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("Configuration:");
    info!("  Server Domain: {}", args.server_domain);
    
    if let Some(ref target) = args.target {
        info!("  Target (CDN optimized): {}", target);
    } else {
        info!("  Target: {} (direct connection)", args.server_domain);
    }
    
    info!("  Port: {}", args.port);
    info!("  WebSocket Path: {}", args.ws_path);
    info!("  UUID: {}", args.uuid.as_deref().unwrap_or("(none)"));
    info!("  ECH: {}", if args.enable_ech { "enabled" } else { "disabled" });
    info!("  Fingerprint Randomization: {}", 
          if args.enable_fingerprint_randomization { "enabled" } else { "disabled" });
    info!("  SOCKS5: {} ({})", args.socks5_bind, 
          if args.enable_socks5 { "enabled" } else { "disabled" });
    info!("  HTTP: {} ({})", args.http_bind, 
          if args.enable_http { "enabled" } else { "disabled" });
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Build TLS configuration
    info!("Building TLS configuration...");
    let tls_config = if args.enable_ech {
        TlsConfigBuilder::new()
            .with_ech(true)
            .with_fingerprint_randomization(args.enable_fingerprint_randomization)
            .fetch_ech_config()
            .await?
            .build()?
    } else {
        TlsConfigBuilder::new()
            .with_ech(false)
            .with_fingerprint_randomization(args.enable_fingerprint_randomization)
            .build()?
    };

    info!("✅ TLS configuration built");

    // Build tunnel configuration
    let mut tunnel_config = if let Some(target) = args.target.clone() {
        TunnelConfig::with_target(
            args.server_domain.clone(),
            target,
            args.port,
        )
    } else {
        TunnelConfig::new(
            args.server_domain.clone(),
            args.port,
        )
    }
    .with_ws_path(args.ws_path.clone());
    
    // Only add UUID if provided (avoid empty string)
    if let Some(uuid) = args.uuid.clone() {
        if !uuid.is_empty() {
            tunnel_config = tunnel_config.with_uuid(uuid);
        }
    }
    
    let tunnel_config = tunnel_config;

    // Create tunnel client
    let tunnel_client = Arc::new(
        TunnelClient::new(tunnel_config.clone(), tls_config.clone())?
    );

    // Connect to server
    info!("Connecting to server...");
    let ws_stream = tunnel_client.connect().await?;
    info!("✅ Connected to server");

    // Create Yamux multiplexer
    info!("Initializing Yamux multiplexer...");
    let multiplex_config = MultiplexConfig::default();
    let yamux = Arc::new(Mutex::new(
        YamuxMultiplexer::new(ws_stream, &multiplex_config)?
    ));
    info!("✅ Yamux multiplexer initialized");

    // Start proxy servers
    let mut tasks = Vec::new();

    // Start SOCKS5 proxy
    if args.enable_socks5 {
        let yamux = yamux.clone();
        let socks5_server = Socks5Server::new(args.socks5_bind.clone());
        
        tasks.push(tokio::spawn(async move {
            socks5_server
                .run(move |client_stream, target| {
                    let yamux = yamux.clone();
                    async move {
                        handle_proxy_connection(client_stream, target, None, yamux).await
                    }
                })
                .await
        }));
    }

    // Start HTTP proxy
    if args.enable_http {
        let yamux = yamux.clone();
        let http_server = HttpProxyServer::new(args.http_bind.clone());
        
        tasks.push(tokio::spawn(async move {
            http_server
                .run(move |client_stream, target, initial_data| {
                    let yamux = yamux.clone();
                    async move {
                        handle_proxy_connection(client_stream, target, initial_data, yamux).await
                    }
                })
                .await
        }));
    }

    info!("✅ All proxy servers started");
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("Proxy Core is running. Press Ctrl+C to stop.");
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Wait for all tasks
    for task in tasks {
        if let Err(e) = task.await {
            error!("Task error: {}", e);
        }
    }

    Ok(())
}

/// Handle a proxy connection through Yamux stream
async fn handle_proxy_connection(
    mut client_stream: TcpStream,
    target: TargetAddr,
    initial_data: Option<Vec<u8>>,
    yamux: Arc<Mutex<YamuxMultiplexer>>,
) -> Result<()> {
    debug!("Handling proxy connection to {}:{}", target.host(), target.port());

    // Open a new Yamux stream
    let mut yamux_stream = {
        let mut yamux_guard = yamux.lock().await;
        yamux_guard.open_stream().await?
    };

    debug!("✅ Yamux stream opened for {}:{}", target.host(), target.port());

    // Send CONNECT command through Yamux stream
    use tokio::io::AsyncWriteExt;
    let connect_msg = if let Some(data) = &initial_data {
        format!(
            "CONNECT:{}:{}|{}",
            target.host(),
            target.port(),
            String::from_utf8_lossy(data)
        )
    } else {
        format!("CONNECT:{}:{}|", target.host(), target.port())
    };

    yamux_stream.write_all(connect_msg.as_bytes()).await?;
    debug!("Sent CONNECT command");

    // Wait for CONNECTED response
    use tokio::io::AsyncReadExt;
    let mut response = vec![0u8; 9]; // "CONNECTED"
    yamux_stream.read_exact(&mut response).await?;
    
    if response != b"CONNECTED" {
        anyhow::bail!("Failed to establish tunnel: unexpected response");
    }

    debug!("✅ Tunnel established for {}:{}", target.host(), target.port());

    // Bidirectional copy between client and Yamux stream
    match tokio::io::copy_bidirectional(&mut client_stream, &mut yamux_stream).await {
        Ok((client_to_server, server_to_client)) => {
            debug!(
                "Connection closed for {}:{} - {} bytes sent, {} bytes received",
                target.host(), target.port(), client_to_server, server_to_client
            );
        }
        Err(e) => {
            warn!("Connection error for {}:{}: {}", target.host(), target.port(), e);
        }
    }

    Ok(())
}
