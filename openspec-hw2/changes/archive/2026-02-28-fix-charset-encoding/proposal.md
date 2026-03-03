## Why

CSS/JS 静态文件通过 `ServeDir` 提供时，Content-Type 头缺少 `charset=utf-8`（例如 `text/css` 而非 `text/css; charset=utf-8`）。浏览器对 `text/*` 类型默认可能按 ISO-8859-1 解读，导致 JS 文件中的中文字符串（toast 消息、UI 文案等）在页面上显示为乱码。HTML 文件不受影响，因为已通过 `axum::response::Html` 显式设置了 charset。

## What Changes

- 在 Axum 应用层添加 `map_response` 中间件，对所有 `text/*` Content-Type 响应自动追加 `; charset=utf-8`
- `sw.js` 显式路由的 Content-Type 从 `application/javascript` 改为 `application/javascript; charset=utf-8`

## Capabilities

### New Capabilities
- `static-file-charset`: 确保所有文本类型静态资源响应头包含 charset=utf-8，防止中文乱码

### Modified Capabilities

## Impact

- `server/src/main.rs`: 新增 `map_response` 中间件层 + sw.js 路由 Content-Type 修改
- 影响所有通过 `ServeDir` fallback 提供的静态文件（CSS、JS、纯文本等）
- 无破坏性变更，仅追加 charset 声明
