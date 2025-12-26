use anyhow::{Context, Result};
use bytes::Bytes;
use futures::{AsyncRead, AsyncWrite, SinkExt, StreamExt};
use std::io;
use std::pin::Pin;
use std::task::{Context as TaskContext, Poll};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::tungstenite::protocol::Message;
use tracing::{debug, error, trace, warn};
use yamux::{Config as YamuxConfig, Connection, Mode, Stream as YamuxStream};

use super::WsStream;
use crate::config::MultiplexingConfig;

/// Wrapper to make WebSocket stream compatible with Yamux
pub struct WebSocketAdapter {
    ws: WsStream,
    read_buffer: Vec<u8>,
    read_pos: usize,
}

impl WebSocketAdapter {
    pub fn new(ws: WsStream) -> Self {
        Self {
            ws,
            read_buffer: Vec::new(),
            read_pos: 0,
        }
    }
}

impl AsyncRead for WebSocketAdapter {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        // If we have buffered data, return it first
        if self.read_pos < self.read_buffer.len() {
            let remaining = self.read_buffer.len() - self.read_pos;
            let to_copy = remaining.min(buf.len());
            buf[..to_copy].copy_from_slice(&self.read_buffer[self.read_pos..self.read_pos + to_copy]);
            self.read_pos += to_copy;
            
            // Clear buffer if fully consumed
            if self.read_pos >= self.read_buffer.len() {
                self.read_buffer.clear();
                self.read_pos = 0;
            }
            
            return Poll::Ready(Ok(to_copy));
        }

        // Try to read next WebSocket message
        match Pin::new(&mut self.ws).poll_next(cx) {
            Poll::Ready(Some(Ok(msg))) => {
                match msg {
                    Message::Binary(data) => {
                        let len = data.len().min(buf.len());
                        buf[..len].copy_from_slice(&data[..len]);
                        
                        // Buffer remaining data if any
                        if data.len() > len {
                            self.read_buffer = data[len..].to_vec();
                            self.read_pos = 0;
                        }
                        
                        trace!("WebSocket read {} bytes", len);
                        Poll::Ready(Ok(len))
                    }
                    Message::Text(text) => {
                        // Handle text messages (e.g., CONNECTED)
                        if text == "CONNECTED" {
                            trace!("Received CONNECTED message, continuing read");
                            // Re-poll to get next message
                            cx.waker().wake_by_ref();
                            Poll::Pending
                        } else {
                            let data = text.into_bytes();
                            let len = data.len().min(buf.len());
                            buf[..len].copy_from_slice(&data[..len]);
                            
                            if data.len() > len {
                                self.read_buffer = data[len..].to_vec();
                                self.read_pos = 0;
                            }
                            
                            Poll::Ready(Ok(len))
                        }
                    }
                    Message::Close(_) => {
                        debug!("WebSocket closed");
                        Poll::Ready(Ok(0))
                    }
                    Message::Ping(_) | Message::Pong(_) => {
                        // Ignore ping/pong, continue reading
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    }
                    _ => {
                        warn!("Unexpected WebSocket message type");
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    }
                }
            }
            Poll::Ready(Some(Err(e))) => {
                error!("WebSocket read error: {}", e);
                Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, e)))
            }
            Poll::Ready(None) => {
                debug!("WebSocket stream ended");
                Poll::Ready(Ok(0))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl AsyncWrite for WebSocketAdapter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let msg = Message::Binary(buf.to_vec());
        
        match Pin::new(&mut self.ws).poll_ready(cx) {
            Poll::Ready(Ok(())) => {
                match Pin::new(&mut self.ws).start_send(msg) {
                    Ok(()) => {
                        trace!("WebSocket wrote {} bytes", buf.len());
                        Poll::Ready(Ok(buf.len()))
                    }
                    Err(e) => {
                        error!("WebSocket write error: {}", e);
                        Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, e)))
                    }
                }
            }
            Poll::Ready(Err(e)) => {
                error!("WebSocket not ready: {}", e);
                Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, e)))
            }
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<io::Result<()>> {
        match Pin::new(&mut self.ws).poll_flush(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(e)) => Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, e))),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<io::Result<()>> {
        match Pin::new(&mut self.ws).poll_close(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(e)) => Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, e))),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Yamux transport layer for multiplexing
pub struct YamuxTransport {
    connection: Connection<WebSocketAdapter>,
}

impl YamuxTransport {
    /// Create a new Yamux transport over WebSocket
    pub fn new(ws: WsStream, config: &MultiplexingConfig) -> Result<Self> {
        debug!("Creating Yamux transport with config: max_streams={}, window_size={}", 
               config.max_streams, config.window_size);

        let adapter = WebSocketAdapter::new(ws);
        
        let mut yamux_config = YamuxConfig::default();
        yamux_config.set_window_update_mode(yamux::WindowUpdateMode::OnRead);
        yamux_config.set_max_num_streams(config.max_streams);
        
        let connection = Connection::new(adapter, yamux_config, Mode::Client);
        
        debug!("✅ Yamux transport created");
        
        Ok(Self { connection })
    }

    /// Open a new stream for a connection
    pub async fn open_stream(&mut self) -> Result<YamuxStream<WebSocketAdapter>> {
        self.connection
            .open_stream()
            .await
            .context("Failed to open Yamux stream")
    }

    /// Get the underlying connection
    pub fn connection_mut(&mut self) -> &mut Connection<WebSocketAdapter> {
        &mut self.connection
    }

    /// Run the connection (process control messages)
    pub async fn run(mut self) -> Result<()> {
        loop {
            match self.connection.next_stream().await {
                Ok(Some(stream)) => {
                    debug!("New incoming stream (unexpected in client mode)");
                    drop(stream);
                }
                Ok(None) => {
                    debug!("Yamux connection closed");
                    break;
                }
                Err(e) => {
                    error!("Yamux connection error: {}", e);
                    return Err(e.into());
                }
            }
        }
        Ok(())
    }
}

/// Copy data bidirectionally between two async streams
pub async fn copy_bidirectional<A, B>(mut a: A, mut b: B) -> Result<(u64, u64)>
where
    A: AsyncRead + AsyncWrite + Unpin,
    B: AsyncRead + AsyncWrite + Unpin,
{
    let (a_to_b, b_to_a) = tokio::io::copy_bidirectional(&mut a, &mut b)
        .await
        .context("Bidirectional copy failed")?;
    
    debug!("Bidirectional copy completed: {} bytes A->B, {} bytes B->A", a_to_b, b_to_a);
    Ok((a_to_b, b_to_a))
}
