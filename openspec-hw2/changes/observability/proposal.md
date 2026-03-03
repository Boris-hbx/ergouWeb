## Why

用户报告问题时，无法有效定位根因：前端无全局错误拦截、后端日志全用 `eprintln!()`、手机端无调试手段。需要让「用户报告 → 定位根因」的路径从"猜"变成"查"。

## What Changes

- 前端添加全局错误捕获（`window.onerror` + `unhandledrejection`）+ 操作轨迹（Breadcrumb 环形缓冲区）
- 新增 `POST /api/client-errors` 端点，前端错误上报到 SQLite `client_errors` 表
- 前端离线缓冲：上报失败存 localStorage，下次加载补发
- 后端添加 `tracing` + `tracing-subscriber` + `TraceLayer` 中间件，替代 `eprintln!()`
- 自托管 Eruda.js，URL 参数或隐藏手势触发手机端 DevTools
- Service Worker 错误捕获增强

## Capabilities

### New Capabilities
- `client-error-reporting`: 前端全局错误拦截 + 操作轨迹 + 上报端点 + SQLite 存储 + 离线缓冲 + 自动清理
- `backend-tracing`: 后端 tracing 初始化 + TraceLayer 中间件 + 日志级别规范
- `mobile-devtools`: Eruda.js 自托管 + 条件加载（URL 参数 + 隐藏手势）

### Modified Capabilities
- `frontend-safety`: SW 错误捕获增强，静默 catch 块加 console.error

## Impact

- **后端依赖**: 新增 `tracing`, `tracing-subscriber`, `tower-http`（TraceLayer）
- **前端**: 新增 `frontend/assets/js/observability.js`，修改 `api.js`（Breadcrumb 记录）、`sw.js`（错误捕获）
- **前端资源**: 新增 `frontend/assets/vendor/eruda.min.js`（~400KB，不入 SW 缓存）
- **数据库**: 新增 `client_errors` 表
- **文档**: 更新 CLAUDE.md、BACKEND.md、FRONTEND.md
