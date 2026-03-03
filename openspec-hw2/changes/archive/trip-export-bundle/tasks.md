## 1. 后端: 提取公共函数 + bundle 端点

- [x] 1.1 提取 `build_xlsx_buffer(db, trip_id)` 公共函数，`export_xlsx` 改为调用它
- [x] 1.2 实现 `sanitize_folder_name(date, description)` — 生成安全文件夹名
- [x] 1.3 实现 `export_bundle()` 端点：生成 zip 包含 xlsx + 按事项分文件夹的照片
- [x] 1.4 在 `main.rs` 注册路由 `GET /api/trips/:id/export/bundle`
- [x] 1.5 `cargo test` + `cargo clippy`

## 2. 前端: 导出菜单更新

- [x] 2.1 `showExportMenu()` 新增"打包下载（Excel + 照片）"按钮，调用 bundle 端点
- [x] 2.2 `exportBundle()` 函数实现

## 3. 验证

- [x] 3.1 递增缓存版本号
- [x] 3.2 部署 staging 验证打包下载
