# jarustls ECH API 完整使用指南

## 概述

jarustls是rustls的一个分支，原生支持ECH (Encrypted Client Hello)。本文档详细说明如何使用ECH API。

## ECH基础概念

### 什么是ECH？

ECH (Encrypted Client Hello) 是TLS 1.3的扩展，用于加密ClientHello消息中的敏感信息：
- SNI (Server Name Indication)
- ALPN协议列表
- 其他扩展信息

### 为什么需要ECH？

1. **隐私保护**: SNI在传统TLS中是明文的，可被监听
2. **抗审查**: 防止基于SNI的过滤和封锁
3. **标准化**: 符合IETF标准（RFC draft）

## API结构

### 核心类型

```rust
// ECH配置
pub struct EchConfig { ... }

// ECH模式
pub enum EchMode {
    Grease,           // GREASE模式（测试）
    Outer(EchConfig), // 使用ECH配置
}

// ECH配置列表字节
pub type EchConfigListBytes<'a> = &'a [u8];
```

### 关键导入

```rust
use rustls::client::{EchConfig, EchMode};
use rustls::crypto::aws_lc_rs;
use rustls::crypto::aws_lc_rs::hpke::ALL_SUPPORTED_SUITES;
use rustls::pki_types::EchConfigListBytes;
use rustls::ClientConfig;
```

## 完整使用流程

### 步骤1: 获取ECH配置

ECH配置通常通过DNS TXT记录获取：

```rust
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

// 通过DoH查询cloudflare-ech.com的TXT记录
async fn fetch_ech_config() -> Result<EchConfigListBytes<'static>> {
    let resolver = DohResolver::new()?;
    
    // 查询TXT记录
    let txt_records = resolver.query_txt("cloudflare-ech.com").await?;
    
    // 解码Base64
    for record in txt_records {
        if let Ok(decoded) = BASE64.decode(record.trim_matches('"')) {
            return Ok(EchConfigListBytes::from(decoded));
        }
    }
    
    anyhow::bail!("No valid ECH config found")
}
```

### 步骤2: 创建EchConfig

使用ECH配置字节和HPKE套件创建EchConfig：

```rust
use rustls::client::EchConfig;
use rustls::crypto::aws_lc_rs::hpke::ALL_SUPPORTED_SUITES;

// ECH配置字节（从DNS获取）
let ech_config_bytes: EchConfigListBytes = fetch_ech_config().await?;

// 创建EchConfig
let ech_config = EchConfig::new(
    ech_config_bytes,
    ALL_SUPPORTED_SUITES  // 使用所有支持的HPKE套件
)?;
```

**重要**: `ALL_SUPPORTED_SUITES`包含7个HPKE套件：
- DH_KEM_P256_HKDF_SHA256_AES_128
- DH_KEM_P256_HKDF_SHA256_AES_256
- DH_KEM_P256_HKDF_SHA256_CHACHA20_POLY1305
- DH_KEM_P384_HKDF_SHA384_AES_256
- DH_KEM_X25519_HKDF_SHA256_AES_128
- DH_KEM_X25519_HKDF_SHA256_AES_256
- DH_KEM_X25519_HKDF_SHA256_CHACHA20_POLY1305

### 步骤3: 创建EchMode

```rust
use rustls::client::EchMode;

// 从EchConfig创建EchMode
let ech_mode = EchMode::from(ech_config);

// 或者使用GREASE模式（测试用）
let ech_mode = EchMode::Grease;
```

### 步骤4: 构建ClientConfig

```rust
use rustls::ClientConfig;
use rustls::crypto::aws_lc_rs;
use std::sync::Arc;

// 加载根证书
let root_store = rustls::RootCertStore {
    roots: webpki_roots::TLS_SERVER_ROOTS.iter().cloned().collect(),
};

// 构建ClientConfig with ECH
let config = ClientConfig::builder(Arc::new(aws_lc_rs::default_provider()))
    .with_ech(ech_mode)  // 关键：添加ECH
    .with_root_certificates(root_store)
    .with_no_client_auth()?;
```

### 步骤5: 使用ClientConfig

```rust
use tokio_rustls::TlsConnector;
use rustls::pki_types::ServerName;

// 创建TLS连接器
let connector = TlsConnector::from(Arc::new(config));

// 连接到服务器
let server_name = ServerName::try_from("example.com")?.to_owned();
let tls_stream = connector.connect(server_name, tcp_stream).await?;

// TLS握手时自动使用ECH
```

## 完整示例代码

