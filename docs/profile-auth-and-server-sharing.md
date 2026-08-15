# Profile 认证与 MCP Server 共享

Moor 使用 Profile Token 作为 `/mcp` 的唯一认证和路由依据。多个 Profile 可以同时连接同一个 `/mcp` 端点；需要复用的 MCP Server 通过 Profile 关联实现共享，不需要复制 Server 配置，也不需要全局 MCP Token。

## 核心模型

```text
一个 Profile = 一个独立 Profile Token = 一个确定的工具视图
一个 MCP Server = 一份连接配置 + 一个运行实例 = 可显式关联多个 Profile
```

| 概念 | 职责 | 隔离或共享方式 |
| --- | --- | --- |
| Profile | 定义一个 MCP 客户端身份及其可见工具范围 | 每个 Profile 独立存在，可被不同客户端并行使用 |
| Profile Token | 认证 `/mcp` 请求并确定唯一 Profile | 每个 Profile 独立生成，Token 在数据库中唯一 |
| MCP Server | 保存上游连接方式、环境变量、请求头和运行状态 | 配置和运行实例只有一份，可以关联多个 Profile |
| Profile-Server 关联 | 决定某个 Profile 能否使用某个 Server | 每个关联可以独立启用或禁用 Server，并控制工具开关 |

Profile Token 与 GitHub Token 等上游凭据不是同一种 Token：

- Profile Token 由 MCP 客户端发送给 Moor，用于确定请求属于哪个 Profile。
- GitHub Token 保存在对应 GitHub MCP Server 的环境变量或请求头中，由该 Server 访问 GitHub。

把一个带凭据的 Server 关联给多个 Profile，意味着这些 Profile 会使用同一个上游账号。需要隔离个人和公司账号时，必须创建两个 GitHub MCP Server；只有允许共享同一上游身份的 Server 才应关联给多个 Profile。

## 并行访问与请求路由

所有 MCP 客户端都访问相同地址：

```text
https://moor.example.com/mcp
```

每次请求必须携带某个 Profile 的 Token：

```http
Authorization: Bearer <profile-token>
```

Moor 根据 Token 找到唯一 Profile，并只暴露该 Profile 已关联且已启用的 Server 和工具。请求不读取全局状态，也不会切换其他连接使用的 Profile，因此 Personal、Work 等多个 Profile 可以同时保持连接并并发调用。

系统没有以下状态或回退规则：

- 全局 MCP Token
- 活动 Profile
- 特殊的默认 Profile
- 未指定 Profile 时选择第一个 Profile

轮换某个 Profile 的 Token 后，只有该 Profile 的旧 Token 立即失效，其他 Profile 的连接和 Token 不受影响。

## 个人与公司 GitHub 账号

推荐拓扑如下：

```text
Personal 客户端
  +-- Bearer Personal Profile Token --> /mcp --> Personal Profile
                                                +-- GitHub Personal Server
                                                +-- Shared Server

Work 客户端
  +-- Bearer Work Profile Token -----> /mcp --> Work Profile
                                                +-- GitHub Work Server
                                                +-- Shared Server
```

配置步骤：

1. 创建 `Personal` 和 `Work` 两个 Profile。Moor 会分别生成 Token。
2. 创建 `GitHub Personal` Server，在它的环境变量或请求头中保存个人 GitHub Token，并且只关联 `Personal`。
3. 创建 `GitHub Work` Server，在它的环境变量或请求头中保存公司 GitHub Token，并且只关联 `Work`。
4. 对文件系统、搜索或其他允许共用的 Server，在 **Profiles** 中同时选择 `Personal` 和 `Work`。
5. 为个人与公司的 MCP 客户端分别设置对应 Profile Token，然后同时连接 `/mcp`。

`Shared Server` 在数据库中只有一条配置，也只有一个运行实例。两个 Profile 可以分别控制它是否启用以及哪些工具可见，但它们共享 Server 的连接参数、环境变量、请求头和进程状态。

## 显式 Profile 规则

任何可能改变 Server 所属范围或展示 Profile 工具视图的操作都要求显式选择 Profile。

| 操作 | Profile 参数 | 未提供时的行为 |
| --- | --- | --- |
| 创建 Server | JSON `profileIds`，至少一个 | 请求被拒绝 |
| 执行配置导入 | JSON `profileIds`，至少一个 | 请求被拒绝 |
| 修改 Server 关联 | JSON `profileIds`，完整的目标列表 | 只按提交的列表更新，不自动补充 Profile |
| 查询 Server 工具 | 查询参数 `profile_id` | 请求被拒绝 |
| Client Config 页面 | 手动选择 Profile | 不显示任何 Profile Token |
| Server 工具区域 | 手动选择已关联的 Profile | 不查询或展示工具视图 |
| 调用 `/mcp` | `Authorization` 中的 Profile Token | 返回 `401 Unauthorized` |

创建或导入时可以一次选择多个 Profile。后续也可以在 Server 详情页的 **Profiles** 区域修改关联，无需复制或重新创建 Server。

## 管理 API 示例

