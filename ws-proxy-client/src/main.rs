mod config;
mod proxy;
mod transport;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use config::Config;
use proxy::{HttpProxyServer, Socks5Server, TargetAddr};
use transport::{WebSocketClient, YamuxTransport};

#[derive(Parser, Debug)]
#[command(name = "ws-proxy-client")]
#[command(about = "WebSocket proxy client with TLS fingerprint randomization", long_about = None)]
struct Args {
    /// Configuration file path
    #[arg(short, long, default_value = "config.toml")]
    config: PathBuf,

    /// Override server address
    #[arg(long)]
    server: Option<String>,

    /// Override server port
    #[arg(long)]
    port: Option<u16>,

    /// Override UUID
    #[arg(long)]
    uuid: Option<String>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let log_level = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level)))
        .init();

    info!("🚀 WebSocket Proxy Client starting...");

    // Load configuration
    let mut config = if args.config.exists() {
        Config::from_file(&args.config)?
    } else {
        warn!("Config file not found, using defaults");
        Config::default()
    };

    // Apply CLI overrides
    if let Some(server) = args.server {
        config.server.address = server;
    }
    if let Some(port) = args.port {
        config.server.port = port;
    }
    if let Some(uuid) = args.uuid {
        config.server.uuid = uuid;
    }

    info!("Configuration loaded:");
    info!("  Server: {}:{}", config.server.address, config.server.port);
    info!("  UUID: {}", config.server.uuid);
    info!("  TLS Fingerprint Randomization: {}", config.tls.randomize_fingerprint);
    info!("  SOCKS5: {}:{} (enabled: {})", 
          config.proxy.socks5_bind, config.proxy.socks5_port, config.proxy.socks5_enabled);
    info!("  HTTP: {}:{} (enabled: {})", 
          config.proxy.http_bind, config.proxy.http_port, config.proxy.http_enabled);

    // Create WebSocket client
    let ws_client = Arc::new(WebSocketClient::new(config.clone()));

    // Connect to server
    info!("Connecting to WebSocket server...");
    let ws_stream = ws_client.connect().await?;
    info!("✅ Connected to server");

    // Create Yamux transport
    let yamux_transport = Arc::new(Mutex::new(
        YamuxTransport::new(ws_stream, &config.multiplexing)?
    ));

    info!("✅ Yamux multiplexing initialized");

    // Start proxy servers
    let mut tasks = Vec::new();

    // Start SOCKS5 proxy
    if config.proxy.socks5_enabled {
        let yamux = yamux_transport.clone();
        let socks5_server = Socks5Server::new(config.proxy.clone());
        
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
    if config.proxy.http_enabled {
        let yamux = yamux_transport.clone();
        let http_server = HttpProxyServer::new(config.proxy.clone());
        
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
    info!("Press Ctrl+C to stop");

    // Wait for all tasks
    for task in tasks {
        if let Err(e) = task.await {
            error!("Task error: {}", e);
        }
    }

    Ok(())
}

/// Handle a proxy connection through Yamux
async fn handle_proxy_connection(
    mut client_stream: TcpStream,
    target: TargetAddr,
    initial_data: Option<Vec<u8>>,
    yamux: Arc<Mutex<YamuxTransport>>,
) -> Result<()> {
    debug!("Opening Yamux stream for {}:{}", target.host(), target.port());

    // Open a new Yamux stream
    let mut yamux_stream = {
        let mut yamux_guard = yamux.lock().await;
        yamux_guard.open_stream().await?
    };

    debug!("✅ Yamux stream opened");

    // Send CONNECT command through Yamux stream
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
    let mut response = vec![0u8; 9]; // "CONNECTED"
    yamux_stream.read_exact(&mut response).await?;
    
    if response != b"CONNECTED" {
        anyhow::bail!("Failed to establish tunnel: unexpected response");
    }

    debug!("✅ Tunnel established");

    // If we have initial data and it wasn't sent in CONNECT, send it now
    if let Some(data) = initial_data {
        if !connect_msg.ends_with('|') {
            // Data was already sent in CONNECT message
        } else {
            yamux_stream.write_all(&data).await?;
        }
    }

    // Bidirectional copy
    match tokio::io::copy_bidirectional(&mut client_stream, &mut yamux_stream).await {
        Ok((client_to_server, server_to_client)) => {
            debug!(
                "Connection closed: {} bytes sent, {} bytes received",
                client_to_server, server_to_client
            );
        }
        Err(e) => {
            warn!("Connection error: {}", e);
        }
    }

    Ok(())
}
