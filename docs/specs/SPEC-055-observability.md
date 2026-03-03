# SPEC-055: 前端可观测性与跨端调试体系
> 起草日期: 2026-02-28
> 状态: 草稿

## 背景

应用同时运行在桌面和手机浏览器上，当用户报告问题时，缺乏有效手段定位根因：

- **前端无全局错误拦截**：`window.onerror` / `unhandledrejection` 均未设置，未捕获的错误静默丢失
- **后端无结构化日志**：全部用 `eprintln!()`，无 tracing，日志不可搜索
- **手机端无调试手段**：没有远程调试工具，手机上出问题只能靠猜
- **Service Worker 静默吞错**：`catch(e) {}` 无任何记录

## 目标

使「用户报告问题 → 定位根因」的路径从"猜"变成"查"。

**设计原则**：
- 收集的数据必须有明确的消费方式，否则不收集
- 可观测性在中间件/全局层实现，新模块无需额外代码即被覆盖
- 复用现有技术栈（Axum + SQLite），不引入额外基础设施

## 设计

### 第一层：前端全局错误收集 + 操作轨迹

#### 1.1 捕获点

| 捕获点 | 覆盖范围 |
|--------|---------|
| `window.onerror` | 未捕获的同步错误 |
| `window.addEventListener('unhandledrejection')` | 未处理的 Promise 拒绝 |
| `API.request()` catch 增强 | 网络/业务错误（已有，增强） |
| SW `error` + `unhandledrejection` 事件 | Service Worker 错误 |

#### 1.2 操作轨迹（Breadcrumb）

内存环形缓冲区，容量 20 条，满时淘汰最旧的。

**只记录两类安全信息**（避免隐私泄露）：

| 类别 | 记录内容 | 来源 |
|------|---------|------|
| `api` | `METHOD /path → STATUS (耗时ms)` | `API.request()` |
| `nav` | Tab/视图切换的标识符 | 路由/Tab 变化 |

**不记录**：点击文本（可能含敏感信息）、input 内容、console 输出。

示例：
```
[14:30:02] api: POST /api/todos → 200 (145ms)
[14:30:05] nav: trips
[14:30:06] api: GET /api/trips → 200 (89ms)
[14:30:08] api: PUT /api/trips/42 → 500 (312ms)
```

#### 1.3 错误上报数据结构

```json
{
  "error_message": "Cannot read property 'name' of null",
  "stack": "TypeError: ...\n    at renderTrip (trip.js:142:15)",
  "app_version": "20260228a",
  "url": "https://next-boris.fly.dev/#trips",
  "user_agent": "Mozilla/5.0 ...",
  "screen_size": "375x667",
  "network_online": true,
  "user_id": "user-xyz",
  "breadcrumbs": [
    {"ts": "14:30:02", "cat": "api", "msg": "POST /api/todos → 200 (145ms)"},
    {"ts": "14:30:08", "cat": "api", "msg": "PUT /api/trips/42 → 500 (312ms)"}
  ],
  "timestamp": "2026-02-28T12:34:56.789Z"
}
```

#### 1.4 上报端点

- `POST /api/client-errors`
- 无需认证（允许登录页错误上报），IP 级限流：10 条/分钟
- 前端防洪：每个页面会话最多上报 10 条，同一 `error_message` 去重
- 离线缓冲：上报失败时存入 `localStorage`（最多 5 条），下次页面加载时补发

#### 1.5 存储

SQLite `client_errors` 表：

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER PK | 自增 |
| error_message | TEXT | 错误信息 |
| stack | TEXT | 堆栈（可为空） |
| app_version | TEXT | 前端版本号 |
| url | TEXT | 出错页面 |
| user_agent | TEXT | 浏览器/设备 |
| screen_size | TEXT | 屏幕尺寸 |
| network_online | INTEGER | 1=在线 0=离线 |
| user_id | TEXT | 可为空 |
| breadcrumbs | TEXT | JSON 数组 |
| created_at | TEXT | ISO 8601 |

自动清理：复用现有定时任务，每日清理 14 天前的记录。

---

### 第二层：后端结构化日志

#### 2.1 技术选型

| 组件 | 用途 |
|------|------|
| `tracing` + `tracing-subscriber` | 替代 `eprintln!()`，结构化日志 |
| `tower-http::TraceLayer` | Axum 中间件，自动记录每个请求的方法、路径、状态码、耗时 |

不做 Request ID 中间件。单体应用中 `user_id + timestamp` 足够定位问题。

#### 2.2 日志级别规范

| 级别 | 用途 | 示例 |
|------|------|------|
| `error!` | 需要排查的故障 | DB 错误、外部 API 失败 |
| `warn!` | 可恢复的异常 | 限流触发、token 过期 |
| `info!` | 关键业务事件 | 用户注册、AI 调用完成 |
| `debug!` | 开发调试 | SQL 参数、请求体详情 |

#### 2.3 日志输出

生产环境使用 `tracing-subscriber` 默认的紧凑格式（非 JSON），保持 `fly logs` 可读性：

```
2026-02-28T12:34:56Z ERROR [todos] db error: UNIQUE constraint user_id=xyz
2026-02-28T12:34:57Z INFO  [http] 200 PUT /api/trips/42 89ms
```

不做 JSON 格式切换——当前规模不需要机器解析日志，人眼可读更重要。

#### 2.4 迁移策略

**渐进式，不设专门阶段**：
1. 添加 `tracing` / `tracing-subscriber` 依赖，`main.rs` 初始化
2. 添加 `TraceLayer`（立即获得请求级自动日志）
3. **新代码**必须用 `tracing` 宏
4. **旧代码**的 `eprintln!()` 不专门迁移，遇到修改该文件时顺手替换
5. CLAUDE.md 约定：新代码禁止 `eprintln!()`

