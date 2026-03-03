## 1. 后端中间件

- [x] 1.1 在 `server/src/main.rs` 添加 `map_response` 中间件，对 `text/*` Content-Type 追加 `; charset=utf-8`
- [x] 1.2 将 sw.js 路由的 Content-Type 从 `application/javascript` 改为 `application/javascript; charset=utf-8`

## 2. 验证

- [x] 2.1 `cargo build --release` 编译通过
- [x] 2.2 `cargo test` 全部测试通过
- [x] 2.3 部署后 curl 验证 CSS 响应头包含 `charset=utf-8`
- [x] 2.4 部署后 curl 验证 JS 响应头包含 `charset=utf-8`
- [x] 2.5 部署后 curl 验证 sw.js 响应头包含 `charset=utf-8`
- [x] 2.6 浏览器打开页面确认中文不再乱码
