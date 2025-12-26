# 快速开始

## 5分钟快速部署

### 第一步：编译项目

```bash
cd proxy-core
cargo build --release
```

### 第二步：运行客户端

```bash
./target/release/proxy-core \
  --server-domain my-proxy.com \
  --target 1.1.1.1 \
  --port 443 \
  --uuid your-uuid-here
```

### 第三步：测试代理

```bash
# 测试SOCKS5
curl --socks5 127.0.0.1:1080 https://www.google.com

# 测试HTTP
curl --proxy http://127.0.0.1:8080 https://www.google.com
```

## 参数说明

### 必需参数

| 参数 | 说明 | 示例 |
|------|------|------|
| `--server-domain` | 真实服务器域名（用于SNI和证书验证） | `my-proxy.com` |
| `--target` | 优选IP或CNAME（实际连接的地址） | `1.1.1.1` 或 `cdn.example.com` |

### 可选参数

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `--port` | `443` | 服务器端口 |
| `--ws-path` | `/` | WebSocket路径 |
| `--uuid` | 无 | 认证UUID |
| `--enable-ech` | `true` | 启用ECH |
| `--enable-fingerprint-randomization` | `true` | 启用TLS指纹随机化 |
| `--socks5-bind` | `127.0.0.1:1080` | SOCKS5监听地址 |
| `--http-bind` | `127.0.0.1:8080` | HTTP监听地址 |
| `--enable-socks5` | `true` | 启用SOCKS5 |
| `--enable-http` | `true` | 启用HTTP |
| `--log-level` | `info` | 日志级别 |

## 使用场景

### 场景1：使用优选IP

```bash
# 服务器域名: my-proxy.example.com
# 优选IP: 1.1.1.1（Cloudflare的某个快速节点）

proxy-core \
  --server-domain my-proxy.example.com \
  --target 1.1.1.1 \
  --port 443 \
  --uuid abc123
```

**工作原理**:
1. TCP连接到 `1.1.1.1:443`
2. TLS握手时SNI使用 `my-proxy.example.com`
3. Cloudflare根据SNI路由到正确的源站
4. 证书验证 `my-proxy.example.com`

### 场景2：使用优选CNAME

```bash
# 服务器域名: my-proxy.example.com
# 优选CNAME: optimized.cloudflare.com

proxy-core \
  --server-domain my-proxy.example.com \
  --target optimized.cloudflare.com \
  --port 443 \
  --uuid abc123
```

**工作原理**:
1. DoH解析 `optimized.cloudflare.com` -> `1.1.1.1`
2. TCP连接到 `1.1.1.1:443`
3. 后续流程同场景1

### 场景3：禁用ECH

```bash
# 如果服务器不支持ECH，或者想测试普通TLS

proxy-core \
  --server-domain my-proxy.com \
  --target 1.1.1.1 \
  --enable-ech false
```

### 场景4：自定义端口

```bash
# 使用不同的本地代理端口

proxy-core \
  --server-domain my-proxy.com \
  --target 1.1.1.1 \
  --socks5-bind 127.0.0.1:1081 \
  --http-bind 127.0.0.1:8081
```

## 环境变量

所有参数都支持环境变量：

```bash
export SERVER_DOMAIN=my-proxy.com
export TARGET=1.1.1.1
export PORT=443
export UUID=abc123
export ENABLE_ECH=true
export SOCKS5_BIND=127.0.0.1:1080
export HTTP_BIND=127.0.0.1:8080

# 直接运行，无需参数
proxy-core
```

## 浏览器配置

### Chrome/Edge

1. 设置 → 系统 → 打开代理设置
2. 手动代理配置：
   - SOCKS主机：`127.0.0.1`
   - 端口：`1080`
   - 类型：SOCKS v5

### Firefox

1. 设置 → 网络设置 → 手动代理配置
2. SOCKS主机：`127.0.0.1`
3. 端口：`1080`
4. 选择：SOCKS v5

### 系统代理（Windows）

```powershell
# 设置系统代理
netsh winhttp set proxy proxy-server="socks=127.0.0.1:1080" bypass-list="localhost"

# 取消系统代理
netsh winhttp reset proxy
```

### 系统代理（macOS/Linux）

```bash
# 设置环境变量
export http_proxy=socks5://127.0.0.1:1080
export https_proxy=socks5://127.0.0.1:1080

# 或使用HTTP代理
export http_proxy=http://127.0.0.1:8080
export https_proxy=http://127.0.0.1:8080
```

