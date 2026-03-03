## Context

Next 是单体应用（Axum + SQLite + Vanilla JS），部署在 Fly.io。当前无前端错误收集、后端日志全用 `eprintln!()`、手机端无调试手段。需要在不引入外部基础设施的前提下建立可观测性。

## Goals / Non-Goals

**Goals:**
- 前端全局错误拦截 + 操作轨迹 → 上报到 SQLite，通过 `fly ssh` 查询
- 后端 `tracing` 结构化日志替代 `eprintln!()`，TraceLayer 自动记录请求
- 手机端按需加载 Eruda.js DevTools
- SW 错误捕获增强

**Non-Goals:**
- 不引入外部监控服务（Sentry、Datadog）
- 不做全量 Session Replay
- 不做管理后台（fly ssh + sqlite3 够用）
- 不做 Request ID 前后端关联
- 不批量迁移旧 `eprintln!()`

## Decisions

### 1. 前端错误收集：独立 `observability.js` 模块

**选择**: 新建 `frontend/assets/js/observability.js` 封装所有可观测性逻辑（全局拦截、breadcrumb、上报、离线缓冲）。

**替代方案**: 分散到各模块中。
**理由**: 集中管理便于开关和维护，新模块无需任何代码即被覆盖。

### 2. Breadcrumb 注入点：`API.request()` 内部

**选择**: 在 `api.js` 的 `request()` 函数中添加 breadcrumb 记录调用。

**替代方案**: 用 fetch 拦截器（monkey-patch `window.fetch`）。
**理由**: `API.request()` 是所有业务请求的唯一入口，直接注入最简单可靠，不影响第三方库的 fetch 行为。

### 3. 导航 breadcrumb：复用 `switchPage()`

**选择**: 在 `app.js` 的 `switchPage()` 中添加导航 breadcrumb 记录。

**理由**: 所有 tab 切换都经过此函数，单一注入点。

### 4. 后端 tracing：紧凑格式，不做 JSON

**选择**: `tracing-subscriber` 默认紧凑格式。
**替代方案**: JSON 格式输出。
**理由**: 当前规模不需要机器解析日志，`fly logs` 人眼可读更重要。

### 5. Eruda.js：自托管 + 动态加载

**选择**: 将 `eruda.min.js` 放在 `frontend/assets/vendor/`，不加入 SW 缓存，按需动态 `<script>` 加载。

**替代方案**: CDN 加载。
**理由**: 自托管避免 CDN 不可用时失败；不入 SW 缓存避免正常用户缓存体积膨胀。

### 6. 上报端点：无需认证 + IP 限流

**选择**: `POST /api/client-errors` 无需 cookie/token 认证，IP 级限流 10 条/分钟。

**理由**: 需要捕获登录页错误（用户未认证状态）。IP 限流防滥用。

## Risks / Trade-offs

- **[Risk] 错误上报被恶意刷量** → IP 限流 10/min + 前端会话限制 10 条 + 去重
- **[Risk] localStorage 存满** → 离线缓冲最多 5 条，有上限保护
- **[Risk] Eruda.js 文件较大（~400KB）** → 仅按需加载，不入 SW 缓存，不影响正常用户
- **[Risk] tracing 依赖增加编译时间** → 可接受，tracing 是 Rust 生态标准库
- **[Trade-off] 不做管理后台** → 数据只能通过 fly ssh 查询，但够用