```rust
use anyhow::{Context, Result};
use rustls::client::{EchConfig, EchMode};
use rustls::crypto::aws_lc_rs;
use rustls::crypto::aws_lc_rs::hpke::ALL_SUPPORTED_SUITES;
use rustls::pki_types::{EchConfigListBytes, ServerName};
use rustls::ClientConfig;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

async fn create_ech_client() -> Result<Arc<ClientConfig>> {
    // 1. 获取ECH配置
    let ech_config_bytes = fetch_ech_config().await?;
    
    // 2. 创建EchConfig
    let ech_config = EchConfig::new(ech_config_bytes, ALL_SUPPORTED_SUITES)
        .context("Failed to create ECH config")?;
    
    // 3. 创建EchMode
    let ech_mode = EchMode::from(ech_config);
    
    // 4. 加载根证书
    let root_store = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.iter().cloned().collect(),
    };
    
    // 5. 构建ClientConfig
    let config = ClientConfig::builder(Arc::new(aws_lc_rs::default_provider()))
        .with_ech(ech_mode)
        .with_root_certificates(root_store)
        .with_no_client_auth()?;
    
    Ok(Arc::new(config))
}

async fn connect_with_ech(
    config: Arc<ClientConfig>,
    host: &str,
    port: u16,
) -> Result<tokio_rustls::client::TlsStream<TcpStream>> {
    // 1. 建立TCP连接
    let tcp_stream = TcpStream::connect((host, port)).await?;
    
    // 2. 创建TLS连接器
    let connector = TlsConnector::from(config);
    
    // 3. TLS握手（自动使用ECH）
    let server_name = ServerName::try_from(host)?.to_owned();
    let tls_stream = connector.connect(server_name, tcp_stream).await?;
    
    Ok(tls_stream)
}
```

## API详解

### EchConfig::new()

```rust
pub fn new(
    ech_config_list: EchConfigListBytes<'_>,
    hpke_suites: &[&'static dyn Hpke],
) -> Result<Self, Error>
```

**参数**:
- `ech_config_list`: ECH配置列表字节（从DNS获取）
- `hpke_suites`: 支持的HPKE套件列表

**返回**:
- `Ok(EchConfig)`: 成功创建
- `Err(Error)`: 没有兼容的HPKE套件

**示例**:
```rust
let ech_config = EchConfig::new(
    ech_config_bytes,
    ALL_SUPPORTED_SUITES
)?;
```

### EchMode::from()

```rust
impl From<EchConfig> for EchMode
```

**用法**:
```rust
let ech_mode = EchMode::from(ech_config);
```

### ClientConfig::builder().with_ech()

```rust
pub fn with_ech(self, ech_mode: EchMode) -> ConfigBuilder<...>
```

**参数**:
- `ech_mode`: ECH模式（Outer或Grease）

**返回**:
- 配置构建器，继续链式调用

**示例**:
```rust
let config = ClientConfig::builder(Arc::new(aws_lc_rs::default_provider()))
    .with_ech(ech_mode)
    .with_root_certificates(root_store)
    .with_no_client_auth()?;
```

## HPKE套件详解

### 什么是HPKE？

HPKE (Hybrid Public Key Encryption) 是ECH使用的加密方案。

### 支持的套件

```rust
pub static ALL_SUPPORTED_SUITES: &[&dyn Hpke] = &[
    // P-256曲线
    DH_KEM_P256_HKDF_SHA256_AES_128,
    DH_KEM_P256_HKDF_SHA256_AES_256,
    DH_KEM_P256_HKDF_SHA256_CHACHA20_POLY1305,
    
    // P-384曲线
    DH_KEM_P384_HKDF_SHA384_AES_256,
    
    // X25519曲线
    DH_KEM_X25519_HKDF_SHA256_AES_128,
    DH_KEM_X25519_HKDF_SHA256_AES_256,
    DH_KEM_X25519_HKDF_SHA256_CHACHA20_POLY1305,
];
```

### 套件选择

EchConfig::new()会自动选择第一个兼容的套件：

```rust
// 自动选择
let ech_config = EchConfig::new(ech_config_bytes, ALL_SUPPORTED_SUITES)?;

// 或者手动指定
let custom_suites = &[
    DH_KEM_X25519_HKDF_SHA256_CHACHA20_POLY1305,
    DH_KEM_P256_HKDF_SHA256_AES_256,
];
let ech_config = EchConfig::new(ech_config_bytes, custom_suites)?;
```

## 错误处理

### 常见错误

1. **没有兼容的HPKE套件**
```rust
match EchConfig::new(ech_config_bytes, hpke_suites) {
    Ok(config) => config,
    Err(e) => {
        eprintln!("No compatible HPKE suite: {}", e);
        // 回退到不使用ECH
    }
}
```

2. **无效的ECH配置**
```rust
// ECH配置可能已过期或格式错误
if let Err(e) = EchConfig::new(ech_config_bytes, ALL_SUPPORTED_SUITES) {
    warn!("Invalid ECH config: {}, falling back to no ECH", e);
    // 使用普通TLS
}
```

3. **DNS查询失败**
```rust
match fetch_ech_config().await {
    Ok(config) => config,
    Err(e) => {
        warn!("Failed to fetch ECH config: {}", e);
        // 使用普通TLS或GREASE模式
    }
}
```

## 最佳实践

### 1. 优雅降级