## 常见问题

### Q1: 连接失败

**检查清单**:
1. 服务器是否在运行？
2. target IP是否可达？
3. 端口是否正确？
4. UUID是否匹配？

**调试**:
```bash
# 启用详细日志
proxy-core --log-level debug ...

# 测试TCP连接
telnet 1.1.1.1 443

# 测试TLS连接
openssl s_client -connect 1.1.1.1:443 -servername my-proxy.com
```

### Q2: ECH配置获取失败

**原因**: DoH查询失败或cloudflare-ech.com不可达

**解决方案**:
```bash
# 方案1: 禁用ECH
proxy-core --enable-ech false ...

# 方案2: 测试DoH连接
curl "https://dns.alidns.com/dns-query?name=cloudflare-ech.com&type=TXT"
```

### Q3: 代理不工作

**检查清单**:
1. 代理服务器是否启动？（查看日志）
2. 端口是否被占用？
3. 应用程序代理配置是否正确？

**测试**:
```bash
# 测试SOCKS5
curl -v --socks5 127.0.0.1:1080 https://www.google.com

# 测试HTTP
curl -v --proxy http://127.0.0.1:8080 https://www.google.com
```

### Q4: 性能问题

**优化建议**:
1. 使用地理位置最近的优选IP
2. 启用TLS会话复用
3. 调整TCP参数（如果需要）

## 高级用法

### 作为系统服务运行

**Linux (systemd)**:
```ini
# /etc/systemd/system/proxy-core.service
[Unit]
Description=Proxy Core
After=network.target

[Service]
Type=simple
User=proxy
Environment="SERVER_DOMAIN=my-proxy.com"
Environment="TARGET=1.1.1.1"
Environment="UUID=abc123"
ExecStart=/usr/local/bin/proxy-core
Restart=always

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable proxy-core
sudo systemctl start proxy-core
```

**Windows (NSSM)**:
```powershell
# 下载NSSM
# 安装服务
nssm install proxy-core "C:\path\to\proxy-core.exe"
nssm set proxy-core AppParameters "--server-domain my-proxy.com --target 1.1.1.1"
nssm start proxy-core
```

### Docker部署

```dockerfile
FROM rust:1.83 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/proxy-core /usr/local/bin/
EXPOSE 1080 8080
ENTRYPOINT ["proxy-core"]
```

```bash
# 构建
docker build -t proxy-core .

# 运行
docker run -d \
  -p 1080:1080 \
  -p 8080:8080 \
  -e SERVER_DOMAIN=my-proxy.com \
  -e TARGET=1.1.1.1 \
  -e UUID=abc123 \
  proxy-core
```

## 性能调优

### 系统参数

**Linux**:
```bash
# 增加文件描述符限制
ulimit -n 65535

# 优化TCP参数
sysctl -w net.ipv4.tcp_fin_timeout=30
sysctl -w net.ipv4.tcp_keepalive_time=1200
sysctl -w net.ipv4.tcp_tw_reuse=1
```

**Windows**:
```powershell
# 增加TCP连接数限制
netsh int ipv4 set dynamicport tcp start=10000 num=55535
```

### 应用参数

```bash
# 使用更多的工作线程
TOKIO_WORKER_THREADS=8 proxy-core ...

# 调整日志级别（减少I/O）
proxy-core --log-level warn ...
```

## 监控和维护

### 日志查看

```bash
# 实时查看日志
proxy-core --log-level info ... 2>&1 | tee proxy-core.log

# 搜索错误
grep ERROR proxy-core.log

# 统计连接数
grep "New.*connection" proxy-core.log | wc -l
```

### 健康检查

```bash
#!/bin/bash
# health-check.sh

# 检查进程
if ! pgrep -x "proxy-core" > /dev/null; then
    echo "Process not running"
    exit 1
fi

# 检查SOCKS5端口
if ! nc -z 127.0.0.1 1080; then
    echo "SOCKS5 port not listening"
    exit 1
fi

# 检查HTTP端口
if ! nc -z 127.0.0.1 8080; then
    echo "HTTP port not listening"
    exit 1
fi

echo "Health check passed"
exit 0
```

## 获取帮助

### 查看帮助

```bash
proxy-core --help
```

### 查看版本

```bash
proxy-core --version
```

### 报告问题

在GitHub上提交Issue，包含：
1. 完整的命令行参数
2. 错误日志（使用 `--log-level debug`）
3. 系统信息（OS、Rust版本等）

---

**祝你使用愉快！** 🚀
