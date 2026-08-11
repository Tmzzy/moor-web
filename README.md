# Moor Web

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Container](https://img.shields.io/badge/ghcr.io-tmzzy%2Fmoor--web-2496ED?logo=docker&logoColor=white)](https://github.com/Tmzzy/moor-web/pkgs/container/moor-web)

Moor Web 是一个自托管的 MCP 网关管理工具，由 Rust/Axum 后端和 React 前端组成。它将多个 MCP Server 聚合到统一的 `/mcp` 端点，并提供 Server、Profile、工具开关、配置导入、客户端配置和审计日志管理。

本项目基于 [varandrew/moor](https://github.com/varandrew/moor) 修改为面向 Docker 的 Web 服务。来源和许可证信息见 [NOTICE](NOTICE) 与 [LICENSE](LICENSE)。

## 功能

- 管理 stdio 与远程 HTTP MCP Server
- 使用 Profile 组合 Server，并控制工具暴露范围
- 扫描、导入和转换常见 MCP 客户端配置
- 为 Claude Code、Codex、OpenCode 和 Cursor 生成连接片段
- 通过 SSE 实时同步 Server 状态和设置变更
- 记录、筛选并脱敏展示 MCP 工具调用审计日志
- 使用独立登录页保护管理界面，自动生成 MCP Access Token

## Docker 快速启动

镜像发布到 `ghcr.io/tmzzy/moor-web`，支持 `linux/amd64` 和 `linux/arm64`。

```bash
git clone https://github.com/Tmzzy/moor-web.git
cd moor-web
cp .env.example .env
```

编辑 `.env`，至少替换 `MOOR_PASSWORD`。本地使用时可以保留其他默认值，然后启动：

```bash
docker compose up -d
docker compose ps
```

打开 <http://localhost:9223>，使用用户名 `moor` 和 `.env` 中的密码登录。MCP Token 会在首次启动时自动生成并保存到 `/data/moor.db`，无需预先设置。

不使用 Compose 时：

```bash
docker run -d \
  --name moor \
  --restart unless-stopped \
  -p 127.0.0.1:9223:9223 \
  -e MOOR_PASSWORD='replace-with-a-long-random-password' \
  -e MOOR_PUBLIC_URL='http://localhost:9223' \
  -v moor-data:/data \
  ghcr.io/tmzzy/moor-web:latest
```

## 首次登录与 MCP Token

管理界面使用 Moor 自己的登录页。登录成功后，服务端签发 HttpOnly、SameSite Cookie；会话连续 7 天无活动后过期，退出登录或重启 Moor 也会使当前会话失效。Moor 前端不会把管理密码写入 localStorage 或 sessionStorage，也不会再显示原生 Basic Auth 弹窗。

进入 **Client Config** 页面，在 **MCP Access Token** 区域可以：

- 按需显示 Token
- 直接复制 Token
- 确认后轮换 Token

Token 默认不会随普通管理 API 返回，也不会写入日志。轮换会立即使旧 Token 对后续请求失效，所有 MCP 客户端都必须更新。

客户端配置片段使用 `MOOR_MCP_TOKEN` 环境变量。复制页面中的 Token，并在运行 MCP 客户端的环境中设置：

```bash
export MOOR_MCP_TOKEN='moor_replace_with_the_generated_value'
```

默认 MCP 地址为：

```text
http://localhost:9223/mcp
```

## 使用域名和 HTTPS

公网部署必须使用 HTTPS，因为登录密码、会话 Cookie 和 MCP Bearer Token 都依赖传输层保密。仓库提供了宿主机 Nginx 示例：[deploy/nginx/moor.conf](deploy/nginx/moor.conf)。

### 1. 配置 DNS 与 Moor

先把域名的 A/AAAA 记录指向服务器，然后修改 `.env`：

```dotenv
MOOR_PASSWORD=replace-with-a-long-random-password
MOOR_USERNAME=moor
MOOR_PUBLIC_URL=https://moor.example.com
MOOR_BIND_ADDRESS=127.0.0.1
MOOR_HTTP_PORT=9223
```

启动 Moor 后，后端只监听宿主机回环地址，公网流量必须经过 Nginx：

```bash
docker compose up -d
```

### 2. 安装 Nginx 配置

以下命令以 Debian/Ubuntu 为例。把示例域名替换为真实域名：

```bash
sudo apt-get update
sudo apt-get install -y nginx certbot python3-certbot-nginx
sudo cp deploy/nginx/moor.conf /etc/nginx/sites-available/moor
sudo sed -i 's/moor\.example\.com/your-domain.example/g' /etc/nginx/sites-available/moor
sudo ln -s /etc/nginx/sites-available/moor /etc/nginx/sites-enabled/moor
sudo nginx -t
sudo systemctl reload nginx
```

示例为 `/api/events` 和 `/mcp` 禁用了代理缓冲并设置长连接超时，同时对登录接口进行按 IP 限速。

### 3. 签发证书

在通过浏览器登录前完成 HTTPS 配置：

```bash
sudo certbot --nginx --redirect -d your-domain.example
sudo nginx -t
sudo systemctl reload nginx
```

完成后访问 `https://your-domain.example`。生成的客户端地址会使用 `MOOR_PUBLIC_URL`，因此应始终与外部真实地址一致。

## 配置

| 环境变量 | 默认值 | 说明 |
| --- | --- | --- |
| `MOOR_PASSWORD` | 无 | 管理界面密码，必须设置且不能为空 |
| `MOOR_USERNAME` | `moor` | 管理界面用户名 |
| `MOOR_PUBLIC_URL` | `http://localhost:9223` | 生成 MCP 客户端配置时使用的外部地址，也决定会话 Cookie 是否启用 `Secure` |
| `MOOR_BIND_ADDRESS` | `127.0.0.1` | Compose 映射端口绑定的宿主机地址 |
| `MOOR_HTTP_PORT` | `9223` | Compose 映射到宿主机的端口 |
| `MOOR_MCP_TOKEN` | 自动生成 | 仅用于旧部署首次升级时导入已有 Token；数据库已有 Token 后不再覆盖 |

如需让同一局域网直接访问端口，可以显式设置 `MOOR_BIND_ADDRESS=0.0.0.0`。此方式绕过宿主机反向代理，必须自行保证防火墙和 HTTPS 安全。

## 认证边界

- `/`、前端静态资源和 `/api/auth/*` 可公开加载，以便显示登录页
- `/api/health` 无需认证，供容器健康检查使用
- 其他 `/api/*` 使用管理会话；主动携带 Basic Header 的脚本仍兼容
- `/mcp` 只接受 `Authorization: Bearer <MCP Token>`
- 未认证的管理 API 返回 JSON 401，但不发送 Basic challenge，因此不会触发浏览器原生登录框
- 带 `Origin` 的跨源请求会被拒绝；Nginx 必须保留原始 `Host`

管理 API 自动化示例：

```bash
curl --user 'moor:your-password' https://moor.example.com/api/runtime
```

## 更新

```bash
docker compose pull
docker compose up -d
```

升级到自动 Token 版本时，如果旧部署仍设置了 `MOOR_MCP_TOKEN`，Moor 会在数据库尚无 Token 时导入该值，保持现有客户端连接不变。导入完成后可以从部署环境中删除该变量。

## 数据与备份

Moor 数据保存在 `/data/moor.db`。数据库包含 Server 配置、设置、审计日志和可恢复的 MCP Token，应把整个数据卷视为敏感数据。

备份前可以停止容器，或同时保存 SQLite 的 `moor.db`、`moor.db-wal` 和 `moor.db-shm`：

```bash
docker compose stop moor
# 备份 moor-data 数据卷
docker compose start moor
```

## MCP Server 运行时

官方镜像只包含 Moor Rust 服务和 CA 证书，不包含 Node.js、Python、uv 或其他 stdio 运行时。远程 HTTP MCP Server 可直接使用；如需运行 stdio Server，应基于官方镜像构建派生镜像并安装对应命令和运行时。

## 本地开发

需要 Node.js 24、pnpm 11 和 Rust 1.88 或更高版本。

```bash
corepack enable
pnpm install
MOOR_PASSWORD='development-password' pnpm server:dev
```

另开一个终端启动 Vite：

```bash
pnpm dev
```

前端默认访问 <http://localhost:1420>，并把 `/api` 与 `/mcp` 代理到 `http://127.0.0.1:9223`。

完整验证命令：

```bash
pnpm lint
pnpm test
pnpm build:frontend
cargo test --locked --manifest-path backend/Cargo.toml
```

## GHCR 发布

推送 `main` 或 `v*` 标签后，[Container workflow](.github/workflows/container.yml) 会验证前端与 Rust 后端，构建 `linux/amd64` 和 `linux/arm64` 镜像并发布到 GHCR，同时附带 SBOM 和 provenance。工作流生成 `latest`、语义版本、分支和提交 SHA 标签。

## License

本项目按照 Apache License 2.0 发布。修改版本保留原项目来源说明，容器镜像内也包含 `LICENSE` 和 `NOTICE`。详见 [LICENSE](LICENSE) 与 [NOTICE](NOTICE)。
