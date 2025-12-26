# WebSocket Proxy Client

Windows平台内核代理客户端，使用jarustls实现TLS指纹混淆，支持WebSocket + ECH + TLS1.3传输，Yamux多路复用，以及SOCKS5和HTTP代理。

## 功能特性

### 🔒 TLS指纹混淆
- 使用jarustls库实现TLS指纹随机化
- 支持加密套件随机排序（加权分布）
- 支持随机填充扩展（70%概率，80-300字节）
- 支持扩展顺序随机化
- 规避JA3/JA4指纹识别

### 🌐 传输层
- WebSocket over TLS 1.3
- 支持ECH (Encrypted Client Hello)
- UUID认证机制
- 自动重连（待实现）

### 🔀 多路复用
- Yamux协议实现多路复用
- 单一WebSocket连接支持多个并发流
- 可配置最大流数量和窗口大小

### 🔌 代理协议
- **SOCKS5代理**：完整的SOCKS5协议支持
- **HTTP代理**：支持HTTP和HTTPS（CONNECT方法）

## 安装

### 前置要求
- Rust 1.83 或更高版本
- Windows 10/11 或 Linux/macOS（跨平台）

### 编译

```bash
cd ws-proxy-client
cargo build --release
```

编译后的可执行文件位于 `target/release/ws-proxy-client.exe`（Windows）或 `target/release/ws-proxy-client`（Linux/macOS）。

## 配置

创建 `config.toml` 配置文件：

```toml
[server]
address = "example.com"
port = 443
uuid = "d342d11e-d424-4583-b36e-524ab1f0afa4"
ws_path = "/"

[tls]
randomize_fingerprint = true
enable_ech = true
sni = "example.com"

[proxy]
socks5_enabled = true
socks5_bind = "127.0.0.1"
socks5_port = 1080

http_enabled = true
http_bind = "127.0.0.1"
http_port = 8080

[multiplexing]
max_streams = 256
window_size = 1048576
keep_alive_interval = 30

[logging]
level = "info"
```

### 配置说明

#### Server配置
- `address`: 服务器地址（域名或IP）
- `port`: 服务器端口（通常为443）
- `uuid`: 认证UUID（需与服务端配置一致）
- `ws_path`: WebSocket路径

#### TLS配置
- `randomize_fingerprint`: 启用TLS指纹随机化
- `enable_ech`: 启用ECH（需要服务器支持）
- `sni`: SNI服务器名称

#### Proxy配置
- `socks5_enabled`: 启用SOCKS5代理
- `socks5_bind`: SOCKS5监听地址
- `socks5_port`: SOCKS5监听端口
- `http_enabled`: 启用HTTP代理
- `http_bind`: HTTP监听地址
- `http_port`: HTTP监听端口

#### Multiplexing配置
- `max_streams`: 最大并发流数量
- `window_size`: 流窗口大小（字节）
- `keep_alive_interval`: 保活间隔（秒）

## 使用方法

### 基本使用

```bash
# 使用默认配置文件（config.toml）
./ws-proxy-client

# 指定配置文件
./ws-proxy-client --config /path/to/config.toml

# 启用详细日志
./ws-proxy-client --verbose
```

### 命令行参数

```bash
./ws-proxy-client --help

Options:
  -c, --config <CONFIG>  Configuration file path [default: config.toml]
      --server <SERVER>  Override server address
      --port <PORT>      Override server port
      --uuid <UUID>      Override UUID
  -v, --verbose          Enable verbose logging
  -h, --help             Print help
```

### 使用代理

#### SOCKS5代理

配置应用程序使用SOCKS5代理：
- 地址：`127.0.0.1`
- 端口：`1080`（或配置文件中指定的端口）

示例（curl）：
```bash
curl --socks5 127.0.0.1:1080 https://example.com
```

#### HTTP代理

配置应用程序使用HTTP代理：
- 地址：`127.0.0.1`
- 端口：`8080`（或配置文件中指定的端口）

示例（curl）：
```bash
curl --proxy http://127.0.0.1:8080 https://example.com
```

示例（环境变量）：
```bash
export HTTP_PROXY=http://127.0.0.1:8080
export HTTPS_PROXY=http://127.0.0.1:8080
```

## 架构设计

