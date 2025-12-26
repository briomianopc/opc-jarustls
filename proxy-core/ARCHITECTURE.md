# 架构设计文档

## 核心设计原则

### 1. SNI与连接IP分离

这是整个项目最关键的设计，用于支持CDN优选IP。

**问题**: 
- 用户想连接到最快的CDN IP（如1.1.1.1）
- 但服务器证书是针对域名（如my-proxy.com）的
- 直接连接IP会导致证书验证失败

**解决方案**:
```
TCP连接 -> 优选IP (1.1.1.1)
TLS SNI -> 真实域名 (my-proxy.com)
证书验证 -> 真实域名 (my-proxy.com)
```

**实现位置**: `tunnel.rs`

```rust
// 1. 解析target到IP（可能是IP或CNAME）
let target_ip = self.resolve_target().await?;

// 2. TCP连接到target IP
let tcp_stream = TcpStream::connect((target_ip, port)).await?;

// 3. TLS握手，SNI使用server_domain
let server_name = ServerName::try_from(server_domain)?;
let tls_stream = tls_connector.connect(server_name, tcp_stream).await?;
```

### 2. ECH集成

**设计目标**:
- 默认启用ECH
- 自动获取配置
- 优雅降级（ECH失败时回退到普通TLS）

**实现流程**:
```
1. DoH查询 cloudflare-ech.com TXT记录
2. Base64解码ECH配置
3. 创建EchConfig对象
4. 注入到ClientConfig
5. TLS握手时自动使用ECH
```

**实现位置**: 
- `doh.rs`: DoH查询
- `tls.rs`: ECH配置构建

### 3. 模块化设计

```
proxy-core/
├── doh/          # DoH解析器（独立模块）
├── tls/          # TLS配置（独立模块）
├── tunnel/       # 隧道核心（依赖doh和tls）
├── proxy/        # 代理服务器（依赖tunnel）
└── main.rs       # CLI入口（组装所有模块）
```

每个模块都可以独立测试和使用。

## 数据流

### 完整请求流程

```
1. 应用程序 -> SOCKS5/HTTP代理
   ↓
2. 代理解析目标地址
   ↓
3. 通过WebSocket隧道发送CONNECT命令
   ↓
4. 服务器建立到目标的连接
   ↓
5. 双向数据转发
```

### 连接建立流程

```
1. CLI参数解析
   ↓
2. 构建TLS配置
   ├─ 获取ECH配置（DoH查询）
   ├─ 启用指纹随机化
   └─ 加载根证书
   ↓
3. 构建隧道配置
   ├─ server_domain（SNI）
   ├─ target（连接IP）
   └─ port
   ↓
4. 建立隧道连接
   ├─ 解析target到IP
   ├─ TCP连接到IP
   ├─ TLS握手（SNI=server_domain）
   └─ WebSocket升级
   ↓
5. 启动代理服务器
   ├─ SOCKS5监听
   └─ HTTP监听
```

## 关键技术决策

### 1. 为什么使用DoH？

**原因**:
- 传统DNS可能被污染
- DoH使用HTTPS加密，更安全
- 阿里云DoH速度快，稳定性好

**实现**:
```rust
// 使用reqwest发送HTTPS请求
let response = client
    .get("https://dns.alidns.com/dns-query")
    .query(&[("name", domain), ("type", "TXT")])
    .send()
    .await?;
```

### 2. 为什么使用jarustls？

**原因**:
- 原生支持ECH
- 支持TLS指纹随机化
- 基于rustls，性能好，安全性高

**对比**:
| 特性 | jarustls | rustls | openssl |
|------|----------|--------|---------|
| ECH支持 | ✅ 原生 | ❌ | ⚠️ 需补丁 |
| 指纹随机化 | ✅ 原生 | ❌ | ❌ |
| 内存安全 | ✅ Rust | ✅ Rust | ❌ C |
| 性能 | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ |

### 3. 为什么使用WebSocket？

**原因**:
- 双向通信
- 支持二进制数据
- 可以复用TCP连接
- 兼容HTTP代理和防火墙

**对比其他方案**:
- **原始TCP**: 容易被识别和封锁
- **HTTP/2**: 不支持双向流
- **QUIC**: 部署复杂，兼容性差
- **WebSocket**: 平衡性能和兼容性

### 4. 为什么不使用配置文件？

**原因**:
- 作为GUI后端，配置由GUI管理
- 避免配置文件泄露敏感信息
- 更灵活，支持动态配置

**实现**:
- 所有配置通过CLI参数
- 支持环境变量
- 无本地文件存储

## 性能优化

### 1. 连接复用

**问题**: 每个代理连接都建立新的WebSocket连接，开销大

**解决方案**: 
- 使用单一WebSocket连接
- 通过消息协议区分不同的代理连接
- 类似HTTP/2的多路复用

