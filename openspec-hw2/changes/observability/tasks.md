## 1. 前端全局错误拦截 + Breadcrumb + 上报

- [x] 1.1 创建 `frontend/assets/js/observability.js`：全局错误拦截（window.onerror + unhandledrejection）、breadcrumb 环形缓冲区（20 条）、错误上报（组装 payload + POST /api/client-errors）、会话限制（10 条 + 去重）、离线缓冲（localStorage 最多 5 条，页面加载时补发）
- [x] 1.2 `api.js` 的 `request()` 中添加 breadcrumb 记录（api 类别：METHOD /path → STATUS (duration_ms)）
- [x] 1.3 `app.js` 的 `switchPage()` 中添加导航 breadcrumb 记录（nav 类别）
- [x] 1.4 `index.html` 中引入 `observability.js`（在其他 JS 之前加载，确保全局拦截先就位）

## 2. 后端 client-errors 端点 + SQLite 表

- [x] 2.1 `db.rs` 添加 `client_errors` 表创建（id, error_message, stack, app_version, url, user_agent, screen_size, network_online, user_id, breadcrumbs, created_at）+ 14 天自动清理
- [x] 2.2 新建 `routes/observability.rs`：`POST /api/client-errors` 处理器，无需认证，IP 限流 10/min，写入 SQLite
- [x] 2.3 `main.rs` 注册路由

## 3. 后端 tracing 初始化

- [x] 3.1 `Cargo.toml` 添加 `tracing`、`tracing-subscriber`、`tower-http`（trace feature）依赖
- [x] 3.2 `main.rs` 初始化 `tracing-subscriber`（紧凑格式，默认 info 级别）
- [x] 3.3 `main.rs` 添加 `TraceLayer` 中间件到 Axum router

## 4. Eruda.js 手机端调试

- [x] 4.1 下载 `eruda.min.js` 到 `frontend/assets/vendor/eruda.min.js`
- [x] 4.2 `observability.js` 中添加 Eruda 条件加载逻辑：URL 参数 `?debug=1/0` + localStorage 持久化 + 隐藏手势（版本号 5 连击）+ 加载失败静默忽略

## 5. Service Worker 错误捕获增强

- [x] 5.1 `sw.js` 中所有静默 catch 块添加 `console.error('[SW]', error)`
- [x] 5.2 `sw.js` 添加全局 `error` 和 `unhandledrejection` 事件监听

## 6. 文档更新

- [x] 6.1 `CLAUDE.md` 必知约定：新增后端用 tracing 宏、前端 catch 不能为空
- [x] 6.2 `docs/ref/BACKEND.md` 新增"日志规范"章节
- [x] 6.3 `docs/ref/FRONTEND.md` 新增"错误处理与可观测性"章节