```
┌─────────────────────────────────────────────────────────────┐
│                     应用程序                                  │
└────────────────┬────────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────────┐
│              本地代理服务器                                    │
│  ┌──────────────────┐      ┌──────────────────┐             │
│  │  SOCKS5 Server   │      │   HTTP Server    │             │
│  │  (127.0.0.1:1080)│      │ (127.0.0.1:8080) │             │
│  └────────┬─────────┘      └────────┬─────────┘             │
│           │                         │                        │
│           └─────────┬───────────────┘                        │
│                     ▼                                        │
│           ┌──────────────────┐                               │
│           │ Yamux Multiplexer│                               │
│           └────────┬─────────┘                               │
└────────────────────┼─────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│              WebSocket Client                                │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  TLS 1.3 with Fingerprint Randomization (jarustls)  │   │
│  │  - Cipher suite randomization                        │   │
│  │  - Random padding extension                          │   │
│  │  - Extension order randomization                     │   │
│  │  - ECH support                                       │   │
│  └──────────────────────────────────────────────────────┘   │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│                  远程服务器                                    │
│              (Node.js WebSocket Server)                      │
└─────────────────────────────────────────────────────────────┘
```

## TLS指纹随机化原理

### 1. 加密套件随机化
- 45%概率：交换前两个套件（AES_128 ↔ CHACHA20）
- 45%概率：保持原始顺序
- 10%概率：将AES_256移到首位

### 2. 填充扩展
- 70%概率添加填充扩展
- 随机长度：80-300字节
- 符合RFC 7685标准

### 3. 扩展顺序随机化
- 自动随机化扩展顺序
- 遵守协议约束（PreSharedKey始终最后）

### 效果
每次连接生成不同的JA3/JA4指纹，无法创建稳定的指纹规则进行封锁。

## 性能指标

- **延迟**：< 1μs 每次握手（TLS指纹随机化开销）
- **内存**：+80-300字节每连接（填充扩展）
- **CPU**：< 0.1%开销
- **吞吐量**：无可测量影响

## 故障排除

### 连接失败

1. 检查服务器地址和端口是否正确
2. 验证UUID是否与服务端一致
3. 确认服务器支持TLS 1.3
4. 检查防火墙设置

### 代理不工作

1. 确认代理服务器已启动（查看日志）
2. 检查端口是否被占用
3. 验证应用程序代理配置
4. 使用 `--verbose` 查看详细日志

### TLS握手失败

1. 禁用 `randomize_fingerprint` 测试
2. 检查服务器证书是否有效
3. 验证SNI配置
4. 查看详细日志排查问题

## 开发

### 项目结构

```
ws-proxy-client/
├── src/
│   ├── main.rs              # 主入口
│   ├── config/              # 配置模块
│   │   └── mod.rs
│   ├── transport/           # 传输层
│   │   ├── mod.rs
│   │   ├── tls.rs          # TLS配置
│   │   ├── websocket.rs    # WebSocket客户端
│   │   └── yamux_transport.rs  # Yamux多路复用
│   └── proxy/               # 代理服务器
│       ├── mod.rs
│       ├── socks5.rs       # SOCKS5服务器
│       └── http.rs         # HTTP服务器
├── Cargo.toml
├── config.toml
└── README.md
```

### 运行测试

```bash
cargo test
```

### 构建发布版本

```bash
cargo build --release
```

## 安全注意事项

1. **UUID保密**：UUID用于认证，应保密存储
2. **TLS证书验证**：默认验证服务器证书，不要禁用
3. **日志安全**：生产环境避免使用 `--verbose`，防止敏感信息泄露
4. **网络隔离**：建议在受信任的网络环境中使用

## 限制

### 能防护的
✅ 被动JA3/JA4指纹识别  
✅ 基于TLS指纹的静态黑名单  
✅ 自动化机器人检测（TLS层面）

### 不能防护的
❌ 应用层指纹识别（HTTP头、User-Agent）  
❌ 行为分析（时序、请求模式）  
❌ 复杂系统的主动探测

## 许可证

与jarustls相同：Apache-2.0 / ISC / MIT

## 致谢

- [rustls](https://github.com/rustls/rustls) - TLS库
- [jarustls](https://github.com/briomianopc/jarustls) - TLS指纹随机化
- [yamux](https://github.com/libp2p/rust-yamux) - 多路复用协议
- [tokio](https://tokio.rs/) - 异步运行时
