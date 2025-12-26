# 代码修复和改进

## 修复的问题

### 1. TLS模块 - ECH集成

**问题**: ECH集成代码引用了未实现的API

**修复**:
- 简化了TLS配置构建逻辑
- 移除了对未确认的ECH API的直接调用
- 添加了TODO注释，标记ECH集成待完成
- 保留了ECH配置获取逻辑，为将来集成做准备

**代码变更**:
```rust
// 之前：尝试直接使用EchConfig::new()
let ech_config = EchConfig::new(ech_config_bytes, &hpke_suites)?;

// 现在：先构建基础配置，ECH集成待完成
let mut config = ClientConfig::builder()
    .with_root_certificates(root_store)
    .with_no_client_auth()?;

if self.enable_ech && self.ech_config.is_some() {
    info!("✅ ECH config available (integration pending)");
    // TODO: Integrate ECH config when jarustls API is confirmed
}
```

### 2. Tunnel模块 - Target参数可选化

**问题**: Target参数是必需的，但实际上CDN优选IP是可选功能

**修复**:
- 将`target`字段改为`Option<String>`
- 添加两个构造函数：
  - `TunnelConfig::new()` - 不使用CDN优选IP
  - `TunnelConfig::with_target()` - 使用CDN优选IP
- 更新解析逻辑，当target为None时使用server_domain

**代码变更**:
```rust
// 之前：target是必需的
pub struct TunnelConfig {
    pub server_domain: String,
    pub target: String,  // 必需
    pub port: u16,
}

// 现在：target是可选的
pub struct TunnelConfig {
    pub server_domain: String,
    pub target: Option<String>,  // 可选
    pub port: u16,
}

// 新的构造函数
impl TunnelConfig {
    // 直接连接（不使用CDN优选IP）
    pub fn new(server_domain: String, port: u16) -> Self { ... }
    
    // 使用CDN优选IP
    pub fn with_target(server_domain: String, target: String, port: u16) -> Self { ... }
}
```

### 3. Main.rs - CLI参数更新

**问题**: CLI强制要求target参数

**修复**:
- 将`--target`参数改为可选
- 更新帮助文档，说明target是可选的
- 根据target是否存在，使用不同的TunnelConfig构造函数

**代码变更**:
```rust
// 之前：target是必需的
#[arg(long, env = "TARGET")]
target: String,

// 现在：target是可选的
#[arg(long, env = "TARGET")]
target: Option<String>,

// 构建tunnel配置时的逻辑
let tunnel_config = if let Some(target) = args.target.clone() {
    TunnelConfig::with_target(args.server_domain.clone(), target, args.port)
} else {
    TunnelConfig::new(args.server_domain.clone(), args.port)
};
```

### 4. 导入修复

**问题**: 一些未使用的导入和错误的API调用

**修复**:
- 移除了未使用的`EchConfig`导入
- 移除了未使用的`Connector`导入
- 修复了`client_async_tls_with_config`的参数

## 使用示例

### 场景1: 直接连接（不使用CDN优选IP）

```bash
# 最简单的用法
proxy-core --server-domain my-proxy.com

# 完整参数
proxy-core \
  --server-domain my-proxy.com \
  --port 443 \
  --uuid abc123
```

**工作流程**:
1. DNS解析 my-proxy.com -> IP
2. TCP连接到解析出的IP
3. TLS握手，SNI = my-proxy.com
4. WebSocket升级

### 场景2: 使用CDN优选IP

```bash
# 使用优选IP
proxy-core \
  --server-domain my-proxy.com \
  --target 1.1.1.1

# 使用优选CNAME
proxy-core \
  --server-domain my-proxy.com \
  --target optimized.cloudflare.com
```

**工作流程**:
1. 解析target（1.1.1.1或optimized.cloudflare.com）
2. TCP连接到解析出的IP
3. TLS握手，SNI = my-proxy.com（关键！）
4. WebSocket升级

## 配置对比

### 直接连接 vs CDN优选IP

| 特性 | 直接连接 | CDN优选IP |
|------|---------|-----------|
| 参数 | `--server-domain` | `--server-domain` + `--target` |
| DNS解析 | 解析server-domain | 解析target |
| TCP连接 | 连接到server-domain的IP | 连接到target的IP |
| TLS SNI | server-domain | server-domain |
| 证书验证 | server-domain | server-domain |
| 使用场景 | 普通连接 | Cloudflare优选IP |

## 待完成的工作

### 1. ECH集成

需要确认jarustls的ECH API：

```rust
// 需要实现
let ech_config = EchConfig::new(ech_config_bytes, hpke_suites)?;

// 或者使用builder模式
let config = ClientConfig::builder()
    .with_ech(ech_config)
    .with_root_certificates(root_store)
    .with_no_client_auth()?;
```

**参考**:
- 查看jarustls的ECH示例代码
- 查看rustls的ECH文档
- 确认HPKE suites的获取方式

### 2. 双向数据转发

当前handle_proxy_connection中的双向转发逻辑需要完善：

```rust
// 需要实现
async fn handle_proxy_connection(
    mut client_stream: TcpStream,
    target: TargetAddr,
    initial_data: Option<Vec<u8>>,
    ws_stream: Arc<Mutex<WebSocketStream>>,
) -> Result<()> {
    // 1. 发送CONNECT命令
    // 2. 等待CONNECTED响应
    // 3. 实现双向数据转发
    //    - client_stream <-> ws_stream
    //    - 处理Arc<Mutex>的共享访问
}
```

### 3. 连接池

实现WebSocket连接池以提高性能：

```rust
struct ConnectionPool {
    connections: Vec<WebSocketStream>,
    max_size: usize,
}
```

## 测试建议

### 1. 基本连接测试

```bash
# 测试直接连接
proxy-core --server-domain example.com --log-level debug

# 测试CDN优选IP
proxy-core --server-domain example.com --target 1.1.1.1 --log-level debug
```

### 2. 代理功能测试

```bash
# 启动客户端
proxy-core --server-domain my-proxy.com --target 1.1.1.1

# 测试SOCKS5
curl --socks5 127.0.0.1:1080 https://www.google.com

# 测试HTTP
curl --proxy http://127.0.0.1:8080 https://www.google.com
```

### 3. ECH测试

```bash
# 启用ECH
proxy-core --server-domain my-proxy.com --enable-ech true --log-level debug

# 禁用ECH
proxy-core --server-domain my-proxy.com --enable-ech false
```

## 性能优化建议

### 1. DNS缓存

```rust
use lru::LruCache;

struct DnsCache {
    cache: Arc<Mutex<LruCache<String, IpAddr>>>,
}
```

### 2. 连接复用

```rust
// 复用WebSocket连接
let ws_pool = Arc::new(Mutex::new(WebSocketPool::new()));
```

### 3. 异步优化

```rust
// 使用tokio::spawn处理每个连接
tokio::spawn(async move {
    handle_connection(stream).await
});
```

## 总结

所有关键的逻辑和语法错误已修复：

✅ TLS模块 - 简化ECH集成，移除未实现的API调用  
✅ Tunnel模块 - Target参数改为可选  
✅ Main.rs - CLI参数更新  
✅ 导入修复 - 移除未使用的导入  
✅ 测试更新 - 更新测试用例以匹配新API  

代码现在应该可以编译通过（除了ECH集成部分需要根据实际API调整）。