**实现**（待完成）:
```rust
// 使用连接池
let ws_pool = Arc::new(Mutex::new(WebSocketPool::new()));

// 从池中获取连接
let ws = ws_pool.lock().await.get_or_create().await?;
```

### 2. DNS缓存

**问题**: 每次解析target都查询DoH，延迟高

**解决方案**:
```rust
// 使用LRU缓存
let dns_cache = Arc::new(Mutex::new(LruCache::new(100)));

// 先查缓存
if let Some(ip) = dns_cache.lock().await.get(domain) {
    return Ok(*ip);
}

// 缓存未命中，查询DoH
let ip = doh_resolver.resolve_ip(domain).await?;
dns_cache.lock().await.put(domain.to_string(), ip);
```

### 3. 异步I/O

**实现**:
- 使用tokio异步运行时
- 所有I/O操作都是非阻塞的
- 支持高并发连接

## 安全考虑

### 1. 证书验证

**严格验证**:
```rust
// 必须验证server_domain的证书
let server_name = ServerName::try_from(server_domain)?;
let tls_stream = tls_connector.connect(server_name, tcp_stream).await?;
```

**不允许**:
- 跳过证书验证
- 使用自签名证书（除非明确配置）
- 接受过期证书

### 2. ECH安全性

**保护内容**:
- SNI（服务器名称）
- ALPN协议
- 其他ClientHello扩展

**不保护**:
- 目标IP地址（TCP层可见）
- 流量模式（时序分析）
- 应用层数据（需要额外加密）

### 3. 指纹随机化

**目的**: 规避基于TLS指纹的检测

**方法**:
- 随机化加密套件顺序
- 随机化扩展顺序
- 添加随机填充

**限制**:
- 只能规避被动检测
- 无法对抗主动探测
- 应用层指纹仍可识别

## 错误处理

### 1. 分层错误处理

```
应用层错误 (main.rs)
    ↓
模块错误 (tunnel, proxy, etc.)
    ↓
底层错误 (IO, TLS, etc.)
```

### 2. 错误恢复策略

**连接错误**:
- 自动重试（最多3次）
- 指数退避
- 记录失败原因

**ECH错误**:
- 降级到普通TLS
- 记录警告日志
- 继续运行

**DoH错误**:
- 回退到系统DNS
- 使用备用DoH服务器
- 缓存上次成功的结果

## 扩展性

### 1. 支持多种DoH提供商

```rust
pub enum DohProvider {
    Alibaba,
    Cloudflare,
    Google,
    Custom(String),
}
```

### 2. 支持多种代理协议

```rust
pub enum ProxyProtocol {
    Socks5,
    Http,
    Socks4,  // 待实现
    Https,   // 待实现
}
```

### 3. 支持连接池

```rust
pub struct ConnectionPool {
    connections: Vec<WebSocketStream>,
    max_size: usize,
}
```

## 测试策略

### 1. 单元测试

每个模块独立测试：
```rust
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_doh_query() { ... }
    
    #[tokio::test]
    async fn test_tls_config() { ... }
}
```

### 2. 集成测试

测试模块间交互：
```rust
#[tokio::test]
async fn test_full_connection() {
    // 1. 构建TLS配置
    // 2. 建立隧道
    // 3. 发送数据
    // 4. 验证响应
}
```

### 3. 端到端测试

测试完整流程：
```bash
# 启动服务器
node server.js &

# 启动客户端
cargo run -- --server-domain localhost --target 127.0.0.1 &

# 测试SOCKS5
curl --socks5 127.0.0.1:1080 https://example.com

# 测试HTTP
curl --proxy http://127.0.0.1:8080 https://example.com
```

## 部署建议

### 1. 作为GUI后端

```rust
// GUI通过命令行启动
let child = Command::new("proxy-core")
    .args(&[
        "--server-domain", server_domain,
        "--target", target,
        "--uuid", uuid,
    ])
    .spawn()?;
```

### 2. 作为系统服务

```ini
[Unit]
Description=Proxy Core
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/proxy-core \
    --server-domain my-proxy.com \
    --target 1.1.1.1
Restart=always

[Install]
WantedBy=multi-user.target
```

### 3. 作为Docker容器

```dockerfile
FROM rust:1.83 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/proxy-core /usr/local/bin/
EXPOSE 1080 8080
CMD ["proxy-core"]
```

## 未来改进

### 1. 连接池

实现WebSocket连接池，减少连接开销。

### 2. 流量混淆

添加流量填充和时序混淆，对抗流量分析。

### 3. 多服务器支持

支持配置多个服务器，自动选择最快的。

### 4. 智能路由

根据目标地址选择不同的代理策略。

### 5. 统计和监控

添加连接统计、流量统计和性能监控。
