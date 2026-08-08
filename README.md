# Moor Web

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Container](https://img.shields.io/badge/ghcr.io-tmzzy%2Fmoor--web-2496ED?logo=docker&logoColor=white)](https://github.com/Tmzzy/moor-web/pkgs/container/moor-web)

Moor Web 是一个由 Rust/Axum 后端和 React 前端组成的 MCP 网关管理工具。它将多个 MCP Server 聚合到一个 `/mcp` 端点，并提供 Server、Profile、工具开关、配置导入和审计日志管理。

本项目基于 [varandrew/moor](https://github.com/varandrew/moor) 修改为 Docker 部署的 Web 服务。原项目来源和许可证信息见 [NOTICE](NOTICE) 与 [LICENSE](LICENSE)。

## Docker 安装

镜像发布到 `ghcr.io/tmzzy/moor-web`，支持 `linux/amd64` 和 `linux/arm64`。

```bash
export MOOR_PASSWORD='replace-with-a-long-password'
export MOOR_MCP_TOKEN='replace-with-a-random-token'
docker compose up -d
```

打开 <http://localhost:9223>，使用用户名 `moor` 和上面设置的密码登录。MCP 客户端连接地址为：

```text
http://localhost:9223/mcp
```

管理面板生成的 Claude Code、Codex、OpenCode 和 Cursor 配置会通过客户端环境变量 `MOOR_MCP_TOKEN` 发送 Bearer Token。启动 MCP 客户端前，在客户端所在环境设置与服务端相同的值；真实 Token 不会由管理 API 返回到浏览器。

更新镜像：

```bash
docker compose pull
docker compose up -d
```

不使用 Compose 时：

```bash
docker run -d \
  --name moor \
  --restart unless-stopped \
  -p 9223:9223 \
  -e MOOR_PASSWORD='replace-with-a-long-password' \
  -e MOOR_MCP_TOKEN='replace-with-a-random-token' \
  -e MOOR_PUBLIC_URL='http://localhost:9223' \
  -v moor-data:/data \
  ghcr.io/tmzzy/moor-web:latest
```

## 配置

| 环境变量 | 默认值 | 说明 |
| --- | --- | --- |
| `MOOR_PASSWORD` | 无 | 管理界面和 `/api/*` 的 Basic Auth 密码，必须设置 |
| `MOOR_MCP_TOKEN` | 无 | `/mcp` 的 Bearer Token，必须设置 |
| `MOOR_USERNAME` | `moor` | Basic Auth 用户名 |
| `MOOR_PUBLIC_URL` | `http://localhost:9223` | 生成 MCP 客户端配置时使用的外部地址 |
| `MOOR_HTTP_PORT` | `9223` | Compose 暴露到宿主机的端口 |

Compose 中可用 `MOOR_HTTP_PORT` 修改宿主机端口，同时应把 `MOOR_PUBLIC_URL` 改为实际访问地址：

```bash
MOOR_HTTP_PORT=8080 \
MOOR_PUBLIC_URL='https://moor.example.com' \
MOOR_PASSWORD='replace-with-a-long-password' \
MOOR_MCP_TOKEN='replace-with-a-random-token' \
docker compose up -d
```

`/api/health` 无需认证；管理界面和其他 `/api/*` 使用 Basic Auth；`/mcp` 必须发送 `Authorization: Bearer <MOOR_MCP_TOKEN>`。Basic Auth 和 Bearer Token 都依赖传输层保密，公网部署必须使用 HTTPS 反向代理。

## MCP Server 运行时

官方镜像只包含 Moor Rust 服务和 CA 证书，不包含 Node.js、Python、uv 或其他 stdio 运行时，因此远程 HTTP MCP Server 是开箱即用的上游类型。代码仍保留 stdio 支持；需要 stdio 时，应基于官方镜像构建派生镜像并安装对应命令和运行时。

## 数据

Moor 数据保存在 `/data/moor.db`。备份前可停止容器，或同时保存 SQLite 的 `moor.db`、`moor.db-wal` 和 `moor.db-shm` 文件。

## GHCR 发布

推送 `main` 或 `v*` 标签后，[Container workflow](.github/workflows/container.yml) 会先验证前端与 Rust 后端，再构建 `linux/amd64` 和 `linux/arm64` 镜像并发布到 GHCR，同时附带 SBOM 和 provenance。工作流生成 `latest`、语义版本、分支和提交 SHA 标签。

仓库需要允许 GitHub Actions 对 Packages 的写权限；镜像首次发布后请在 Package settings 中设为公开。

## License

本项目按照 Apache License 2.0 发布。修改版本保留原项目来源说明，容器镜像内也包含 `LICENSE` 和 `NOTICE`。详见 [LICENSE](LICENSE) 与 [NOTICE](NOTICE)。
