## 1. 前端 API

- [x] 1.1 在 `api.js` 中添加 `deleteExpensePhoto(photoId)` 方法，调用 `DELETE /expenses/photos/{photo_id}`

## 2. 记账模块：编辑模式照片删除

- [x] 2.1 修改 `expense.js` 的 `editEntry()` 函数：为现有照片添加 × 删除按钮，onclick 调用 `deleteExistingPhoto(photoId)`，按钮复用 `expense-photo-remove` 样式
- [x] 2.2 在 `expense.js` 中新增 `deleteExistingPhoto(photoId)` 函数：confirm 确认 → 调用 `API.deleteExpensePhoto` → 成功后 DOM 移除该照片元素 + showToast → 失败显示错误 toast，编辑界面保持打开

## 3. 差旅模块：删除照片后留在编辑界面

- [x] 3.1 修改 `trip.js` 的 `deletePhoto()` 函数：成功后不再调用 `closeItemModal()` + `showDetail()`，改为从 DOM 移除该照片的 `.trip-photo-thumb-wrap` 元素

## 4. 验证

- [ ] 4.1 记账验证：编辑有照片的记录 → 看到 × 删除按钮 → 删除一张 → 确认留在编辑界面 → 再删一张 → 保存
- [ ] 4.2 差旅验证：编辑有照片的行程项 → 删除一张照片 → 确认留在编辑界面 → 再删一张
- [ ] 4.3 错误场景验证：断网时删除照片 → 显示错误 toast → 照片仍在 → 编辑界面不关闭