不使用 Clippy `disallowed-macros` 硬禁止——启动信息等场景用 `println!()` 是合理的，靠文档约定软性引导。

---

### 第三层：手机端现场调试（Eruda.js）

#### 3.1 方案

条件加载 Eruda.js，在手机浏览器中提供页面内 DevTools 面板（Console、Network、Elements、Storage）。

#### 3.2 加载方式

**自托管**：将 `eruda.min.js` 放在 `frontend/assets/vendor/eruda.min.js`。

- 不加入 SW 缓存列表（不影响正常用户的缓存体积）
- 按需动态 `<script>` 加载，仅在触发时发起请求
- 加载失败静默忽略（离线时不可用，可接受——离线时本身也无法上报错误）

#### 3.3 触发方式

| 方式 | 触发条件 | 说明 |
|------|---------|------|
| URL 参数 | `?debug=1` | 需已登录 |
| 隐藏手势 | 连续点击版本号 5 次 | 类似 Android 开发者选项 |

开关状态存入 `localStorage('eruda_enabled')`，刷新后保持。`?debug=0` 或再次 5 连击关闭。

---

## 数据消费方案

> **核心原则：没人看的数据不要收集。**

### 日常工作流

不设管理后台。通过 `fly ssh` + `sqlite3` 按需查询：

```bash
# 查看最近 24h 的错误概览（按消息分组计数）
sqlite3 /data/next.db "
  SELECT error_message, app_version, count(*) as cnt
  FROM client_errors
  WHERE created_at > datetime('now', '-1 day')
  GROUP BY error_message
  ORDER BY cnt DESC
  LIMIT 20;
"

# 查看某个错误的面包屑轨迹
sqlite3 /data/next.db "
  SELECT breadcrumbs, user_agent, screen_size, created_at
  FROM client_errors
  WHERE error_message LIKE '%Cannot read%'
  ORDER BY created_at DESC
  LIMIT 5;
"
```

### 部署后检查

每次部署后，等 1-2 小时，跑一次错误概览查询，确认新版本没引入回归。加入部署 checklist。

### 用户报告问题时

1. 问用户"大概什么时间？手机还是电脑？"
2. 按时间 + user_agent 过滤 `client_errors` 表
3. 看面包屑轨迹定位出错路径
4. 看 `fly logs` 的同一时间段后端日志

---

## 开发规范保障

### 文档更新（随实施同步）

| 文档 | 更新内容 |
|------|---------|
| `CLAUDE.md` 必知约定 | 新增：后端新代码用 `tracing` 宏，禁止 `eprintln`；前端 catch 禁止静默吞错 |
| `docs/ref/BACKEND.md` | 新增"日志规范"章节：级别定义、tracing 用法 |
| `docs/ref/FRONTEND.md` | 新增"错误处理"章节：全局拦截机制说明、面包屑记录规则 |

### 架构保障（零成本遵守）

新模块自动被覆盖，开发者不需要做任何事：
- `window.onerror` / `unhandledrejection` → 全局生效
- `API.request()` → 面包屑自动记录
- `TraceLayer` → 新路由自动有请求日志

唯一需要开发者注意的：**catch 块不能为空**，必须 `console.error('[模块名]', error)` 或 re-throw。

---

## 实施计划

| 阶段 | 内容 | 说明 |
|------|------|------|
| Phase A | 前端全局错误拦截 + 操作轨迹 + `POST /api/client-errors` + SQLite 表 + 离线缓冲 + 自动清理 | 一次做完，这是一个整体 |
| Phase B | Eruda.js 自托管 + 条件加载（URL 参数 + 隐藏手势） | 独立模块，不依赖 Phase A |
| Phase C | 后端 `tracing` 初始化 + `TraceLayer` 中间件 | 只加框架，不迁移旧代码 |
| Phase D | SW 错误捕获（给现有静默 catch 加上 console.error） | 小改动 |
| - | 文档更新（CLAUDE.md / BACKEND.md / FRONTEND.md） | 随各阶段同步 |
| - | 旧 `eprintln!()` → `tracing` 宏 | 不设专门阶段，修改文件时顺手替换 |

## 不做的事

- **不引入外部监控服务**（Sentry、Datadog 等）— 自建端点足够当前规模
- **不做全量 Session Replay** — API 轨迹 + 导航轨迹覆盖主要调试场景
- **不做 Request ID 前后端关联** — 单体应用，user_id + timestamp 够用
- **不做管理后台** — fly ssh + sqlite3 查询，够用再说
- **不做 Prometheus/Grafana** — 当前规模不需要指标监控
- **不记录点击文本** — 隐私风险高于调试价值
- **不用 Clippy 硬禁止 eprintln** — 文档约定软性引导，避免过度约束

## 修订记录

- 2026-02-28: 初稿
- 2026-02-28: v2 — 采纳评审意见精简方案
  - 面包屑从 50 条缩减到 20 条，砍掉点击/console 记录，只保留 API + 导航（隐私保护）
  - 砍掉 Request ID 关联（Phase F）— 单体应用不需要
  - Eruda 改为自托管、不入 SW 缓存 — 解决 PWA 离线冲突
  - eprintln 迁移改为渐进式，不设专门阶段 — 降低工作量
  - 砍掉 Clippy 硬禁止规则 — 避免过度约束
  - 新增"数据消费方案"章节 — 确保收集的数据有人看
  - 新增离线缓冲机制 — 解决服务器不可达时的错误丢失问题
  - 日志格式简化为单一可读格式，不做 JSON 切换