```rust
async fn create_tls_config(enable_ech: bool) -> Result<Arc<ClientConfig>> {
    let root_store = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.iter().cloned().collect(),
    };
    
    if enable_ech {
        // 尝试使用ECH
        match fetch_ech_config().await {
            Ok(ech_config_bytes) => {
                if let Ok(ech_config) = EchConfig::new(ech_config_bytes, ALL_SUPPORTED_SUITES) {
                    let ech_mode = EchMode::from(ech_config);
                    return Ok(Arc::new(
                        ClientConfig::builder(Arc::new(aws_lc_rs::default_provider()))
                            .with_ech(ech_mode)
                            .with_root_certificates(root_store)
                            .with_no_client_auth()?
                    ));
                }
            }
            Err(e) => warn!("ECH config fetch failed: {}", e),
        }
    }
    
    // 回退到普通TLS
    Ok(Arc::new(
        ClientConfig::builder(Arc::new(aws_lc_rs::default_provider()))
            .with_root_certificates(root_store)
            .with_no_client_auth()?
    ))
}
```

### 2. 缓存ECH配置

```rust
use once_cell::sync::OnceCell;

static ECH_CONFIG: OnceCell<EchConfigListBytes<'static>> = OnceCell::new();

async fn get_ech_config() -> Result<&'static EchConfigListBytes<'static>> {
    if let Some(config) = ECH_CONFIG.get() {
        return Ok(config);
    }
    
    let config = fetch_ech_config().await?;
    ECH_CONFIG.set(config).ok();
    Ok(ECH_CONFIG.get().unwrap())
}
```

### 3. 定期更新

```rust
use tokio::time::{interval, Duration};

async fn ech_config_updater() {
    let mut interval = interval(Duration::from_secs(86400)); // 24小时
    
    loop {
        interval.tick().await;
        
        match fetch_ech_config().await {
            Ok(new_config) => {
                info!("ECH config updated");
                // 更新全局配置
            }
            Err(e) => {
                warn!("Failed to update ECH config: {}", e);
            }
        }
    }
}
```

## 调试和验证

### 启用日志

```rust
use tracing::{info, debug};

// 创建ECH配置时
debug!("Creating ECH config from {} bytes", ech_config_bytes.len());
let ech_config = EchConfig::new(ech_config_bytes, ALL_SUPPORTED_SUITES)?;
info!("✅ ECH config created successfully");

// TLS握手时
debug!("Starting TLS handshake with ECH");
let tls_stream = connector.connect(server_name, tcp_stream).await?;
info!("✅ TLS handshake completed with ECH");
```

### 使用Wireshark验证

1. 抓包TLS握手
2. 查找ClientHello
3. 检查是否有`encrypted_client_hello`扩展
4. 验证SNI是否被加密

### 测试代码

```rust
#[tokio::test]
async fn test_ech_integration() {
    // 获取ECH配置
    let ech_config_bytes = fetch_ech_config().await.unwrap();
    assert!(!ech_config_bytes.is_empty());
    
    // 创建EchConfig
    let ech_config = EchConfig::new(ech_config_bytes, ALL_SUPPORTED_SUITES).unwrap();
    
    // 创建ClientConfig
    let config = create_ech_client().await.unwrap();
    
    // 测试连接
    let tls_stream = connect_with_ech(config, "cloudflare.com", 443).await.unwrap();
    
    // 验证连接成功
    assert!(tls_stream.get_ref().0.is_handshaking() == false);
}
```

## 常见问题

### Q1: ECH配置从哪里获取？

A: 通常从DNS TXT记录获取，例如cloudflare-ech.com。

### Q2: 如何知道ECH是否生效？

A: 使用Wireshark抓包，查看ClientHello中是否有encrypted_client_hello扩展。

### Q3: ECH失败会怎样？

A: 如果ECH配置无效或不兼容，EchConfig::new()会返回错误。应该优雅降级到普通TLS。

### Q4: 性能影响如何？

A: ECH增加约1ms的握手延迟和200字节的ClientHello大小。

### Q5: 所有服务器都支持ECH吗？

A: 不是。只有支持ECH的服务器（如Cloudflare）才能使用。

## 参考资源

- [jarustls源码](https://github.com/briomianopc/jarustls)
- [rustls文档](https://docs.rs/rustls/)
- [ECH规范](https://datatracker.ietf.org/doc/html/draft-ietf-tls-esni)
- [HPKE规范](https://datatracker.ietf.org/doc/html/rfc9180)

## 总结

jarustls的ECH API设计简洁且强大：

1. **获取配置**: 通过DoH查询DNS TXT记录
2. **创建EchConfig**: 使用配置字节和HPKE套件
3. **创建EchMode**: 从EchConfig转换
4. **构建ClientConfig**: 使用with_ech()方法
5. **使用**: TLS握手时自动应用ECH

关键点：
- 使用`ALL_SUPPORTED_SUITES`确保最大兼容性
- 实现优雅降级处理错误
- 定期更新ECH配置
- 使用日志和抓包验证
