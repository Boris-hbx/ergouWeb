# 洞察工厂生产 Worker 运维（T-217）

> 面向 PM：如何注入 codex 订阅身份、部署，并判断 worker 正在跑 / 失败 / 空闲。
> 架构：服务器主机托管 codex 订阅身份（方案 B）。Web-only 体验，Boris 不进终端。

## 1. 运行时构成

- 运行时镜像内置 codex musl 静态二进制：`/usr/local/bin/codex`（`Dockerfile` ARG `CODEX_VERSION`，当前 `0.140.0`，无 Node）。
- 订阅登录态放持久卷：`CODEX_HOME=/data/.codex`（`fly.toml [env]`），重启存活。
- `start.sh` 在 root 阶段把 Fly secret `CODEX_AUTH_JSON_B64`（base64）解码写入 `/data/.codex/auth.json`，`chmod 600` 并 chown 给 `nextapp`(uid 999)；worker 以 nextapp 运行。
- worker 默认开启（未设 `INSIGHT_FACTORY_WORKER`）。
- **严禁**设 `OPENAI_API_KEY`：worker 检测到会 block，坚持订阅路径、不走 API 计费。

## 2. PM 注入 auth.json + 部署（W 不做这步）

```bash
# 1) 本地用订阅身份登录 codex
codex login

# 2) 取登录态并 base64（去掉换行）
#    Windows PowerShell:
#    [Convert]::ToBase64String([IO.File]::ReadAllBytes("$env:USERPROFILE\.codex\auth.json"))
#    *nix:
base64 -w0 ~/.codex/auth.json

# 3) 注入为 Fly secret
"C:/Users/huai/.fly/bin/flyctl.exe" secrets set CODEX_AUTH_JSON_B64='<上一步输出>' -a next-boris

# 4) 部署（发版流程：dev 合 main 后从 main 部署）
"C:/Users/huai/.fly/bin/flyctl.exe" deploy
```

## 3. 容器内烟测（部署后）

```bash
"C:/Users/huai/.fly/bin/flyctl.exe" ssh console -a next-boris
# 容器内：
codex --version                 # 二进制装进去了吗
ls -l /data/.codex/auth.json    # 登录态注入了吗（属主应为 nextapp）
```

## 4. 怎么判断 worker 状态（不进容器）

### 4.1 受鉴权诊断接口
`GET /api/insight-factory/worker/health`（需登录会话）：

| 字段 | 含义 |
|------|------|
| `status=ready` + `authPresent=true` | worker 就绪，可生成 |
| `status=blocked` + `gate=auth_missing` | codex 在但 auth.json 缺失 → 需注入/重注 secret |
| `gate=cli_unavailable` / `cliAvailable=false` | codex 二进制没装进去或不可执行 |
| `gate=api_key_detected` | 误设了 OPENAI_API_KEY，需删 |
| `lastRefresh` | auth.json 上次刷新时间，用于判断登录态新旧 |

### 4.2 Fly 日志（结构化）
worker 每处理一个 job 打一行：成功 `info`、失败/阻塞 `warn`，含 `job_id / task_id / mode / provider / status / error`。
`error` 带分类前缀，便于定位：

- `[codex_missing]` — 容器内没有 codex（os error 2）。
- `[auth_expired]` — 登录态失效（401/未登录）→ 重注 auth.json。
- `[sandbox_failed]` — 沙箱初始化失败 → 确认 `INSIGHT_FACTORY_CODEX_SANDBOX` 为 bypass。
- `[quota_blocked]` — 命中 API key 闸门。
- `[timeout]` / `[other]`。

无日志输出 + `worker/health=ready` = 空闲（没有 pending job）。

## 5. 沙箱开关

- 默认 `bypass`（容器即隔离边界）。
- 需要回退到 codex 自带只读沙箱：`flyctl secrets set INSIGHT_FACTORY_CODEX_SANDBOX=read-only`，无需改代码。

## 6. 已知风险

- refresh-token 最终会过期 → `worker/health` 报 `auth_missing` 或 job `[auth_expired]` 时，PM 重新 `codex login` 取 auth.json 重注。本令不做自动续期。
- 256MB VM + 大 codex 二进制：codex exec 期间内存可能吃紧，必要时升 VM size。
- 镜像构建新增 GitHub release 下载，偶发网络失败会让构建失败，重试即可。
