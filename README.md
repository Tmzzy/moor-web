# Moor Web

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Container](https://img.shields.io/badge/ghcr.io-tmzzy%2Fmoor--web-2496ED?logo=docker&logoColor=white)](https://github.com/Tmzzy/moor-web/pkgs/container/moor-web)

Moor Web 是一个自托管的 MCP 网关管理工具，由 Rust/Axum 后端和 React 前端组成。它将多个 MCP Server 聚合到统一的 `/mcp` 端点，并提供 Server、Profile、工具开关、配置导入、客户端配置和审计日志管理。

本项目基于 [varandrew/moor](https://github.com/varandrew/moor) 修改为面向 Docker 的 Web 服务。来源和许可证信息见 [NOTICE](NOTICE) 与 [LICENSE](LICENSE)。

## 功能

- 管理 stdio 与远程 HTTP MCP Server
- 使用可并行访问的 Profile 组合 Server，并控制工具暴露范围
- 为每个 Profile 生成独立 MCP Token，同一 Server 可关联多个 Profile
- 扫描、导入和转换常见 MCP 客户端配置
- 为 Claude Code、Codex、OpenCode 和 Cursor 生成连接片段
- 通过 SSE 实时同步 Server 状态和设置变更
- 记录、筛选并脱敏展示 MCP 工具调用审计日志
- 使用独立登录页保护管理界面，自动生成每个 Profile 的 MCP Access Token

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

打开 <http://localhost:9223>，使用用户名 `moor` 和 `.env` 中的密码登录。Moor 会创建初始的 `Main` Profile，并为每个 Profile 自动生成独立 Token，保存到 `/data/moor.db`。

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

## 首次登录与 Profile Token

管理界面使用 Moor 自己的登录页。登录成功后，服务端签发 HttpOnly、SameSite Cookie；会话连续 7 天无活动后过期，退出登录或重启 Moor 也会使当前会话失效。Moor 前端不会把管理密码写入 localStorage 或 sessionStorage，也不会再显示原生 Basic Auth 弹窗。

进入 **Client Config** 页面并明确选择一个 Profile，或者直接进入 Profile 详情页，即可管理该 Profile 的 Token：

- 按需显示 Token
- 直接复制 Token
- 确认后轮换 Token

Token 默认不会随普通管理 API 返回，也不会写入日志。轮换只影响对应 Profile，并会立即使它的旧 Token 对后续请求失效；其他 Profile 的 Token 不受影响。

客户端配置片段使用 `MOOR_PROFILE_TOKEN` 环境变量。复制所选 Profile 的 Token，并在对应 MCP 客户端的环境中设置：

```bash
export MOOR_PROFILE_TOKEN='moor_replace_with_the_generated_value'
```

默认 MCP 地址为：

```text
http://localhost:9223/mcp
```

### Profile Token 与多账号隔离

每个 Profile 都有独立的 MCP Token。所有客户端仍连接同一个 `/mcp` 地址，Moor 根据 Bearer Token 将每个请求固定路由到对应 Profile。因此多个 Profile 可以被不同客户端同时使用，系统中不存在全局 Token、活动 Profile 或默认 Profile。

这套模型只有两条核心规则：

- 一个 Profile 对应一个独立 Profile Token 和一个确定的工具视图。
- 一个 MCP Server 只保存一份配置并运行一个实例，但可以显式关联一个或多个 Profile。

个人和公司 GitHub 账号可以按下面的方式配置：

1. 创建 `Personal` 和 `Work` 两个 Profile。
2. 创建两个独立的 GitHub MCP Server，分别保存个人和公司的 GitHub Token。
3. 将个人 GitHub Server 只关联到 `Personal`，公司 GitHub Server 只关联到 `Work`。
4. 对需要共用的 MCP Server，在创建、导入或 Server 详情页的 **Profiles** 区域一次选择两个或更多 Profile。Server 配置和运行实例只保留一份，不会复制。
5. 在两个客户端中分别使用对应 Profile 详情页的 Token；它们可以同时连接 `/mcp`。

Profile 提供的是 MCP 网关层的可见性与调用路由隔离，不是操作系统或多租户沙箱。所有 stdio Server 仍由同一个 Moor 服务用户运行，管理员也能查看和修改所有 Server 配置。公司凭据应只放在公司的 GitHub Server 配置中，不要放进共用 Server。

完整的配置方式、显式 Profile 规则、管理 API 示例和升级边界见 [Profile 认证与 MCP Server 共享](docs/profile-auth-and-server-sharing.md)。

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

如需让同一局域网直接访问端口，可以显式设置 `MOOR_BIND_ADDRESS=0.0.0.0`。此方式绕过宿主机反向代理，必须自行保证防火墙和 HTTPS 安全。

## 认证边界

- `/`、前端静态资源和 `/api/auth/*` 可公开加载，以便显示登录页
- `/api/health` 无需认证，供容器健康检查使用
- 其他 `/api/*` 使用管理会话；主动携带 Basic Header 的脚本仍兼容
- `/mcp` 只接受 `Authorization: Bearer <Profile Token>`，并固定路由到该 Token 所属的 Profile
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

从使用全局 MCP Token 的版本升级属于破坏性升级。原全局 Token 不会导入，也不能继续访问 `/mcp`；升级后必须从每个 Profile 获取各自的新 Token，并更新对应客户端。服务端不再读取 `MOOR_MCP_TOKEN`，客户端配置使用 `MOOR_PROFILE_TOKEN`。升级前先备份数据卷，完整迁移说明见 [升级边界](docs/profile-auth-and-server-sharing.md#升级边界)。

## 数据与备份

Moor 数据保存在 `/data/moor.db`。数据库包含 Server 配置、设置、审计日志和各 Profile Token，应把整个数据卷视为敏感数据。Node.js 和 Python stdio Server 首次运行后，`/data/runtime` 还会保存 npm/uv 缓存、uv 管理的 Python 与工具环境；这些运行时数据可以重新下载，但会增加数据卷和备份体积。

备份前可以停止容器，或同时保存 SQLite 的 `moor.db`、`moor.db-wal` 和 `moor.db-shm`：

```bash
docker compose stop moor
# 备份 moor-data 数据卷
docker compose start moor
```

## MCP Server 运行时

官方镜像包含 Node.js 24、npm/npx 和 uv/uvx，可直接运行 Node.js 或 Python 包形式的 stdio MCP Server。镜像不预装 system Python；`uvx` 第一次运行 Python 工具时会按需下载 uv-managed Python。下载的 Python、工具环境和包缓存都保存在 `/data/runtime`，后续启动会复用数据卷中的内容。

添加 Node.js MCP Server 时，Transport 选择 `stdio`、Launcher 选择 `npx`。对应的底层配置仍然是标准 stdio 配置：

```json
{
  "connectionType": "stdio",
  "command": "npx",
  "args": [
    "--yes",
    "@modelcontextprotocol/server-filesystem",
    "/data"
  ]
}
```

添加 Python MCP Server 时，Launcher 选择 `uvx`：

```json
{
  "connectionType": "stdio",
  "command": "uvx",
  "args": ["--with", "mcp<2", "mcp-server-fetch"]
}
```

这里对 `mcp` 使用 `<2` 约束，是因为当前 `mcp-server-fetch` 仍使用 MCP Python SDK 1.x API。其他已经适配 SDK 2.x 的 Python Server 不需要这个约束；Moor 会把参数原样交给 `uvx`。

首次下载需要容器能够访问对应的 npm、PyPI、GitHub/Astral 下载源，耗时也会高于缓存后的启动。新部署的 MCP Server 启动超时默认为 120 秒；升级部署会保留数据库中已有的超时值，可在 **Settings > Advanced > Server Start Timeout** 中调整。

`npx` 和 `uvx` 可以下载并执行任意第三方代码。stdio 子进程与 `moor-server` 使用相同的 `moor` 用户，继承 Moor 的运行环境，并可以访问 `/data` 和容器网络。因此只有受信任的管理员才能添加或修改 Server，包名和版本也应由管理员审核。当前运行模型面向自托管单管理员场景，不提供多租户沙箱、包白名单或 Marketplace 隔离。

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
