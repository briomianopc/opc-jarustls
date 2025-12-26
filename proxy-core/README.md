# jarustls Proxy - ECH + Yamux + WebSocket Proxy Client

A Windows proxy client with TLS fingerprint obfuscation, Encrypted Client Hello (ECH), Yamux multiplexing, and CDN optimization support.

## 🌟 Features

- **ECH (Encrypted Client Hello)**: Hide SNI from network observers using TLS 1.3 ECH
- **TLS Fingerprint Randomization**: Evade JA3/JA4 fingerprinting detection
- **Yamux Multiplexing**: Multiple streams over single WebSocket connection
- **CDN Optimization**: Separate connection IP from SNI domain for Cloudflare optimization
- **Dual Proxy Support**: SOCKS5 and HTTP proxy protocols
- **DNS-over-HTTPS**: Secure DNS resolution via Cloudflare DoH
- **WebSocket Transport**: Reliable tunneling over WebSocket + TLS 1.3

## 🚀 Quick Start

### Build

```bash
cargo build --release
```

### Run

```bash
# SOCKS5 proxy
./target/release/proxy-core --listen 127.0.0.1:1080 --server example.com

# HTTP proxy
./target/release/proxy-core --mode http --listen 127.0.0.1:8080 --server example.com

# With ECH
./target/release/proxy-core --listen 127.0.0.1:1080 \
    --server example.com \
    --ech-config <base64-encoded-config>

# With CDN optimization
./target/release/proxy-core --listen 127.0.0.1:1080 \
    --server real-server.com \
    --target 1.1.1.1
```

## 📖 Documentation

- **[../JARUSTLS_ECH_API.md](../JARUSTLS_ECH_API.md)**: ECH API documentation
- **[../COMPLETE_GUIDE.md](../COMPLETE_GUIDE.md)**: Full implementation guide
- **[../STATUS.md](../STATUS.md)**: Project status and testing

## 🔧 Configuration

### Command-Line Options

```
--mode <MODE>              Proxy mode: socks5 or http [default: socks5]
--listen <ADDR>            Local listen address [default: 127.0.0.1:1080]
--server <DOMAIN>          Server domain (for SNI) [required]
--target <IP/CNAME>        Target IP/CNAME (CDN optimized) [optional]
--port <PORT>              Server port [default: 443]
--ws-path <PATH>           WebSocket path [default: /]
--uuid <UUID>              Authentication UUID [optional]
--ech-config <BASE64>      Base64 ECH config [optional]
--enable-fingerprint-randomization  Enable TLS fingerprint randomization
```

### Environment Variables

All options can be set via environment variables with `PROXY_` prefix:

```bash
export PROXY_MODE=socks5
export PROXY_LISTEN=127.0.0.1:1080
export PROXY_SERVER=example.com
export PROXY_TARGET=1.1.1.1
export PROXY_ECH_CONFIG=base64-config
```

## 🏗️ Architecture

```
Client → SOCKS5/HTTP → Yamux Stream → WebSocket → TLS 1.3 + ECH → Server
                                                   ↓
                                         SNI: real-server.com
                                         IP: 1.1.1.1 (CDN)
```

### Components

- **TLS Module** (`src/tls/`): ECH integration and fingerprint randomization
- **Tunnel Module** (`src/tunnel/`): WebSocket + Yamux multiplexing
- **Proxy Module** (`src/proxy/`): SOCKS5 and HTTP handlers
- **DoH Module** (`src/doh/`): DNS-over-HTTPS resolver

## 🧪 Testing

```bash
# Build and run
cargo build --release
./target/release/proxy-core --listen 127.0.0.1:1080 --server example.com

# Test with curl (in another terminal)
curl -x socks5h://127.0.0.1:1080 https://www.google.com
```

## 📦 Dependencies

- **rustls 0.24**: TLS library with ECH support (local path)
- **tokio**: Async runtime
- **yamux 0.13**: Multiplexing protocol
- **tokio-tungstenite 0.24**: WebSocket implementation
- **reqwest 0.12**: HTTP client for DoH

See [Cargo.toml](Cargo.toml) for full dependency list.

## 🔐 Security

### ECH Configuration

ECH configs must be fetched from DNS HTTPS records:

```bash
# Fetch ECH config
dig +short HTTPS example.com | grep ech= | cut -d= -f2

# Or use DoH
curl -H "accept: application/dns-json" \
  "https://cloudflare-dns.com/dns-query?name=example.com&type=HTTPS"
```

### TLS Fingerprinting

Fingerprint randomization changes:
- Cipher suite order
- Extension order
- Padding length

This evades JA3/JA4 detection but may affect compatibility.

## 🐛 Troubleshooting

### Connection Fails

1. Verify ECH config is valid: `echo $ECH_CONFIG | base64 -d | xxd`
2. Check server supports ECH
3. Try without CDN optimization: omit `--target`
4. Check firewall rules

### Certificate Errors

1. Verify server domain matches certificate
2. Check system time is correct
3. Try without ECH: omit `--ech-config`

### Slow Performance

1. Increase Yamux window size (code modification)
2. Use geographically closer CDN IP
3. Check network latency: `ping <target-ip>`

## 📄 License

MIT License - see [../LICENSE](../LICENSE) for details.

## 🙏 Acknowledgments

- **rustls**: TLS library foundation
- **aws-lc-rs**: Cryptographic provider
- **Yamux**: Multiplexing protocol
- **tokio**: Async runtime

---

For more information, see the [main project README](../README.md).