以下 `/api/*` 请求使用管理会话或管理账号的 Basic Authentication；不要用 Profile Token 调用管理 API。示例中的 ID 先通过 `GET /api/profiles` 获取。

### 创建一个共享 Server

`POST /api/servers`

```json
{
  "name": "Shared Filesystem",
  "connectionType": "stdio",
  "command": "npx",
  "args": [
    "--yes",
    "@modelcontextprotocol/server-filesystem",
    "/data/shared"
  ],
  "autoStart": true,
  "profileIds": [
    "personal-profile-id",
    "work-profile-id"
  ]
}
```

`profileIds` 不能为空。列表中的每个 Profile 必须已经存在；整个创建操作在参数无效时失败，不会创建未关联的 Server。

### 修改已有 Server 的 Profile 关联

`PUT /api/servers/{serverId}/profiles`

```json
{
  "profileIds": [
    "personal-profile-id",
    "work-profile-id"
  ]
}
```

该字段表示更新后的完整关联列表，不是增量追加。提交空数组会清除该 Server 当前启用的全部 Profile 关联。

### 将导入的 Server 一次关联到多个 Profile

先通过扫描或解析接口获得 `servers`，再调用 `POST /api/import/execute`：

```json
{
  "servers": [
    {
      "name": "Shared Filesystem",
      "connectionType": "stdio",
      "command": "npx",
      "args": [
        "--yes",
        "@modelcontextprotocol/server-filesystem",
        "/data/shared"
      ],
      "source": "json-import"
    }
  ],
  "profileIds": [
    "personal-profile-id",
    "work-profile-id"
  ]
}
```

同一次导入中的每个新 Server 都会关联到这里明确列出的 Profile。`profileIds` 不能为空。

### 在指定 Profile 下查询工具

```http
GET /api/servers/{serverId}/tools?profile_id={profileId}
```

工具开关属于 Profile-Server 关联，因此工具列表必须带 `profile_id`。同一个 Server 在不同 Profile 下可能返回不同的启用状态。

### 读取和轮换 Profile Token

```http
GET  /api/profiles/{profileId}/mcp-token
POST /api/profiles/{profileId}/mcp-token/rotate
```

这两个接口返回 `Cache-Control: no-store`。轮换成功后应立即更新使用该 Profile 的所有 MCP 客户端。

客户端配置片段统一引用 `MOOR_PROFILE_TOKEN`：

```bash
export MOOR_PROFILE_TOKEN='moor_replace_with_the_selected_profile_token'
```

该变量属于 MCP 客户端的运行环境，不是 Moor 服务端配置。个人与公司客户端应在各自的进程、容器或环境文件中设置不同值。如果同一个 MCP 客户端进程支持注册多个 Moor 连接，可以为每个连接使用不同的客户端环境变量名，并让每个连接引用对应的 Profile Token。

## 升级边界

从全局 MCP Token 模型升级到 Profile Token 模型是一次破坏性升级。升级前先备份 `/data` 数据卷或完整的 SQLite 数据库文件。

升级后执行以下唯一规则：

- 原全局 MCP Token 不会导入，也不能继续访问 `/mcp`。
- Moor 服务端不读取 `MOOR_MCP_TOKEN`，应从部署环境中删除该变量。
- 原 `/api/security/mcp-token` 和 `/api/security/mcp-token/rotate` 不再存在。
- 每个已有 Profile 会获得一个独立的新 Token；新建 Profile 也会自动生成 Token。
- 客户端配置必须改为某个 Profile 的 Token，生成的片段使用 `MOOR_PROFILE_TOKEN`。
- 数据库迁移会删除旧的 `is_active` 字段和 `secrets` 表。
- 已有 Profile、Server、Profile-Server 关联和审计数据会保留。
- 已有名为 `Default` 的 Profile 保留原名称，但它只是普通 Profile，不具有特殊行为。

升级完成后：

1. 登录 Moor 管理界面。
2. 打开每个 Profile 的详情页或 **Client Config**，读取对应 Token。
3. 按客户端身份分别更新 Token 和 `MOOR_PROFILE_TOKEN`。
4. 使用每个 Profile 分别连接 `/mcp`，确认工具列表符合预期。
5. 删除部署系统、密钥管理器和旧客户端配置中不再使用的全局 Token。

使用原全局 Token 得到 `401 Unauthorized` 是预期行为，不存在旧 Token 回退路径。

## 安全边界

Profile 提供 MCP 网关层的认证、可见性和调用路由隔离，不是操作系统级或多租户安全沙箱：

- 所有 stdio Server 由同一个 Moor 服务用户运行。
- 共享 Server 的进程、连接参数、环境变量和请求头对所有关联 Profile 相同。
- Moor 管理员可以查看和修改全部 Profile 与 Server 配置。
- `/data/moor.db` 包含各 Profile Token 和 Server 凭据，备份必须按敏感数据保护。

因此，账号凭据不同的 Server 应保持独立；只有明确允许共用配置、进程和上游身份的 Server 才应关联多个 Profile。
