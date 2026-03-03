## Context

Axum 的 `tower_http::services::ServeDir` 根据文件扩展名推断 MIME 类型（通过 `mime_guess`），但不附加 `charset` 参数。对于 `text/css` 和 `text/javascript`，HTTP 规范历史上默认 `text/*` 类型为 ISO-8859-1（RFC 2616 §3.7.1），导致包含 UTF-8 中文字符的文件被浏览器错误解码，出现乱码。

HTML 文件已通过 `axum::response::Html` 显式设置 `text/html; charset=utf-8`，不受影响。`sw.js` 通过显式路由返回 `application/javascript`，同样缺少 charset。

## Goals / Non-Goals

**Goals:**
- 所有 `text/*` 响应自动附带 `charset=utf-8`
- `sw.js` 显式路由也包含 charset
- 零破坏性：不影响已有 charset 的响应，不修改非文本类型

**Non-Goals:**
- 不改变 ServeDir 本身或引入自定义 MIME 映射
- 不处理 `application/*` 类型（除 sw.js 外），因为 `application/javascript` 不受 ISO-8859-1 默认规则影响

## Decisions

### 使用 `axum::middleware::map_response` 全局中间件

**选择**: 在整个 app 上添加 `map_response` 层，检查 Content-Type 头，对 `text/*` 且无 charset 的响应追加 `; charset=utf-8`。

**替代方案**:
- 逐路由添加 charset — 维护负担大，新增静态文件容易遗漏
- 自定义 ServeDir 替代品 — 过度工程化
- 在 Fly.io CDN 层配置 — 不可控，且本地开发环境无法复现

**理由**: 全局中间件一处改动覆盖所有场景，且条件判断确保不会覆盖已有 charset。

### sw.js 路由硬编码 charset

**选择**: 将显式路由的 Content-Type 从 `application/javascript` 改为 `application/javascript; charset=utf-8`。

**理由**: sw.js 是显式路由，不经过 ServeDir，也不经过 `map_response`（`application/*` 不是 `text/*`）。直接在路由定义中修改最简单。

## Risks / Trade-offs

- [中间件性能] 每个响应都会检查 Content-Type 头 → 开销极小（一次字符串比较），可忽略
- [误修改] 可能给不需要 charset 的 `text/plain` 也加上 → 无害，UTF-8 是正确的通用选择
