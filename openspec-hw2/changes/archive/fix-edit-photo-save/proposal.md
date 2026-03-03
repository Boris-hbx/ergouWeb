## Why

记账和差旅两个模块的照片处理、保存/AI 分析按钮逻辑各自实现，存在不一致和代码重复：

1. **按钮逻辑不一致**：记账新建添加照片后"保存"消失只剩"识别账单"，用户无法跳过 AI 直接保存；差旅模块保存和 AI 分析始终独立，体验更好
2. **照片处理重复**：选择、预览缩略图、删除逻辑两个模块各写一套，几乎相同
3. **Base64 转换两套实现**：记账自写 `fileToBase64`(2048px/0.82)，差旅用 utils 的 `imageFileToBase64`(1024px/0.85)，参数不一致
4. **文件处理不一致**：记账有 10MB 硬拒绝（体验差），差旅无校验；应统一为自动压缩

随着未来可能新增更多带照片+AI 的功能模块，需要抽象公共能力，保证一致性并减少重复。

## What Changes

### Phase 1: 抽象公共照片能力到 utils.js
- 抽取 `PhotoManager` — 统一照片选择、自动压缩、预览渲染、删除
- 统一 `imageFileToBase64()` 参数（统一 maxPx 和 quality）
- 记账和差旅模块改为调用公共 PhotoManager

### Phase 2: 统一按钮模式
- 记账模块：添加照片后同时显示"保存"和"识别账单 ✨"（与差旅模式对齐）
- 确立统一原则：**保存始终可用，AI 分析始终可选**
- 编辑模式行为不变（已正确）

### Phase 3: 文档保障
- `CLAUDE.md` — 新增"公共 UI 组件"约定，要求新模块优先复用 PhotoManager 等公共能力
- `docs/ref/FRONTEND.md` — 新增公共组件章节，记录 PhotoManager API、使用示例和按钮模式规范

## Capabilities

### New Capabilities

- `photo-manager`: 公共照片处理能力 — 选择、预览网格渲染、删除、文件大小校验、Base64 转换，供所有模块复用

### Modified Capabilities

（无现有 openspec-hw2 spec 需要修改）

## Impact

- `frontend/assets/js/utils.js` — 新增 PhotoManager 公共能力
- `frontend/assets/js/expense.js` — 重构照片处理逻辑，改为调用 PhotoManager；修改 `updateFooterButtons()` 按钮逻辑
- `frontend/assets/js/trip.js` — 重构照片处理逻辑，改为调用 PhotoManager；补充文件校验
- `frontend/assets/css/style.css` — footer 双按钮布局样式（保存 + AI 并排）
- `CLAUDE.md` — 新增公共 UI 组件复用约定
- `docs/ref/FRONTEND.md` — 新增公共组件 API 文档章节
