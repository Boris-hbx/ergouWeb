## Why

当前差旅导出是 Excel 和照片分开下载，用户需要手动对照。报销场景需要一个完整的压缩包：Excel 报销清单 + 按报销事项分类的票据照片，直接提交给财务。

## What Changes

- **新增打包下载端点**: `GET /api/trips/:id/export/bundle`，返回一个 zip 包含 Excel + 照片文件夹
- **照片目录结构改为按事项分组**: 每个报销事项（trip_item）一个文件夹，文件夹名格式为 `日期 - 描述`（如 `2026-3-1 - Uber Waterloo→Toronto Pearson机场`），一个文件夹内可有多张图片
- **前端导出菜单新增"打包下载"选项**: 合并 Excel + 照片为一键操作
- 保留现有的单独下载 Excel / 单独下载照片功能

## Capabilities

### New Capabilities

- `trip-export-bundle`: 差旅打包导出——将 Excel 报销清单和按事项分类的票据照片打包为单个 zip 下载

### Modified Capabilities

（无）

## Impact

### 后端

| 文件 | 改动 |
|------|------|
| `server/src/routes/trips.rs` | 新增 `export_bundle()` 端点，复用现有 `export_xlsx` 的 workbook 生成逻辑 + 新的按事项分文件夹的 zip 打包逻辑 |
| `server/src/main.rs` | 注册 `/api/trips/:id/export/bundle` 路由 |

### 前端

| 文件 | 改动 |
|------|------|
| `frontend/assets/js/trip.js` | `showExportMenu()` 新增"打包下载"按钮，调用 bundle 端点 |

### 依赖

- 已有: `rust_xlsxwriter`、`zip` crate，无需新增依赖
