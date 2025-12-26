# 使用指南

## 快速开始

### 1. 安装Rust环境

#### Windows
下载并安装 [rustup](https://rustup.rs/)：
```powershell
# 下载并运行 rustup-init.exe
# 或使用 winget
winget install Rustlang.Rustup
```

#### Linux/macOS
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 2. 编译项目

```bash
cd ws-proxy-client
cargo build --release
```

### 3. 配置服务器

编辑 `config.toml`：

```toml
[server]
address = "your-server.com"  # 修改为你的服务器地址
port = 443
uuid = "d342d11e-d424-4583-b36e-524ab1f0afa4"  # 修改为你的UUID
ws_path = "/"

[tls]
randomize_fingerprint = true  # 启用TLS指纹随机化
enable_ech = true
sni = "your-server.com"  # 修改为你的服务器域名
```

### 4. 运行客户端

```bash
# Windows
.\target\release\ws-proxy-client.exe

# Linux/macOS
./target/release/ws-proxy-client
```

## 详细配置

### 服务器配置

```toml
[server]
# 服务器地址（支持域名和IP）
address = "example.com"

# 服务器端口（通常为443用于HTTPS）
port = 443

# 认证UUID（必须与服务端配置一致）
uuid = "d342d11e-d424-4583-b36e-524ab1f0afa4"

# WebSocket路径
ws_path = "/"
```

### TLS配置

```toml
[tls]
# 启用TLS指纹随机化（推荐）
randomize_fingerprint = true

# 启用ECH（需要服务器支持）
enable_ech = true

# SNI服务器名称（通常与address相同）
sni = "example.com"
```

**TLS指纹随机化效果：**
- ✅ 每次连接生成不同的JA3/JA4指纹
- ✅ 规避基于指纹的封锁
- ✅ 性能开销 < 1μs

### 代理配置

```toml
[proxy]
# SOCKS5代理
socks5_enabled = true
socks5_bind = "127.0.0.1"
socks5_port = 1080

# HTTP代理
http_enabled = true
http_bind = "127.0.0.1"
http_port = 8080
```

### 多路复用配置

```toml
[multiplexing]
# 最大并发流数量
max_streams = 256

# 流窗口大小（字节）
window_size = 1048576  # 1MB

# 保活间隔（秒）
keep_alive_interval = 30
```

## 使用场景

### 场景1：浏览器代理

#### Chrome/Edge
1. 打开设置 → 系统 → 打开代理设置
2. 配置代理服务器：
   - SOCKS5: `127.0.0.1:1080`
   - 或 HTTP: `127.0.0.1:8080`

#### Firefox
1. 设置 → 网络设置 → 手动代理配置
2. SOCKS主机：`127.0.0.1` 端口：`1080`
3. 选择 SOCKS v5

### 场景2：命令行工具

#### curl
```bash
# 使用SOCKS5
curl --socks5 127.0.0.1:1080 https://example.com

# 使用HTTP代理
curl --proxy http://127.0.0.1:8080 https://example.com
```

#### wget
```bash
# 使用HTTP代理
export http_proxy=http://127.0.0.1:8080
export https_proxy=http://127.0.0.1:8080
wget https://example.com
```

#### git
```bash
# 使用SOCKS5
git config --global http.proxy socks5://127.0.0.1:1080
git config --global https.proxy socks5://127.0.0.1:1080

# 使用HTTP代理
git config --global http.proxy http://127.0.0.1:8080
git config --global https.proxy http://127.0.0.1:8080
```

### 场景3：编程语言

#### Python
```python
import requests

proxies = {
    'http': 'socks5://127.0.0.1:1080',
    'https': 'socks5://127.0.0.1:1080',
}

response = requests.get('https://example.com', proxies=proxies)
```

#### Node.js
```javascript
const axios = require('axios');
const SocksProxyAgent = require('socks-proxy-agent');

const agent = new SocksProxyAgent('socks5://127.0.0.1:1080');

axios.get('https://example.com', { httpAgent: agent, httpsAgent: agent })
    .then(response => console.log(response.data));
```

#### Go
```go
package main

import (
    "net/http"
    "net/url"
)

func main() {
    proxyURL, _ := url.Parse("socks5://127.0.0.1:1080")
    client := &http.Client{
        Transport: &http.Transport{
            Proxy: http.ProxyURL(proxyURL),
        },
    }
    
    resp, _ := client.Get("https://example.com")
    defer resp.Body.Close()
}
```

## 命令行选项

```bash
ws-proxy-client [OPTIONS]

Options:
  -c, --config <CONFIG>  配置文件路径 [default: config.toml]
      --server <SERVER>  覆盖服务器地址
      --port <PORT>      覆盖服务器端口
      --uuid <UUID>      覆盖UUID
  -v, --verbose          启用详细日志
  -h, --help             显示帮助信息
```

### 示例

```bash
# 使用自定义配置文件
ws-proxy-client --config /path/to/config.toml

# 临时覆盖服务器地址
ws-proxy-client --server example.com --port 443

# 启用详细日志（调试用）
ws-proxy-client --verbose

# 组合使用
ws-proxy-client --config custom.toml --uuid "new-uuid" --verbose
```

## 日志级别

通过环境变量 `RUST_LOG` 控制日志级别：

```bash
# Windows PowerShell
$env:RUST_LOG="debug"
.\ws-proxy-client.exe

# Linux/macOS
RUST_LOG=debug ./ws-proxy-client
```

日志级别：
- `error`: 仅错误
- `warn`: 警告和错误
- `info`: 信息、警告和错误（默认）
- `debug`: 调试信息
- `trace`: 详细跟踪信息

## 性能优化

### 1. 调整多路复用参数

```toml
[multiplexing]
# 增加最大流数量（适用于高并发）
max_streams = 512

# 增加窗口大小（适用于大文件传输）
window_size = 2097152  # 2MB
```

### 2. 禁用不需要的代理

```toml
[proxy]
# 如果只需要SOCKS5，禁用HTTP
http_enabled = false
```

### 3. 调整日志级别

生产环境使用 `info` 或 `warn` 级别，避免性能开销。

## 故障排除

### 问题1：连接失败

**症状**：无法连接到服务器

**解决方案**：
1. 检查服务器地址和端口
2. 验证UUID是否正确
3. 确认服务器正在运行
4. 检查防火墙设置
5. 使用 `--verbose` 查看详细日志

```bash
ws-proxy-client --verbose
```

### 问题2：TLS握手失败

**症状**：TLS握手错误

**解决方案**：
1. 临时禁用指纹随机化测试：
```toml
[tls]
randomize_fingerprint = false
```

2. 检查服务器证书是否有效
3. 验证SNI配置
4. 确认服务器支持TLS 1.3

### 问题3：代理不工作

**症状**：应用程序无法通过代理连接

**解决方案**：
1. 确认代理服务器已启动（查看日志）
2. 检查端口是否被占用：
```bash
# Windows
netstat -ano | findstr :1080

# Linux/macOS
lsof -i :1080
```

3. 验证应用程序代理配置
4. 测试代理连接：
```bash
curl --socks5 127.0.0.1:1080 https://example.com
```

### 问题4：性能问题

**症状**：连接速度慢

**解决方案**：
1. 增加窗口大小
2. 检查网络延迟
3. 使用 `release` 构建版本
4. 调整 `max_streams` 参数

### 问题5：内存占用高

**症状**：内存使用过多

**解决方案**：
1. 减少 `max_streams`
2. 减少 `window_size`
3. 检查是否有连接泄漏

## 安全建议

### 1. UUID管理
- 使用强随机UUID
- 定期更换UUID
- 不要在公开场合分享UUID

### 2. TLS配置
- 始终启用 `randomize_fingerprint`
- 使用有效的TLS证书
- 不要禁用证书验证

### 3. 日志安全
- 生产环境避免使用 `--verbose`
- 定期清理日志文件
- 不要记录敏感信息

### 4. 网络隔离
- 仅在受信任的网络使用
- 使用防火墙限制访问
- 考虑使用VPN

## 高级用法

### 1. 多实例运行

运行多个客户端实例（不同端口）：

**实例1（config1.toml）：**
```toml
[proxy]
socks5_port = 1080
http_port = 8080
```

**实例2（config2.toml）：**
```toml
[proxy]
socks5_port = 1081
http_port = 8081
```

```bash
ws-proxy-client --config config1.toml &
ws-proxy-client --config config2.toml &
```

### 2. 系统服务

#### Windows服务（使用NSSM）

```powershell
# 下载 NSSM
# 安装服务
nssm install ws-proxy-client "C:\path\to\ws-proxy-client.exe"
nssm set ws-proxy-client AppDirectory "C:\path\to"
nssm set ws-proxy-client AppParameters "--config config.toml"
nssm start ws-proxy-client
```

#### Linux systemd服务

创建 `/etc/systemd/system/ws-proxy-client.service`：

```ini
[Unit]
Description=WebSocket Proxy Client
After=network.target

[Service]
Type=simple
User=your-user
WorkingDirectory=/path/to/ws-proxy-client
ExecStart=/path/to/ws-proxy-client --config config.toml
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

启动服务：
```bash
sudo systemctl daemon-reload
sudo systemctl enable ws-proxy-client
sudo systemctl start ws-proxy-client
```

### 3. Docker部署

创建 `Dockerfile`：

```dockerfile
FROM rust:1.83 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/ws-proxy-client /usr/local/bin/
COPY config.toml /etc/ws-proxy-client/config.toml
EXPOSE 1080 8080
CMD ["ws-proxy-client", "--config", "/etc/ws-proxy-client/config.toml"]
```

构建和运行：
```bash
docker build -t ws-proxy-client .
docker run -d -p 1080:1080 -p 8080:8080 ws-proxy-client
```

## 监控和维护

### 1. 日志监控

```bash
# 实时查看日志
tail -f ws-proxy-client.log

# 搜索错误
grep ERROR ws-proxy-client.log
```

### 2. 性能监控

使用系统工具监控：

```bash
# CPU和内存使用
top -p $(pgrep ws-proxy-client)

# 网络连接
netstat -anp | grep ws-proxy-client
```

### 3. 健康检查

创建健康检查脚本：

```bash
#!/bin/bash
# health-check.sh

# 检查进程是否运行
if ! pgrep -x "ws-proxy-client" > /dev/null; then
    echo "Process not running"
    exit 1
fi

# 检查SOCKS5端口
if ! nc -z 127.0.0.1 1080; then
    echo "SOCKS5 port not listening"
    exit 1
fi

echo "Health check passed"
exit 0
```

## 更新和升级

### 更新客户端

```bash
cd ws-proxy-client
git pull
cargo build --release
```

### 备份配置

```bash
cp config.toml config.toml.backup
```

### 回滚

```bash
# 恢复配置
cp config.toml.backup config.toml

# 使用旧版本
git checkout <old-version>
cargo build --release
```

## 常见问题

**Q: 支持哪些操作系统？**  
A: Windows 10/11, Linux, macOS（跨平台）

**Q: 需要root/管理员权限吗？**  
A: 不需要，除非使用特权端口（< 1024）

**Q: 可以同时使用SOCKS5和HTTP代理吗？**  
A: 可以，两者可以同时启用

**Q: TLS指纹随机化会影响性能吗？**  
A: 几乎没有影响（< 1μs开销）

**Q: 支持IPv6吗？**  
A: 支持，SOCKS5和HTTP代理都支持IPv6

**Q: 如何验证TLS指纹随机化是否工作？**  
A: 使用Wireshark抓包，观察ClientHello的变化

**Q: 可以用于生产环境吗？**  
A: 可以，但建议充分测试并做好监控

## 获取帮助

- 查看日志：使用 `--verbose` 选项
- 提交Issue：在GitHub仓库提交问题
- 查看文档：阅读README.md和源代码注释

## 贡献

欢迎提交Pull Request和Issue！
