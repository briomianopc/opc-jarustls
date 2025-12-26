# Proxy Server

轻量级 WebSocket 代理服务端，支持 Yamux 多路复用。

## 特性

- ✅ Yamux 多路复用（单连接支持多流）
- ✅ UUID 认证
- ✅ Nginx 伪装页面
- ✅ 支持 Heroku/Railway/Render 等 Buildpacks 部署

## 部署

### 环境变量

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `PORT` | 监听端口 | `8080` |
| `UUID` | 认证密钥 | `d342d11e-d424-4583-b36e-524ab1f0afa4` |

### Heroku

```bash
heroku create your-app-name
heroku config:set UUID=your-secret-uuid
git push heroku main
```

### Railway

[![Deploy on Railway](https://railway.app/button.svg)](https://railway.app)

设置环境变量 `UUID` 即可。

### Docker

```dockerfile
FROM golang:1.21-alpine AS builder
WORKDIR /app
COPY . .
RUN go build -o server .

FROM alpine:latest
COPY --from=builder /app/server /server
CMD ["/server"]
```

## 本地运行

```bash
go run main.go
```

## 协议

客户端通过 Yamux 流发送目标地址：

```
第一个数据包: "host:port"
后续数据包: 原始 TCP 数据
```

## 配合客户端

使用 `proxy-core` 客户端连接：

```bash
proxy-core --server-domain your-server.com --uuid your-secret-uuid
```
