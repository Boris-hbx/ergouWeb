## Context

两个模块涉及照片编辑：
- **记账** (`expense.js`)：编辑模式下现有照片用 `expense-photo-existing` 类渲染，纯只读，无删除功能。后端 `DELETE /api/expenses/photos/{photo_id}` 已实现，但前端无 API 方法。
- **差旅** (`trip.js`)：编辑模式下现有照片有删除按钮，但 `deletePhoto()` 成功后调用 `closeItemModal()` + `showDetail()`，关闭了编辑界面。

## Goals / Non-Goals

**Goals:**
- 记账编辑模式：现有照片可删除，删除后留在编辑界面
- 差旅编辑模式：删除照片后留在编辑界面，原地刷新照片区域
- 两个模块统一交互模式

**Non-Goals:**
- 不修改后端 API（已有）
- 不做照片排序/拖拽
- 不做批量删除（逐张删除已满足需求）

## Decisions

### 1. 记账：editEntry() 中为现有照片添加删除按钮

**方案**：在 `editEntry()` 渲染现有照片时，给每个 thumb 元素添加 × 按钮（复用新照片的 `expense-photo-remove` 样式），onclick 调用 `deleteExistingPhoto(photoId)`。

需要将 `photo.id` 传入，当前 `detail.photos` 数组中每项已包含 `id` 字段。

### 2. 记账：deleteExistingPhoto() 原地刷新

**方案**：
1. `confirm('删除这张照片？')` 确认
2. 调用 `API.deleteExpensePhoto(photoId)`
3. 成功后，从 DOM 中移除该 thumb 元素（不需要重新加载整个编辑表单）
4. 不调用 `closeAddModal()`，不调用 `loadEntries()`

直接操作 DOM 而非重新渲染整个表单，因为：
- 用户可能已经修改了金额/备注等字段，重新渲染会丢失这些未保存的输入
- 只需要 `thumb.remove()` 移除一个元素即可

### 3. 差旅：deletePhoto() 改为原地刷新

**现有代码**（trip.js:860-874）：
```javascript
async function deletePhoto(photoId) {
    if (!confirm('删除这张票据？')) return;
    var data = await API.deleteTripPhoto(photoId);
    if (data.success) {
        showToast('已删除');
        closeItemModal();          // ← 要去掉
        showDetail(_currentTrip.id); // ← 要去掉
    }
}
```

**改为**：成功后直接从 DOM 移除该照片元素，不关闭 modal。同样使用 DOM 操作而非重新渲染，避免丢失用户编辑中的表单数据。

具体实现：删除按钮的 onclick 传入 photoId，`deletePhoto()` 中通过 `querySelector` 或闭包引用找到对应的 `.trip-photo-thumb-wrap` 元素并 `.remove()`。

### 4. API 方法：一行添加

在 `api.js` 中添加，模式与已有的 `deleteTripPhoto` 一致：
```javascript
deleteExpensePhoto: async function(photoId) {
    return await request('DELETE', '/expenses/photos/' + encodeURIComponent(photoId));
}
```

## Risks / Trade-offs

- **[风险] DOM 操作后照片 ID 与内存数据不同步** → 可接受。编辑界面是临时视图，保存时只处理 `_pendingPhotos`（新照片），不依赖已删除照片的 ID。下次打开详情/编辑会从服务器重新加载。
- **[风险] 用户误删照片不可撤销** → 通过 confirm 弹窗缓解。照片一旦从服务器删除无法恢复，但这与差旅模块现有行为一致。
- **[取舍] DOM 操作 vs 重新渲染** → 选择 DOM 操作虽然不如重新渲染"干净"，但保留了用户正在编辑的表单数据，用户体验更好。
