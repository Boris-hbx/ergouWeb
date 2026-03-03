## Why

记账和差旅的照片编辑体验有两个问题：
1. **记账**：编辑模式下现有照片纯只读显示，无法删除。后端 API 已有 `DELETE /api/expenses/photos/{photo_id}`，但前端完全没接入。
2. **差旅**：删除照片后会关闭编辑界面回到详情页（`closeItemModal()` + `showDetail()`），导致连续删除多张照片时需要反复进出编辑界面。

## What Changes

- **记账编辑模式**：现有照片增加删除按钮（×），点击后确认并调用 API 删除，删除后留在编辑界面、刷新照片显示
- **记账前端 API**：新增 `API.deleteExpensePhoto(photoId)` 方法，对接已有的后端端点
- **差旅编辑模式**：删除照片成功后，不再关闭编辑界面，改为原地刷新照片列表，用户可继续删除或编辑其他内容
- 两个模块统一行为：删除照片 = 确认弹窗 → API 删除 → 留在编辑界面 → 刷新照片区域

## Capabilities

### New Capabilities

- `expense-photo-delete`: 记账模块编辑模式下的现有照片删除功能

### Modified Capabilities

（无现有 openspec spec 需要修改）

## Impact

- **前端 JS**: `frontend/assets/js/expense.js` — editEntry() 中为现有照片添加删除按钮 + 新增 deleteExpensePhoto() 函数
- **前端 JS**: `frontend/assets/js/trip.js` — deletePhoto() 改为留在编辑界面刷新照片
- **前端 JS**: `frontend/assets/js/api.js` — 新增 `deleteExpensePhoto` API 方法
- **后端**: 无变更（`DELETE /api/expenses/photos/{photo_id}` 已实现）
